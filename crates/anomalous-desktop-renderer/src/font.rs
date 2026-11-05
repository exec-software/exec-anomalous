use crate::capsule::{capsule_rect, capsule_slot, label_selector};

pub(crate) const glyph_width: u32 = 5;
pub(crate) const glyph_height: u32 = 7;
const supported_glyphs: [u8; 20] = *b"ACDEGHIKLMNOPRSTUVWY";

/// Frozen build-time fingerprint of every supported glyph and row.
///
/// The target image retains this marker so normal-image exclusion can prove
/// that the private fixed-font surface did not leak across the feature gate.
pub const glyph_table_fingerprint: [u8; 16] = build_glyph_table_fingerprint();

#[derive(Clone, Copy)]
pub(crate) struct text_layout {
    pub(crate) x: u32,
    pub(crate) y: u32,
    pub(crate) scale: u32,
}

pub(crate) const fn layout(slot: capsule_slot, bounds: capsule_rect) -> text_layout {
    let scale = match slot {
        capsule_slot::masthead | capsule_slot::heading => 3,
        _ => 2,
    };
    let x = match slot {
        capsule_slot::masthead | capsule_slot::navigation => bounds.x() + 24,
        capsule_slot::primary_action | capsule_slot::secondary_action => bounds.x() + 20,
        capsule_slot::heading | capsule_slot::status => bounds.x(),
        _ => bounds.x(),
    };
    let text_height = glyph_height * scale;
    let y = match slot {
        capsule_slot::navigation => bounds.y() + 24,
        capsule_slot::status => bounds.y() + 16,
        _ => bounds.y() + (bounds.height() - text_height) / 2,
    };
    text_layout { x, y, scale }
}

pub(crate) fn label_has_supported_glyphs(text: &str) -> bool {
    !text.is_empty()
        && text
            .bytes()
            .all(|byte| byte == b' ' || glyph_rows(byte).is_some())
}

pub(crate) fn label_pixel(selector: label_selector, layout: text_layout, x: u32, y: u32) -> bool {
    if y < layout.y || y >= layout.y + glyph_height * layout.scale || x < layout.x {
        return false;
    }
    let local_x = x - layout.x;
    let advance = (glyph_width + 1) * layout.scale;
    let character_index = (local_x / advance) as usize;
    let text = selector.text().as_bytes();
    if character_index >= text.len() {
        return false;
    }
    let within_character = local_x % advance;
    if within_character >= glyph_width * layout.scale {
        return false;
    }
    let glyph_x = within_character / layout.scale;
    let glyph_y = (y - layout.y) / layout.scale;
    let Some(rows) = glyph_rows(text[character_index]) else {
        return false;
    };
    rows[glyph_y as usize] & (1 << (glyph_width - 1 - glyph_x)) != 0
}

const fn glyph_rows(byte: u8) -> Option<[u8; glyph_height as usize]> {
    let upper = if byte >= b'a' && byte <= b'z' {
        byte - (b'a' - b'A')
    } else {
        byte
    };
    match upper {
        b'A' => Some([0x0e, 0x11, 0x11, 0x1f, 0x11, 0x11, 0x11]),
        b'C' => Some([0x0e, 0x11, 0x10, 0x10, 0x10, 0x11, 0x0e]),
        b'D' => Some([0x1e, 0x11, 0x11, 0x11, 0x11, 0x11, 0x1e]),
        b'E' => Some([0x1f, 0x10, 0x10, 0x1e, 0x10, 0x10, 0x1f]),
        b'G' => Some([0x0e, 0x11, 0x10, 0x17, 0x11, 0x11, 0x0f]),
        b'H' => Some([0x11, 0x11, 0x11, 0x1f, 0x11, 0x11, 0x11]),
        b'I' => Some([0x1f, 0x04, 0x04, 0x04, 0x04, 0x04, 0x1f]),
        b'K' => Some([0x11, 0x12, 0x14, 0x18, 0x14, 0x12, 0x11]),
        b'L' => Some([0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x1f]),
        b'M' => Some([0x11, 0x1b, 0x15, 0x15, 0x11, 0x11, 0x11]),
        b'N' => Some([0x11, 0x19, 0x15, 0x13, 0x11, 0x11, 0x11]),
        b'O' => Some([0x0e, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0e]),
        b'P' => Some([0x1e, 0x11, 0x11, 0x1e, 0x10, 0x10, 0x10]),
        b'R' => Some([0x1e, 0x11, 0x11, 0x1e, 0x14, 0x12, 0x11]),
        b'S' => Some([0x0f, 0x10, 0x10, 0x0e, 0x01, 0x01, 0x1e]),
        b'T' => Some([0x1f, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04]),
        b'U' => Some([0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0e]),
        b'V' => Some([0x11, 0x11, 0x11, 0x11, 0x11, 0x0a, 0x04]),
        b'W' => Some([0x11, 0x11, 0x11, 0x15, 0x15, 0x15, 0x0a]),
        b'Y' => Some([0x11, 0x11, 0x0a, 0x04, 0x04, 0x04, 0x04]),
        _ => None,
    }
}

const fn build_glyph_table_fingerprint() -> [u8; 16] {
    let forward = hash_glyph_table(false).to_le_bytes();
    let reverse = hash_glyph_table(true).to_le_bytes();
    let mut fingerprint = [0_u8; 16];
    let mut index = 0;
    while index < 8 {
        fingerprint[index] = forward[index];
        fingerprint[index + 8] = reverse[index];
        index += 1;
    }
    fingerprint
}

const fn hash_glyph_table(reverse: bool) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    let mut index = 0;
    while index < supported_glyphs.len() {
        let glyph_index = if reverse {
            supported_glyphs.len() - 1 - index
        } else {
            index
        };
        let glyph = supported_glyphs[glyph_index];
        hash = hash_byte(hash, glyph);
        let rows = match glyph_rows(glyph) {
            Some(rows) => rows,
            None => [0_u8; glyph_height as usize],
        };
        let mut row = 0;
        while row < rows.len() {
            hash = hash_byte(hash, rows[row]);
            row += 1;
        }
        index += 1;
    }
    hash
}

const fn hash_byte(hash: u64, byte: u8) -> u64 {
    (hash ^ byte as u64).wrapping_mul(0x0000_0100_0000_01b3)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glyph_table_fingerprint_is_frozen() {
        assert_eq!(
            glyph_table_fingerprint,
            [
                0x58, 0x8d, 0x26, 0x27, 0xb9, 0x45, 0x4b, 0x5f, 0x50, 0x39, 0xff, 0xaf, 0x7c, 0xc4,
                0xe3, 0x53,
            ]
        );
    }
}
