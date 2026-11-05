//! Experimental copied application profile, not a stable general application ABI.
//!
//! All integers are encoded explicitly in little-endian order. Rust layouts are
//! never wire layouts. The exact task, channel, surface, focus, and grants belong
//! to the receiver's trusted binding; none can be asserted by these packets.
//! Sequence/revision values are correlation, not authorization or replay policy.
//! Decoding borrows immutable bytes after the caller has copied them into owned
//! storage. The caller must retain that storage through use of the decoded view.

pub const application_surface_v1_size: usize = 3_168;
pub const application_surface_v1_header_size: usize = 96;
pub const application_input_v1_size: usize = 64;
pub const application_text_capacity: usize = 31;
pub const application_surface_width: u16 = 128;
pub const application_surface_height: u16 = 8;
pub const application_surface_stride: u16 = 384;
pub const application_surface_pixel_size: usize = 3_072;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum application_wire_error {
    bad_size,
    bad_magic,
    bad_version,
    bad_reserved,
    bad_sequence,
    bad_geometry,
    bad_text,
    bad_selection,
    bad_dirty,
    bad_theme,
    bad_undo_depth,
    bad_command,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum application_theme {
    light,
    dark,
}

impl application_theme {
    pub const fn raw(self) -> u8 {
        match self {
            Self::light => 0,
            Self::dark => 1,
        }
    }

    pub const fn from_raw(value: u8) -> Result<Self, application_wire_error> {
        match value {
            0 => Ok(Self::light),
            1 => Ok(Self::dark),
            _ => Err(application_wire_error::bad_theme),
        }
    }
}

/// Semantic state accompanying one complete fixed-size RGB8 frame.
///
/// Text is empty or printable ASCII, without NUL, through 31 bytes. Selection
/// offsets count bytes and may name either end of the text, in either order.
/// This bounded profile has at most one undo entry. It defines no file identity,
/// saving, text shaping, Unicode input, clipboard, or general editor protocol.
/// The application advances document_revision only for text edits and undo;
/// selection/theme changes do not advance it. This stateless codec validates
/// values, while the model and channel enforce transitions and nonwrapping order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct application_surface_state<'a> {
    pub request_sequence: u64,
    pub document_revision: u64,
    pub text: &'a [u8],
    pub selection_anchor: u8,
    pub selection_head: u8,
    pub dirty: bool,
    pub theme: application_theme,
    pub undo_depth: u8,
}

impl application_surface_state<'_> {
    fn validate(self) -> Result<(), application_wire_error> {
        if self.request_sequence == 0 || self.document_revision == 0 {
            return Err(application_wire_error::bad_sequence);
        }
        validate_text(self.text)?;
        if usize::from(self.selection_anchor) > self.text.len()
            || usize::from(self.selection_head) > self.text.len()
        {
            return Err(application_wire_error::bad_selection);
        }
        if self.undo_depth > 1 {
            return Err(application_wire_error::bad_undo_depth);
        }
        Ok(())
    }
}

/// Borrowed validated surface packet. Pixel content is opaque RGB8 here;
/// agreement between pixels and semantic state needs a separate renderer oracle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct application_surface_v1<'a> {
    state: application_surface_state<'a>,
    pixels: &'a [u8],
}

impl<'a> application_surface_v1<'a> {
    pub fn try_new(
        state: application_surface_state<'a>,
        pixels: &'a [u8],
    ) -> Result<Self, application_wire_error> {
        state.validate()?;
        if pixels.len() != application_surface_pixel_size {
            return Err(application_wire_error::bad_size);
        }
        Ok(Self { state, pixels })
    }

    pub const fn state(&self) -> application_surface_state<'a> {
        self.state
    }

    pub const fn pixels(&self) -> &'a [u8] {
        self.pixels
    }

    /// Encode a complete packet, rejecting a wrong destination length without
    /// changing any destination byte. All reserved bytes and text padding are zero.
    pub fn encode_into(&self, bytes: &mut [u8]) -> Result<(), application_wire_error> {
        if bytes.len() != application_surface_v1_size {
            return Err(application_wire_error::bad_size);
        }
        bytes.fill(0);
        bytes[0..4].copy_from_slice(b"ANSF");
        bytes[4..6].copy_from_slice(&1_u16.to_le_bytes());
        bytes[6..8].copy_from_slice(&(application_surface_v1_header_size as u16).to_le_bytes());
        bytes[8..12].copy_from_slice(&(application_surface_v1_size as u32).to_le_bytes());
        bytes[16..24].copy_from_slice(&self.state.request_sequence.to_le_bytes());
        bytes[24..32].copy_from_slice(&self.state.document_revision.to_le_bytes());
        bytes[32..34].copy_from_slice(&application_surface_width.to_le_bytes());
        bytes[34..36].copy_from_slice(&application_surface_height.to_le_bytes());
        bytes[36..38].copy_from_slice(&application_surface_stride.to_le_bytes());
        bytes[38..40].copy_from_slice(&1_u16.to_le_bytes());
        bytes[40] = self.state.text.len() as u8;
        bytes[41] = self.state.selection_anchor;
        bytes[42] = self.state.selection_head;
        bytes[43] = u8::from(self.state.dirty);
        bytes[44] = self.state.theme.raw();
        bytes[45] = self.state.undo_depth;
        bytes[64..64 + self.state.text.len()].copy_from_slice(self.state.text);
        bytes[application_surface_v1_header_size..].copy_from_slice(self.pixels);
        Ok(())
    }

    /// Decode only this exact version/profile. Unknown flags, reserved bytes,
    /// nonzero text padding, trailing bytes, and different geometry are rejected.
    pub fn decode(bytes: &'a [u8]) -> Result<Self, application_wire_error> {
        if bytes.len() != application_surface_v1_size {
            return Err(application_wire_error::bad_size);
        }
        if &bytes[0..4] != b"ANSF" {
            return Err(application_wire_error::bad_magic);
        }
        if read_u16(bytes, 4) != 1 {
            return Err(application_wire_error::bad_version);
        }
        if read_u16(bytes, 6) as usize != application_surface_v1_header_size
            || read_u32(bytes, 8) as usize != application_surface_v1_size
        {
            return Err(application_wire_error::bad_size);
        }
        if bytes[12..16]
            .iter()
            .chain(&bytes[46..64])
            .any(|byte| *byte != 0)
        {
            return Err(application_wire_error::bad_reserved);
        }
        if read_u16(bytes, 32) != application_surface_width
            || read_u16(bytes, 34) != application_surface_height
            || read_u16(bytes, 36) != application_surface_stride
            || read_u16(bytes, 38) != 1
        {
            return Err(application_wire_error::bad_geometry);
        }
        let text = decode_text(&bytes[64..96], bytes[40])?;
        if bytes[43] > 1 {
            return Err(application_wire_error::bad_dirty);
        }
        Self::try_new(
            application_surface_state {
                request_sequence: read_u64(bytes, 16),
                document_revision: read_u64(bytes, 24),
                text,
                selection_anchor: bytes[41],
                selection_head: bytes[42],
                dirty: bytes[43] != 0,
                theme: application_theme::from_raw(bytes[44])?,
                undo_depth: bytes[45],
            },
            &bytes[application_surface_v1_header_size..],
        )
    }
}

/// Commands carry no source principal, focus assertion, or granted rights.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum application_input_action<'a> {
    replace_selection(&'a [u8]),
    set_selection { anchor: u8, head: u8 },
    set_theme(application_theme),
    undo,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct application_input_v1<'a> {
    sequence: u64,
    action: application_input_action<'a>,
}

impl<'a> application_input_v1<'a> {
    pub fn try_new(
        sequence: u64,
        action: application_input_action<'a>,
    ) -> Result<Self, application_wire_error> {
        if sequence == 0 {
            return Err(application_wire_error::bad_sequence);
        }
        match action {
            application_input_action::replace_selection(text) => validate_text(text)?,
            application_input_action::set_selection { anchor, head }
                if usize::from(anchor) > application_text_capacity
                    || usize::from(head) > application_text_capacity =>
            {
                return Err(application_wire_error::bad_selection);
            }
            _ => {}
        }
        Ok(Self { sequence, action })
    }

    pub const fn sequence(&self) -> u64 {
        self.sequence
    }

    pub const fn action(&self) -> application_input_action<'a> {
        self.action
    }

    /// Exact-length encoding. Input selection bounds against the current
    /// document and replacement capacity are checked by the application model.
    pub fn encode_into(&self, bytes: &mut [u8]) -> Result<(), application_wire_error> {
        if bytes.len() != application_input_v1_size {
            return Err(application_wire_error::bad_size);
        }
        bytes.fill(0);
        bytes[0..4].copy_from_slice(b"ANIN");
        bytes[4..6].copy_from_slice(&1_u16.to_le_bytes());
        bytes[6..8].copy_from_slice(&(application_input_v1_size as u16).to_le_bytes());
        bytes[8..16].copy_from_slice(&self.sequence.to_le_bytes());
        let kind = match self.action {
            application_input_action::replace_selection(text) => {
                bytes[18] = text.len() as u8;
                bytes[32..32 + text.len()].copy_from_slice(text);
                1_u16
            }
            application_input_action::set_selection { anchor, head } => {
                bytes[20] = anchor;
                bytes[21] = head;
                2
            }
            application_input_action::set_theme(theme) => {
                bytes[19] = theme.raw();
                3
            }
            application_input_action::undo => 4,
        };
        bytes[16..18].copy_from_slice(&kind.to_le_bytes());
        Ok(())
    }

    pub fn decode(bytes: &'a [u8]) -> Result<Self, application_wire_error> {
        if bytes.len() != application_input_v1_size {
            return Err(application_wire_error::bad_size);
        }
        if &bytes[0..4] != b"ANIN" {
            return Err(application_wire_error::bad_magic);
        }
        if read_u16(bytes, 4) != 1 {
            return Err(application_wire_error::bad_version);
        }
        if read_u16(bytes, 6) as usize != application_input_v1_size {
            return Err(application_wire_error::bad_size);
        }
        if bytes[22..32].iter().any(|byte| *byte != 0) {
            return Err(application_wire_error::bad_reserved);
        }
        let text = decode_text(&bytes[32..64], bytes[18])?;
        let action = match read_u16(bytes, 16) {
            1 => {
                if bytes[19..22].iter().any(|byte| *byte != 0) {
                    return Err(application_wire_error::bad_reserved);
                }
                application_input_action::replace_selection(text)
            }
            2 => {
                if bytes[18] != 0 || bytes[19] != 0 {
                    return Err(application_wire_error::bad_reserved);
                }
                application_input_action::set_selection {
                    anchor: bytes[20],
                    head: bytes[21],
                }
            }
            3 => {
                if bytes[18] != 0 || bytes[20] != 0 || bytes[21] != 0 {
                    return Err(application_wire_error::bad_reserved);
                }
                application_input_action::set_theme(application_theme::from_raw(bytes[19])?)
            }
            4 => {
                if bytes[18..22].iter().any(|byte| *byte != 0) {
                    return Err(application_wire_error::bad_reserved);
                }
                application_input_action::undo
            }
            _ => return Err(application_wire_error::bad_command),
        };
        Self::try_new(read_u64(bytes, 8), action)
    }
}

fn validate_text(text: &[u8]) -> Result<(), application_wire_error> {
    if text.len() > application_text_capacity
        || text.iter().any(|byte| !(b' '..=b'~').contains(byte))
    {
        return Err(application_wire_error::bad_text);
    }
    Ok(())
}

fn decode_text(area: &[u8], length: u8) -> Result<&[u8], application_wire_error> {
    let length = usize::from(length);
    if length > application_text_capacity {
        return Err(application_wire_error::bad_text);
    }
    let text = &area[..length];
    validate_text(text)?;
    if area[length..].iter().any(|byte| *byte != 0) {
        return Err(application_wire_error::bad_reserved);
    }
    Ok(text)
}

fn read_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([bytes[offset], bytes[offset + 1]])
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(
        bytes[offset..offset + 4]
            .try_into()
            .expect("checked packet size"),
    )
}

fn read_u64(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(
        bytes[offset..offset + 8]
            .try_into()
            .expect("checked packet size"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(text: &[u8]) -> application_surface_state<'_> {
        application_surface_state {
            request_sequence: 0x0807_0605_0403_0201,
            document_revision: 0x1817_1615_1413_1211,
            text,
            selection_anchor: 0,
            selection_head: text.len() as u8,
            dirty: true,
            theme: application_theme::dark,
            undo_depth: 1,
        }
    }

    #[test]
    fn surface_encoding_has_exact_little_endian_golden_header_and_borrowed_pixels() {
        let mut pixels = [0; application_surface_pixel_size];
        pixels[0] = 17;
        pixels[application_surface_pixel_size - 1] = 93;
        let packet = application_surface_v1::try_new(state(b"Ny"), &pixels).unwrap();
        let mut bytes = [0xcc; application_surface_v1_size];
        packet.encode_into(&mut bytes).unwrap();
        assert_eq!(&bytes[..16], b"ANSF\x01\0\x60\0\x60\x0c\0\0\0\0\0\0");
        assert_eq!(
            &bytes[16..32],
            &[1, 2, 3, 4, 5, 6, 7, 8, 17, 18, 19, 20, 21, 22, 23, 24]
        );
        assert_eq!(
            &bytes[32..46],
            &[128, 0, 8, 0, 128, 1, 1, 0, 2, 0, 2, 1, 1, 1]
        );
        assert!(bytes[46..64].iter().all(|byte| *byte == 0));
        assert_eq!(&bytes[64..66], b"Ny");
        assert!(bytes[66..96].iter().all(|byte| *byte == 0));
        assert_eq!(&bytes[96..], &pixels);
        let decoded = application_surface_v1::decode(&bytes).unwrap();
        assert_eq!(decoded.state(), packet.state());
        assert_eq!(decoded.pixels(), &pixels);
        assert_eq!(decoded.pixels().as_ptr(), bytes[96..].as_ptr());
    }

    #[test]
    fn surface_rejects_every_reserved_byte_bad_fixed_fields_and_noncanonical_values() {
        let pixels = [0; application_surface_pixel_size];
        let packet = application_surface_v1::try_new(state(b"Ny"), &pixels).unwrap();
        let mut valid = [0; application_surface_v1_size];
        packet.encode_into(&mut valid).unwrap();
        for offset in (12..16).chain(46..64).chain(66..96) {
            let mut bytes = valid;
            bytes[offset] = 1;
            assert!(
                application_surface_v1::decode(&bytes).is_err(),
                "offset {offset}"
            );
        }
        for (offset, value) in [
            (0, b'X'),
            (4, 2),
            (5, 1),
            (6, 95),
            (8, 0),
            (11, 1),
            (32, 127),
            (34, 9),
            (36, 0),
            (38, 2),
            (40, 32),
            (41, 3),
            (42, 3),
            (43, 2),
            (44, 2),
            (45, 2),
            (64, 0),
            (65, 127),
        ] {
            let mut bytes = valid;
            bytes[offset] = value;
            assert!(
                application_surface_v1::decode(&bytes).is_err(),
                "offset {offset}"
            );
        }
        for range in [16..24, 24..32] {
            let mut bytes = valid;
            bytes[range].fill(0);
            assert_eq!(
                application_surface_v1::decode(&bytes),
                Err(application_wire_error::bad_sequence)
            );
        }
    }

    #[test]
    fn surface_accepts_empty_and_maximum_text_and_rejects_unrepresentable_state() {
        let pixels = [0; application_surface_pixel_size];
        for text in [&b""[..], &b"1234567890123456789012345678901"[..]] {
            let mut semantic = state(text);
            semantic.selection_anchor = semantic.selection_head;
            semantic.selection_head = 0;
            semantic.request_sequence = u64::MAX;
            semantic.document_revision = u64::MAX;
            let packet = application_surface_v1::try_new(semantic, &pixels).unwrap();
            let mut bytes = [0; application_surface_v1_size];
            packet.encode_into(&mut bytes).unwrap();
            assert_eq!(
                application_surface_v1::decode(&bytes).unwrap().state(),
                semantic
            );
        }
        for text in [
            &b"bad\n"[..],
            &b"bad\0"[..],
            &b"\x80"[..],
            &b"12345678901234567890123456789012"[..],
        ] {
            assert!(application_surface_v1::try_new(state(text), &pixels).is_err());
        }
        let mut invalid = state(b"Ny");
        invalid.undo_depth = 2;
        assert!(application_surface_v1::try_new(invalid, &pixels).is_err());
        invalid = state(b"Ny");
        invalid.selection_anchor = 3;
        assert!(application_surface_v1::try_new(invalid, &pixels).is_err());
        assert!(application_surface_v1::try_new(state(b"Ny"), &pixels[..3071]).is_err());
    }

    #[test]
    fn input_encoding_has_exact_layout_and_each_command_roundtrips() {
        for action in [
            application_input_action::replace_selection(b"Ny"),
            application_input_action::replace_selection(b""),
            application_input_action::set_selection {
                anchor: 31,
                head: 0,
            },
            application_input_action::set_theme(application_theme::light),
            application_input_action::set_theme(application_theme::dark),
            application_input_action::undo,
        ] {
            let input = application_input_v1::try_new(0x0807_0605_0403_0201, action).unwrap();
            let mut bytes = [0xcc; application_input_v1_size];
            input.encode_into(&mut bytes).unwrap();
            assert_eq!(
                &bytes[..16],
                b"ANIN\x01\0\x40\0\x01\x02\x03\x04\x05\x06\x07\x08"
            );
            assert_eq!(application_input_v1::decode(&bytes).unwrap(), input);
            assert!(bytes[22..32].iter().all(|byte| *byte == 0));
        }
        let input =
            application_input_v1::try_new(1, application_input_action::replace_selection(b"Ny"))
                .unwrap();
        let mut bytes = [0; application_input_v1_size];
        input.encode_into(&mut bytes).unwrap();
        assert_eq!(&bytes[16..22], &[1, 0, 2, 0, 0, 0]);
        assert_eq!(&bytes[32..34], b"Ny");
        assert!(bytes[34..64].iter().all(|byte| *byte == 0));
    }

    #[test]
    fn input_rejects_all_reserved_or_unused_arguments_and_unknown_commands() {
        for (action, unused) in [
            (
                application_input_action::replace_selection(b"Ny"),
                &[19_usize, 20, 21][..],
            ),
            (
                application_input_action::set_selection { anchor: 2, head: 1 },
                &[18, 19][..],
            ),
            (
                application_input_action::set_theme(application_theme::dark),
                &[18, 20, 21][..],
            ),
            (application_input_action::undo, &[18, 19, 20, 21][..]),
        ] {
            let input = application_input_v1::try_new(1, action).unwrap();
            let mut valid = [0; application_input_v1_size];
            input.encode_into(&mut valid).unwrap();
            let padding_start = 32 + usize::from(valid[18]);
            for offset in (22..32)
                .chain(padding_start..64)
                .chain(unused.iter().copied())
            {
                let mut bytes = valid;
                bytes[offset] = 1;
                assert!(
                    application_input_v1::decode(&bytes).is_err(),
                    "offset {offset}"
                );
            }
            for (offset, value) in [
                (0, b'X'),
                (4, 2),
                (6, 63),
                (16, 0),
                (16, 5),
                (17, 1),
                (18, 32),
            ] {
                let mut bytes = valid;
                bytes[offset] = value;
                assert!(
                    application_input_v1::decode(&bytes).is_err(),
                    "offset {offset}"
                );
            }
            let mut bytes = valid;
            bytes[8..16].fill(0);
            assert_eq!(
                application_input_v1::decode(&bytes),
                Err(application_wire_error::bad_sequence)
            );
        }
        assert!(
            application_input_v1::try_new(0, application_input_action::replace_selection(b""))
                .is_err()
        );
        assert!(
            application_input_v1::try_new(
                1,
                application_input_action::set_selection {
                    anchor: 32,
                    head: 0
                }
            )
            .is_err()
        );
        assert!(
            application_input_v1::try_new(1, application_input_action::replace_selection(b"\n"))
                .is_err()
        );
    }

    #[test]
    fn input_decoder_rejects_invalid_theme_selection_and_text_values() {
        for (action, offset, value) in [
            (
                application_input_action::set_theme(application_theme::light),
                19,
                2,
            ),
            (
                application_input_action::set_selection { anchor: 0, head: 0 },
                20,
                32,
            ),
            (
                application_input_action::set_selection { anchor: 0, head: 0 },
                21,
                255,
            ),
            (application_input_action::replace_selection(b"x"), 32, 0),
            (application_input_action::replace_selection(b"x"), 32, b'\n'),
            (application_input_action::replace_selection(b"x"), 32, 128),
        ] {
            let input = application_input_v1::try_new(1, action).unwrap();
            let mut bytes = [0; application_input_v1_size];
            input.encode_into(&mut bytes).unwrap();
            bytes[offset] = value;
            assert!(application_input_v1::decode(&bytes).is_err());
        }
        let text = b"1234567890123456789012345678901";
        let input = application_input_v1::try_new(
            u64::MAX,
            application_input_action::replace_selection(text),
        )
        .unwrap();
        let mut bytes = [0; application_input_v1_size];
        input.encode_into(&mut bytes).unwrap();
        assert_eq!(application_input_v1::decode(&bytes).unwrap(), input);
    }

    #[test]
    fn exact_lengths_are_required_and_failed_encoding_preserves_destination() {
        let pixels = [0; application_surface_pixel_size];
        let surface = application_surface_v1::try_new(state(b"Ny"), &pixels).unwrap();
        let input =
            application_input_v1::try_new(1, application_input_action::replace_selection(b"Ny"))
                .unwrap();
        let mut surface_bytes = [0xa5; application_surface_v1_size + 1];
        let mut input_bytes = [0xa5; application_input_v1_size + 1];
        for length in [
            0,
            1,
            application_surface_v1_size - 1,
            application_surface_v1_size + 1,
        ] {
            assert_eq!(
                surface.encode_into(&mut surface_bytes[..length]),
                Err(application_wire_error::bad_size)
            );
            assert!(surface_bytes.iter().all(|byte| *byte == 0xa5));
            assert!(application_surface_v1::decode(&surface_bytes[..length]).is_err());
        }
        for length in [
            0,
            1,
            application_input_v1_size - 1,
            application_input_v1_size + 1,
        ] {
            assert_eq!(
                input.encode_into(&mut input_bytes[..length]),
                Err(application_wire_error::bad_size)
            );
            assert!(input_bytes.iter().all(|byte| *byte == 0xa5));
            assert!(application_input_v1::decode(&input_bytes[..length]).is_err());
        }
    }
}
