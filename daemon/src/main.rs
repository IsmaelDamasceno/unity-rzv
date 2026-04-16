use memchr::memmem;
use prost::Message;
use std::io::{self, Write};
use std::str;
mod block_mapper;

pub mod unity_data {
    include!(concat!(env!("OUT_DIR"), "/unity_tool.rs"));
}

struct Finders {
    sep: memmem::Finder<'static>,
    whitespace: memmem::Finder<'static>,
    ampersand: memmem::Finder<'static>,
    line_feed: memmem::Finder<'static>,
    name: memmem::Finder<'static>,
}

impl Finders {
    fn new() -> Self {
        Self {
            sep: memmem::Finder::new(b"--- !u!"),
            whitespace: memmem::Finder::new(b" "),
            ampersand: memmem::Finder::new(b"&"),
            line_feed: memmem::Finder::new(b"\n"),
            name: memmem::Finder::new(b"m_Name: "),
        }
    }
}

/// Parses the header line "--- !u!<class_id> &<local_id>"
/// Returns (class_id, local_id, byte position just after the header newline)
fn parse_header<'a>(
    data: &'a [u8],
    offset: usize,
    finders: &Finders,
) -> anyhow::Result<(&'a str, &'a str, usize)> {
    let class_id_end = finders
        .whitespace
        .find(&data[offset..])
        .ok_or_else(|| anyhow::anyhow!("no space after class ID at offset {}", offset))?;
    let class_id = str::from_utf8(&data[offset..offset + class_id_end])?;

    let header_line_end = finders
        .line_feed
        .find(&data[offset..])
        .ok_or_else(|| anyhow::anyhow!("no newline after class ID at offset {}", offset))?;
    let header_line = &data[offset..offset + header_line_end];

    let ampersand_pos = finders
        .ampersand
        .find(header_line)
        .ok_or_else(|| anyhow::anyhow!("no '&' on header line at offset {}", offset))?;
    let id_start = offset + ampersand_pos + 1;

    let id_end = finders
        .line_feed
        .find(&data[id_start..])
        .ok_or_else(|| anyhow::anyhow!("no newline after local ID at offset {}", id_start))?;
    let local_id = str::from_utf8(&data[id_start..id_start + id_end])?;

    let block_start = id_start + id_end + 1;
    Ok((class_id, local_id, block_start))
}

/// Searches for "m_Name: <value>" within the block body.
/// Returns the name, or "Unnamed" if not found.
fn parse_name<'a>(
    data: &'a [u8],
    block_start: usize,
    block_end: usize,
    finders: &Finders,
) -> &'a str {
    let window = &data[block_start..block_end];

    let Some(name_pos) = finders.name.find(window) else {
        return "Unnamed";
    };

    let name_start = name_pos + finders.name.needle().len();
    let name_line_end = finders
        .line_feed
        .find(&window[name_start..])
        .unwrap_or(window.len() - name_start);

    str::from_utf8(&window[name_start..name_start + name_line_end])
        .unwrap_or("Unnamed")
        .trim()
}

fn parse_obj_game_object<'a>(
    data: &'a [u8],
    block_start: usize,
    block_end: usize,
    finders: &Finders,
) -> unity_data::UnityObjGameObject {
    let mut obj = unity_data::UnityObjGameObject::default();
    
    // Get the specific slice for this block
    let block_data = &data[block_start..block_end];
    let content = std::str::from_utf8(block_data).unwrap_or("");


}

/// Returns the byte offset of the next "--- !u!" block after `from`,
/// or `data.len()` if there are no more blocks.
fn find_next_block(data: &[u8], from: usize, finders: &Finders) -> usize {
    finders
        .sep
        .find(&data[from..])
        .map(|p| from + p)
        .unwrap_or(data.len())
}

pub fn parse_manually(path: &str) -> anyhow::Result<Vec<unity_data::UnityObjGameObject>> {
    let file = std::fs::File::open(path)?;
    let mmap = unsafe { memmap2::Mmap::map(&file)? };
    let finders = Finders::new();

    let data: &[u8] = &mmap;
    let mut offset = 0;
    let mut objects = Vec::new();

    while offset < data.len() {
        let block_start = match finders.sep.find(&data[offset..]) {
            Some(pos) => offset + pos,
            None => break,
        };
        offset = block_start + finders.sep.needle().len();

        let (class_id, local_id, body_start) = parse_header(data, offset, &finders)?;
        let next = find_next_block(data, body_start, &finders);
        let obj_name = parse_name(data, body_start, next, &finders);

        objects.push(unity_data::UnityObjectInfo {
            gui_id: 0,
            local_id: local_id.to_string(),
            object_name: format!("{}", obj_name),
            class_id: class_id.to_string(),
        });

        offset = next;
    }

    Ok(objects)
}

fn main() -> anyhow::Result<()> {
    let path = std::env::args().nth(1).expect("No path provided");

    let mut result = unity_data::SceneResult::default();
    result.objects = parse_manually(&path)?;

    let mut buf = Vec::with_capacity(result.encoded_len());
    result.encode(&mut buf)?;

    let mut stdout = io::stdout().lock();
    stdout.write_all(&buf)?;
    stdout.flush()?;

    Ok(())
}
