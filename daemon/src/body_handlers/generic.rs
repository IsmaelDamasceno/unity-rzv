/// Parses top-level scalar key-value fields from a YAML body.
/// Skips nested blocks (lines that start a sub-object or array).
pub fn parse(body: &[u8]) -> Vec<(String, String)> {
    let Ok(text) = std::str::from_utf8(body) else {
        return Vec::new();
    };

    let mut fields = Vec::new();

    for line in text.lines() {
        // Top-level fields in Unity YAML have exactly 2 spaces of indentation.
        if !line.starts_with("  ") || line.starts_with("   ") {
            continue;
        }
        let trimmed = &line[2..];

        let Some(colon) = trimmed.find(": ") else { continue };
        let key   = trimmed[..colon].trim();
        let value = trimmed[colon + 2..].trim();

        // Skip empty values and object/array references (handled elsewhere).
        if key.is_empty() || value.is_empty() || value.starts_with('{') || value.starts_with('-') {
            continue;
        }

        fields.push((key.to_string(), value.to_string()));
    }

    fields
}
