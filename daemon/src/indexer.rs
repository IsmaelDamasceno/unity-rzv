use std::collections::HashMap;
use std::path::Path;
use std::string;
use std::time::{Instant, SystemTime};

use rusqlite::Connection;
use tracing::{debug, info, trace, warn};
use walkdir::WalkDir;

use crate::types::{BlockData, ParsedBlock};
use crate::{Finders, block_mapper, db};

#[derive(Debug, Default)]
pub struct IndexStats {
    pub assets_indexed:  u32,
    pub objects_indexed: u32,
    pub assets_deleted:  usize,
    pub errors:          Vec<String>,
}

// ── Public entry point ────────────────────────────────────────────────────────

/// Walks `assets_path`, indexes every relevant file into `conn`.
/// Files whose modification time matches the stored value are skipped.
pub fn index_project(assets_path: &Path, conn: &mut Connection) -> anyhow::Result<IndexStats> {
    let finders = Finders::new();
    let mut stats = IndexStats::default();
    let mut timestamps = db::list_asset_timestamps(conn)?;

    for entry in WalkDir::new(assets_path)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let path = entry.path();
        let path_str = path.to_string_lossy().into_owned();

        let asset_type = match path.extension().and_then(|e| e.to_str()) {
            Some("unity") => "scene",
            Some("prefab") => "prefab",
            Some("asset") => "asset",
            Some("cs") => "script",
            _ => continue,
        };

        let timestamp_data = timestamps.remove(&path_str);
        let modified_ms = modified_ms(path).unwrap_or(0);

        if let Some(data) = timestamp_data && data.timestamp == modified_ms {
            trace!(path = path_str.as_str(), "skipping unchanged file");
            continue;
        }

        let guid = read_guid_from_meta(&format!("{path_str}.meta"));
        debug!(path = path_str.as_str(), asset_type, "indexing file");

        match index_file(
            conn,
            &path_str,
            guid.as_deref(),
            asset_type,
            modified_ms,
            &finders,
        ) {
            Ok(count) => {
                debug!(path = path_str.as_str(), objects = count, "indexed");
                stats.assets_indexed += 1;
                stats.objects_indexed += count;
            }
            Err(e) => {
                warn!(path = path_str.as_str(), error = %e, "failed to index file");
                stats.errors.push(format!("{path_str}: {e}"));
            }
        }
    }

    stats.assets_deleted = timestamps.len();

    for (_, data) in &timestamps {
        db::delete_asset(&conn, data.id)?;
    }

    Ok(stats)
}

// ── Per-file indexing ─────────────────────────────────────────────────────────

fn index_file(
    conn: &mut Connection,
    path: &str,
    guid: Option<&str>,
    asset_type: &str,
    modified_ms: i64,
    finders: &Finders,
) -> anyhow::Result<u32> {
    let file_start = Instant::now();
    info!(path, asset_type, "indexing file");

    if asset_type == "script" {
        let t = Instant::now();
        let tx = conn.transaction()?;
        let asset_id = db::upsert_asset(&tx, path, guid, asset_type, modified_ms)?;
        db::delete_asset_objects(&tx, asset_id)?;
        db::mark_asset_indexed(&tx, asset_id, modified_ms)?;
        tx.commit()?;
        debug!(path, db_ms = t.elapsed().as_millis(), "script recorded");
        return Ok(0);
    }

    let t = Instant::now();
    let file = std::fs::File::open(path)?;
    let mmap = unsafe { memmap2::Mmap::map(&file)? };
    let data: &[u8] = &mmap;
    debug!(
        path,
        bytes = data.len(),
        read_ms = t.elapsed().as_millis(),
        "file mapped"
    );

    let t = Instant::now();
    let blocks = block_mapper::parse_unity_doc(data, finders)?;
    let count = blocks.len() as u32;
    debug!(
        path,
        blocks = count,
        parse_ms = t.elapsed().as_millis(),
        "blocks parsed"
    );

    let t = Instant::now();
    let tx = conn.transaction()?;
    let asset_id = db::upsert_asset(&tx, path, guid, asset_type, modified_ms)?;
    db::delete_asset_objects(&tx, asset_id)?;
    insert_blocks(&tx, asset_id, &blocks)?;
    db::mark_asset_indexed(&tx, asset_id, modified_ms)?;
    tx.commit()?;
    debug!(
        path,
        blocks = count,
        db_ms = t.elapsed().as_millis(),
        "blocks written to db"
    );

    info!(
        path,
        blocks = count,
        total_ms = file_start.elapsed().as_millis(),
        "file indexed"
    );
    Ok(count)
}

// ── Two-pass block insertion ──────────────────────────────────────────────────
//
// Pass 1 – insert every block into `objects`, build local_id → object_id map.
// Pass 2 – use the map to resolve local_id references and write typed tables.

fn insert_blocks(conn: &Connection, asset_id: i64, blocks: &[ParsedBlock]) -> anyhow::Result<()> {
    let id_map = insert_objects_pass(conn, asset_id, blocks)?;
    insert_type_data_pass(conn, blocks, &id_map)?;
    Ok(())
}

fn insert_objects_pass(
    conn: &Connection,
    asset_id: i64,
    blocks: &[ParsedBlock],
) -> anyhow::Result<HashMap<String, i64>> {
    let mut id_map = HashMap::with_capacity(blocks.len());

    for block in blocks {
        let class_id: i64 = block.class_id.parse().unwrap_or(0);
        let oid = db::insert_object(conn, asset_id, &block.local_id, class_id)?;
        id_map.insert(block.local_id.clone(), oid);

        // Store cross-asset references immediately — they have no FK dependency.
        for ar in &block.asset_refs {
            if let Err(e) = db::insert_asset_reference(conn, oid, ar) {
                warn!(local_id = block.local_id.as_str(), error = %e, "asset_ref insert failed");
            }
        }
    }

    Ok(id_map)
}

fn insert_type_data_pass(
    conn: &Connection,
    blocks: &[ParsedBlock],
    id_map: &HashMap<String, i64>,
) -> anyhow::Result<()> {
    for block in blocks {
        let Some(&oid) = id_map.get(&block.local_id) else {
            continue;
        };

        if let Err(e) = insert_typed(conn, oid, block, id_map) {
            warn!(local_id = block.local_id.as_str(), oid, error = %e, "type insert failed");
        }
    }
    Ok(())
}

fn insert_typed(
    conn: &Connection,
    oid: i64,
    block: &ParsedBlock,
    id_map: &HashMap<String, i64>,
) -> anyhow::Result<()> {
    match &block.data {
        BlockData::GameObject(go) => {
            db::insert_game_object(conn, oid, go)?;
            for (i, comp_lid) in go.components.iter().enumerate() {
                if let Some(&comp_oid) = id_map.get(comp_lid) {
                    db::insert_game_object_component(conn, oid, comp_oid, i as i64)?;
                }
            }
        }

        BlockData::Transform(t) => {
            let go_oid = t
                .game_object_file_id
                .as_deref()
                .and_then(|l| id_map.get(l))
                .copied();
            let parent_oid = t
                .parent_file_id
                .as_deref()
                .and_then(|l| id_map.get(l))
                .copied();
            db::insert_transform(conn, oid, go_oid, parent_oid, t)?;
        }

        BlockData::PrefabInstance(pi) => {
            let parent_oid = pi
                .transform_parent_file_id
                .as_deref()
                .and_then(|l| id_map.get(l))
                .copied();
            db::insert_prefab_instance(conn, oid, pi.source_prefab_guid.as_deref(), parent_oid)?;

            for ov in &pi.property_overrides {
                db::insert_property_override(conn, oid, ov)?;
            }
            for r in &pi.removals {
                db::insert_prefab_removal(conn, oid, r)?;
            }
            for a in &pi.additions {
                // Additions reference objects that may be in the same file.
                let added_oid = id_map.get(&a.added_file_id).copied();
                if let Some(added_oid) = added_oid {
                    db::insert_prefab_addition(conn, oid, a, added_oid)?;
                }
            }
        }

        BlockData::Stripped(s) => {
            let instance_oid = id_map.get(&s.prefab_instance_file_id).copied();
            if let Some(inst_oid) = instance_oid {
                db::update_object_prefab_membership(
                    conn,
                    oid,
                    inst_oid,
                    &s.source_file_id,
                    &s.source_guid,
                )?;
            }
        }

        BlockData::Generic(g) => {
            for field in &g.fields {
                db::insert_object_field(conn, oid, &field.key, &field.value)?;
            }
        }
    }

    Ok(())
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn modified_ms(path: &Path) -> anyhow::Result<i64> {
    let meta = path.metadata()?;
    let ms = meta
        .modified()?
        .duration_since(SystemTime::UNIX_EPOCH)?
        .as_millis() as i64;
    Ok(ms)
}

fn read_guid_from_meta(meta_path: &str) -> Option<String> {
    let content = std::fs::read_to_string(meta_path).ok()?;
    for line in content.lines() {
        if let Some(guid) = line.strip_prefix("guid: ") {
            return Some(guid.trim().to_string());
        }
    }
    None
}
