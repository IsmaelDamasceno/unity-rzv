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
    let id_start = offset + ampersand_pos + finders.ampersand.needle().len();

    let id_end = finders
        .line_feed
        .find(&data[id_start..])
        .ok_or_else(|| anyhow::anyhow!("no newline after local ID at offset {}", id_start))?;
    let local_id = str::from_utf8(&data[id_start..id_start + id_end])?;

    let body_start = id_start + id_end + finders.line_feed.needle().len();
    Ok((class_id, local_id, body_start))
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

/// Returns the absolute byte of the next separator "--- !u!" block after `from`,
/// or `data.len()` if there are no more blocks.
fn find_next_block(data: &[u8], from: usize, finders: &Finders) -> usize {
    finders
        .sep
        .find(&data[from..])
        .map(|p| from + p)
        .unwrap_or(data.len())
}

fn main() -> anyhow::Result<()> {
    let path = std::env::args().nth(1).expect("No path provided");

    let file = std::fs::File::open(path)?;
    let mmap = unsafe { memmap2::Mmap::map(&file)? };

    let data: &[u8] = &mmap;
    let finders = Finders::new();
    block_mapper::parse_unity_doc(&data, &finders)?;

    Ok(())
}
