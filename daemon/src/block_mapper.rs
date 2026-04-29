use crate::{Finders, find_next_block, parse_header};

const UC_PREFAB_INSTANCE: &str = "1004";

/// Finds the absolute index of the block separator within `data`, starting from `offset`,
/// and returns the index immediately following the separator.
///
/// This is useful for advancing a cursor or "skipping" a block of data during parsing.
///
/// # Arguments
/// * `data` - The byte slice to search through.
/// * `finders` - A struct containing the `sep` finder used to locate the needle.
/// * `offset` - The starting position in `data` from which to begin searching.
///
/// # Returns
/// * `Some(usize)` - The absolute index of the first byte *after* the found separator.
/// * `None` - If the separator was not found in the range `data[offset..]`.
///
/// # Panics
/// This function will panic if `offset` is greater than `data.len()`.
///
/// # Examples
///
/// ```
/// haystack: [0, 1, 2, 3, 4], needle: [2]
/// If offset is 0, finds '2' at index 2, skips 1 byte, returns Some(3).
/// ```
fn find_next_block_start(data: &[u8], finders: &Finders, offset: usize) -> Option<usize> {
    if let Some(pos) = finders.sep.find(&data[offset..]) {
        Some(offset + pos + finders.sep.needle().len())
    } else {
        None
    }
}

fn parse_unity_doc(data: &[u8], finders: &Finders) -> anyhow::Result<()> {
    let mut offset = 0;
    while offset < data.len() {
        let block_start: usize = match find_next_block_start(&data, &finders, offset) {
            Some(pos) => pos,
            None => break,
        };

        let (class_id, local_id, body_start) = parse_header(data, block_start, &finders)?;
        let body_end = find_next_block(&data, body_start, &finders);

        match class_id {
            UC_PREFAB_INSTANCE => {
                handle_prefab(&data[body_start..body_end]);
            }
            _ => {
                handle_generic(&data[body_start..body_end]);
            }
        }

        offset = body_end;
    }
    Ok(())
}
