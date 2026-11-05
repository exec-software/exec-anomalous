use super::ps2_sample;
use crate::event::{event_loss, event_record};

/// The direction of one physical key transition.
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum key_state {
    pressed,
    released,
}

/// Physical scan-set-1 key identity, before layout or text interpretation.
///
/// The numeric make code is preserved so unfamiliar keys remain distinguishable
/// without pretending the bootstrap decoder knows their layout meaning. This
/// type deliberately has no formatting implementation because keys are private
/// input content and must never be copied into telemetry.
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) struct physical_key {
    make_code: u8,
    extended: bool,
}

impl physical_key {
    #[must_use]
    pub(crate) const fn make_code(self) -> u8 {
        self.make_code
    }

    #[must_use]
    pub(crate) const fn is_extended(self) -> bool {
        self.extended
    }
}

/// A physical press or release, independent of any text keymap.
///
/// This type deliberately has no formatting implementation. Its source event
/// metadata may be observed, but the key identity remains private content.
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) struct physical_key_transition {
    key: physical_key,
    state: key_state,
}

impl physical_key_transition {
    #[must_use]
    pub(crate) const fn key(self) -> physical_key {
        self.key
    }

    #[must_use]
    pub(crate) const fn state(self) -> key_state {
        self.state
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum prefix_state {
    none,
    extended,
    pause_tail(u8),
    print_screen_make_tail(u8),
    print_screen_break_tail(u8),
}

const pause_tail: [u8; 5] = [0x1d, 0x45, 0xe1, 0x9d, 0xc5];
const print_screen_make_tail: [u8; 2] = [0xe0, 0x37];
const print_screen_break_tail: [u8; 2] = [0xe0, 0xaa];
pub(super) const invalid_transport_bytes: [u8; 7] = [0x00, 0xee, 0xfa, 0xfc, 0xfd, 0xfe, 0xff];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum scan_decode_error {
    malformed_stream,
}

/// Pure PS/2 scan-code-set-1 decoder.
///
/// It performs no port I/O, tracks only multi-byte scan framing, and emits
/// physical transitions rather than text. The six-byte Pause sequence has no
/// ordinary break code and is intentionally consumed without a transition in
/// this bounded bootstrap decoder. Standard Print Screen composites are
/// collapsed into one extended physical edge so their synthetic Shift bytes
/// never become physical modifier activity. Outside an architecture-owned
/// device-reset exchange, an ordinary `0xaa` is necessarily interpreted as
/// left-Shift release: an unsolicited BAT byte is indistinguishable from that
/// valid scan byte and is not claimed as a steady-state reset signal here.
pub(crate) struct scan_set_1_decoder {
    prefix: prefix_state,
}

impl scan_set_1_decoder {
    #[must_use]
    pub(crate) const fn new() -> Self {
        Self {
            prefix: prefix_state::none,
        }
    }

    /// Consume one opaque scan sample and possibly emit one physical edge.
    /// Malformed bytes, device protocol responses, and incomplete multi-byte
    /// sequences are an explicit discontinuity; callers must not preserve
    /// derived key state.
    pub(crate) fn decode(
        &mut self,
        sample: ps2_sample,
    ) -> Result<Option<physical_key_transition>, scan_decode_error> {
        let scan_byte = sample.scan_byte();
        if invalid_transport_bytes.contains(&scan_byte) {
            self.prefix = prefix_state::none;
            return Err(scan_decode_error::malformed_stream);
        }

        if let prefix_state::pause_tail(index) = self.prefix {
            if pause_tail[usize::from(index)] != scan_byte {
                self.prefix = prefix_state::none;
                return Err(scan_decode_error::malformed_stream);
            }
            self.prefix = if usize::from(index) + 1 == pause_tail.len() {
                prefix_state::none
            } else {
                prefix_state::pause_tail(index + 1)
            };
            return Ok(None);
        }

        let print_screen_tail = match self.prefix {
            prefix_state::print_screen_make_tail(index) => {
                Some((index, false, &print_screen_make_tail))
            }
            prefix_state::print_screen_break_tail(index) => {
                Some((index, true, &print_screen_break_tail))
            }
            _ => None,
        };
        if let Some((index, released, tail)) = print_screen_tail {
            if tail[usize::from(index)] != scan_byte {
                self.prefix = prefix_state::none;
                return Err(scan_decode_error::malformed_stream);
            }
            if usize::from(index) + 1 != tail.len() {
                self.prefix = if released {
                    prefix_state::print_screen_break_tail(index + 1)
                } else {
                    prefix_state::print_screen_make_tail(index + 1)
                };
                return Ok(None);
            }

            self.prefix = prefix_state::none;
            return Ok(Some(physical_key_transition {
                key: physical_key {
                    make_code: 0x37,
                    extended: true,
                },
                state: if released {
                    key_state::released
                } else {
                    key_state::pressed
                },
            }));
        }

        if scan_byte == 0xe0 {
            if self.prefix != prefix_state::none {
                self.prefix = prefix_state::none;
                return Err(scan_decode_error::malformed_stream);
            }
            self.prefix = prefix_state::extended;
            return Ok(None);
        }
        if scan_byte == 0xe1 {
            if self.prefix != prefix_state::none {
                self.prefix = prefix_state::none;
                return Err(scan_decode_error::malformed_stream);
            }
            // E1 1D 45 E1 9D C5 is Pause in scan set 1. Consume the remaining
            // five bytes without leaking a synthetic press that cannot later
            // receive a matching release.
            self.prefix = prefix_state::pause_tail(0);
            return Ok(None);
        }

        let extended = self.prefix == prefix_state::extended;
        self.prefix = prefix_state::none;
        if extended {
            match scan_byte {
                0x2a => {
                    self.prefix = prefix_state::print_screen_make_tail(0);
                    return Ok(None);
                }
                0xb7 => {
                    self.prefix = prefix_state::print_screen_break_tail(0);
                    return Ok(None);
                }
                // An extended 0xaa outside the exact Print Screen break tail
                // is synthetic framing, not a physical Shift release.
                0xaa => return Err(scan_decode_error::malformed_stream),
                _ => {}
            }
        }
        let make_code = scan_byte & 0x7f;
        if make_code == 0 {
            return Err(scan_decode_error::malformed_stream);
        }
        Ok(Some(physical_key_transition {
            key: physical_key {
                make_code,
                extended,
            },
            state: if scan_byte & 0x80 == 0 {
                key_state::pressed
            } else {
                key_state::released
            },
        }))
    }

    /// Discard partial framing after any delivery discontinuity.
    pub(crate) fn reset_after_loss(&mut self) {
        self.prefix = prefix_state::none;
    }
}

/// A decoded key press accepted by the temporary bootstrap shell.
///
/// This is deliberately downstream from physical transitions. It is one US
/// ASCII keymap for bring-up, not a device event, stable ABI, locale model, or
/// future Canticle input contract. It has no formatting implementation because
/// character content must never enter telemetry.
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum key_event {
    character(u8),
    enter,
    backspace,
}

/// Temporary US-ASCII projection of physical scan-set-1 transitions.
pub(crate) struct bootstrap_keymap {
    left_shift: bool,
    right_shift: bool,
    left_control: bool,
    right_control: bool,
    left_alt: bool,
    right_alt: bool,
    caps_lock: bool,
    caps_lock_down: bool,
}

/// Physical delivery with source, sequence, and timestamp metadata preserved.
pub(crate) enum physical_input_delivery {
    transition(event_record<physical_key_transition>),
    loss(event_loss),
}

impl bootstrap_keymap {
    #[must_use]
    pub(crate) const fn new() -> Self {
        Self {
            left_shift: false,
            right_shift: false,
            left_control: false,
            right_control: false,
            left_alt: false,
            right_alt: false,
            caps_lock: false,
            caps_lock_down: false,
        }
    }

    /// Apply one physical edge and possibly produce one bootstrap-shell key.
    pub(crate) fn map(&mut self, transition: physical_key_transition) -> Option<key_event> {
        let key = transition.key();
        let pressed = transition.state() == key_state::pressed;
        match (key.is_extended(), key.make_code()) {
            (false, 0x2a) => {
                self.left_shift = pressed;
                return None;
            }
            (false, 0x36) => {
                self.right_shift = pressed;
                return None;
            }
            (false, 0x1d) => {
                self.left_control = pressed;
                return None;
            }
            (true, 0x1d) => {
                self.right_control = pressed;
                return None;
            }
            (false, 0x38) => {
                self.left_alt = pressed;
                return None;
            }
            (true, 0x38) => {
                self.right_alt = pressed;
                return None;
            }
            (false, 0x3a) => {
                if pressed && !self.caps_lock_down {
                    self.caps_lock = !self.caps_lock;
                }
                self.caps_lock_down = pressed;
                return None;
            }
            _ => {}
        }

        if !pressed || key.is_extended() || self.control_or_alt_down() {
            return None;
        }
        match key.make_code() {
            0x1c => Some(key_event::enter),
            0x0e => Some(key_event::backspace),
            _ => self
                .decode_character(key.make_code())
                .map(key_event::character),
        }
    }

    /// Clear both held and latched state after any delivery discontinuity.
    pub(crate) fn reset_after_loss(&mut self) {
        *self = Self::new();
    }

    const fn control_or_alt_down(&self) -> bool {
        self.left_control || self.right_control || self.left_alt || self.right_alt
    }

    fn decode_character(&self, make_code: u8) -> Option<u8> {
        let shifted = self.left_shift || self.right_shift;
        let letter_uppercase = shifted ^ self.caps_lock;
        let letter = match make_code {
            0x1e => Some(b'a'),
            0x30 => Some(b'b'),
            0x2e => Some(b'c'),
            0x20 => Some(b'd'),
            0x12 => Some(b'e'),
            0x21 => Some(b'f'),
            0x22 => Some(b'g'),
            0x23 => Some(b'h'),
            0x17 => Some(b'i'),
            0x24 => Some(b'j'),
            0x25 => Some(b'k'),
            0x26 => Some(b'l'),
            0x32 => Some(b'm'),
            0x31 => Some(b'n'),
            0x18 => Some(b'o'),
            0x19 => Some(b'p'),
            0x10 => Some(b'q'),
            0x13 => Some(b'r'),
            0x1f => Some(b's'),
            0x14 => Some(b't'),
            0x16 => Some(b'u'),
            0x2f => Some(b'v'),
            0x11 => Some(b'w'),
            0x2d => Some(b'x'),
            0x15 => Some(b'y'),
            0x2c => Some(b'z'),
            _ => None,
        };
        if let Some(letter) = letter {
            return Some(if letter_uppercase {
                letter.to_ascii_uppercase()
            } else {
                letter
            });
        }

        let pair = match make_code {
            0x02 => (b'1', b'!'),
            0x03 => (b'2', b'@'),
            0x04 => (b'3', b'#'),
            0x05 => (b'4', b'$'),
            0x06 => (b'5', b'%'),
            0x07 => (b'6', b'^'),
            0x08 => (b'7', b'&'),
            0x09 => (b'8', b'*'),
            0x0a => (b'9', b'('),
            0x0b => (b'0', b')'),
            0x0c => (b'-', b'_'),
            0x0d => (b'=', b'+'),
            0x1a => (b'[', b'{'),
            0x1b => (b']', b'}'),
            0x27 => (b';', b':'),
            0x28 => (b'\'', b'"'),
            0x29 => (b'`', b'~'),
            0x2b => (b'\\', b'|'),
            0x33 => (b',', b'<'),
            0x34 => (b'.', b'>'),
            0x35 => (b'/', b'?'),
            0x39 => (b' ', b' '),
            _ => return None,
        };
        Some(if shifted { pair.1 } else { pair.0 })
    }
}

/// Delivery projection used by the temporary bootstrap shell.
pub(crate) enum bootstrap_input_delivery {
    key(event_record<key_event>),
    loss(event_loss),
}

/// Stateful consumer that enforces loss resynchronization before keymapping.
pub(crate) struct bootstrap_input_decoder {
    keymap: bootstrap_keymap,
}

impl bootstrap_input_decoder {
    #[must_use]
    pub(crate) const fn new() -> Self {
        Self {
            keymap: bootstrap_keymap::new(),
        }
    }

    /// Consume one physical delivery. A loss is always returned to the caller
    /// and resets held and latched keymap state first.
    pub(crate) fn consume(
        &mut self,
        delivery: physical_input_delivery,
    ) -> Option<bootstrap_input_delivery> {
        match delivery {
            physical_input_delivery::loss(loss) => {
                self.keymap.reset_after_loss();
                Some(bootstrap_input_delivery::loss(loss))
            }
            physical_input_delivery::transition(record) => {
                let key = self.keymap.map(*record.payload())?;
                Some(bootstrap_input_delivery::key(record.replace_payload(key)))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        bootstrap_keymap, key_event, key_state, physical_key, physical_key_transition,
        scan_set_1_decoder,
    };
    use crate::input::ps2_sample;

    fn transition(make_code: u8, extended: bool, state: key_state) -> physical_key_transition {
        physical_key_transition {
            key: physical_key {
                make_code,
                extended,
            },
            state,
        }
    }

    fn press(make_code: u8) -> physical_key_transition {
        transition(make_code, false, key_state::pressed)
    }

    fn release(make_code: u8) -> physical_key_transition {
        transition(make_code, false, key_state::released)
    }

    fn decode(
        decoder: &mut scan_set_1_decoder,
        scan_byte: u8,
    ) -> Result<Option<physical_key_transition>, super::scan_decode_error> {
        decoder.decode(ps2_sample::new(scan_byte))
    }

    fn decoded_transition(
        decoder: &mut scan_set_1_decoder,
        scan_byte: u8,
    ) -> physical_key_transition {
        decode(decoder, scan_byte)
            .expect("ordinary scan byte must be valid")
            .expect("ordinary scan byte must produce a transition")
    }

    fn assert_accepted_without_transition(decoder: &mut scan_set_1_decoder, scan_byte: u8) {
        assert!(matches!(decode(decoder, scan_byte), Ok(None)));
    }

    #[test]
    fn every_single_byte_make_code_has_a_press_and_release_edge() {
        // Scan set 1's ordinary PC keyboard make codes end at F12 (0x58);
        // higher values collide with E0/E1 framing when their break bit is set.
        for make_code in 1..=0x58 {
            let mut decoder = scan_set_1_decoder::new();
            let pressed = decoded_transition(&mut decoder, make_code);
            assert_eq!(pressed.key().make_code(), make_code);
            assert!(!pressed.key().is_extended());
            assert!(pressed.state() == key_state::pressed);

            let released = decoded_transition(&mut decoder, make_code | 0x80);
            assert_eq!(released.key().make_code(), make_code);
            assert!(!released.key().is_extended());
            assert!(released.state() == key_state::released);
        }
    }

    #[test]
    fn extended_prefix_applies_to_exactly_one_edge() {
        let mut decoder = scan_set_1_decoder::new();
        assert_accepted_without_transition(&mut decoder, 0xe0);
        let extended = decoded_transition(&mut decoder, 0x1d);
        assert!(extended.key().is_extended());
        assert_eq!(extended.key().make_code(), 0x1d);
        assert!(extended.state() == key_state::pressed);

        let ordinary = decoded_transition(&mut decoder, 0x1e);
        assert!(!ordinary.key().is_extended());
    }

    #[test]
    fn repeated_extended_prefix_is_malformed_and_resets_framing() {
        let mut decoder = scan_set_1_decoder::new();
        assert_accepted_without_transition(&mut decoder, 0xe0);
        assert!(decode(&mut decoder, 0xe0).is_err());
        assert!(!decoded_transition(&mut decoder, 0x9d).key().is_extended());
    }

    #[test]
    fn pause_sequence_is_consumed_without_an_unpaired_transition() {
        let mut decoder = scan_set_1_decoder::new();
        for byte in [0xe1, 0x1d, 0x45, 0xe1, 0x9d, 0xc5] {
            assert_accepted_without_transition(&mut decoder, byte);
        }
        assert!(decode(&mut decoder, 0x1e).unwrap() == Some(press(0x1e)));
    }

    #[test]
    fn print_screen_composites_emit_exactly_one_physical_edge() {
        let mut decoder = scan_set_1_decoder::new();
        for byte in [0xe0, 0x2a, 0xe0] {
            assert_accepted_without_transition(&mut decoder, byte);
        }
        let pressed = decoded_transition(&mut decoder, 0x37);
        assert!(pressed.key().is_extended());
        assert_eq!(pressed.key().make_code(), 0x37);
        assert!(pressed.state() == key_state::pressed);

        for byte in [0xe0, 0xb7, 0xe0] {
            assert_accepted_without_transition(&mut decoder, byte);
        }
        let released = decoded_transition(&mut decoder, 0xaa);
        assert!(released.key().is_extended());
        assert_eq!(released.key().make_code(), 0x37);
        assert!(released.state() == key_state::released);
    }

    #[test]
    fn malformed_print_screen_tails_are_rejected() {
        for (start, tail) in [
            ([0xe0, 0x2a], super::print_screen_make_tail),
            ([0xe0, 0xb7], super::print_screen_break_tail),
        ] {
            for malformed_index in 0..tail.len() {
                let mut decoder = scan_set_1_decoder::new();
                for byte in start {
                    assert_accepted_without_transition(&mut decoder, byte);
                }
                for (index, expected) in tail.into_iter().enumerate() {
                    if index == malformed_index {
                        let malformed = if expected == 0xaa { 0xb6 } else { 0xaa };
                        assert!(decode(&mut decoder, malformed).is_err());
                        break;
                    }
                    assert_accepted_without_transition(&mut decoder, expected);
                }
                assert!(decode(&mut decoder, 0x1e).unwrap() == Some(press(0x1e)));
            }
        }
    }

    #[test]
    fn orphaned_print_screen_shift_suffix_is_rejected() {
        let mut decoder = scan_set_1_decoder::new();
        assert_accepted_without_transition(&mut decoder, 0xe0);
        assert!(decode(&mut decoder, 0xaa).is_err());
        assert!(decode(&mut decoder, 0x1e).unwrap() == Some(press(0x1e)));
    }

    #[test]
    fn every_malformed_pause_tail_position_is_rejected() {
        for malformed_index in 0..super::pause_tail.len() {
            let mut decoder = scan_set_1_decoder::new();
            assert_accepted_without_transition(&mut decoder, 0xe1);
            for (index, expected) in super::pause_tail.into_iter().enumerate() {
                if index == malformed_index {
                    // This is also a possible left-Shift release. Rejecting it
                    // explicitly ensures the source creates a loss boundary
                    // instead of silently retaining modifier state.
                    assert!(decode(&mut decoder, 0xaa).is_err());
                    break;
                }
                assert_accepted_without_transition(&mut decoder, expected);
            }
            assert!(decode(&mut decoder, 0x1e).unwrap() == Some(press(0x1e)));
        }
    }

    #[test]
    fn invalid_transport_bytes_clear_partial_prefix_state() {
        for invalid in super::invalid_transport_bytes {
            for prefix in [
                None,
                Some(&[0xe0][..]),
                Some(&[0xe1, 0x1d][..]),
                Some(&[0xe0, 0x2a, 0xe0][..]),
            ] {
                let mut decoder = scan_set_1_decoder::new();
                if let Some(prefix) = prefix {
                    for byte in prefix {
                        assert_accepted_without_transition(&mut decoder, *byte);
                    }
                }
                assert!(decode(&mut decoder, invalid).is_err());
                assert!(decode(&mut decoder, 0x1e).unwrap() == Some(press(0x1e)));
            }
        }
    }

    #[test]
    fn malformed_prefix_combinations_are_rejected() {
        for second_prefix in [0xe0, 0xe1] {
            let mut decoder = scan_set_1_decoder::new();
            assert_accepted_without_transition(&mut decoder, 0xe0);
            assert!(decode(&mut decoder, second_prefix).is_err());
            assert!(decode(&mut decoder, 0x1e).unwrap() == Some(press(0x1e)));
        }

        let mut decoder = scan_set_1_decoder::new();
        assert!(decode(&mut decoder, 0x80).is_err());
        assert!(decode(&mut decoder, 0x1e).unwrap() == Some(press(0x1e)));
    }

    #[test]
    fn explicit_loss_reset_clears_a_partial_prefix() {
        let mut decoder = scan_set_1_decoder::new();
        assert_accepted_without_transition(&mut decoder, 0xe0);
        decoder.reset_after_loss();
        assert!(decode(&mut decoder, 0x1e).unwrap() == Some(press(0x1e)));
    }

    #[test]
    fn keymap_tracks_independent_shift_edges() {
        let mut keymap = bootstrap_keymap::new();
        assert!(keymap.map(press(0x2a)).is_none());
        assert!(keymap.map(press(0x36)).is_none());
        assert!(keymap.map(release(0x2a)).is_none());
        assert!(matches!(
            keymap.map(press(0x1e)),
            Some(key_event::character(b'A'))
        ));
        assert!(keymap.map(release(0x36)).is_none());
        assert!(matches!(
            keymap.map(press(0x1e)),
            Some(key_event::character(b'a'))
        ));
    }

    #[test]
    fn caps_lock_toggles_once_per_physical_press() {
        let mut keymap = bootstrap_keymap::new();
        keymap.map(press(0x3a));
        keymap.map(press(0x3a));
        assert!(matches!(
            keymap.map(press(0x1e)),
            Some(key_event::character(b'A'))
        ));
        keymap.map(release(0x3a));
        keymap.map(press(0x3a));
        assert!(matches!(
            keymap.map(press(0x1e)),
            Some(key_event::character(b'a'))
        ));
    }

    #[test]
    fn shift_and_caps_lock_use_xor_for_letters() {
        let mut keymap = bootstrap_keymap::new();
        keymap.map(press(0x3a));
        keymap.map(release(0x3a));
        keymap.map(press(0x2a));
        assert!(matches!(
            keymap.map(press(0x1e)),
            Some(key_event::character(b'a'))
        ));
    }

    #[test]
    fn all_bootstrap_letters_map_in_scan_order() {
        let cases = [
            (0x1e, b'a'),
            (0x30, b'b'),
            (0x2e, b'c'),
            (0x20, b'd'),
            (0x12, b'e'),
            (0x21, b'f'),
            (0x22, b'g'),
            (0x23, b'h'),
            (0x17, b'i'),
            (0x24, b'j'),
            (0x25, b'k'),
            (0x26, b'l'),
            (0x32, b'm'),
            (0x31, b'n'),
            (0x18, b'o'),
            (0x19, b'p'),
            (0x10, b'q'),
            (0x13, b'r'),
            (0x1f, b's'),
            (0x14, b't'),
            (0x16, b'u'),
            (0x2f, b'v'),
            (0x11, b'w'),
            (0x2d, b'x'),
            (0x15, b'y'),
            (0x2c, b'z'),
        ];
        let mut keymap = bootstrap_keymap::new();
        for (make_code, expected) in cases {
            assert!(matches!(
                keymap.map(press(make_code)),
                Some(key_event::character(byte)) if byte == expected
            ));
        }
    }

    #[test]
    fn all_bootstrap_punctuation_has_shifted_and_plain_forms() {
        let cases = [
            (0x02, b'1', b'!'),
            (0x03, b'2', b'@'),
            (0x04, b'3', b'#'),
            (0x05, b'4', b'$'),
            (0x06, b'5', b'%'),
            (0x07, b'6', b'^'),
            (0x08, b'7', b'&'),
            (0x09, b'8', b'*'),
            (0x0a, b'9', b'('),
            (0x0b, b'0', b')'),
            (0x0c, b'-', b'_'),
            (0x0d, b'=', b'+'),
            (0x1a, b'[', b'{'),
            (0x1b, b']', b'}'),
            (0x27, b';', b':'),
            (0x28, b'\'', b'"'),
            (0x29, b'`', b'~'),
            (0x2b, b'\\', b'|'),
            (0x33, b',', b'<'),
            (0x34, b'.', b'>'),
            (0x35, b'/', b'?'),
            (0x39, b' ', b' '),
        ];
        for (make_code, plain, shifted) in cases {
            let mut keymap = bootstrap_keymap::new();
            assert!(matches!(
                keymap.map(press(make_code)),
                Some(key_event::character(byte)) if byte == plain
            ));
            keymap.map(press(0x2a));
            assert!(matches!(
                keymap.map(press(make_code)),
                Some(key_event::character(byte)) if byte == shifted
            ));
        }
    }

    #[test]
    fn release_extended_and_unsupported_keys_do_not_make_text() {
        let mut keymap = bootstrap_keymap::new();
        assert!(keymap.map(release(0x1e)).is_none());
        assert!(
            keymap
                .map(transition(0x1c, true, key_state::pressed))
                .is_none()
        );
        assert!(keymap.map(press(0x01)).is_none());
    }

    #[test]
    fn enter_and_backspace_are_semantic_keys() {
        let mut keymap = bootstrap_keymap::new();
        assert!(matches!(keymap.map(press(0x1c)), Some(key_event::enter)));
        assert!(matches!(
            keymap.map(press(0x0e)),
            Some(key_event::backspace)
        ));
    }

    #[test]
    fn control_and_alt_suppress_bootstrap_text() {
        for modifier in [
            press(0x1d),
            transition(0x1d, true, key_state::pressed),
            press(0x38),
            transition(0x38, true, key_state::pressed),
        ] {
            let mut keymap = bootstrap_keymap::new();
            keymap.map(modifier);
            assert!(keymap.map(press(0x1e)).is_none());
        }
    }

    #[test]
    fn loss_reset_clears_all_held_and_latched_modifiers() {
        let mut keymap = bootstrap_keymap::new();
        for modifier in [
            press(0x2a),
            press(0x36),
            press(0x1d),
            transition(0x1d, true, key_state::pressed),
            press(0x38),
            transition(0x38, true, key_state::pressed),
            press(0x3a),
        ] {
            keymap.map(modifier);
        }
        keymap.reset_after_loss();
        assert!(matches!(
            keymap.map(press(0x1e)),
            Some(key_event::character(b'a'))
        ));
    }
}
