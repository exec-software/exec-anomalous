/// Maximum accepted bytes in one bootstrap-shell command line.
pub const line_capacity: usize = 128;

/// Fixed-capacity ASCII line editor with no allocator or hidden truncation.
pub struct line_editor {
    bytes: [u8; line_capacity],
    length: usize,
}

impl line_editor {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            bytes: [0; line_capacity],
            length: 0,
        }
    }

    /// Append one printable ASCII byte, returning false when the line is full.
    pub fn push(&mut self, byte: u8) -> bool {
        if !(b' '..=b'~').contains(&byte) || self.length == self.bytes.len() {
            return false;
        }
        self.bytes[self.length] = byte;
        self.length += 1;
        true
    }

    /// Remove one byte, returning whether the visible line changed.
    pub fn backspace(&mut self) -> bool {
        if self.length == 0 {
            return false;
        }
        self.length -= 1;
        // SAFETY: `length` was positive and is now a valid index into `bytes`.
        // A volatile store prevents removal of this best-effort privacy wipe.
        unsafe { core::ptr::write_volatile(self.bytes.as_mut_ptr().add(self.length), 0) };
        true
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        // Only printable ASCII reaches the buffer through `push`.
        core::str::from_utf8(&self.bytes[..self.length]).unwrap_or("")
    }

    pub fn clear(&mut self) {
        for byte in &mut self.bytes {
            // SAFETY: Each reference identifies one live byte in the editor's
            // unique buffer. Volatile stores keep this best-effort wipe from
            // being optimized away; they cannot erase external copies.
            unsafe { core::ptr::write_volatile(byte, 0) };
        }
        self.length = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::{line_capacity, line_editor};

    #[test]
    fn editing_is_bounded_and_reversible() {
        let mut editor = line_editor::new();
        for _ in 0..line_capacity {
            assert!(editor.push(b'a'));
        }
        assert!(!editor.push(b'b'));
        assert!(editor.backspace());
        assert!(editor.push(b'b'));
        assert!(editor.as_str().ends_with('b'));
        editor.clear();
        assert_eq!(editor.as_str(), "");
        assert!(editor.bytes.iter().all(|byte| *byte == 0));
        assert!(!editor.backspace());
    }

    #[test]
    fn non_printable_bytes_are_rejected() {
        let mut editor = line_editor::new();
        assert!(!editor.push(0));
        assert!(!editor.push(b'\n'));
        assert_eq!(editor.as_str(), "");
    }
}
