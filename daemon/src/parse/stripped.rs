use crate::types::ParsedStripped;
use super::util::{body_to_str, parse_file_ref, split_kv};

pub fn parse(body: &[u8]) -> Option<ParsedStripped> {
    let text = body_to_str(body)?;
    let mut prefab_instance_file_id: Option<String> = None;
    let mut source_file_id: Option<String> = None;
    let mut source_guid: Option<String> = None;

    for line in text.lines() {
        let trimmed = line.trim();
        let Some((key, val)) = split_kv(trimmed) else {
            continue;
        };

        match key {
            "m_PrefabInstance" => {
                if let Some(r) = parse_file_ref(val) {
                    if r.file_id != "0" {
                        prefab_instance_file_id = Some(r.file_id);
                    }
                }
            }
            "m_CorrespondingSourceObject" => {
                if let Some(r) = parse_file_ref(val) {
                    source_file_id = Some(r.file_id);
                    source_guid = r.guid;
                }
            }
            _ => {}
        }
    }

    Some(ParsedStripped {
        prefab_instance_file_id: prefab_instance_file_id?,
        source_file_id: source_file_id.unwrap_or_default(),
        source_guid: source_guid.unwrap_or_default(),
    })
}
