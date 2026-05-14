use crate::parser::{extract_file_id};
use super::ParsedTransform;

pub fn parse(body: &[u8]) -> ParsedTransform {
    let Ok(text) = std::str::from_utf8(body) else {
        return ParsedTransform::identity();
    };

    let lines: Vec<&str> = text.lines().collect();
    let mut t = ParsedTransform::identity();
    let mut i = 0;

    while i < lines.len() {
        let trimmed = lines[i].trim();

        if trimmed == "m_LocalPosition:" {
            parse_xyz(&lines, i + 1, &mut t.pos_x, &mut t.pos_y, &mut t.pos_z);
        } else if trimmed == "m_LocalRotation:" {
            parse_xyzw(&lines, i + 1, &mut t.rot_x, &mut t.rot_y, &mut t.rot_z, &mut t.rot_w);
        } else if trimmed == "m_LocalScale:" {
            parse_xyz(&lines, i + 1, &mut t.scale_x, &mut t.scale_y, &mut t.scale_z);
        } else if let Some(rest) = trimmed.strip_prefix("m_Father: ") {
            t.parent_local_id = extract_file_id(rest).map(str::to_string);
        } else if let Some(rest) = trimmed.strip_prefix("m_RootOrder: ") {
            t.sibling_index = rest.trim().parse().unwrap_or(0);
        } else if let Some(rest) = trimmed.strip_prefix("m_GameObject: ") {
            t.game_object_local_id = extract_file_id(rest).map(str::to_string);
        }

        i += 1;
    }

    t
}

fn parse_xyz(lines: &[&str], start: usize, x: &mut f64, y: &mut f64, z: &mut f64) {
    for line in lines.iter().skip(start).take(3) {
        let t = line.trim();
        if      let Some(v) = t.strip_prefix("x: ") { *x = v.parse().unwrap_or(0.0); }
        else if let Some(v) = t.strip_prefix("y: ") { *y = v.parse().unwrap_or(0.0); }
        else if let Some(v) = t.strip_prefix("z: ") { *z = v.parse().unwrap_or(0.0); }
    }
}

fn parse_xyzw(lines: &[&str], start: usize, x: &mut f64, y: &mut f64, z: &mut f64, w: &mut f64) {
    for line in lines.iter().skip(start).take(4) {
        let t = line.trim();
        if      let Some(v) = t.strip_prefix("x: ") { *x = v.parse().unwrap_or(0.0); }
        else if let Some(v) = t.strip_prefix("y: ") { *y = v.parse().unwrap_or(0.0); }
        else if let Some(v) = t.strip_prefix("z: ") { *z = v.parse().unwrap_or(0.0); }
        else if let Some(v) = t.strip_prefix("w: ") { *w = v.parse().unwrap_or(1.0); }
    }
}
