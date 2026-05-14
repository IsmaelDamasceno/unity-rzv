pub mod game_object;
pub mod generic;
pub mod prefab;
pub mod stripped;
pub mod transform;
pub mod util;

use crate::types::BlockData;

// Known class IDs
const CLASS_GAME_OBJECT: &str = "1";
const CLASS_TRANSFORM: &str = "4";
const CLASS_RECT_TRANSFORM: &str = "224";
const CLASS_PREFAB_INSTANCE_OLD: &str = "1001";
const CLASS_PREFAB_INSTANCE: &str = "1004";

/// Dispatches a block body to the correct parser and returns the typed result.
pub fn dispatch(class_id: &str, is_stripped: bool, body: &[u8]) -> BlockData {
    if is_stripped {
        return match stripped::parse(body) {
            Some(s) => BlockData::Stripped(s),
            None => BlockData::Generic(generic::parse(body)),
        };
    }

    match class_id {
        CLASS_GAME_OBJECT => {
            let (go, _refs) = game_object::parse(body);
            BlockData::GameObject(go)
        }
        CLASS_TRANSFORM | CLASS_RECT_TRANSFORM => {
            let (t, _refs) = transform::parse(body);
            BlockData::Transform(t)
        }
        CLASS_PREFAB_INSTANCE | CLASS_PREFAB_INSTANCE_OLD => {
            let (pi, _refs) = prefab::parse(body);
            BlockData::PrefabInstance(pi)
        }
        _ => BlockData::Generic(generic::parse(body)),
    }
}

/// Same as dispatch but also returns cross-asset references collected during parsing.
pub fn dispatch_with_refs(
    class_id: &str,
    is_stripped: bool,
    body: &[u8],
) -> (BlockData, Vec<crate::types::ParsedAssetRef>) {
    use crate::types::ParsedAssetRef;

    if is_stripped {
        return match stripped::parse(body) {
            Some(s) => (BlockData::Stripped(s), Vec::new()),
            None => {
                let g = generic::parse(body);
                let refs = g.asset_refs.clone();
                (BlockData::Generic(g), refs)
            }
        };
    }

    match class_id {
        CLASS_GAME_OBJECT => {
            let (go, refs) = game_object::parse(body);
            (BlockData::GameObject(go), refs)
        }
        CLASS_TRANSFORM | CLASS_RECT_TRANSFORM => {
            let (t, refs) = transform::parse(body);
            (BlockData::Transform(t), refs)
        }
        CLASS_PREFAB_INSTANCE | CLASS_PREFAB_INSTANCE_OLD => {
            let (pi, refs) = prefab::parse(body);
            (BlockData::PrefabInstance(pi), refs)
        }
        _ => {
            let g = generic::parse(body);
            let refs: Vec<ParsedAssetRef> = g.asset_refs.clone();
            (BlockData::Generic(g), refs)
        }
    }
}
