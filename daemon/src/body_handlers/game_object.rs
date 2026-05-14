use aho_corasick::AhoCorasick;
use super::ParsedGameObject;

pub fn parse(body: &[u8]) -> ParsedGameObject {
    // Pattern indices
    const M_NAME:      usize = 0;
    const M_TAG:       usize = 1;
    const M_LAYER:     usize = 2;
    const M_IS_ACTIVE: usize = 3;
    const M_COMPONENT: usize = 4;

    let ac = AhoCorasick::new([
        b"m_Name: ".as_ref(),
        b"m_TagString: ".as_ref(),
        b"m_Layer: ".as_ref(),
        b"m_IsActive: ".as_ref(),
        b"  - component: {fileID: ".as_ref(),
    ])
    .expect("static AhoCorasick patterns are valid");

    let mut name      = String::new();
    let mut tag       = String::from("Untagged");
    let mut layer     = 0i64;
    let mut is_active = true;
    let mut component_local_ids: Vec<String> = Vec::new();

    for mat in ac.find_iter(body) {
        let after    = &body[mat.end()..];
        let line_end = memchr::memchr(b'\n', after).unwrap_or(after.len());
        let Ok(raw)  = std::str::from_utf8(&after[..line_end]) else { continue };
        let value    = raw.trim_end_matches(['\r', ' ']);

        match mat.pattern().as_usize() {
            M_NAME      => name      = value.to_string(),
            M_TAG       => tag       = value.to_string(),
            M_LAYER     => layer     = value.parse().unwrap_or(0),
            M_IS_ACTIVE => is_active = value != "0",
            M_COMPONENT => {
                // value is the remainder of "fileID: X}" — grab digits up to '}'
                let end = value.find('}').unwrap_or(value.len());
                let id  = value[..end].trim();
                if !id.is_empty() && id != "0" {
                    component_local_ids.push(id.to_string());
                }
            }
            _ => {}
        }
    }

    ParsedGameObject { name, tag, layer, is_active, component_local_ids }
}
