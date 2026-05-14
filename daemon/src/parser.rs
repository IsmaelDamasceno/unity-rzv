use memchr::memmem;

pub struct Finders {
    pub sep:       memmem::Finder<'static>,
    pub ampersand: memmem::Finder<'static>,
    pub line_feed: memmem::Finder<'static>,
    pub space:     memmem::Finder<'static>,
}

impl Finders {
    pub fn new() -> Self {
        Self {
            sep:       memmem::Finder::new(b"--- !u!"),
            ampersand: memmem::Finder::new(b"&"),
            line_feed: memmem::Finder::new(b"\n"),
            space:     memmem::Finder::new(b" "),
        }
    }
}

pub struct ParsedHeader {
    pub class_id:   i64,
    pub local_id:   String,
    pub is_stripped: bool,
    pub body_start: usize,
}

/// Parses `--- !u!<class_id> &<local_id>[ stripped]` starting at `offset`.
/// `offset` points to the first byte after the `--- !u!` separator.
pub fn parse_header(data: &[u8], offset: usize, finders: &Finders) -> anyhow::Result<ParsedHeader> {
    let slice = &data[offset..];

    let line_end = finders.line_feed.find(slice)
        .ok_or_else(|| anyhow::anyhow!("no newline on header at offset {offset}"))?;
    let header = &slice[..line_end];

    let space = finders.space.find(header)
        .ok_or_else(|| anyhow::anyhow!("no space on header at offset {offset}"))?;
    let class_id: i64 = std::str::from_utf8(&header[..space])?.parse()?;

    let amp = finders.ampersand.find(header)
        .ok_or_else(|| anyhow::anyhow!("no '&' on header at offset {offset}"))?;
    let id_start = amp + 1;
    let id_end = header[id_start..]
        .iter()
        .position(|&b| b == b' ' || b == b'\r')
        .unwrap_or(header.len() - id_start);
    let local_id = std::str::from_utf8(&header[id_start..id_start + id_end])?.to_string();

    let is_stripped = header[id_start + id_end..].windows(8).any(|w| w == b"stripped");

    Ok(ParsedHeader {
        class_id,
        local_id,
        is_stripped,
        body_start: offset + line_end + 1,
    })
}

/// Returns the absolute byte position of the next `--- !u!` separator at or
/// after `from`, or `data.len()` if there are no more blocks.
pub fn find_next_block(data: &[u8], from: usize, finders: &Finders) -> usize {
    finders.sep.find(&data[from..]).map(|p| from + p).unwrap_or(data.len())
}

// ── Small parsing helpers used by multiple body handlers ─────────────────────

/// Extracts the value of a `{fileID: X}` reference, returning `None` for `0`.
pub fn extract_file_id(s: &str) -> Option<&str> {
    let start = s.find("fileID: ")? + 8;
    let end = s[start..]
        .find(|c: char| !c.is_ascii_digit() && c != '-')
        .unwrap_or(s.len() - start);
    let id = &s[start..start + end];
    if id == "0" { None } else { Some(id) }
}

/// Extracts the `guid` from a `{fileID: X, guid: Y, type: Z}` reference.
pub fn extract_guid(s: &str) -> Option<&str> {
    let start = s.find("guid: ")? + 6;
    let end = s[start..]
        .find(|c: char| !c.is_ascii_alphanumeric())
        .unwrap_or(s.len() - start);
    Some(&s[start..start + end])
}
