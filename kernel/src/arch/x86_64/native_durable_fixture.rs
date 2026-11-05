//! Read-only QEMU provisioning input for the experimental durable proof.
//!
//! Reviewed interface: https://www.qemu.org/docs/master/specs/fw_cfg.html
//! The x86 selector is a 16-bit OUT at 0x510; data is byte IN at 0x511.
//! Directory count/size/selector are big-endian; the fixture itself is LE.
//! No data-port writes, DMA, firmware mutation, or device grants are exposed.
//! `-fw_cfg name=opt/anomalous-durable,file=...` supplies exactly 48 bytes:
//! ANDI[0..4], version:u32@4=1, phase:u64@8, length:u8@16, text[17..48].
//! Phase 0 carries 1..31 printable ASCII bytes; phase 2 carries no text. All
//! unused bytes are zero. Recovery therefore receives no expected answer.
//! The separate failure proof admits ANDI v2: exactly 56 bytes, the same
//! phase/text fields, and a case:u64@48 in 1..=7. Normal profiles reject v2.
//!
//! This item supplies fixture phase/data only, never task, device, extent, or
//! formatting authority. The trusted proof controller must install those grants
//! independently. The bounded directory is a QEMU proof limit, not a native
//! filesystem path or name limit. Missing/malformed input has no fallback.

use core::arch::asm;

const selector_port: u16 = 0x510;
const data_port: u16 = 0x511;
const directory_selector: u16 = 0x19;
const directory_capacity: usize = 128;
const fixture_name: &[u8] = b"opt/anomalous-durable";
const fixture_size: usize = 48;
const failure_fixture_size: usize = 56;
const text_capacity: usize = 31;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u64)]
pub(super) enum fixture_phase {
    provision_save = 0,
    cold_recovery = 2,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct durable_fixture {
    phase: fixture_phase,
    text: [u8; text_capacity],
    length: usize,
    case: u64,
}

impl durable_fixture {
    pub(super) const fn phase(&self) -> fixture_phase {
        self.phase
    }

    pub(super) fn challenge(&self) -> &[u8] {
        &self.text[..self.length]
    }

    #[cfg_attr(not(feature = "native_durable_failure_test"), allow(dead_code))]
    pub(super) const fn case(&self) -> u64 {
        self.case
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum fixture_error {
    interrupts_enabled,
    signature,
    revision,
    directory_count,
    directory_entry,
    duplicate_entry,
    missing_entry,
    blob_size,
    blob_header,
    phase,
    challenge,
    case,
}

impl fixture_error {
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::interrupts_enabled => "durable_fixture_interrupts",
            Self::signature => "durable_fixture_signature",
            Self::revision => "durable_fixture_revision",
            Self::directory_count => "durable_fixture_directory_count",
            Self::directory_entry => "durable_fixture_directory_entry",
            Self::duplicate_entry => "durable_fixture_duplicate",
            Self::missing_entry => "durable_fixture_missing",
            Self::blob_size => "durable_fixture_size",
            Self::blob_header => "durable_fixture_header",
            Self::phase => "durable_fixture_phase",
            Self::challenge => "durable_fixture_challenge",
            Self::case => "durable_fixture_case",
        }
    }
}

fn decode(bytes: &[u8]) -> Result<durable_fixture, fixture_error> {
    decode_profile(
        bytes,
        cfg!(any(test, feature = "native_durable_failure_test")),
    )
}

fn decode_profile(bytes: &[u8], failure_enabled: bool) -> Result<durable_fixture, fixture_error> {
    if bytes.len() != fixture_size && !(failure_enabled && bytes.len() == failure_fixture_size) {
        return Err(fixture_error::blob_size);
    }
    let expected_version = if bytes.len() == fixture_size { 1u32 } else { 2 };
    if &bytes[..4] != b"ANDI" || bytes[4..8] != expected_version.to_le_bytes() {
        return Err(fixture_error::blob_header);
    }
    let case = if bytes.len() == failure_fixture_size {
        let value = u64::from_le_bytes(bytes[48..56].try_into().unwrap());
        if !(1..=7).contains(&value) {
            return Err(fixture_error::case);
        }
        value
    } else {
        0
    };
    let phase = match u64::from_le_bytes(bytes[8..16].try_into().unwrap()) {
        0 => fixture_phase::provision_save,
        2 => fixture_phase::cold_recovery,
        _ => return Err(fixture_error::phase),
    };
    let length = usize::from(bytes[16]);
    if length > text_capacity
        || (phase == fixture_phase::provision_save && length == 0)
        || (phase == fixture_phase::cold_recovery && length != 0)
        || bytes[17..17 + length]
            .iter()
            .any(|byte| !(b' '..=b'~').contains(byte))
        || bytes[17 + length..fixture_size]
            .iter()
            .any(|byte| *byte != 0)
    {
        return Err(fixture_error::challenge);
    }
    let mut text = [0; text_capacity];
    text[..length].copy_from_slice(&bytes[17..17 + length]);
    Ok(durable_fixture {
        phase,
        text,
        length,
        case,
    })
}

/// The complete directory is scanned before changing selectors. A duplicate
/// target never wins by directory order, and an incomplete scan cannot finish.
struct directory_scan {
    remaining: usize,
    selected: Option<(u16, usize)>,
}

impl directory_scan {
    fn new(count: u32) -> Result<Self, fixture_error> {
        let count = usize::try_from(count).map_err(|_| fixture_error::directory_count)?;
        if count > directory_capacity {
            return Err(fixture_error::directory_count);
        }
        Ok(Self {
            remaining: count,
            selected: None,
        })
    }

    fn observe(&mut self, entry: &[u8; 64]) -> Result<(), fixture_error> {
        if self.remaining == 0 {
            return Err(fixture_error::directory_count);
        }
        let name = &entry[8..];
        let terminator = name
            .iter()
            .position(|byte| *byte == 0)
            .ok_or(fixture_error::directory_entry)?;
        if terminator == 0 || entry[6..8] != [0, 0] {
            return Err(fixture_error::directory_entry);
        }
        // Unrelated file names are opaque. Only the exact fixture name may
        // select its payload, with canonical zero padding in this proof.
        if &name[..terminator] == fixture_name {
            if self.selected.is_some() {
                return Err(fixture_error::duplicate_entry);
            }
            let size = u32::from_be_bytes(entry[..4].try_into().unwrap());
            let selector = u16::from_be_bytes(entry[4..6].try_into().unwrap());
            if !(size == fixture_size as u32
                || (cfg!(any(test, feature = "native_durable_failure_test"))
                    && size == failure_fixture_size as u32))
                || !(0x20..0x4000).contains(&selector)
                || name[terminator..].iter().any(|byte| *byte != 0)
            {
                return Err(fixture_error::directory_entry);
            }
            self.selected = Some((selector, size as usize));
        }
        self.remaining -= 1;
        Ok(())
    }

    fn finish(self) -> Result<(u16, usize), fixture_error> {
        if self.remaining != 0 {
            return Err(fixture_error::directory_count);
        }
        self.selected.ok_or(fixture_error::missing_entry)
    }
}

/// Read the single reviewed fixture during exclusive, interrupt-free boot.
///
/// # Safety
/// The caller must own the x86 QEMU fw_cfg ports, execute on the sole BSP before
/// any concurrent firmware/driver access, and retain that exclusivity for the
/// entire read. IF is checked before any port operation. Presence validation
/// does not authorize probing arbitrary machines or supply disk authority.
pub(super) unsafe fn read() -> Result<durable_fixture, fixture_error> {
    if super::interrupts::interrupts_enabled() {
        return Err(fixture_error::interrupts_enabled);
    }
    // SAFETY: Caller owns the fixed interface under the checked IF-clear boot
    // contract. Only selector OUT and data IN occur, all with bounded counts.
    unsafe {
        select(0);
        if read_bytes::<4>() != *b"QEMU" {
            return Err(fixture_error::signature);
        }
        select(1);
        if u32::from_le_bytes(read_bytes::<4>()) & 1 == 0 {
            return Err(fixture_error::revision);
        }
        select(directory_selector);
        let count = u32::from_be_bytes(read_bytes::<4>());
        let mut scan = directory_scan::new(count)?;
        for _ in 0..count {
            scan.observe(&read_bytes::<64>())?;
        }
        let (selector, size) = scan.finish()?;
        select(selector);
        match size {
            fixture_size => decode(&read_bytes::<fixture_size>()),
            #[cfg(any(test, feature = "native_durable_failure_test"))]
            failure_fixture_size => decode(&read_bytes::<failure_fixture_size>()),
            _ => Err(fixture_error::blob_size),
        }
    }
}

unsafe fn select(selector: u16) {
    // SAFETY: Only the caller-owned selector register is written. No DMA or
    // fw_cfg write-mode selector is admitted by the public read sequence.
    unsafe {
        asm!("out dx, ax", in("dx") selector_port, in("ax") selector,
            options(nomem, nostack, preserves_flags));
    }
}

unsafe fn read_bytes<const N: usize>() -> [u8; N] {
    let mut bytes = [0; N];
    for byte in &mut bytes {
        // SAFETY: Caller owns the selected read-only byte stream. Exactly N
        // bytes are read; no device-provided size can allocate or loop here.
        unsafe {
            asm!("in al, dx", in("dx") data_port, out("al") *byte,
                options(nomem, nostack, preserves_flags));
        }
    }
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    fn blob(phase: u64, text: &[u8]) -> [u8; fixture_size] {
        let mut bytes = [0; fixture_size];
        bytes[..4].copy_from_slice(b"ANDI");
        bytes[4..8].copy_from_slice(&1u32.to_le_bytes());
        bytes[8..16].copy_from_slice(&phase.to_le_bytes());
        bytes[16] = text.len() as u8;
        bytes[17..17 + text.len()].copy_from_slice(text);
        bytes
    }

    fn entry(name: &[u8], selector: u16) -> [u8; 64] {
        let mut bytes = [0; 64];
        bytes[..4].copy_from_slice(&(fixture_size as u32).to_be_bytes());
        bytes[4..6].copy_from_slice(&selector.to_be_bytes());
        bytes[8..8 + name.len()].copy_from_slice(name);
        bytes
    }

    #[test]
    fn exact_phase_and_text_decode_without_recovery_answer() {
        for text in [b"!".as_slice(), b"run-specific editor text", &[b'~'; 31]] {
            let value = decode(&blob(0, text)).unwrap();
            assert_eq!(value.phase(), fixture_phase::provision_save);
            assert_eq!(value.challenge(), text);
            assert_eq!(value.case(), 0);
        }
        let value = decode(&blob(2, &[])).unwrap();
        assert_eq!(value.phase(), fixture_phase::cold_recovery);
        assert!(value.challenge().is_empty());
        assert_eq!(decode(&blob(0, &[])), Err(fixture_error::challenge));
        assert_eq!(decode(&blob(2, b"answer")), Err(fixture_error::challenge));
    }

    #[test]
    fn failure_version_is_exact_feature_scoped_and_never_supplies_cold_answer() {
        for case in 1u64..=7 {
            for (phase, text) in [(0, b"attempted".as_slice()), (2, b"".as_slice())] {
                let mut bytes = [0; failure_fixture_size];
                bytes[..fixture_size].copy_from_slice(&blob(phase, text));
                bytes[4..8].copy_from_slice(&2u32.to_le_bytes());
                bytes[48..].copy_from_slice(&case.to_le_bytes());
                let value = decode(&bytes).unwrap();
                assert_eq!(value.case(), case);
                assert_eq!(value.challenge(), text);
                assert_eq!(decode_profile(&bytes, false), Err(fixture_error::blob_size));
                for length in 0..failure_fixture_size {
                    assert!(decode(&bytes[..length]).is_err());
                }
                let mut too_long = bytes.to_vec();
                too_long.push(0);
                assert_eq!(decode(&too_long), Err(fixture_error::blob_size));
                for invalid in [0u64, 8, u64::MAX] {
                    let mut changed = bytes;
                    changed[48..].copy_from_slice(&invalid.to_le_bytes());
                    assert_eq!(decode(&changed), Err(fixture_error::case));
                }
                for offset in 17 + text.len()..48 {
                    let mut changed = bytes;
                    changed[offset] = 1;
                    assert_eq!(decode(&changed), Err(fixture_error::challenge));
                }
                let mut wrong_version = bytes;
                wrong_version[4..8].copy_from_slice(&1u32.to_le_bytes());
                assert_eq!(decode(&wrong_version), Err(fixture_error::blob_header));
            }
        }
        let mut wrong_version = blob(0, b"attempted");
        wrong_version[4..8].copy_from_slice(&2u32.to_le_bytes());
        assert_eq!(decode(&wrong_version), Err(fixture_error::blob_header));
        let mut selected = entry(fixture_name, 0x30);
        selected[..4].copy_from_slice(&(failure_fixture_size as u32).to_be_bytes());
        let mut scan = directory_scan::new(1).unwrap();
        scan.observe(&selected).unwrap();
        assert_eq!(scan.finish(), Ok((0x30, failure_fixture_size)));
    }

    #[test]
    fn malformed_header_phase_length_text_and_every_noncanonical_tail_byte_reject() {
        let valid = blob(0, b"value");
        for length in 0..fixture_size {
            assert_eq!(decode(&valid[..length]), Err(fixture_error::blob_size));
        }
        assert_eq!(
            decode(&[0; fixture_size + 1]),
            Err(fixture_error::blob_size)
        );
        for offset in 0..8 {
            let mut bytes = valid;
            bytes[offset] ^= 1;
            assert_eq!(decode(&bytes), Err(fixture_error::blob_header));
        }
        for phase in [1, 3, 256, u64::MAX] {
            assert_eq!(decode(&blob(phase, b"value")), Err(fixture_error::phase));
        }
        for length in 32..=255 {
            let mut bytes = valid;
            bytes[16] = length;
            assert_eq!(decode(&bytes), Err(fixture_error::challenge));
        }
        for value in [0, b'\n', b'\t', 0x1f, 0x7f, 0x80, 0xff] {
            let mut bytes = valid;
            bytes[17] = value;
            assert_eq!(decode(&bytes), Err(fixture_error::challenge));
        }
        for offset in 22..fixture_size {
            let mut bytes = valid;
            bytes[offset] = 1;
            assert_eq!(decode(&bytes), Err(fixture_error::challenge));
        }
        for offset in 17..fixture_size {
            let mut bytes = blob(2, &[]);
            bytes[offset] = b'x';
            assert_eq!(decode(&bytes), Err(fixture_error::challenge));
        }
    }

    #[test]
    fn directory_requires_one_exact_target_and_complete_bounded_scan() {
        assert!(matches!(
            directory_scan::new(129),
            Err(fixture_error::directory_count)
        ));
        assert!(matches!(
            directory_scan::new(u32::MAX),
            Err(fixture_error::directory_count)
        ));
        assert_eq!(
            directory_scan::new(0).unwrap().finish(),
            Err(fixture_error::missing_entry)
        );
        let target = entry(fixture_name, 0x30);
        let mut scan = directory_scan::new(3).unwrap();
        scan.observe(&entry(b"etc/other", 0x20)).unwrap();
        scan.observe(&target).unwrap();
        scan.observe(&entry(b"opt/anomalous-durable-extra", 0x31))
            .unwrap();
        assert_eq!(scan.finish(), Ok((0x30, fixture_size)));
        let mut scan = directory_scan::new(2).unwrap();
        scan.observe(&target).unwrap();
        assert_eq!(scan.finish(), Err(fixture_error::directory_count));
        let mut scan = directory_scan::new(2).unwrap();
        scan.observe(&target).unwrap();
        assert_eq!(scan.observe(&target), Err(fixture_error::duplicate_entry));
        let mut scan = directory_scan::new(1).unwrap();
        scan.observe(&entry(b"opt/anomalous-durable-extra", 0x30))
            .unwrap();
        assert_eq!(scan.observe(&target), Err(fixture_error::directory_count));
        assert_eq!(scan.finish(), Err(fixture_error::missing_entry));
    }

    #[test]
    fn selected_directory_entry_rejects_size_selector_reserved_and_padding_mutations() {
        let valid = entry(fixture_name, 0x20);
        for size in [0, 47, 49, u32::MAX] {
            let mut bytes = valid;
            bytes[..4].copy_from_slice(&size.to_be_bytes());
            assert_eq!(
                directory_scan::new(1).unwrap().observe(&bytes),
                Err(fixture_error::directory_entry)
            );
        }
        for selector in [0u16, 0x19, 0x1f, 0x4000, 0x8000, 0xffff] {
            let mut bytes = valid;
            bytes[4..6].copy_from_slice(&selector.to_be_bytes());
            assert_eq!(
                directory_scan::new(1).unwrap().observe(&bytes),
                Err(fixture_error::directory_entry)
            );
        }
        for offset in [6, 7] {
            let mut bytes = valid;
            bytes[offset] = 1;
            assert_eq!(
                directory_scan::new(1).unwrap().observe(&bytes),
                Err(fixture_error::directory_entry)
            );
        }
        for offset in 8 + fixture_name.len() + 1..64 {
            let mut bytes = valid;
            bytes[offset] = 1;
            assert_eq!(
                directory_scan::new(1).unwrap().observe(&bytes),
                Err(fixture_error::directory_entry)
            );
        }
        let mut bytes = valid;
        bytes[8..].fill(b'x');
        assert_eq!(
            directory_scan::new(1).unwrap().observe(&bytes),
            Err(fixture_error::directory_entry)
        );
        bytes[8..].fill(0);
        assert_eq!(
            directory_scan::new(1).unwrap().observe(&bytes),
            Err(fixture_error::directory_entry)
        );
    }
}
