//! Bounded interactive display and an observation codec. ANUI carries no
//! authority: its sender and receiver remain bound by the application route.
use crate::{
    editor_error, interactive_editor_feedback as feedback, interactive_editor_view, save_outcome,
};

pub const interactive_audit_size: usize = 128;
pub const interactive_frame_width: usize = 128;
pub const interactive_frame_height: usize = 48;
pub const interactive_frame_byte_count: usize =
    interactive_frame_width * interactive_frame_height * 3;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum interactive_audit_error {
    length,
    encoding,
    fields,
}

/// Explicit fields permit an independent host oracle to supply literal expected
/// observations without replaying the client state machine. Codes are local to
/// this experimental v1 format; unknown codes fail closed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct interactive_audit<'a> {
    pub sequence: u64,
    pub revision: u64,
    pub version: u64,
    pub pending_request: u64,
    pub pending_revision: u64,
    pub anchor: u8,
    pub head: u8,
    pub undo_depth: u8,
    pub dirty: bool,
    pub outcome: u8,
    pub feedback: u8,
    pub close_armed: bool,
    pub closed: bool,
    pub text: &'a [u8],
}

impl<'a> interactive_audit<'a> {
    pub fn from_view(view: &'a interactive_editor_view) -> Self {
        let (anchor, head) = view.selection();
        Self {
            sequence: view.sequence(),
            revision: view.revision(),
            version: view.version(),
            pending_request: view.pending_request().unwrap_or(0),
            pending_revision: view.pending_revision().unwrap_or(0),
            anchor: anchor as u8,
            head: head as u8,
            undo_depth: view.undo_depth(),
            dirty: view.is_dirty(),
            outcome: match view.save_outcome() {
                save_outcome::idle => 0,
                save_outcome::awaiting => 1,
                save_outcome::unknown => 2,
            },
            feedback: feedback_code(view.feedback()),
            close_armed: view.close_armed(),
            closed: view.is_closed(),
            text: view.text(),
        }
    }
    fn validate(&self) -> Result<(), interactive_audit_error> {
        if self.sequence == 0
            || self.revision == 0
            || self.version == 0
            || self.text.len() > 31
            || self.text.iter().any(|b| !(0x20..=0x7e).contains(b))
            || usize::from(self.anchor) > self.text.len()
            || usize::from(self.head) > self.text.len()
            || self.undo_depth > 1
            || self.outcome > 2
            || feedback_label(self.feedback).is_none()
            || (self.outcome == 0 && (self.pending_request != 0 || self.pending_revision != 0))
            || (self.outcome != 0
                && (self.pending_request == 0
                    || self.pending_revision == 0
                    || self.pending_revision > self.revision))
            || (self.outcome == 2 && !self.dirty)
            || (self.closed && self.close_armed)
        {
            return Err(interactive_audit_error::fields);
        }
        Ok(())
    }
    pub fn encode(&self) -> Result<[u8; interactive_audit_size], interactive_audit_error> {
        self.validate()?;
        let mut bytes = [0; interactive_audit_size];
        bytes[..8].copy_from_slice(b"ANUI\x01\x00\x80\x00");
        for (index, word) in [
            self.sequence,
            self.revision,
            self.version,
            self.pending_request,
            self.pending_revision,
        ]
        .iter()
        .enumerate()
        {
            bytes[8 + index * 8..16 + index * 8].copy_from_slice(&word.to_le_bytes());
        }
        bytes[48..57].copy_from_slice(&[
            self.text.len() as u8,
            self.anchor,
            self.head,
            self.undo_depth,
            u8::from(self.dirty),
            self.outcome,
            self.feedback,
            u8::from(self.close_armed),
            u8::from(self.closed),
        ]);
        bytes[64..64 + self.text.len()].copy_from_slice(self.text);
        Ok(bytes)
    }
    pub fn decode(bytes: &'a [u8]) -> Result<Self, interactive_audit_error> {
        if bytes.len() != interactive_audit_size {
            return Err(interactive_audit_error::length);
        }
        if &bytes[..8] != b"ANUI\x01\x00\x80\x00"
            || bytes[48] > 31
            || [52, 55, 56].iter().any(|i| bytes[*i] > 1)
            || bytes[57..64].iter().any(|b| *b != 0)
            || bytes[64 + usize::from(bytes[48])..].iter().any(|b| *b != 0)
        {
            return Err(interactive_audit_error::encoding);
        }
        let word = |i| u64::from_le_bytes(bytes[i..i + 8].try_into().unwrap());
        let value = Self {
            sequence: word(8),
            revision: word(16),
            version: word(24),
            pending_request: word(32),
            pending_revision: word(40),
            anchor: bytes[49],
            head: bytes[50],
            undo_depth: bytes[51],
            dirty: bytes[52] != 0,
            outcome: bytes[53],
            feedback: bytes[54],
            close_armed: bytes[55] != 0,
            closed: bytes[56] != 0,
            text: &bytes[64..64 + usize::from(bytes[48])],
        };
        value.validate()?;
        Ok(value)
    }
}

fn feedback_code(value: feedback) -> u8 {
    match value {
        feedback::ready => 0,
        feedback::edited => 1,
        feedback::history_replaced => 2,
        feedback::selection_changed => 3,
        feedback::unchanged => 4,
        feedback::undone => 5,
        feedback::input_reset => 6,
        feedback::unsupported_key => 7,
        feedback::save_started => 8,
        feedback::save_acknowledged => 9,
        feedback::save_rejected => 10,
        feedback::save_unknown => 11,
        feedback::close_confirmation => 12,
        feedback::closed => 13,
        feedback::rejected(error) => match error {
            editor_error::invalid_selection => 16,
            editor_error::unsupported_text => 17,
            editor_error::text_capacity => 18,
            editor_error::undo_capacity => 19,
            editor_error::no_undo => 20,
            editor_error::revision_exhausted => 21,
        },
    }
}
fn feedback_label(code: u8) -> Option<&'static [u8]> {
    Some(match code {
        0 => b"READY",
        1 => b"EDITED",
        2 => b"UNDO HISTORY REPLACED",
        3 => b"SELECTION CHANGED",
        4 => b"UNCHANGED",
        5 => b"UNDONE",
        6 => b"INPUT RESET",
        7 => b"UNSUPPORTED KEY",
        8 => b"SAVE REQUESTED",
        9 => b"SAVE ACKNOWLEDGED",
        10 => b"SAVE REJECTED",
        11 => b"SAVE OUTCOME UNKNOWN",
        12 => b"^Q AGAIN TO DISCARD",
        13 => b"CLOSED",
        16 => b"INVALID SELECTION",
        17 => b"ASCII TEXT ONLY",
        18 => b"TEXT LIMIT: 31 BYTES",
        19 => b"UNDO LIMIT",
        20 => b"NOTHING TO UNDO",
        21 => b"REVISION EXHAUSTED",
        _ => return None,
    })
}

pub fn render_interactive(
    view: &interactive_editor_view,
    pixels: &mut [u8; interactive_frame_byte_count],
) {
    render_interactive_audit(&interactive_audit::from_view(view), pixels).unwrap();
}

/// Validates before any output write. RGB8 is tightly packed, no framebuffer or
/// source pointers are embedded. Text scroll follows the caret in a 21-cell row.
pub fn render_interactive_audit(
    view: &interactive_audit<'_>,
    pixels: &mut [u8; interactive_frame_byte_count],
) -> Result<(), interactive_audit_error> {
    view.validate()?;
    for pixel in pixels.chunks_exact_mut(3) {
        pixel.copy_from_slice(&[18, 24, 34]);
    }
    line(pixels, 0, b"ANOMALOUS EDITOR", [144, 205, 255]);
    let start = usize::from(view.head).saturating_sub(20);
    let selection = (
        usize::from(view.anchor.min(view.head)),
        usize::from(view.anchor.max(view.head)),
    );
    for cell in 0..21 {
        let index = start + cell;
        if let Some(byte) = view.text.get(index) {
            glyph(pixels, cell * 6, 8, *byte, [236, 241, 247]);
        }
        if index >= selection.0 && index < selection.1 {
            for x in cell * 6..cell * 6 + 5 {
                pixel(pixels, x, 15, [99, 191, 255]);
            }
        }
        if index == usize::from(view.head) {
            for y in 8..15 {
                pixel(pixels, cell * 6, y, [255, 210, 94]);
            }
        }
    }
    let status: &[u8] = match (view.outcome, view.dirty) {
        (2, _) => b"SAVE UNCERTAIN",
        (1, _) if view.pending_revision < view.revision => b"SAVING + NEWER EDITS",
        (1, _) => b"SAVING",
        (_, true) => b"DIRTY",
        _ => b"SAVED",
    };
    line(
        pixels,
        16,
        status,
        if view.outcome == 2 {
            [255, 174, 107]
        } else {
            [151, 224, 181]
        },
    );
    let mut info = [b' '; 21];
    info[0] = b'V';
    decimal(&mut info[1..7], view.version);
    info[8] = b'R';
    decimal(&mut info[9..15], view.revision);
    info[17..21].copy_from_slice(if view.dirty { b"DIRT" } else { b"SYNC" });
    line(pixels, 24, &info, [177, 190, 207]);
    line(
        pixels,
        32,
        feedback_label(view.feedback).unwrap(),
        [236, 241, 247],
    );
    line(pixels, 40, b"^S SAVE  ^Q CLOSE", [177, 190, 207]);
    Ok(())
}
fn decimal(output: &mut [u8], mut value: u64) {
    if value > 999_999 {
        output.fill(b'#');
        return;
    }
    let mut digits = [0; 6];
    let mut count = 0;
    loop {
        digits[count] = b'0' + (value % 10) as u8;
        count += 1;
        value /= 10;
        if value == 0 {
            break;
        }
    }
    for i in 0..count {
        output[i] = digits[count - i - 1];
    }
}
fn line(pixels: &mut [u8], y: usize, bytes: &[u8], color: [u8; 3]) {
    for (i, b) in bytes.iter().take(21).enumerate() {
        glyph(pixels, i * 6, y, *b, color);
    }
}
fn glyph(pixels: &mut [u8], x: usize, y: usize, byte: u8, color: [u8; 3]) {
    for (row, bits) in crate::font::glyph(char::from(byte)).iter().enumerate() {
        for col in 0..5 {
            if bits & (1 << (4 - col)) != 0 {
                pixel(pixels, x + col, y + row, color);
            }
        }
    }
}
fn pixel(pixels: &mut [u8], x: usize, y: usize, color: [u8; 3]) {
    let i = (y * interactive_frame_width + x) * 3;
    pixels[i..i + 3].copy_from_slice(&color);
}

#[cfg(test)]
mod tests {
    use super::*;
    fn audit() -> interactive_audit<'static> {
        interactive_audit {
            sequence: 1,
            revision: 1,
            version: 1,
            pending_request: 0,
            pending_revision: 0,
            anchor: 0,
            head: 0,
            undo_depth: 0,
            dirty: false,
            outcome: 0,
            feedback: 0,
            close_armed: false,
            closed: false,
            text: b"alpha",
        }
    }
    #[test]
    fn canonical_codec_rejects_every_truncation_padding_and_inconsistent_pending() {
        let expected = audit();
        let bytes = expected.encode().unwrap();
        assert_eq!(interactive_audit::decode(&bytes), Ok(expected));
        for n in 0..128 {
            assert!(interactive_audit::decode(&bytes[..n]).is_err());
        }
        for i in [57, 63, 69, 95, 127] {
            let mut bad = bytes;
            bad[i] = 1;
            assert!(interactive_audit::decode(&bad).is_err());
        }
        for i in [52, 55, 56] {
            let mut bad = bytes;
            bad[i] = 2;
            assert!(interactive_audit::decode(&bad).is_err());
        }
        let mut bad = expected;
        bad.outcome = 1;
        assert!(bad.encode().is_err());
        bad.pending_request = 1;
        bad.pending_revision = 1;
        assert!(bad.encode().is_ok());
        bad.outcome = 2;
        assert!(bad.encode().is_err());
    }
    #[test]
    fn rendering_distinguishes_text_save_states_and_preserves_output_on_rejection() {
        let mut a = [0; interactive_frame_byte_count];
        let mut b = a;
        let mut view = audit();
        render_interactive_audit(&view, &mut a).unwrap();
        view.text = b"blpha";
        render_interactive_audit(&view, &mut b).unwrap();
        assert_ne!(a, b);
        view = audit();
        view.outcome = 1;
        view.pending_request = 1;
        view.pending_revision = 1;
        view.dirty = true;
        render_interactive_audit(&view, &mut b).unwrap();
        assert_ne!(a, b);
        a = b;
        view.outcome = 2;
        render_interactive_audit(&view, &mut b).unwrap();
        assert_ne!(a, b);
        a = b;
        view.dirty = false;
        assert!(render_interactive_audit(&view, &mut b).is_err());
        assert_eq!(a, b);
    }
    #[test]
    fn selection_and_scroll_are_visible_and_contained() {
        let mut a = [0; interactive_frame_byte_count];
        let mut b = a;
        let mut view = audit();
        render_interactive_audit(&view, &mut a).unwrap();
        view.head = 5;
        render_interactive_audit(&view, &mut b).unwrap();
        assert_ne!(&a[128 * 8 * 3..128 * 16 * 3], &b[128 * 8 * 3..128 * 16 * 3]);
        view.text = b"012345678901234567890123456789x";
        view.head = 31;
        view.anchor = 31;
        render_interactive_audit(&view, &mut b).unwrap();
        // Head 31 scrolls to index 11. The end caret occupies the last
        // visible cell without indexing a nonexistent text byte.
        for y in 8..15 {
            let index = (y * 128 + 120) * 3;
            assert_eq!(&b[index..index + 3], &[255, 210, 94]);
        }
        view.text = b"A";
        view.head = 1;
        view.anchor = 1;
        render_interactive_audit(&view, &mut b).unwrap();
        // Independent 5x7 A anchor: first row is 01110; the caret is at
        // x=6, so neither sampled glyph pixel is obscured by it.
        let lit = (8 * 128 + 2) * 3;
        let blank = (8 * 128) * 3;
        assert_eq!(&b[lit..lit + 3], &[236, 241, 247]);
        assert_eq!(&b[blank..blank + 3], &[18, 24, 34]);
    }
}
