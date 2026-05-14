use crate::types::{ParsedAssetRef, ParsedTransform};
use super::util::{body_to_str, parse_f64, parse_file_ref, parse_i64, split_kv};

#[derive(PartialEq)]
enum State {
    Root,
    InPosition,
    InRotation,
    InScale,
}

pub fn parse(body: &[u8]) -> (ParsedTransform, Vec<ParsedAssetRef>) {
    let mut t = ParsedTransform::identity();
    let mut refs: Vec<ParsedAssetRef> = Vec::new();
    let mut state = State::Root;

    let text = match body_to_str(body) {
        Some(s) => s,
        None => return (t, refs),
    };

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Handle sub-struct fields (x/y/z/w) before root dispatch
        match state {
            State::InPosition => {
                if let Some((k, v)) = split_kv(trimmed) {
                    match k {
                        "x" => t.pos_x = parse_f64(v),
                        "y" => t.pos_y = parse_f64(v),
                        "z" => {
                            t.pos_z = parse_f64(v);
                            state = State::Root;
                        }
                        _ => state = State::Root,
                    }
                } else {
                    state = State::Root;
                }
                continue;
            }
            State::InRotation => {
                if let Some((k, v)) = split_kv(trimmed) {
                    match k {
                        "x" => t.rot_x = parse_f64(v),
                        "y" => t.rot_y = parse_f64(v),
                        "z" => t.rot_z = parse_f64(v),
                        "w" => {
                            t.rot_w = parse_f64(v);
                            state = State::Root;
                        }
                        _ => state = State::Root,
                    }
                } else {
                    state = State::Root;
                }
                continue;
            }
            State::InScale => {
                if let Some((k, v)) = split_kv(trimmed) {
                    match k {
                        "x" => t.scale_x = parse_f64(v),
                        "y" => t.scale_y = parse_f64(v),
                        "z" => {
                            t.scale_z = parse_f64(v);
                            state = State::Root;
                        }
                        _ => state = State::Root,
                    }
                } else {
                    state = State::Root;
                }
                continue;
            }
            State::Root => {}
        }

        let Some((key, val)) = split_kv(trimmed) else {
            continue;
        };

        match key {
            "m_LocalPosition" => state = State::InPosition,
            "m_LocalRotation" => state = State::InRotation,
            "m_LocalScale" => state = State::InScale,
            "m_RootOrder" => t.root_order = parse_i64(val),
            "m_GameObject" => {
                if let Some(r) = parse_file_ref(val) {
                    if r.file_id != "0" {
                        t.game_object_file_id = Some(r.file_id);
                    }
                }
            }
            "m_Father" => {
                if let Some(r) = parse_file_ref(val) {
                    if r.file_id != "0" {
                        if let Some(ref guid) = r.guid {
                            refs.push(ParsedAssetRef {
                                field_path: "m_Father".to_string(),
                                to_guid: guid.clone(),
                                to_file_id: Some(r.file_id.clone()),
                                ref_type: r.ref_type,
                            });
                        }
                        t.parent_file_id = Some(r.file_id);
                    }
                }
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

    (t, refs)
}
