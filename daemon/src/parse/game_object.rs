use crate::types::{ParsedAssetRef, ParsedGameObject};
use super::util::{body_to_str, parse_file_ref, parse_i64, split_kv};

pub fn parse(body: &[u8]) -> (ParsedGameObject, Vec<ParsedAssetRef>) {
    let mut go = ParsedGameObject::default();
    go.tag = "Untagged".to_string();
    go.is_active = true;
    let mut refs: Vec<ParsedAssetRef> = Vec::new();
    let mut in_components = false;

    let text = match body_to_str(body) {
        Some(s) => s,
        None => return (go, refs),
    };

    for line in text.lines() {
        let trimmed = line.trim();

        // Component list items: "- component: {fileID: X}" or "{fileID: X}"
        if in_components {
            if trimmed.starts_with("- component:") {
                let after = trimmed["- component:".len()..].trim();
                if let Some(r) = parse_file_ref(after) {
                    if r.file_id != "0" {
                        go.components.push(r.file_id);
                    }
                }
                continue;
            }
            // Stay in component mode only for blank lines or list items
            if trimmed.is_empty() || trimmed.starts_with('-') {
                continue;
            }
            // A non-indented, non-list line means we left the component block
            in_components = false;
        }

        let Some((key, val)) = split_kv(trimmed) else {
            continue;
        };

        match key {
            "m_Name" => {
                go.name = val.to_string();
            }
            "m_TagString" => {
                go.tag = val.to_string();
            }
            "m_Layer" => {
                go.layer = parse_i64(val);
            }
            "m_IsActive" => {
                go.is_active = val == "1";
            }
            "m_Component" => {
                in_components = true;
            }
            _ => {
                if let Some(r) = parse_file_ref(val) {
                    if let Some(guid) = r.guid {
                        refs.push(ParsedAssetRef {
                            field_path: key.to_string(),
                            to_guid: guid,
                            to_file_id: Some(r.file_id),
                            ref_type: r.ref_type,
                        });
                    }
                }
            }
        }
    }

    (go, refs)
}
