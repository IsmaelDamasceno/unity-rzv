use tracing::{trace, warn};

use crate::types::ParsedBlock;
use crate::{find_next_block, parse_header, Finders};

/// Parses every `--- !u!` block in `data` and returns them as a flat list.
/// Errors on individual blocks are logged and skipped rather than aborting.
pub fn parse_unity_doc(data: &[u8], finders: &Finders) -> anyhow::Result<Vec<ParsedBlock>> {
    let mut blocks = Vec::new();
    let mut offset = 0;

    while offset < data.len() {
        // Advance to the next separator and step past it.
        let sep_pos = match finders.sep.find(&data[offset..]) {
            Some(p) => offset + p + finders.sep.needle().len(),
            None    => break,
        };

        let (class_id, local_id, is_stripped, body_start) =
            match parse_header(data, sep_pos, finders) {
                Ok(h)  => h,
                Err(e) => {
                    warn!(byte = sep_pos, error = %e, "skipping malformed header");
                    offset = sep_pos;
                    continue;
                }
            };

        let body_end = find_next_block(data, body_start, finders);
        let body     = &data[body_start..body_end];

        let (block_data, asset_refs) = crate::parse::dispatch_with_refs(class_id, is_stripped, body);

        trace!(
            class_id,
            local_id,
            is_stripped,
            body_bytes = body.len(),
            asset_refs  = asset_refs.len(),
            kind        = block_data.kind(),
            "parsed block"
        );

        blocks.push(ParsedBlock {
            class_id:   class_id.to_string(),
            local_id:   local_id.to_string(),
            data:       block_data,
            asset_refs,
        });

        offset = body_end;
    }

    Ok(blocks)
}
