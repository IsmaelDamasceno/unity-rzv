#[derive(Debug)]
pub struct FileRef {
    pub file_id: String,
    pub guid: Option<String>,
    pub ref_type: Option<i64>,
}

#[derive(Debug, Default, Clone)]
pub struct ParsedGameObject {
    pub name: String,
    pub tag: String,
    pub layer: i64,
    pub is_active: bool,
    pub components: Vec<String>,
}

#[derive(Debug, Default, Clone)]
pub struct ParsedTransform {
    pub game_object_file_id: Option<String>,
    pub parent_file_id: Option<String>,
    pub root_order: i64,
    pub pos_x: f64,
    pub pos_y: f64,
    pub pos_z: f64,
    pub rot_x: f64,
    pub rot_y: f64,
    pub rot_z: f64,
    pub rot_w: f64,
    pub scale_x: f64,
    pub scale_y: f64,
    pub scale_z: f64,
}

impl ParsedTransform {
    pub fn identity() -> Self {
        Self {
            rot_w: 1.0,
            scale_x: 1.0,
            scale_y: 1.0,
            scale_z: 1.0,
            ..Default::default()
        }
    }
}

#[derive(Debug, Clone)]
pub struct ParsedPropertyOverride {
    pub target_file_id: String,
    pub target_guid: String,
    pub property_path: String,
    pub value: Option<String>,
    pub obj_ref_file_id: Option<String>,
    pub obj_ref_guid: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ParsedRemoval {
    pub target_file_id: String,
    pub target_guid: String,
    pub removal_type: String,
}

#[derive(Debug, Clone)]
pub struct ParsedAddition {
    pub added_file_id: String,
    pub addition_type: String,
    pub parent_file_id: Option<String>,
    pub parent_guid: Option<String>,
}

#[derive(Debug, Default, Clone)]
pub struct ParsedPrefabInstance {
    pub source_prefab_guid: Option<String>,
    pub transform_parent_file_id: Option<String>,
    pub property_overrides: Vec<ParsedPropertyOverride>,
    pub removals: Vec<ParsedRemoval>,
    pub additions: Vec<ParsedAddition>,
}

#[derive(Debug, Clone)]
pub struct ParsedField {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone)]
pub struct ParsedAssetRef {
    pub field_path: String,
    pub to_guid: String,
    pub to_file_id: Option<String>,
    pub ref_type: Option<i64>,
}

#[derive(Debug, Default, Clone)]
pub struct ParsedGeneric {
    pub fields: Vec<ParsedField>,
    pub asset_refs: Vec<ParsedAssetRef>,
}

#[derive(Debug, Clone)]
pub struct ParsedStripped {
    pub prefab_instance_file_id: String,
    pub source_file_id: String,
    pub source_guid: String,
}

#[derive(Debug)]
pub enum BlockData {
    GameObject(ParsedGameObject),
    Transform(ParsedTransform),
    PrefabInstance(ParsedPrefabInstance),
    Stripped(ParsedStripped),
    Generic(ParsedGeneric),
}

impl BlockData {
    pub fn kind(&self) -> &'static str {
        match self {
            BlockData::GameObject(_)     => "GameObject",
            BlockData::Transform(_)      => "Transform",
            BlockData::PrefabInstance(_) => "PrefabInstance",
            BlockData::Stripped(_)       => "Stripped",
            BlockData::Generic(_)        => "Generic",
        }
    }
}

#[derive(Debug)]
pub struct ParsedBlock {
    pub class_id: String,
    pub local_id: String,
    pub data: BlockData,
    pub asset_refs: Vec<ParsedAssetRef>,
}
