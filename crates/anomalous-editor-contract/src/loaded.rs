use crate::{
    document, edit_effect, editor_error, editor_state, frame_byte_count, frame_metadata,
    text_capacity, theme,
};

/// A bounded document initialized from copied startup content. Its saved
/// baseline is the supplied text; this type does not open or persist files.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct loaded_editor_state {
    inner: editor_state,
    saved: [u8; text_capacity],
    saved_length: usize,
}

impl loaded_editor_state {
    pub fn from_text(text: &[u8]) -> Result<Self, editor_error> {
        if text.len() > text_capacity {
            return Err(editor_error::text_capacity);
        }
        if text.iter().any(|byte| !(b' '..=b'~').contains(byte)) {
            return Err(editor_error::unsupported_text);
        }
        let mut saved = [0; text_capacity];
        saved[..text.len()].copy_from_slice(text);
        Ok(Self {
            inner: editor_state {
                document: document {
                    text: saved,
                    length: text.len(),
                    anchor: 0,
                    head: 0,
                },
                previous: None,
                revision: 1,
                appearance: theme::dark,
            },
            saved,
            saved_length: text.len(),
        })
    }

    #[must_use]
    pub fn text(&self) -> &[u8] {
        self.inner.text()
    }
    #[must_use]
    pub fn selection(&self) -> (usize, usize) {
        self.inner.selection()
    }
    #[must_use]
    pub fn caret(&self) -> usize {
        self.inner.caret()
    }
    #[must_use]
    pub fn revision(&self) -> u64 {
        self.inner.revision()
    }
    #[must_use]
    pub fn appearance(&self) -> theme {
        self.inner.appearance()
    }
    #[must_use]
    pub fn undo_depth(&self) -> u8 {
        self.inner.undo_depth()
    }
    #[must_use]
    pub fn is_dirty(&self) -> bool {
        self.text() != &self.saved[..self.saved_length]
    }
    pub fn set_selection(&mut self, anchor: usize, head: usize) -> Result<(), editor_error> {
        self.inner.set_selection(anchor, head)
    }
    pub fn set_theme(&mut self, appearance: theme) {
        self.inner.set_theme(appearance);
    }
    pub fn insert(&mut self, text: &[u8]) -> Result<(), editor_error> {
        self.inner.insert(text)
    }
    pub(crate) fn insert_repeated(&mut self, text: &[u8]) -> Result<edit_effect, editor_error> {
        self.inner.insert_repeated(text)
    }
    pub(crate) fn delete_repeated(&mut self, forward: bool) -> Result<edit_effect, editor_error> {
        self.inner.delete_repeated(forward)
    }
    pub fn undo(&mut self) -> Result<(), editor_error> {
        self.inner.undo()
    }
}

/// Pixel behavior stays identical to the original renderer. Dirty metadata
/// reflects the loaded saved baseline rather than the historical fixture.
pub fn render_loaded(
    state: &loaded_editor_state,
    pixels: &mut [u8; frame_byte_count],
) -> frame_metadata {
    let mut metadata = crate::render(&state.inner, pixels);
    metadata.dirty = state.is_dirty();
    metadata
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loaded_baseline_edits_and_undo_follow_the_supplied_document() {
        for text in [b"const before = 1;", b"const before = 2;"] {
            let mut state = loaded_editor_state::from_text(text).unwrap();
            assert!(!state.is_dirty());
            state.set_selection(6, 12).unwrap();
            state.insert(b"after").unwrap();
            assert!(state.is_dirty());
            assert_eq!(state.caret(), 11);
            assert_eq!(state.text()[14], text[15]);
            state.set_theme(theme::light);
            state.undo().unwrap();
            assert_eq!(state.text(), text);
            assert_eq!(state.selection(), (6, 12));
            assert!(!state.is_dirty());
            assert_eq!(state.appearance(), theme::light);
        }
    }

    #[test]
    fn rejected_loaded_text_does_not_create_a_document() {
        assert_eq!(
            loaded_editor_state::from_text(&[b'x'; 32]),
            Err(editor_error::text_capacity)
        );
        for text in [b"\0".as_slice(), b"a\nb", &[0xff]] {
            assert_eq!(
                loaded_editor_state::from_text(text),
                Err(editor_error::unsupported_text)
            );
        }
        assert!(!loaded_editor_state::from_text(b"").unwrap().is_dirty());
    }

    #[test]
    fn default_pixels_match_and_loaded_content_changes_pixels_with_clean_metadata() {
        let mut old = [0; frame_byte_count];
        let mut loaded = [0; frame_byte_count];
        let expected = crate::render(&editor_state::new(), &mut old);
        let state = loaded_editor_state::from_text(crate::initial_text).unwrap();
        assert_eq!(render_loaded(&state, &mut loaded), expected);
        assert_eq!(old, loaded);
        let other = loaded_editor_state::from_text(b"const before = 2;").unwrap();
        assert!(!render_loaded(&other, &mut loaded).dirty);
        assert_ne!(old, loaded);
    }
}
