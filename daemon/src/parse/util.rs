use crate::types::FileRef;

/// Splits `"  m_Name: Player"` → `Some(("m_Name", "Player"))`.
/// Also handles struct-opener lines `"m_LocalPosition:"` → `Some(("m_LocalPosition", ""))`.
/// Returns `None` for blank lines, comments, and YAML list markers (`-`).
pub fn split_kv(line: &str) -> Option<(&str, &str)> {
    let t = line.trim();
    if t.is_empty() || t.starts_with('#') || t.starts_with('-') {
        return None;
    }
    if let Some(pos) = t.find(": ") {
        return Some((t[..pos].trim(), t[pos + 2..].trim()));
    }
    if let Some(key) = t.strip_suffix(':') {
        return Some((key.trim(), ""));
    }
    None
}

/// Parses `{fileID: 12345}` or `{fileID: 12345, guid: abc, type: 3}`.
pub fn parse_file_ref(s: &str) -> Option<FileRef> {
    let inner = s.trim().strip_prefix('{')?.strip_suffix('}')?;
    let mut file_id = String::new();
    let mut guid: Option<String> = None;
    let mut ref_type: Option<i64> = None;

    for part in inner.split(',') {
        let part = part.trim();
        if let Some(v) = part.strip_prefix("fileID:") {
            file_id = v.trim().to_string();
        } else if let Some(v) = part.strip_prefix("guid:") {
            let g = v.trim();
            if !g.is_empty() {
                guid = Some(g.to_string());
            }
        } else if let Some(v) = part.strip_prefix("type:") {
            ref_type = v.trim().parse().ok();
        }
    }

    if file_id.is_empty() {
        return None;
    }
    Some(FileRef { file_id, guid, ref_type })
}

pub fn parse_f64(s: &str) -> f64 {
    s.trim().parse().unwrap_or(0.0)
}

pub fn parse_i64(s: &str) -> i64 {
    s.trim().parse().unwrap_or(0)
}

pub fn body_to_str(body: &[u8]) -> Option<&str> {
    std::str::from_utf8(body).ok()
}
