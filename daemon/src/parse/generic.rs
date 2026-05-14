use tracing::trace;

use crate::types::{ParsedAssetRef, ParsedField, ParsedGeneric};
use super::util::{body_to_str, parse_file_ref, split_kv};

pub fn parse(body: &[u8]) -> ParsedGeneric {
    let mut g = ParsedGeneric::default();

    let text = match body_to_str(body) {
        Some(s) => s,
        None => return g,
    };

    let mut arr_field: Option<String> = None; // field name of the active array
    let mut arr_index: usize = 0;
    let mut arr_item_indent: usize = 0; // indent of the `-` markers
    let mut in_struct_item = false;     // current item has sub-fields

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let indent = line.len() - line.trim_start().len();

        // ── Array mode ────────────────────────────────────────────────────────
        if let Some(ref field) = arr_field {
            if trimmed.starts_with('-') && indent == arr_item_indent {
                if in_struct_item {
                    arr_index += 1;
                }
                in_struct_item = false;

                let rest = trimmed[1..].trim();
                if rest.is_empty() {
                    in_struct_item = true;
                } else if let Some((k, v)) = split_kv(rest) {
                    // `- key: val` — first field of a struct item
                    let path = format!("{field}.{arr_index}.{k}");
                    store_field(&mut g, &path, v);
                    in_struct_item = true;
                } else {
                    // `- scalar`
                    let path = format!("{field}.{arr_index}");
                    store_field(&mut g, &path, rest);
                    arr_index += 1;
                }
                continue;
            }

            if in_struct_item && indent > arr_item_indent {
                // Continuation fields of a struct item
                if let Some((k, v)) = split_kv(trimmed) {
                    let path = format!("{field}.{arr_index}.{k}");
                    store_field(&mut g, &path, v);
                }
                continue;
            }

            // Exited the array — fall through to normal parsing below
            arr_field = None;
            arr_index = 0;
            in_struct_item = false;
        }

        // ── Normal key-value mode ─────────────────────────────────────────────
        let Some((key, val)) = split_kv(trimmed) else {
            continue;
        };

        if val.is_empty() {
            // Potential array opener — record and wait for first `-`
            arr_field = Some(key.to_string());
            arr_index = 0;
            arr_item_indent = indent;
            in_struct_item = false;
            continue;
        }

        store_field(&mut g, key, val);
    }

    g
}

fn store_field(g: &mut ParsedGeneric, key: &str, val: &str) {
    if val.is_empty() {
        return;
    }

    if let Some(r) = parse_file_ref(val) {
        if let Some(guid) = r.guid {
            trace!(key, guid = guid.as_str(), file_id = r.file_id.as_str(), "asset ref field");
            g.asset_refs.push(ParsedAssetRef {
                field_path: key.to_string(),
                to_guid: guid,
                to_file_id: Some(r.file_id),
                ref_type: r.ref_type,
            });
            return;
        }
    }

    trace!(key, value = val, "field");
    g.fields.push(ParsedField {
        key: key.to_string(),
        value: val.to_string(),
    });
}
