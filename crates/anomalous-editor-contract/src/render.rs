use crate::{editor_state, font::glyph, theme};

pub const frame_width: usize = 128;
pub const frame_height: usize = 8;
pub const frame_byte_count: usize = frame_width * frame_height * 3;
const cell_width: usize = 6;
const visible_characters: usize = frame_width / cell_width;

/// Semantic evidence accompanies pixels; appearance alone never represents an
/// acknowledged text operation. Text bytes remain available from `editor_state`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct frame_metadata {
    pub text_length: usize,
    pub anchor: usize,
    pub head: usize,
    pub revision: u64,
    pub dirty: bool,
    pub appearance: theme,
    pub undo_depth: u8,
    pub first_visible: usize,
}

/// Software reference rendering with Antimatter's actual base editor colors.
/// Dark values are its vendored Displacer Clay roles; light values come from
/// `renderer/src/theme.ts::paletteFor`. The font is Anomalous's MIT 5x7 bitmap
/// font, not a claim of CodeMirror's browser typography or syntax highlighting.
pub fn render(state: &editor_state, pixels: &mut [u8; frame_byte_count]) -> frame_metadata {
    let (background, ink, primary) = match state.appearance() {
        theme::dark => ([0x14u8, 0x14, 0x14], [0xec, 0xe9, 0xe5], [0xf2, 0x66, 0x1a]),
        theme::light => ([0xf6u8, 0xf5, 0xf2], [0x29, 0x27, 0x23], [0xb1, 0x3f, 0x0d]),
    };
    // CodeEditor uses primary with alpha0x38 for selection backgrounds.
    let selected: [u8; 3] = core::array::from_fn(|index| {
        ((u16::from(primary[index]) * 56 + u16::from(background[index]) * 199 + 127) / 255) as u8
    });
    let (anchor, head) = state.selection();
    let from = anchor.min(head);
    let to = anchor.max(head);
    let first_visible = head.saturating_sub(visible_characters);
    for y in 0..frame_height {
        for x in 0..frame_width {
            let index = first_visible + x / cell_width;
            let mut color = if index >= from && index < to {
                selected
            } else {
                background
            };
            if let Some(byte) = state.text().get(index)
                && y < 7
                && x % cell_width < 5
                && glyph(char::from(*byte))[y] & (1 << (4 - x % cell_width)) != 0
            {
                color = ink;
            }
            // A stable caret is useful to the deterministic proof. No host clock
            // or blink policy is hidden inside this renderer.
            if x == (head - first_visible) * cell_width {
                color = primary;
            }
            let offset = (y * frame_width + x) * 3;
            pixels[offset..offset + 3].copy_from_slice(&color);
        }
    }
    frame_metadata {
        text_length: state.text().len(),
        anchor,
        head,
        revision: state.revision(),
        dirty: state.is_dirty(),
        appearance: state.appearance(),
        undo_depth: state.undo_depth(),
        first_visible,
    }
}
