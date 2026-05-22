use anyhow::{Context, Result};
use rusqlite::{Connection, params};
use std::{collections::HashMap, path::Path};
use tracing::trace;

use crate::types::{
    ParsedAddition, ParsedAssetRef, ParsedGameObject, ParsedPropertyOverride, ParsedRemoval,
    ParsedTransform,
};

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
    let id: i64 = conn.query_row("SELECT id FROM assets WHERE path = ?1", [path], |row| {
        row.get(0)
    })?;
    Ok(id)
}

pub fn delete_asset(conn: &Connection, asset_id: i64) -> Result<()> {
    conn.execute("DELETE FROM assets WHERE id = ?1", params![asset_id])?;
    Ok(())
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
    trace!(
        object_id,
        prefab_instance_db_id, source_file_id, source_guid, "update_object_prefab_membership"
    );
    conn.execute(
        "UPDATE objects
         SET prefab_instance_id    = ?1,
             prefab_source_file_id = ?2,
             prefab_source_guid    = ?3
         WHERE id = ?4",
        params![
            prefab_instance_db_id,
            source_file_id,
            source_guid,
            object_id
        ],
    )?;
    Ok(())
}

pub fn insert_game_object(conn: &Connection, object_id: i64, go: &ParsedGameObject) -> Result<()> {
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
            object_id,
            go_db_id,
            parent_db_id,
            t.root_order,
            t.pos_x,
            t.pos_y,
            t.pos_z,
            t.rot_x,
            t.rot_y,
            t.rot_z,
            t.rot_w,
            t.scale_x,
            t.scale_y,
            t.scale_z
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
    trace!(
        object_id,
        source_prefab_guid, transform_parent_db_id, "insert_prefab_instance"
    );
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
    trace!(
        instance_id,
        property_path = ov.property_path.as_str(),
        "insert_property_override"
    );
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

pub fn insert_prefab_removal(conn: &Connection, instance_id: i64, r: &ParsedRemoval) -> Result<()> {
    trace!(
        instance_id,
        target_file_id = r.target_file_id.as_str(),
        removal_type = r.removal_type.as_str(),
        "insert_prefab_removal"
    );
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
    trace!(
        instance_id,
        added_db_id,
        addition_type = a.addition_type.as_str(),
        "insert_prefab_addition"
    );
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
    trace!(
        from_object_id,
        field_path = ar.field_path.as_str(),
        to_guid = ar.to_guid.as_str(),
        "insert_asset_reference"
    );
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

// ── Query result types ────────────────────────────────────────────────────────

pub struct AssetTimestampData {
    pub id: i64,
    pub timestamp: i64,
}

pub struct AssetRow {
    pub path: String,
    pub guid: Option<String>,
    pub asset_type: String,
}

pub struct HierarchyRow {
    pub local_id: String,
    pub name: String,
    pub depth: i32,
    pub sibling_index: i32,
    pub ancestry_path: String,
}

pub struct GameObjectMatchRow {
    pub scene_path: String,
    pub name: String,
    pub local_id: String,
    pub ancestry_path: Option<String>,
    pub script_path: Option<String>,
}

pub struct FieldMatchRow {
    pub scene_path: String,
    pub game_object_name: String,
    pub game_object_local_id: String,
    pub script_path: Option<String>,
    pub field_key: String,
    pub field_value: String,
}

pub struct FieldReferenceMatchRow {
    pub scene_path: String,
    pub game_object_name: String,
    pub game_object_local_id: String,
    pub script_path: Option<String>,
    pub field_key: String,
    pub class_id: String,
    pub ancestry_path: Option<String>,
}

pub struct GameObjectDetailRow {
    pub name: String,
    pub tag: String,
    pub layer: i64,
    pub is_active: bool,
}

pub struct ComponentDetailRow {
    pub local_id: String,
    pub class_id: String,
    pub script_path: Option<String>,
}

// ── Query functions ───────────────────────────────────────────────────────────

pub fn list_asset_timestamps(conn: &Connection) -> Result<HashMap<String, AssetTimestampData>> {
    trace!("load_asset_timestamps");

    let mut stmt = conn.prepare("SELECT path, id, last_modified_ms FROM assets")?;

    let mut map = HashMap::new();
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            AssetTimestampData {
                id: row.get::<_, i64>(1)?,
                timestamp: row.get::<_, i64>(2)?,
            },
        ))
    })?;

    for row in rows {
        let (path, data) = row?;
        map.insert(path, data);
    }
    Ok(map)
}

pub fn list_assets(
    conn: &Connection,
    asset_type_filter: &str,
    path_filter: &str,
) -> Result<Vec<AssetRow>> {
    let mut stmt = conn.prepare(
        "SELECT path, guid, asset_type FROM assets
         WHERE (?1 = '' OR asset_type = ?1)
           AND (?2 = '' OR path LIKE '%' || ?2 || '%')
         ORDER BY path",
    )?;
    let rows = stmt.query_map(params![asset_type_filter, path_filter], |row| {
        Ok(AssetRow {
            path: row.get(0)?,
            guid: row.get(1)?,
            asset_type: row.get(2)?,
        })
    })?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

pub fn get_scene_hierarchy(
    conn: &Connection,
    scene_filter: &str,
    max_depth: i32,
    exclude_scripts: &[String],
) -> Result<Vec<HierarchyRow>> {
    let (exclude_cte, exclude_filter) = if exclude_scripts.is_empty() {
        (String::new(), String::new())
    } else {
        let conditions: String = (0..exclude_scripts.len())
            .map(|i| format!("sa.path LIKE '%' || ?{} || '%'", i + 3))
            .collect::<Vec<_>>()
            .join(" OR ");
        let cte = format!(
            "excluded(game_object_id) AS (
                 SELECT goc.game_object_id
                 FROM game_object_components goc
                 JOIN asset_references ar ON ar.from_object_id = goc.component_id
                                         AND ar.field_path = 'm_Script'
                 JOIN assets sa           ON sa.guid = ar.to_guid
                 WHERE {conditions}
             ),",
        );
        let filter = "AND t.game_object_id NOT IN (SELECT game_object_id FROM excluded)".to_string();
        (cte, filter)
    };

    let sql = format!(
        "WITH {exclude_cte}
         RECURSIVE hierarchy(object_id, local_id, name, parent_id, sibling_index, depth, ancestry_path) AS (
             SELECT t.object_id, o.local_id, go.name, t.parent_id, t.sibling_index, 0, go.name
             FROM transforms t
             JOIN objects o       ON o.id = t.object_id
             JOIN assets a        ON a.id = o.asset_id
             JOIN game_objects go ON go.object_id = t.game_object_id
             WHERE t.parent_id IS NULL
               AND (?1 = '' OR a.path LIKE '%' || ?1 || '%')
               {exclude_filter}
             UNION ALL
             SELECT t.object_id, o.local_id, go.name, t.parent_id, t.sibling_index,
                    h.depth + 1, h.ancestry_path || char(31) || go.name
             FROM transforms t
             JOIN hierarchy h     ON h.object_id = t.parent_id
             JOIN objects o       ON o.id = t.object_id
             JOIN game_objects go ON go.object_id = t.game_object_id
             WHERE (?2 = 0 OR h.depth + 1 < ?2)
               {exclude_filter}
         )
         SELECT local_id, name, depth, sibling_index, ancestry_path
         FROM hierarchy
         ORDER BY ancestry_path"
    );

    let mut stmt = conn.prepare(&sql)?;
    let mut param_values: Vec<Box<dyn rusqlite::ToSql>> = vec![
        Box::new(scene_filter.to_string()),
        Box::new(max_depth),
    ];
    for s in exclude_scripts {
        param_values.push(Box::new(s.clone()));
    }

    let rows = stmt.query_map(
        rusqlite::params_from_iter(param_values.iter().map(|p| p.as_ref())),
        |row| {
            Ok(HierarchyRow {
                local_id: row.get(0)?,
                name: row.get(1)?,
                depth: row.get(2)?,
                sibling_index: row.get(3)?,
                ancestry_path: row.get(4)?,
            })
        },
    )?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

pub fn find_by_component(
    conn: &Connection,
    script_name: &str,
    scene_filter: &str,
) -> Result<Vec<GameObjectMatchRow>> {
    let mut stmt = conn.prepare(
        "WITH RECURSIVE ancestry(object_id, ancestry_path) AS (
             SELECT t.object_id, go.name
             FROM transforms t
             JOIN game_objects go ON go.object_id = t.game_object_id
             WHERE t.parent_id IS NULL
             UNION ALL
             SELECT t.object_id, anc.ancestry_path || char(31) || go.name
             FROM transforms t
             JOIN ancestry anc    ON anc.object_id = t.parent_id
             JOIN game_objects go ON go.object_id = t.game_object_id
         )
         SELECT a.path, go.name, o_comp.local_id, anc.ancestry_path, script_asset.path
         FROM asset_references ar
         JOIN assets script_asset        ON script_asset.guid = ar.to_guid
                                         AND script_asset.path LIKE '%' || ?1 || '%'
         JOIN objects o_comp             ON o_comp.id = ar.from_object_id
         JOIN game_object_components goc ON goc.component_id = o_comp.id
         JOIN objects o_go               ON o_go.id = goc.game_object_id
         JOIN game_objects go            ON go.object_id = o_go.id
         JOIN assets a                   ON a.id = o_go.asset_id
         LEFT JOIN transforms t_go       ON t_go.game_object_id = o_go.id
         LEFT JOIN ancestry anc          ON anc.object_id = t_go.object_id
         WHERE ar.field_path = 'm_Script'
           AND (?2 = '' OR a.path LIKE '%' || ?2 || '%')",
    )?;
    let rows = stmt.query_map(params![script_name, scene_filter], |row| {
        Ok(GameObjectMatchRow {
            scene_path: row.get(0)?,
            name: row.get(1)?,
            local_id: row.get(2)?,
            ancestry_path: row.get(3)?,
            script_path: row.get(4)?,
        })
    })?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

pub fn find_by_field_value(
    conn: &Connection,
    field_key: &str,
    field_value: &str,
    script_filter: &str,
    scene_filter: &str,
) -> Result<Vec<FieldMatchRow>> {
    let mut stmt = conn.prepare(
        "SELECT a.path, go.name, o_go.local_id, script_asset.path, f.key, f.value
         FROM object_fields f
         JOIN objects o_comp              ON o_comp.id = f.object_id
         JOIN game_object_components goc  ON goc.component_id = o_comp.id
         JOIN objects o_go                ON o_go.id = goc.game_object_id
         JOIN game_objects go             ON go.object_id = o_go.id
         JOIN assets a                    ON a.id = o_go.asset_id
         LEFT JOIN asset_references ar_s  ON ar_s.from_object_id = o_comp.id
                                         AND ar_s.field_path = 'm_Script'
         LEFT JOIN assets script_asset    ON script_asset.guid = ar_s.to_guid
         WHERE (?1 = '' OR f.key = ?1)
           AND (?2 = '' OR f.value LIKE '%' || ?2 || '%')
           AND (?3 = '' OR script_asset.path LIKE '%' || ?3 || '%')
           AND (?4 = '' OR a.path LIKE '%' || ?4 || '%')
           AND a.asset_type = 'scene'",
    )?;
    let rows = stmt.query_map(
        params![field_key, field_value, script_filter, scene_filter],
        |row| {
            Ok(FieldMatchRow {
                scene_path: row.get(0)?,
                game_object_name: row.get(1)?,
                game_object_local_id: row.get(2)?,
                script_path: row.get(3)?,
                field_key: row.get(4)?,
                field_value: row.get(5)?,
            })
        },
    )?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

pub fn find_by_field_reference(
    conn: &Connection,
    target_asset: &str,
    scene_filter: &str,
) -> Result<Vec<FieldReferenceMatchRow>> {
    let mut stmt = conn.prepare(
        "WITH RECURSIVE ancestry(object_id, ancestry_path) AS (
             SELECT t.object_id, go.name
             FROM transforms t
             JOIN game_objects go ON go.object_id = t.game_object_id
             WHERE t.parent_id IS NULL
             UNION ALL
             SELECT t.object_id, anc.ancestry_path || char(31) || go.name
             FROM transforms t
             JOIN ancestry anc    ON anc.object_id = t.parent_id
             JOIN game_objects go ON go.object_id = t.game_object_id
         )
         SELECT a.path, go.name, o_go.local_id, script_asset.path, ar.field_path,
                CAST(o_comp.class_id AS TEXT), anc.ancestry_path
         FROM asset_references ar
         JOIN assets ref_asset            ON ref_asset.guid = ar.to_guid
                                         AND ref_asset.path LIKE '%' || ?1 || '%'
         JOIN objects o_comp              ON o_comp.id = ar.from_object_id
         JOIN game_object_components goc  ON goc.component_id = o_comp.id
         JOIN objects o_go                ON o_go.id = goc.game_object_id
         JOIN game_objects go             ON go.object_id = o_go.id
         JOIN assets a                    ON a.id = o_go.asset_id
         LEFT JOIN asset_references ar_s  ON ar_s.from_object_id = o_comp.id
                                         AND ar_s.field_path = 'm_Script'
         LEFT JOIN assets script_asset    ON script_asset.guid = ar_s.to_guid
         LEFT JOIN transforms t_go        ON t_go.game_object_id = o_go.id
         LEFT JOIN ancestry anc           ON anc.object_id = t_go.object_id
         WHERE ar.field_path != 'm_Script'
           AND (?2 = '' OR a.path LIKE '%' || ?2 || '%')
           AND a.asset_type = 'scene'",
    )?;
    let rows = stmt.query_map(params![target_asset, scene_filter], |row| {
        Ok(FieldReferenceMatchRow {
            scene_path: row.get(0)?,
            game_object_name: row.get(1)?,
            game_object_local_id: row.get(2)?,
            script_path: row.get(3)?,
            field_key: row.get(4)?,
            class_id: row.get(5)?,
            ancestry_path: row.get(6)?,
        })
    })?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

pub fn find_by_local_id(
    conn: &Connection,
    local_id: &str,
    scene_filter: &str,
) -> Result<Vec<GameObjectMatchRow>> {
    let mut stmt = conn.prepare(
        "WITH RECURSIVE ancestry(object_id, ancestry_path) AS (
             SELECT t.object_id, go.name
             FROM transforms t
             JOIN game_objects go ON go.object_id = t.game_object_id
             WHERE t.parent_id IS NULL
             UNION ALL
             SELECT t.object_id, anc.ancestry_path || char(31) || go.name
             FROM transforms t
             JOIN ancestry anc    ON anc.object_id = t.parent_id
             JOIN game_objects go ON go.object_id = t.game_object_id
         )
         SELECT a.path, go.name, o_go.local_id, anc.ancestry_path, script_asset.path
         FROM asset_references ar
         JOIN objects o_target            ON o_target.local_id = ?1
         JOIN objects o_comp              ON o_comp.id = ar.from_object_id
                                         AND o_comp.asset_id = o_target.asset_id
         JOIN assets a                    ON a.id = o_comp.asset_id
         JOIN game_object_components goc  ON goc.component_id = o_comp.id
         JOIN objects o_go                ON o_go.id = goc.game_object_id
         JOIN game_objects go             ON go.object_id = o_go.id
         LEFT JOIN asset_references ar_s  ON ar_s.from_object_id = o_comp.id
                                         AND ar_s.field_path = 'm_Script'
         LEFT JOIN assets script_asset    ON script_asset.guid = ar_s.to_guid
         LEFT JOIN transforms t_go        ON t_go.game_object_id = o_go.id
         LEFT JOIN ancestry anc           ON anc.object_id = t_go.object_id
         WHERE ar.to_file_id = ?1
           AND (?2 = '' OR a.path LIKE '%' || ?2 || '%')
           AND a.asset_type = 'scene'",
    )?;
    let rows = stmt.query_map(params![local_id, scene_filter], |row| {
        Ok(GameObjectMatchRow {
            scene_path: row.get(0)?,
            name: row.get(1)?,
            local_id: row.get(2)?,
            ancestry_path: row.get(3)?,
            script_path: row.get(4)?,
        })
    })?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

pub fn get_game_object(
    conn: &Connection,
    scene_path: &str,
    local_id: &str,
) -> Result<Option<GameObjectDetailRow>> {
    let result = conn.query_row(
        "SELECT go.name, go.tag, go.layer, go.is_active
         FROM game_objects go
         JOIN objects o ON o.id = go.object_id
         JOIN assets a  ON a.id = o.asset_id
         WHERE o.local_id = ?1
           AND (?2 = '' OR a.path LIKE '%' || ?2 || '%')",
        params![local_id, scene_path],
        |row| {
            Ok(GameObjectDetailRow {
                name: row.get(0)?,
                tag: row.get(1)?,
                layer: row.get(2)?,
                is_active: row.get::<_, i64>(3)? != 0,
            })
        },
    );
    match result {
        Ok(row) => Ok(Some(row)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

pub fn get_game_object_components(
    conn: &Connection,
    scene_path: &str,
    local_id: &str,
) -> Result<Vec<ComponentDetailRow>> {
    let mut stmt = conn.prepare(
        "SELECT o_comp.local_id, CAST(o_comp.class_id AS TEXT), script_asset.path
         FROM game_object_components goc
         JOIN objects o_go        ON o_go.id = goc.game_object_id
         JOIN assets a            ON a.id = o_go.asset_id
         JOIN objects o_comp      ON o_comp.id = goc.component_id
         LEFT JOIN asset_references ar ON ar.from_object_id = o_comp.id
                                     AND ar.field_path = 'm_Script'
         LEFT JOIN assets script_asset ON script_asset.guid = ar.to_guid
         WHERE o_go.local_id = ?1
           AND (?2 = '' OR a.path LIKE '%' || ?2 || '%')
         ORDER BY goc.order_index",
    )?;
    let rows = stmt.query_map(params![local_id, scene_path], |row| {
        Ok(ComponentDetailRow {
            local_id: row.get(0)?,
            class_id: row.get(1)?,
            script_path: row.get(2)?,
        })
    })?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}
