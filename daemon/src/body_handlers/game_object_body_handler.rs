
fn handle_game_object(data: &[u8]) {
    let patterns = &["m_Name", "m_Layer", "m_IsActive", "m_TagString", "m_Component"];
    let ac = AhoCorasick::new(patterns).unwrap();

    for mat in ac.find_iter(data) {
        let key = patterns[mat.pattern()];
        let after_key = &data[mat.end()..];

        if let Some(line_end) = memchr::memchr(b'\n', after_key) {
            let raw_value = &after_key[..line_end];
            let trim_value = raw_value.strip_prefix(b": ").unwrap_or(raw_value);

            if let Ok(value_str) = std::str::from_utf8(trim_value) {
                let final_value = value_str.trim();
                println!("{}: {}", key, final_value);
            }
        }
    }
}
