//! Read-only, bounded QEMU provisioning for the native interactive profile.
//! fw_cfg `opt/anomalous/interactive`: ANDF, u16 version1, u16 size64,
//! u64 phase0(format)/1(mount) at8, u64 proof mode0..2 at16, zero tail24..64.
//! It contains no expected text. Task/device grants remain retained separately.

use core::arch::asm;

const selector_port: u16 = 0x510;
const data_port: u16 = 0x511;
const directory_selector: u16 = 0x19;
const directory_capacity: usize = 128;
const fixture_name: &[u8] = b"opt/anomalous/interactive";
const fixture_size: usize = 64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct interactive_fixture {
    pub(super) phase: u64,
    pub(super) proof_mode: u64,
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
    proof_mode,
    reserved,
}
impl fixture_error {
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::interrupts_enabled => "interactive_fixture_interrupts",
            Self::signature => "interactive_fixture_signature",
            Self::revision => "interactive_fixture_revision",
            Self::directory_count => "interactive_fixture_directory_count",
            Self::directory_entry => "interactive_fixture_directory_entry",
            Self::duplicate_entry => "interactive_fixture_duplicate",
            Self::missing_entry => "interactive_fixture_missing",
            Self::blob_size => "interactive_fixture_size",
            Self::blob_header => "interactive_fixture_header",
            Self::phase => "interactive_fixture_phase",
            Self::proof_mode => "interactive_fixture_proof_mode",
            Self::reserved => "interactive_fixture_reserved",
        }
    }
}
fn decode(bytes: &[u8]) -> Result<interactive_fixture, fixture_error> {
    if bytes.len() != fixture_size {
        return Err(fixture_error::blob_size);
    }
    if bytes[..8] != [b'A', b'N', b'D', b'F', 1, 0, 64, 0] {
        return Err(fixture_error::blob_header);
    }
    let phase = u64::from_le_bytes(bytes[8..16].try_into().unwrap());
    let proof_mode = u64::from_le_bytes(bytes[16..24].try_into().unwrap());
    if phase > 1 {
        return Err(fixture_error::phase);
    }
    if proof_mode > 2 {
        return Err(fixture_error::proof_mode);
    }
    if bytes[24..].iter().any(|byte| *byte != 0) {
        return Err(fixture_error::reserved);
    }
    Ok(interactive_fixture { phase, proof_mode })
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
            if size != fixture_size as u32
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
pub(super) unsafe fn read() -> Result<interactive_fixture, fixture_error> {
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
        if size != fixture_size {
            return Err(fixture_error::blob_size);
        }
        decode(&read_bytes::<fixture_size>())
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
    fn blob(phase: u64, mode: u64) -> [u8; fixture_size] {
        let mut b = [0; fixture_size];
        b[..8].copy_from_slice(&[b'A', b'N', b'D', b'F', 1, 0, 64, 0]);
        b[8..16].copy_from_slice(&phase.to_le_bytes());
        b[16..24].copy_from_slice(&mode.to_le_bytes());
        b
    }
    #[test]
    fn canonical_modes_have_no_answer_bytes() {
        for phase in 0..=1 {
            for proof_mode in 0..=2 {
                assert_eq!(
                    decode(&blob(phase, proof_mode)),
                    Ok(interactive_fixture { phase, proof_mode })
                );
            }
        }
        assert_eq!(decode(&blob(2, 0)), Err(fixture_error::phase));
        assert_eq!(decode(&blob(0, 3)), Err(fixture_error::proof_mode));
        let b = blob(0, 0);
        for n in 0..fixture_size {
            assert_eq!(decode(&b[..n]), Err(fixture_error::blob_size));
        }
        for n in 24..fixture_size {
            let mut b = b;
            b[n] = 1;
            assert_eq!(decode(&b), Err(fixture_error::reserved));
        }
    }
    #[test]
    fn directory_requires_complete_unique_exact_entry() {
        let mut entry = [0; 64];
        entry[..4].copy_from_slice(&(fixture_size as u32).to_be_bytes());
        entry[4..6].copy_from_slice(&0x20u16.to_be_bytes());
        entry[8..8 + fixture_name.len()].copy_from_slice(fixture_name);
        let mut scan = directory_scan::new(2).unwrap();
        scan.observe(&entry).unwrap();
        assert_eq!(scan.observe(&entry), Err(fixture_error::duplicate_entry));
        assert_eq!(scan.finish(), Err(fixture_error::directory_count));
        let mut scan = directory_scan::new(1).unwrap();
        scan.observe(&entry).unwrap();
        assert_eq!(scan.finish(), Ok((0x20, fixture_size)));
        assert_eq!(
            directory_scan::new(0).unwrap().finish(),
            Err(fixture_error::missing_entry)
        );
        for size in [0u32, 63, 65, u32::MAX] {
            entry[..4].copy_from_slice(&size.to_be_bytes());
            assert_eq!(
                directory_scan::new(1).unwrap().observe(&entry),
                Err(fixture_error::directory_entry)
            );
        }
    }
}
