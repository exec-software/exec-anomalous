use core::{mem, ptr};

pub const boot_omen_magic: u64 = 0x544f_4f42_4d4f_4e41;
pub const boot_omen_version: u32 = 2;

const framebuffer_present: u32 = 1 << 0;
const memory_map_complete: u32 = 1 << 1;
pub(crate) const maximum_memory_entries: usize = 64;
const identity_map_limit: u64 = 4 * 1024 * 1024 * 1024;

#[derive(Clone, Copy, Debug)]
pub struct color_channel {
    pub position: u8,
    pub size: u8,
}

#[derive(Clone, Copy, Debug)]
pub struct framebuffer_info {
    pub address: u64,
    pub pitch: usize,
    pub width: usize,
    pub height: usize,
    pub bits_per_pixel: u8,
    pub red: color_channel,
    pub green: color_channel,
    pub blue: color_channel,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum framebuffer_error {
    invalid_bit_depth,
    invalid_geometry,
    extent_overflow,
    outside_identity_map,
    invalid_color_channel,
}

impl framebuffer_error {
    pub const fn reason(self) -> &'static str {
        match self {
            Self::invalid_bit_depth => "framebuffer_bit_depth",
            Self::invalid_geometry => "framebuffer_geometry",
            Self::extent_overflow => "framebuffer_extent_overflow",
            Self::outside_identity_map => "framebuffer_outside_identity_map",
            Self::invalid_color_channel => "framebuffer_color_channel",
        }
    }
}

/// Versioned contract passed from Threshold to the Rust kernel.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct boot_omen {
    pub magic: u64,
    pub version: u32,
    pub size: u32,
    pub boot_drive: u32,
    pub flags: u32,
    pub framebuffer_address: u64,
    pub framebuffer_pitch: u32,
    pub framebuffer_width: u32,
    pub framebuffer_height: u32,
    pub framebuffer_bits_per_pixel: u32,
    pub red_position: u32,
    pub red_size: u32,
    pub green_position: u32,
    pub green_size: u32,
    pub blue_position: u32,
    pub blue_size: u32,
    pub memory_map_address: u64,
    pub memory_map_entry_count: u32,
    pub memory_map_entry_size: u32,
}

const _: [(); 88] = [(); mem::size_of::<boot_omen>()];

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(C)]
pub struct memory_region {
    pub(crate) base: u64,
    pub(crate) length: u64,
    pub(crate) kind: u32,
    pub(crate) attributes: u32,
}

const _: [(); 24] = [(); mem::size_of::<memory_region>()];

impl memory_region {
    #[must_use]
    pub const fn base(&self) -> u64 {
        self.base
    }

    #[must_use]
    pub const fn length(&self) -> u64 {
        self.length
    }

    #[must_use]
    pub const fn kind(&self) -> u32 {
        self.kind
    }

    #[must_use]
    pub const fn attributes(&self) -> u32 {
        self.attributes
    }

    #[must_use]
    pub const fn end(&self) -> Option<u64> {
        self.base.checked_add(self.length)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum boot_memory_error {
    invalid_omen,
    incomplete_map,
    invalid_entry_size,
    invalid_entry_count,
    invalid_table_address,
    table_overflow,
    region_overflow,
}

impl boot_memory_error {
    pub const fn reason(self) -> &'static str {
        match self {
            Self::invalid_omen => "invalid_omen",
            Self::incomplete_map => "incomplete_map",
            Self::invalid_entry_size => "invalid_entry_size",
            Self::invalid_entry_count => "invalid_entry_count",
            Self::invalid_table_address => "invalid_table_address",
            Self::table_overflow => "table_overflow",
            Self::region_overflow => "region_overflow",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct boot_memory_map {
    regions: [memory_region; maximum_memory_entries],
    region_count: usize,
}

impl boot_memory_map {
    #[must_use]
    pub fn regions(&self) -> &[memory_region] {
        &self.regions[..self.region_count]
    }
}

impl boot_omen {
    pub fn has_valid_header(&self) -> bool {
        self.magic == boot_omen_magic
            && self.version == boot_omen_version
            && self.size as usize >= mem::size_of::<Self>()
    }

    pub fn is_valid(&self) -> bool {
        self.has_valid_header() && self.flags & memory_map_complete != 0
    }

    /// Copy Threshold's raw memory descriptors into kernel-owned storage.
    ///
    /// The returned slice preserves every descriptor. Each enabled,
    /// non-zero-length descriptor has a non-wrapping `base + length` range;
    /// later normalization decides which descriptors grant usable memory.
    ///
    /// # Safety
    ///
    /// `memory_map_address..memory_map_address + byte_len` must name initialized,
    /// readable storage containing the complete descriptor array produced by
    /// Threshold. That storage must remain live and must not be mutated for the
    /// duration of this call. Integer bounds alone cannot establish this pointer
    /// provenance; the caller must obtain it from the validated Threshold
    /// handoff.
    pub unsafe fn memory_map(&self) -> Result<boot_memory_map, boot_memory_error> {
        if !self.has_valid_header() {
            return Err(boot_memory_error::invalid_omen);
        }
        if self.flags & memory_map_complete == 0 {
            return Err(boot_memory_error::incomplete_map);
        }

        let (source, entry_count) = validate_memory_table(
            self.memory_map_address,
            self.memory_map_entry_count,
            self.memory_map_entry_size,
        )?;

        let mut result = boot_memory_map {
            regions: [memory_region::default(); maximum_memory_entries],
            region_count: entry_count,
        };
        for index in 0..entry_count {
            let address = source + index * mem::size_of::<memory_region>();
            // SAFETY: The caller guarantees provenance, initialization, and a
            // stable lifetime for the source table. `validate_memory_table`
            // bounds every descriptor read to that table and the bootstrap
            // identity map. Unaligned firmware descriptors are permitted.
            let entry = unsafe { ptr::read_unaligned(address as *const memory_region) };
            result.regions[index] = validate_memory_region(entry)?;
        }
        Ok(result)
    }

    pub fn framebuffer(&self) -> Result<Option<framebuffer_info>, framebuffer_error> {
        if self.flags & framebuffer_present == 0 {
            return Ok(None);
        }

        let bits_per_pixel = u8::try_from(self.framebuffer_bits_per_pixel)
            .map_err(|_| framebuffer_error::invalid_bit_depth)?;
        if !(24..=32).contains(&bits_per_pixel) {
            return Err(framebuffer_error::invalid_bit_depth);
        }
        let bytes_per_pixel = usize::from(bits_per_pixel).div_ceil(8);
        let pitch = self.framebuffer_pitch as usize;
        let width = self.framebuffer_width as usize;
        let height = self.framebuffer_height as usize;
        let minimum_pitch = width
            .checked_mul(bytes_per_pixel)
            .ok_or(framebuffer_error::invalid_geometry)?;
        let byte_len = pitch
            .checked_mul(height)
            .ok_or(framebuffer_error::extent_overflow)?;
        let byte_len = u64::try_from(byte_len).map_err(|_| framebuffer_error::extent_overflow)?;
        let end = self
            .framebuffer_address
            .checked_add(byte_len)
            .ok_or(framebuffer_error::extent_overflow)?;

        if self.framebuffer_address == 0 || width == 0 || height == 0 || pitch < minimum_pitch {
            return Err(framebuffer_error::invalid_geometry);
        }
        if end > identity_map_limit {
            return Err(framebuffer_error::outside_identity_map);
        }

        let red = channel(self.red_position, self.red_size, bits_per_pixel)
            .ok_or(framebuffer_error::invalid_color_channel)?;
        let green = channel(self.green_position, self.green_size, bits_per_pixel)
            .ok_or(framebuffer_error::invalid_color_channel)?;
        let blue = channel(self.blue_position, self.blue_size, bits_per_pixel)
            .ok_or(framebuffer_error::invalid_color_channel)?;
        if channels_overlap(red, green)
            || channels_overlap(red, blue)
            || channels_overlap(green, blue)
        {
            return Err(framebuffer_error::invalid_color_channel);
        }

        Ok(Some(framebuffer_info {
            address: self.framebuffer_address,
            pitch,
            width,
            height,
            bits_per_pixel,
            red,
            green,
            blue,
        }))
    }
}

fn validate_memory_table(
    address: u64,
    entry_count: u32,
    entry_size: u32,
) -> Result<(usize, usize), boot_memory_error> {
    if entry_size as usize != mem::size_of::<memory_region>() {
        return Err(boot_memory_error::invalid_entry_size);
    }
    let entry_count = entry_count as usize;
    if entry_count == 0 || entry_count > maximum_memory_entries {
        return Err(boot_memory_error::invalid_entry_count);
    }
    if address == 0 || address >= identity_map_limit {
        return Err(boot_memory_error::invalid_table_address);
    }

    let byte_len = entry_count
        .checked_mul(mem::size_of::<memory_region>())
        .ok_or(boot_memory_error::table_overflow)?;
    let end = address
        .checked_add(byte_len as u64)
        .ok_or(boot_memory_error::table_overflow)?;
    if end > identity_map_limit {
        return Err(boot_memory_error::invalid_table_address);
    }
    let address = usize::try_from(address).map_err(|_| boot_memory_error::invalid_table_address)?;
    Ok((address, entry_count))
}

fn validate_memory_region(region: memory_region) -> Result<memory_region, boot_memory_error> {
    if region.length == 0 || region.attributes & 1 == 0 {
        return Ok(region);
    }
    region
        .base
        .checked_add(region.length)
        .ok_or(boot_memory_error::region_overflow)?;
    Ok(region)
}

fn channel(position: u32, size: u32, bits_per_pixel: u8) -> Option<color_channel> {
    let position = u8::try_from(position).ok()?;
    let size = u8::try_from(size).ok()?;
    if position >= bits_per_pixel
        || size == 0
        || size > 16
        || u16::from(position) + u16::from(size) > u16::from(bits_per_pixel)
    {
        return None;
    }
    Some(color_channel { position, size })
}

const fn channels_overlap(left: color_channel, right: color_channel) -> bool {
    let left_end = left.position as u16 + left.size as u16;
    let right_end = right.position as u16 + right.size as u16;
    (left.position as u16) < right_end && (right.position as u16) < left_end
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_omen() -> boot_omen {
        boot_omen {
            magic: boot_omen_magic,
            version: boot_omen_version,
            size: mem::size_of::<boot_omen>() as u32,
            boot_drive: 0x80,
            flags: memory_map_complete,
            framebuffer_address: 0,
            framebuffer_pitch: 0,
            framebuffer_width: 0,
            framebuffer_height: 0,
            framebuffer_bits_per_pixel: 0,
            red_position: 0,
            red_size: 0,
            green_position: 0,
            green_size: 0,
            blue_position: 0,
            blue_size: 0,
            memory_map_address: 0x1000,
            memory_map_entry_count: 1,
            memory_map_entry_size: mem::size_of::<memory_region>() as u32,
        }
    }

    #[test]
    fn boot_omen_layout_matches_threshold_version_2() {
        assert_eq!(mem::size_of::<boot_omen>(), 88);
        assert_eq!(mem::offset_of!(boot_omen, magic), 0);
        assert_eq!(mem::offset_of!(boot_omen, version), 8);
        assert_eq!(mem::offset_of!(boot_omen, size), 12);
        assert_eq!(mem::offset_of!(boot_omen, boot_drive), 16);
        assert_eq!(mem::offset_of!(boot_omen, flags), 20);
        assert_eq!(mem::offset_of!(boot_omen, framebuffer_address), 24);
        assert_eq!(mem::offset_of!(boot_omen, framebuffer_pitch), 32);
        assert_eq!(mem::offset_of!(boot_omen, framebuffer_width), 36);
        assert_eq!(mem::offset_of!(boot_omen, framebuffer_height), 40);
        assert_eq!(mem::offset_of!(boot_omen, framebuffer_bits_per_pixel), 44);
        assert_eq!(mem::offset_of!(boot_omen, red_position), 48);
        assert_eq!(mem::offset_of!(boot_omen, red_size), 52);
        assert_eq!(mem::offset_of!(boot_omen, green_position), 56);
        assert_eq!(mem::offset_of!(boot_omen, green_size), 60);
        assert_eq!(mem::offset_of!(boot_omen, blue_position), 64);
        assert_eq!(mem::offset_of!(boot_omen, blue_size), 68);
        assert_eq!(mem::offset_of!(boot_omen, memory_map_address), 72);
        assert_eq!(mem::offset_of!(boot_omen, memory_map_entry_count), 80);
        assert_eq!(mem::offset_of!(boot_omen, memory_map_entry_size), 84);
    }

    #[test]
    fn memory_region_layout_matches_threshold_descriptors() {
        assert_eq!(mem::size_of::<memory_region>(), 24);
        assert_eq!(mem::offset_of!(memory_region, base), 0);
        assert_eq!(mem::offset_of!(memory_region, length), 8);
        assert_eq!(mem::offset_of!(memory_region, kind), 16);
        assert_eq!(mem::offset_of!(memory_region, attributes), 20);
    }

    #[test]
    fn boot_omen_requires_version_2_and_a_complete_map() {
        assert_eq!(framebuffer_present, 1 << 0);
        assert_eq!(memory_map_complete, 1 << 1);

        let omen = valid_omen();
        assert!(omen.is_valid());

        let mut version_1 = omen;
        version_1.version = 1;
        assert!(!version_1.is_valid());

        let mut incomplete = omen;
        incomplete.flags &= !memory_map_complete;
        assert!(!incomplete.is_valid());
    }

    #[test]
    fn memory_table_validation_is_bounded_and_exact() {
        assert_eq!(validate_memory_table(0x1000, 2, 24), Ok((0x1000, 2)));
        assert_eq!(
            validate_memory_table(0x1000, 0, 24),
            Err(boot_memory_error::invalid_entry_count)
        );
        assert_eq!(
            validate_memory_table(0x1000, 65, 24),
            Err(boot_memory_error::invalid_entry_count)
        );
        assert_eq!(
            validate_memory_table(0x1000, 1, 20),
            Err(boot_memory_error::invalid_entry_size)
        );
        assert_eq!(
            validate_memory_table(0, 1, 24),
            Err(boot_memory_error::invalid_table_address)
        );
        assert_eq!(
            validate_memory_table(identity_map_limit - 23, 1, 24),
            Err(boot_memory_error::invalid_table_address)
        );
        assert_eq!(
            validate_memory_table(identity_map_limit - 24, 1, 24),
            Ok(((identity_map_limit - 24) as usize, 1))
        );
    }

    #[test]
    fn descriptor_validation_preserves_inactive_entries() {
        let disabled = memory_region {
            base: u64::MAX,
            length: 2,
            kind: 1,
            attributes: 0,
        };
        let empty = memory_region {
            base: u64::MAX,
            length: 0,
            kind: 1,
            attributes: 1,
        };

        assert_eq!(validate_memory_region(disabled), Ok(disabled));
        assert_eq!(validate_memory_region(empty), Ok(empty));
    }

    #[test]
    fn descriptor_validation_rejects_enabled_range_overflow() {
        let overflow = memory_region {
            base: u64::MAX,
            length: 2,
            kind: 1,
            attributes: 1,
        };
        assert_eq!(
            validate_memory_region(overflow),
            Err(boot_memory_error::region_overflow)
        );

        let valid = memory_region {
            base: 0x1000,
            length: 0x2000,
            kind: 1,
            attributes: 1,
        };
        assert_eq!(validate_memory_region(valid), Ok(valid));
        assert_eq!(valid.end(), Some(0x3000));
    }

    #[test]
    fn advertised_framebuffer_is_validated_or_rejected() {
        let mut omen = valid_omen();
        assert!(matches!(omen.framebuffer(), Ok(None)));

        omen.flags |= framebuffer_present;
        omen.framebuffer_address = 0x0080_0000;
        omen.framebuffer_pitch = 3072;
        omen.framebuffer_width = 1024;
        omen.framebuffer_height = 768;
        omen.framebuffer_bits_per_pixel = 24;
        omen.red_position = 16;
        omen.red_size = 8;
        omen.green_position = 8;
        omen.green_size = 8;
        omen.blue_position = 0;
        omen.blue_size = 8;
        let framebuffer = omen.framebuffer().unwrap().unwrap();
        assert_eq!(framebuffer.address, 0x0080_0000);
        assert_eq!(framebuffer.pitch, 3072);

        omen.blue_size = 0;
        assert_eq!(
            omen.framebuffer().unwrap_err(),
            framebuffer_error::invalid_color_channel
        );
        omen.blue_size = 8;
        omen.framebuffer_pitch = 1;
        assert_eq!(
            omen.framebuffer().unwrap_err(),
            framebuffer_error::invalid_geometry
        );
        omen.framebuffer_pitch = 3072;
        omen.blue_position = 24;
        assert_eq!(
            omen.framebuffer().unwrap_err(),
            framebuffer_error::invalid_color_channel
        );
        omen.blue_position = 8;
        assert_eq!(
            omen.framebuffer().unwrap_err(),
            framebuffer_error::invalid_color_channel
        );
    }
}
