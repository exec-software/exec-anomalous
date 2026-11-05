use super::*;

#[test]
fn scratch_deletion_at_exhausted_revision_preserves_original_selection_and_history() {
    for forward in [false, true] {
        for selection in [(4, 4), (6, 2), (2, 6)] {
            let mut state = editor_state::new();
            state.set_selection(0, 0).unwrap();
            state.insert_repeated(b"!").unwrap();
            state.set_selection(selection.0, selection.1).unwrap();
            state.revision = u64::MAX;
            let before = state;
            assert_eq!(
                state.delete_repeated(forward),
                Err(editor_error::revision_exhausted)
            );
            assert_eq!(state, before);
        }
    }
}

#[test]
fn production_selection_replacement_and_appearance_preserve_the_editor_contract() {
    let mut state = editor_state::new();
    assert_eq!(state.text(), b"const before = 1;");
    assert_eq!(state.selection(), (0, 0));
    assert_eq!(state.revision(), 1);
    assert!(!state.is_dirty());
    state.set_selection(6, 12).unwrap();
    state.insert(b"after").unwrap();
    assert_eq!(state.text(), b"const after = 1;");
    assert_eq!(state.selection(), (11, 11));
    assert_eq!(state.revision(), 2);
    assert!(state.is_dirty());
    assert_eq!(state.undo_depth(), 1);
    let edited = state.document;
    for appearance in [theme::light, theme::dark] {
        state.set_theme(appearance);
        assert_eq!(state.document, edited);
        assert_eq!(state.revision(), 2);
        assert_eq!(state.undo_depth(), 1);
    }
    state.set_selection(8, 8).unwrap();
    state.set_theme(theme::light);
    state.undo().unwrap();
    assert_eq!(state.text(), initial_text);
    assert_eq!(state.selection(), (6, 12));
    assert_eq!(state.appearance(), theme::light);
    assert_eq!(state.revision(), 3);
    assert_eq!(state.undo_depth(), 0);
    assert!(!state.is_dirty());
}

#[test]
fn reversed_selection_deletion_and_exact_capacity_are_defined() {
    let mut state = editor_state::new();
    state.set_selection(12, 6).unwrap();
    state.insert(b"after").unwrap();
    assert_eq!(state.text(), b"const after = 1;");
    assert_eq!(state.caret(), 11);
    let mut state = editor_state::new();
    state.set_selection(6, 12).unwrap();
    state.insert(b"").unwrap();
    assert_eq!(state.text(), b"const  = 1;");
    assert_eq!(state.selection(), (6, 6));
    let mut state = editor_state::new();
    state.set_selection(0, initial_text.len()).unwrap();
    state.insert(&[b'x'; text_capacity]).unwrap();
    assert_eq!(state.text(), &[b'x'; text_capacity]);
    assert_eq!(state.caret(), text_capacity);
}

#[test]
fn invalid_input_history_exhaustion_and_revision_exhaustion_are_atomic() {
    let initial = editor_state::new();
    for selection in [(18, 0), (0, 18), (usize::MAX, usize::MAX)] {
        let mut state = initial;
        assert_eq!(
            state.set_selection(selection.0, selection.1),
            Err(editor_error::invalid_selection)
        );
        assert_eq!(state, initial);
    }
    for text in [
        &b"\n"[..],
        &b"\t"[..],
        &b"\0"[..],
        &b"\x7f"[..],
        &b"\xc3\xa9"[..],
    ] {
        let mut state = initial;
        assert_eq!(state.insert(text), Err(editor_error::unsupported_text));
        assert_eq!(state, initial);
    }
    let mut state = initial;
    assert_eq!(state.insert(&[b'x'; 32]), Err(editor_error::text_capacity));
    assert_eq!(state, initial);
    assert_eq!(state.undo(), Err(editor_error::no_undo));
    assert_eq!(state, initial);
    state.insert(b"x").unwrap();
    let before = state;
    assert_eq!(state.insert(b"y"), Err(editor_error::undo_capacity));
    assert_eq!(state, before);
    state.revision = u64::MAX;
    let before = state;
    assert_eq!(state.undo(), Err(editor_error::revision_exhausted));
    assert_eq!(state, before);
    let mut state = editor_state {
        revision: u64::MAX,
        ..initial
    };
    let before = state;
    assert_eq!(state.insert(b"x"), Err(editor_error::revision_exhausted));
    assert_eq!(state, before);
}

#[test]
fn no_text_change_advances_neither_revision_nor_history() {
    let mut state = editor_state::new();
    state.set_selection(6, 12).unwrap();
    state.insert(b"before").unwrap();
    assert_eq!(state.text(), initial_text);
    assert_eq!(state.selection(), (12, 12));
    assert_eq!(state.revision(), 1);
    assert_eq!(state.undo_depth(), 0);
}

#[test]
fn pixels_show_actual_text_selection_caret_and_theme_without_mutating_model() {
    let mut state = editor_state::new();
    let mut before = [0; frame_byte_count];
    let initial = render(&state, &mut before);
    assert_eq!(initial.first_visible, 0);
    assert_eq!(&before[..3], &[0xf2, 0x66, 0x1a]);
    assert_eq!(&before[frame_byte_count - 3..], &[0x14, 0x14, 0x14]);
    state.set_selection(6, 12).unwrap();
    let mut selected = [0; frame_byte_count];
    render(&state, &mut selected);
    assert_ne!(before, selected);
    state.insert(b"after").unwrap();
    let snapshot = state;
    let mut edited = [0; frame_byte_count];
    let metadata = render(&state, &mut edited);
    assert_eq!(state, snapshot);
    assert_eq!(metadata.text_length, 16);
    assert_eq!(metadata.head, 11);
    assert!(metadata.dirty);
    assert_eq!(metadata.undo_depth, 1);
    assert_ne!(before, edited);
    for row in 0..8 {
        let caret = (row * frame_width + 66) * 3;
        assert_eq!(&edited[caret..caret + 3], &[0xf2, 0x66, 0x1a]);
    }
    state.set_theme(theme::light);
    let mut light = [0; frame_byte_count];
    render(&state, &mut light);
    assert_ne!(edited, light);
    assert_eq!(&light[frame_byte_count - 3..], &[0xf6, 0xf5, 0xf2]);
    state.set_theme(theme::dark);
    let mut dark = [0; frame_byte_count];
    render(&state, &mut dark);
    assert_eq!(edited, dark);
}

#[test]
fn full_line_keeps_the_caret_in_the_bounded_viewport() {
    let mut state = editor_state::new();
    state.set_selection(0, initial_text.len()).unwrap();
    state.insert(&[b'x'; text_capacity]).unwrap();
    let mut pixels = [0; frame_byte_count];
    let metadata = render(&state, &mut pixels);
    assert_eq!(metadata.first_visible, 10);
    let caret = 126 * 3;
    assert_eq!(&pixels[caret..caret + 3], &[0xf2, 0x66, 0x1a]);
}

#[test]
fn repeated_edit_revision_exhaustion_never_evicts_history_or_changes_selection() {
    let mut state = editor_state::new();
    state.set_selection(6, 12).unwrap();
    state.insert(b"after").unwrap();
    state.set_selection(11, 6).unwrap();
    state.revision = u64::MAX;
    let stable = state;
    assert_eq!(
        state.insert_repeated(b"later"),
        Err(editor_error::revision_exhausted)
    );
    assert_eq!(state, stable);
    assert_eq!(state.undo(), Err(editor_error::revision_exhausted));
    assert_eq!(state, stable);
    for forward in [false, true] {
        assert_eq!(
            state.delete_repeated(forward),
            Err(editor_error::revision_exhausted)
        );
        assert_eq!(state, stable);
    }
    // A content no-op remains possible at revision exhaustion and keeps the
    // occupied undo slot even though it collapses the current selection.
    assert_eq!(state.insert_repeated(b"after"), Ok(edit_effect::unchanged));
    assert_eq!(state.previous, stable.previous);
    assert_eq!(state.revision(), u64::MAX);
    assert_eq!(state.selection(), (11, 11));
}
