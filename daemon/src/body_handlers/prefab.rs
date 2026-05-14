use crate::parser::{extract_file_id, extract_guid};
use super::{ParsedPrefabInstance, ParsedPropertyOverride, ParsedRemoval};

pub fn parse(body: &[u8]) -> ParsedPrefabInstance {
    let Ok(text) = std::str::from_utf8(body) else {
        return ParsedPrefabInstance {
            source_prefab_guid:        None,
            transform_parent_local_id: None,
            property_overrides:        Vec::new(),
            removals:                  Vec::new(),
        };
    };

    let mut source_prefab_guid        = None;
    let mut transform_parent_local_id = None;
    let mut property_overrides        = Vec::new();
    let mut removals                  = Vec::new();

    #[derive(PartialEq)]
    enum Section { None, Modifications, RemovedComponents, RemovedGameObjects }
    let mut section = Section::None;

    // Pending modification fields (assembled line by line)
    let mut cur_file_id:      Option<String> = None;
    let mut cur_guid:         Option<String> = None;
    let mut cur_path:         Option<String> = None;
    let mut cur_value:        Option<String> = None;
    let mut cur_obj_file_id:  Option<String> = None;
    let mut cur_obj_guid:     Option<String> = None;

    for line in text.lines() {
        let trimmed = line.trim();

        if let Some(rest) = trimmed.strip_prefix("m_SourcePrefab: ") {
            source_prefab_guid = extract_guid(rest).map(str::to_string);
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("m_TransformParent: ") {
            transform_parent_local_id = extract_file_id(rest).map(str::to_string);
            continue;
        }

        match trimmed {
            "m_Modifications:" => { section = Section::Modifications; continue; }
            "m_RemovedComponents:" => {
                flush(&mut cur_file_id, &mut cur_guid, &mut cur_path,
                      &mut cur_value, &mut cur_obj_file_id, &mut cur_obj_guid,
                      &mut property_overrides);
                section = Section::RemovedComponents;
                continue;
            }
            "m_RemovedGameObjects:" => {
                section = Section::RemovedGameObjects;
                continue;
            }
            "m_AddedGameObjects:" | "m_AddedComponents:" => {
                section = Section::None;
                continue;
            }
            _ => {}
        }

        match section {
            Section::Modifications => {
                if let Some(rest) = trimmed.strip_prefix("- target: ") {
                    flush(&mut cur_file_id, &mut cur_guid, &mut cur_path,
                          &mut cur_value, &mut cur_obj_file_id, &mut cur_obj_guid,
                          &mut property_overrides);
                    cur_file_id = extract_file_id(rest).map(str::to_string);
                    cur_guid    = extract_guid(rest).map(str::to_string);
                } else if let Some(rest) = trimmed.strip_prefix("propertyPath: ") {
                    cur_path = Some(rest.to_string());
                } else if let Some(rest) = trimmed.strip_prefix("value: ") {
                    cur_value = Some(rest.to_string());
                } else if let Some(rest) = trimmed.strip_prefix("objectReference: ") {
                    cur_obj_file_id = extract_file_id(rest).map(str::to_string);
                    cur_obj_guid    = extract_guid(rest).map(str::to_string);
                }
            }
            Section::RemovedComponents => {
                if let Some(rest) = trimmed.strip_prefix("- ") {
                    if let (Some(fid), Some(guid)) =
                        (extract_file_id(rest), extract_guid(rest))
                    {
                        removals.push(ParsedRemoval {
                            target_file_id: fid.to_string(),
                            target_guid:    guid.to_string(),
                            removal_type:   "component",
                        });
                    }
                }
            }
            Section::RemovedGameObjects => {
                if let Some(rest) = trimmed.strip_prefix("- ") {
                    if let (Some(fid), Some(guid)) =
                        (extract_file_id(rest), extract_guid(rest))
                    {
                        removals.push(ParsedRemoval {
                            target_file_id: fid.to_string(),
                            target_guid:    guid.to_string(),
                            removal_type:   "game_object",
                        });
                    }
                }
            }
            Section::None => {}
        }
    }

    flush(&mut cur_file_id, &mut cur_guid, &mut cur_path,
          &mut cur_value, &mut cur_obj_file_id, &mut cur_obj_guid,
          &mut property_overrides);

    ParsedPrefabInstance { source_prefab_guid, transform_parent_local_id, property_overrides, removals }
}

fn flush(
    file_id:     &mut Option<String>,
    guid:        &mut Option<String>,
    path:        &mut Option<String>,
    value:       &mut Option<String>,
    obj_file_id: &mut Option<String>,
    obj_guid:    &mut Option<String>,
    out:         &mut Vec<ParsedPropertyOverride>,
) {
    if let (Some(fid), Some(g), Some(p)) = (file_id.take(), guid.take(), path.take()) {
        out.push(ParsedPropertyOverride {
            target_file_id:  fid,
            target_guid:     g,
            property_path:   p,
            value:           value.take(),
            obj_ref_file_id: obj_file_id.take(),
            obj_ref_guid:    obj_guid.take(),
        });
    } else {
        file_id.take();
        guid.take();
        path.take();
        value.take();
        obj_file_id.take();
        obj_guid.take();
    }
}
