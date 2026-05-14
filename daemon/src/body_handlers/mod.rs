pub mod game_object;
pub mod generic;
pub mod prefab;
pub mod transform;

// ── Parsed data types ─────────────────────────────────────────────────────────
// One type per block kind. All fields use owned strings so blocks can be
// collected into a Vec and processed after the file is fully parsed.

#[derive(Debug)]
pub struct ParsedGameObject {
    pub name:                String,
    pub tag:                 String,
    pub layer:               i64,
    pub is_active:           bool,
    pub component_local_ids: Vec<String>,
}

#[derive(Debug)]
pub struct ParsedTransform {
    pub game_object_local_id: Option<String>,
    pub parent_local_id:      Option<String>,
    pub sibling_index:        i64,
    pub pos_x:   f64, pub pos_y:   f64, pub pos_z:   f64,
    pub rot_x:   f64, pub rot_y:   f64, pub rot_z:   f64, pub rot_w: f64,
    pub scale_x: f64, pub scale_y: f64, pub scale_z: f64,
}

impl ParsedTransform {
    pub fn identity() -> Self {
        Self {
            game_object_local_id: None,
            parent_local_id:      None,
            sibling_index: 0,
            pos_x: 0.0, pos_y: 0.0, pos_z: 0.0,
            rot_x: 0.0, rot_y: 0.0, rot_z: 0.0, rot_w: 1.0,
            scale_x: 1.0, scale_y: 1.0, scale_z: 1.0,
        }
    }
}

#[derive(Debug)]
pub struct ParsedPropertyOverride {
    pub target_file_id:  String,
    pub target_guid:     String,
    pub property_path:   String,
    pub value:           Option<String>,
    pub obj_ref_file_id: Option<String>,
    pub obj_ref_guid:    Option<String>,
}

#[derive(Debug)]
pub struct ParsedRemoval {
    pub target_file_id: String,
    pub target_guid:    String,
    pub removal_type:   &'static str, // "component" | "game_object"
}

#[derive(Debug)]
pub struct ParsedPrefabInstance {
    pub source_prefab_guid:        Option<String>,
    pub transform_parent_local_id: Option<String>,
    pub property_overrides:        Vec<ParsedPropertyOverride>,
    pub removals:                  Vec<ParsedRemoval>,
}

/// Data extracted from a stripped object block.
#[derive(Debug)]
pub struct ParsedStripped {
    pub prefab_instance_local_id: Option<String>,
    pub prefab_source_file_id:    Option<String>,
    pub prefab_source_guid:       Option<String>,
}

// ── Top-level block enum ──────────────────────────────────────────────────────

#[derive(Debug)]
pub enum ParsedBlock {
    GameObject {
        local_id: String,
        data:     ParsedGameObject,
    },
    Transform {
        local_id: String,
        data:     ParsedTransform,
    },
    PrefabInstance {
        local_id: String,
        class_id: i64,
        data:     ParsedPrefabInstance,
    },
    Stripped {
        local_id: String,
        class_id: i64,
        data:     ParsedStripped,
    },
    Generic {
        local_id: String,
        class_id: i64,
        fields:   Vec<(String, String)>,
    },
}

impl ParsedBlock {
    pub fn local_id(&self) -> &str {
        match self {
            Self::GameObject     { local_id, .. } => local_id,
            Self::Transform      { local_id, .. } => local_id,
            Self::PrefabInstance { local_id, .. } => local_id,
            Self::Stripped       { local_id, .. } => local_id,
            Self::Generic        { local_id, .. } => local_id,
        }
    }

    pub fn class_id(&self) -> i64 {
        match self {
            Self::GameObject     { .. }          => 1,
            Self::Transform      { .. }          => 4,
            Self::PrefabInstance { class_id, .. } => *class_id,
            Self::Stripped       { class_id, .. } => *class_id,
            Self::Generic        { class_id, .. } => *class_id,
        }
    }
}
