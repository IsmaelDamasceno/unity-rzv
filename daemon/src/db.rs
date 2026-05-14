use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use std::path::Path;
use tracing::trace;

use crate::types::{ParsedAddition, ParsedAssetRef, ParsedGameObject, ParsedPropertyOverride, ParsedRemoval, ParsedTransform};

pub fn open(db_path: &Path) -> Result<Connection> {
    let conn = Connection::open(db_path).context("failed to open database")?;
    conn.execute_batch(
        "PRAGMA journal_mode=WAL;
         PRAGMA synchronous=NORMAL;
         PRAGMA foreign_keys=ON;",
    )?;
    init_schema(&conn)?;
    Ok(conn)
}

fn init_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch(include_str!("schema.sql"))
        .context("failed to initialize schema")?;
    Ok(())
}

pub fn upsert_asset(
    conn: &Connection,
    path: &str,
    guid: Option<&str>,
    asset_type: &str,
    modified_ms: i64,
) -> Result<i64> {
    trace!(path, asset_type, "upsert_asset");
    conn.execute(
        "INSERT INTO assets (path, guid, asset_type, last_modified_ms, last_indexed_ms)
         VALUES (?1, ?2, ?3, ?4, 0)
         ON CONFLICT(path) DO UPDATE SET
           guid             = excluded.guid,
           asset_type       = excluded.asset_type,
           last_modified_ms = excluded.last_modified_ms",
        params![path, guid, asset_type, modified_ms],
    )?;
    let id: i64 = conn.query_row(
        "SELECT id FROM assets WHERE path = ?1",
        params![path],
        |row| row.get(0),
    )?;
    Ok(id)
}

pub fn needs_reindex(conn: &Connection, path: &str, modified_ms: i64) -> Result<bool> {
    trace!(path, modified_ms, "needs_reindex");
    let result: rusqlite::Result<i64> = conn.query_row(
        "SELECT last_modified_ms FROM assets WHERE path = ?1",
        params![path],
        |row| row.get(0),
    );
    match result {
        Ok(stored) => Ok(stored != modified_ms),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(true),
        Err(e) => Err(e.into()),
    }
}

pub fn delete_asset_objects(conn: &Connection, asset_id: i64) -> Result<()> {
    conn.execute("DELETE FROM objects WHERE asset_id = ?1", params![asset_id])?;
    Ok(())
}

pub fn insert_object(
    conn: &Connection,
    asset_id: i64,
    local_id: &str,
    class_id: i64,
) -> Result<i64> {
    trace!(asset_id, local_id, class_id, "insert_object");
    conn.execute(
        "INSERT INTO objects (asset_id, local_id, class_id) VALUES (?1, ?2, ?3)",
        params![asset_id, local_id, class_id],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn update_object_prefab_membership(
    conn: &Connection,
    object_id: i64,
    prefab_instance_db_id: i64,
    source_file_id: &str,
    source_guid: &str,
) -> Result<()> {
    trace!(object_id, prefab_instance_db_id, source_file_id, source_guid, "update_object_prefab_membership");
    conn.execute(
        "UPDATE objects
         SET prefab_instance_id    = ?1,
             prefab_source_file_id = ?2,
             prefab_source_guid    = ?3
         WHERE id = ?4",
        params![prefab_instance_db_id, source_file_id, source_guid, object_id],
    )?;
    Ok(())
}

pub fn insert_game_object(
    conn: &Connection,
    object_id: i64,
    go: &ParsedGameObject,
) -> Result<()> {
    trace!(object_id, name = go.name.as_str(), "insert_game_object");
    conn.execute(
        "INSERT INTO game_objects (object_id, name, tag, layer, is_active)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![object_id, go.name, go.tag, go.layer, go.is_active as i64],
    )?;
    Ok(())
}

pub fn insert_game_object_component(
    conn: &Connection,
    go_id: i64,
    comp_id: i64,
    order_index: i64,
) -> Result<()> {
    trace!(go_id, comp_id, order_index, "insert_game_object_component");
    conn.execute(
        "INSERT OR IGNORE INTO game_object_components
         (game_object_id, component_id, order_index, enabled)
         VALUES (?1, ?2, ?3, 1)",
        params![go_id, comp_id, order_index],
    )?;
    Ok(())
}

pub fn insert_transform(
    conn: &Connection,
    object_id: i64,
    go_db_id: Option<i64>,
    parent_db_id: Option<i64>,
    t: &ParsedTransform,
) -> Result<()> {
    trace!(object_id, go_db_id, parent_db_id, "insert_transform");
    conn.execute(
        "INSERT INTO transforms
         (object_id, game_object_id, parent_id, sibling_index,
          pos_x, pos_y, pos_z,
          rotation_x, rotation_y, rotation_z, rotation_w,
          scale_x, scale_y, scale_z)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)",
        params![
            object_id, go_db_id, parent_db_id, t.root_order,
            t.pos_x, t.pos_y, t.pos_z,
            t.rot_x, t.rot_y, t.rot_z, t.rot_w,
            t.scale_x, t.scale_y, t.scale_z
        ],
    )?;
    Ok(())
}

pub fn insert_prefab_instance(
    conn: &Connection,
    object_id: i64,
    source_prefab_guid: Option<&str>,
    transform_parent_db_id: Option<i64>,
) -> Result<()> {
    trace!(object_id, source_prefab_guid, transform_parent_db_id, "insert_prefab_instance");
    conn.execute(
        "INSERT INTO prefab_instances (object_id, source_prefab_guid, transform_parent_id)
         VALUES (?1, ?2, ?3)",
        params![object_id, source_prefab_guid, transform_parent_db_id],
    )?;
    Ok(())
}

pub fn insert_property_override(
    conn: &Connection,
    instance_id: i64,
    ov: &ParsedPropertyOverride,
) -> Result<()> {
    trace!(instance_id, property_path = ov.property_path.as_str(), "insert_property_override");
    conn.execute(
        "INSERT INTO prefab_property_overrides
         (instance_id, target_file_id, target_guid, property_path,
          value, obj_ref_file_id, obj_ref_guid)
         VALUES (?1,?2,?3,?4,?5,?6,?7)",
        params![
            instance_id,
            ov.target_file_id,
            ov.target_guid,
            ov.property_path,
            ov.value,
            ov.obj_ref_file_id,
            ov.obj_ref_guid
        ],
    )?;
    Ok(())
}

pub fn insert_prefab_removal(
    conn: &Connection,
    instance_id: i64,
    r: &ParsedRemoval,
) -> Result<()> {
    trace!(instance_id, target_file_id = r.target_file_id.as_str(), removal_type = r.removal_type.as_str(), "insert_prefab_removal");
    conn.execute(
        "INSERT OR IGNORE INTO prefab_removals
         (instance_id, target_file_id, target_guid, removal_type)
         VALUES (?1,?2,?3,?4)",
        params![instance_id, r.target_file_id, r.target_guid, r.removal_type],
    )?;
    Ok(())
}

pub fn insert_prefab_addition(
    conn: &Connection,
    instance_id: i64,
    a: &ParsedAddition,
    added_db_id: i64,
) -> Result<()> {
    trace!(instance_id, added_db_id, addition_type = a.addition_type.as_str(), "insert_prefab_addition");
    conn.execute(
        "INSERT INTO prefab_additions
         (instance_id, added_object_id, addition_type, parent_file_id, parent_guid)
         VALUES (?1,?2,?3,?4,?5)",
        params![
            instance_id,
            added_db_id,
            a.addition_type,
            a.parent_file_id,
            a.parent_guid
        ],
    )?;
    Ok(())
}

pub fn insert_object_field(
    conn: &Connection,
    object_id: i64,
    key: &str,
    value: &str,
) -> Result<()> {
    trace!(object_id, key, value, "insert_object_field");
    conn.execute(
        "INSERT OR REPLACE INTO object_fields (object_id, key, value) VALUES (?1,?2,?3)",
        params![object_id, key, value],
    )?;
    Ok(())
}

pub fn insert_asset_reference(
    conn: &Connection,
    from_object_id: i64,
    ar: &ParsedAssetRef,
) -> Result<()> {
    trace!(from_object_id, field_path = ar.field_path.as_str(), to_guid = ar.to_guid.as_str(), "insert_asset_reference");
    conn.execute(
        "INSERT OR REPLACE INTO asset_references
         (from_object_id, field_path, to_guid, to_file_id, ref_type)
         VALUES (?1,?2,?3,?4,?5)",
        params![
            from_object_id,
            ar.field_path,
            ar.to_guid,
            ar.to_file_id,
            ar.ref_type
        ],
    )?;
    Ok(())
}

pub fn mark_asset_indexed(conn: &Connection, asset_id: i64, indexed_ms: i64) -> Result<()> {
    trace!(asset_id, indexed_ms, "mark_asset_indexed");
    conn.execute(
        "UPDATE assets SET last_indexed_ms = ?1, last_modified_ms = last_modified_ms WHERE id = ?2",
        params![indexed_ms, asset_id],
    )?;
    Ok(())
}
