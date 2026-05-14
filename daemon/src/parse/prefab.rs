use crate::types::{ParsedAddition, ParsedAssetRef, ParsedPrefabInstance, ParsedPropertyOverride, ParsedRemoval};
use super::util::{body_to_str, parse_file_ref, split_kv};

#[derive(PartialEq)]
enum State {
    Root,
    InModification,
    InModifications,
    InCurrentMod,
    InRemovedComponents,
    InRemovedGameObjects,
    InAddedGameObjects,
    InAddedComponents,
}

#[derive(Default)]
struct PartialOverride {
    target_file_id: String,
    target_guid: String,
    property_path: String,
    value: Option<String>,
    obj_ref_file_id: Option<String>,
    obj_ref_guid: Option<String>,
}

impl PartialOverride {
    fn finish(self) -> Option<ParsedPropertyOverride> {
        if self.target_file_id.is_empty() || self.property_path.is_empty() {
            return None;
        }
        Some(ParsedPropertyOverride {
            target_file_id: self.target_file_id,
            target_guid: self.target_guid,
            property_path: self.property_path,
            value: self.value,
            obj_ref_file_id: self.obj_ref_file_id,
            obj_ref_guid: self.obj_ref_guid,
        })
    }
}

pub fn parse(body: &[u8]) -> (ParsedPrefabInstance, Vec<ParsedAssetRef>) {
    let mut pi = ParsedPrefabInstance::default();
    let mut refs: Vec<ParsedAssetRef> = Vec::new();
    let mut state = State::Root;
    let mut current_mod: Option<PartialOverride> = None;

    let text = match body_to_str(body) {
        Some(s) => s,
        None => return (pi, refs),
    };

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        match state {
            State::InModifications | State::InCurrentMod => {
                if trimmed.starts_with("- target:") {
                    // Commit previous item
                    if let Some(m) = current_mod.take() {
                        if let Some(o) = m.finish() {
                            pi.property_overrides.push(o);
                        }
                    }
                    let ref_str = trimmed["- target:".len()..].trim();
                    if let Some(r) = parse_file_ref(ref_str) {
                        current_mod = Some(PartialOverride {
                            target_file_id: r.file_id,
                            target_guid: r.guid.unwrap_or_default(),
                            ..Default::default()
                        });
                    }
                    state = State::InCurrentMod;
                    continue;
                }

                if state == State::InCurrentMod {
                    if let Some(cm) = current_mod.as_mut() {
                        if let Some((k, v)) = split_kv(trimmed) {
                            match k {
                                "propertyPath" => cm.property_path = v.to_string(),
                                "value" => cm.value = Some(v.to_string()),
                                "objectReference" => {
                                    if let Some(r) = parse_file_ref(v) {
                                        if r.file_id != "0" {
                                            cm.obj_ref_file_id = Some(r.file_id);
                                            cm.obj_ref_guid = r.guid;
                                        }
                                    }
                                }
                                // A recognised section key means we've left the mod list
                                "m_RemovedComponents"
                                | "m_RemovedGameObjects"
                                | "m_AddedGameObjects"
                                | "m_AddedComponents" => {
                                    if let Some(m) = current_mod.take() {
                                        if let Some(o) = m.finish() {
                                            pi.property_overrides.push(o);
                                        }
                                    }
                                    state = transition_section(k);
                                }
                                _ => {}
                            }
                            continue;
                        }
                    }
                }
                continue;
            }

            State::InRemovedComponents => {
                if let Some(ref_str) = trimmed.strip_prefix('-') {
                    if let Some(r) = parse_file_ref(ref_str.trim()) {
                        pi.removals.push(ParsedRemoval {
                            target_file_id: r.file_id,
                            target_guid: r.guid.unwrap_or_default(),
                            removal_type: "component".to_string(),
                        });
                    }
                    continue;
                }
                if let Some((k, _)) = split_kv(trimmed) {
                    if matches!(
                        k,
                        "m_RemovedGameObjects"
                            | "m_AddedGameObjects"
                            | "m_AddedComponents"
                            | "m_SourcePrefab"
                    ) {
                        state = transition_section(k);
                    }
                }
                // fall through to root handler for m_SourcePrefab etc.
            }

            State::InRemovedGameObjects => {
                if let Some(ref_str) = trimmed.strip_prefix('-') {
                    if let Some(r) = parse_file_ref(ref_str.trim()) {
                        pi.removals.push(ParsedRemoval {
                            target_file_id: r.file_id,
                            target_guid: r.guid.unwrap_or_default(),
                            removal_type: "game_object".to_string(),
                        });
                    }
                    continue;
                }
                if let Some((k, _)) = split_kv(trimmed) {
                    state = transition_section(k);
                }
            }

            State::InAddedGameObjects | State::InAddedComponents => {
                if let Some(ref_str) = trimmed.strip_prefix("- addedObject:") {
                    if let Some(r) = parse_file_ref(ref_str.trim()) {
                        pi.additions.push(ParsedAddition {
                            added_file_id: r.file_id,
                            addition_type: if state == State::InAddedGameObjects {
                                "game_object"
                            } else {
                                "component"
                            }
                            .to_string(),
                            parent_file_id: None,
                            parent_guid: None,
                        });
                    }
                    continue;
                }
                if let Some((k, _)) = split_kv(trimmed) {
                    state = transition_section(k);
                }
            }

            State::Root | State::InModification => {}
        }

        // Root / InModification handler
        let Some((key, val)) = split_kv(trimmed) else {
            continue;
        };

        match key {
            "m_SourcePrefab" => {
                if let Some(r) = parse_file_ref(val) {
                    if let Some(guid) = r.guid {
                        pi.source_prefab_guid = Some(guid.clone());
                        refs.push(ParsedAssetRef {
                            field_path: "m_SourcePrefab".to_string(),
                            to_guid: guid,
                            to_file_id: Some(r.file_id),
                            ref_type: r.ref_type,
                        });
                    }
                }
            }
            "m_TransformParent" => {
                if let Some(r) = parse_file_ref(val) {
                    if r.file_id != "0" {
                        pi.transform_parent_file_id = Some(r.file_id);
                    }
                }
            }
            "m_Modification" => state = State::InModification,
            "m_Modifications" => state = State::InModifications,
            "m_RemovedComponents" => state = State::InRemovedComponents,
            "m_RemovedGameObjects" => state = State::InRemovedGameObjects,
            "m_AddedGameObjects" => state = State::InAddedGameObjects,
            "m_AddedComponents" => state = State::InAddedComponents,
            _ => {}
        }
    }

    // Commit any trailing modification item
    if let Some(m) = current_mod.take() {
        if let Some(o) = m.finish() {
            pi.property_overrides.push(o);
        }
    }

    (pi, refs)
}

fn transition_section(key: &str) -> State {
    match key {
        "m_RemovedComponents" => State::InRemovedComponents,
        "m_RemovedGameObjects" => State::InRemovedGameObjects,
        "m_AddedGameObjects" => State::InAddedGameObjects,
        "m_AddedComponents" => State::InAddedComponents,
        "m_Modifications" => State::InModifications,
        _ => State::InModification,
    }
}
