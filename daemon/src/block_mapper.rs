use memchr::memmem;
use std::collections::HashMap;
use crate::{Finders, parse_header};

type BlockMap = HashMap<String, (usize, usize)>;

fn find_and_skip_block_separator(data: &[u8], finders: &Finders, offset: &mut usize) -> Option<usize> {
    let start_search = *offset;

    if let Some(pos) = finders.sep.find(&data[start_search..]) {
        let found_at = start_search + pos;
        *offset = pos + finders.sep.needle().len();
        Some(pos)
    } else {
        None
    }
}

fn build_block_index(data: &[u8], finders: &Finders) -> BlockMap {
    let mut map = HashMap::new();

    let mut offset = 0;
    while offset < data.len() {
        let block_start = find_and_skip_block_separator(&data, &finders, &mut offset);

        let (class_id, local_id, body_start) = parse_header(data, offset, &finders)?;

        // Slice just the header line to extract the Local ID
        let header_chunk = &data[start_pos..end_pos];
        let line_end = header_chunk
            .iter()
            .position(|&b| b == b'\n')
            .unwrap_or(header_chunk.len());
        let header_line = &header_chunk[..line_end];

        // Extract Local ID (the part after '&')
        if let Some(amp_pos) = header_line.iter().position(|&b| b == b'&') {
            let id_bytes = &header_line[amp_pos + 1..];
            // trim() doesn't exist for [u8], so we trim manually or via str
            if let Ok(id_str) = std::str::from_utf8(id_bytes) {
                map.insert(id_str.trim().to_string(), (start_pos, end_pos));
            }
        }
    }

    map
}
