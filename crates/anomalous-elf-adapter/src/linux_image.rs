#![forbid(unsafe_code)]

//! Structural Linux ELF64 image planning, separate from the qualified ANEL format.
//!
//! Plans borrow immutable source bytes and carry no mapping or execution authority.
//! They do not perform relocation, resolve an interpreter or library, interpret
//! GNU properties, establish TLS, or certify instruction/CPU compatibility.
//! Unknown program headers remain visible in `image`, but prevent a load plan.
//! NOTE/GNU_PROPERTY contents remain explicit metadata requiring caller policy.
//!
//! Geometry follows the System V gABI program-header rules:
//! <https://gabi.xinuos.com/elf/07-pheader.html>. Linux's interpreter/PHDR startup
//! boundary is in <https://github.com/torvalds/linux/blob/v6.17/fs/binfmt_elf.c>.
//! This profile deliberately rejects overlapping rounded LOAD pages and executable
//! stacks. Budgets are supplied per operation; no pathname-size limit is imposed.

pub const page_size: u64 = 4096;
pub const program_header_size: u16 = 56;
pub const user_address_end: u64 = 1 << 47;
pub const pt_null: u32 = 0;
pub const pt_load: u32 = 1;
pub const pt_dynamic: u32 = 2;
pub const pt_interp: u32 = 3;
pub const pt_note: u32 = 4;
pub const pt_phdr: u32 = 6;
pub const pt_tls: u32 = 7;
pub const pt_gnu_eh_frame: u32 = 0x6474_e550;
pub const pt_gnu_stack: u32 = 0x6474_e551;
pub const pt_gnu_relro: u32 = 0x6474_e552;
pub const pt_gnu_property: u32 = 0x6474_e553;

/// Explicit resource budget, independent of filenames and namespace policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct limits {
    pub image_bytes: u64,
    pub program_headers: usize,
    pub load_segments: usize,
    pub mapped_pages: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum image_error {
    image_budget,
    program_header_budget,
    load_budget,
    mapped_page_budget,
    truncated,
    invalid_header,
    unsupported_extended_numbering,
    invalid_section_table,
    invalid_program_header,
    file_range,
    virtual_range,
    alignment,
    load_order,
    overlapping_load_pages,
    permissions,
    no_loads,
    duplicate_header(u32),
    interpreter,
    metadata_not_loaded(u32),
    program_header_address,
    unsupported_program_header(u32),
    load_bias,
    entry,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum elf_type {
    executable,
    shared,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum image_role {
    /// A main program or interpreter: entry must name initialized executable bytes.
    executable,
    /// An object loaded by a linker; e_entry may be zero and is not selected.
    shared_object,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct byte_range {
    start: u64,
    end: u64,
}

impl byte_range {
    pub const fn start(self) -> u64 {
        self.start
    }
    pub const fn end(self) -> u64 {
        self.end
    }
    pub const fn len(self) -> u64 {
        self.end - self.start
    }
    pub const fn is_empty(self) -> bool {
        self.start == self.end
    }
    fn contains(self, other: Self) -> bool {
        self.start <= other.start && other.end <= self.end
    }
    fn overlaps(self, other: Self) -> bool {
        !self.is_empty() && !other.is_empty() && self.start < other.end && other.start < self.end
    }
}

/// Raw inert ELF fields, including ignored p_paddr. Never a physical grant.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct program_header {
    pub kind: u32,
    pub flags: u32,
    pub file_offset: u64,
    pub virtual_address: u64,
    pub physical_address: u64,
    pub file_size: u64,
    pub memory_size: u64,
    pub alignment: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct section_table {
    pub file_offset: u64,
    pub entry_size: u16,
    pub count: u16,
    pub string_table_index: u16,
}

#[derive(Clone, Copy, Debug)]
pub struct image<'a> {
    source: &'a [u8],
    kind: elf_type,
    os_abi: u8,
    entry: u64,
    table: byte_range,
    header_count: usize,
    load_count: usize,
    pages: u64,
    alignment: u64,
    sections: Option<section_table>,
}

impl<'a> image<'a> {
    pub fn parse(source: &'a [u8], budget: limits) -> Result<Self, image_error> {
        if source.len() as u64 > budget.image_bytes {
            return Err(image_error::image_budget);
        }
        if source.len() < 64 {
            return Err(image_error::truncated);
        }
        if &source[..7] != b"\x7fELF\x02\x01\x01"
            || !matches!(source[7], 0 | 3)
            || source[8..16].iter().any(|b| *b != 0)
            || u16_at(source, 18) != 62
            || u32_at(source, 20) != 1
            || u32_at(source, 48) != 0
            || u16_at(source, 52) != 64
            || u16_at(source, 54) != program_header_size
        {
            return Err(image_error::invalid_header);
        }
        let kind = match u16_at(source, 16) {
            2 => elf_type::executable,
            3 => elf_type::shared,
            _ => return Err(image_error::invalid_header),
        };
        let header_count = u16_at(source, 56) as usize;
        if header_count == 0xffff {
            return Err(image_error::unsupported_extended_numbering);
        }
        if header_count == 0 {
            return Err(image_error::no_loads);
        }
        if header_count > budget.program_headers {
            return Err(image_error::program_header_budget);
        }
        let table = range(
            u64_at(source, 32),
            header_count as u64 * u64::from(program_header_size),
        )?;
        if table.start < 64 {
            return Err(image_error::invalid_header);
        }
        file_bytes(source, table)?;
        let sections = sections(source)?;
        let mut value = Self {
            source,
            kind,
            os_abi: source[7],
            entry: u64_at(source, 24),
            table,
            header_count,
            load_count: 0,
            pages: 0,
            alignment: page_size,
            sections,
        };
        let mut previous_load = None;
        for (index, header) in value.program_headers().enumerate() {
            if header.kind == pt_null {
                continue;
            } // All remaining NULL fields are undefined.
            file_bytes(source, range(header.file_offset, header.file_size)?)?;
            range(header.virtual_address, header.memory_size)?;
            if header.alignment > 1 && !header.alignment.is_power_of_two() {
                return Err(image_error::alignment);
            }
            if singleton(header.kind)
                && value
                    .program_headers()
                    .take(index)
                    .any(|h| h.kind == header.kind)
            {
                return Err(image_error::duplicate_header(header.kind));
            }
            match header.kind {
                pt_load => {
                    value.load_count += 1;
                    if value.load_count > budget.load_segments {
                        return Err(image_error::load_budget);
                    }
                    if header.file_size > header.memory_size {
                        return Err(image_error::invalid_program_header);
                    }
                    // x86-64 page permissions cannot express write-only or execute-only.
                    if !matches!(header.flags, 4..=6) {
                        return Err(image_error::permissions);
                    }
                    congruent(header, page_size)?;
                    congruent(header, header.alignment.max(1))?;
                    if previous_load.is_some_and(|address| address > header.virtual_address) {
                        return Err(image_error::load_order);
                    }
                    previous_load = Some(header.virtual_address);
                    let mapped = rounded(header.virtual_address, header.memory_size)?;
                    for other in value
                        .program_headers()
                        .take(index)
                        .filter(|h| h.kind == pt_load)
                    {
                        if mapped.overlaps(rounded(other.virtual_address, other.memory_size)?) {
                            return Err(image_error::overlapping_load_pages);
                        }
                    }
                    value.pages = value
                        .pages
                        .checked_add(mapped.len() / page_size)
                        .ok_or(image_error::mapped_page_budget)?;
                    if value.pages > budget.mapped_pages {
                        return Err(image_error::mapped_page_budget);
                    }
                    value.alignment = value.alignment.max(header.alignment);
                }
                pt_interp => {
                    if previous_load.is_some() {
                        return Err(image_error::invalid_program_header);
                    }
                    let path = file_bytes(source, range(header.file_offset, header.file_size)?)?;
                    if path.len() < 2
                        || path[0] != b'/'
                        || path.last() != Some(&0)
                        || path[..path.len() - 1].contains(&0)
                    {
                        return Err(image_error::interpreter);
                    }
                }
                pt_phdr => {
                    if previous_load.is_some()
                        || header.file_offset != table.start
                        || header.file_size != table.len()
                        || header.memory_size != table.len()
                    {
                        return Err(image_error::program_header_address);
                    }
                }
                pt_dynamic => {
                    if header.file_size == 0
                        || !header.file_size.is_multiple_of(16)
                        || header.file_size != header.memory_size
                    {
                        return Err(image_error::invalid_program_header);
                    }
                }
                pt_gnu_eh_frame | pt_gnu_property => {
                    if header.file_size == 0 || header.file_size != header.memory_size {
                        return Err(image_error::invalid_program_header);
                    }
                }
                pt_tls => {
                    if header.file_size > header.memory_size || header.flags != 4 {
                        return Err(image_error::invalid_program_header);
                    }
                    congruent(header, header.alignment.max(1))?;
                }
                pt_gnu_stack => {
                    if header.flags & !7 != 0 || header.flags & 1 != 0 {
                        return Err(image_error::permissions);
                    }
                    if header.file_size != 0 || header.memory_size != 0 {
                        return Err(image_error::invalid_program_header);
                    }
                }
                _ => {}
            }
        }
        if value.load_count == 0 || value.pages == 0 {
            return Err(image_error::no_loads);
        }
        value.validate_metadata()?;
        Ok(value)
    }

    pub const fn source(self) -> &'a [u8] {
        self.source
    }
    pub const fn elf_type(self) -> elf_type {
        self.kind
    }
    pub const fn os_abi(self) -> u8 {
        self.os_abi
    }
    pub const fn entry_offset(self) -> u64 {
        self.entry
    }
    pub const fn program_header_file_range(self) -> byte_range {
        self.table
    }
    pub const fn program_header_count(self) -> usize {
        self.header_count
    }
    pub const fn load_count(self) -> usize {
        self.load_count
    }
    pub const fn mapped_pages(self) -> u64 {
        self.pages
    }
    pub const fn required_bias_alignment(self) -> u64 {
        self.alignment
    }
    pub const fn sections(self) -> Option<section_table> {
        self.sections
    }
    pub fn program_headers(self) -> program_headers<'a> {
        program_headers {
            bytes: &self.source[self.table.start as usize..self.table.end as usize],
            index: 0,
        }
    }
    pub fn header(self, kind: u32) -> Option<program_header> {
        self.program_headers().find(|h| h.kind == kind)
    }
    /// Absolute byte path, without its terminator. Resolution is separate policy:
    /// dot components, repeated slashes, non-UTF8 names and long paths are retained.
    pub fn interp(self) -> Option<&'a [u8]> {
        self.header(pt_interp).map(|h| {
            &self.source[h.file_offset as usize..(h.file_offset + h.file_size - 1) as usize]
        })
    }
    /// Access a parsed header's bytes without trusting a publicly constructed header.
    pub fn header_bytes(self, header_index: usize) -> Option<&'a [u8]> {
        let h = self.program_headers().nth(header_index)?;
        if h.kind == pt_null {
            return None;
        }
        file_bytes(self.source, range(h.file_offset, h.file_size).ok()?).ok()
    }

    fn validate_metadata(self) -> Result<(), image_error> {
        for h in self.program_headers() {
            match h.kind {
                pt_phdr | pt_dynamic | pt_gnu_eh_frame | pt_gnu_property => {
                    if h.flags & !7 != 0
                        || h.flags & 4 == 0
                        || h.flags & 1 != 0
                        || !self.initialized_metadata(h)?
                    {
                        return Err(image_error::metadata_not_loaded(h.kind));
                    }
                }
                pt_tls => {
                    // Only the template's initialized bytes belong to a LOAD.
                    // Its thread-local zero tail is allocated separately by a linker.
                    if h.file_size != 0 && !self.initialized_metadata(h)? {
                        return Err(image_error::metadata_not_loaded(h.kind));
                    }
                }
                pt_gnu_relro => {
                    if h.flags != 4 || h.file_size > h.memory_size || !self.memory_metadata(h)? {
                        return Err(image_error::metadata_not_loaded(h.kind));
                    }
                    if h.file_size != 0 && !self.initialized_metadata(h)? {
                        return Err(image_error::metadata_not_loaded(h.kind));
                    }
                }
                _ => {}
            }
        }
        if let Some(h) = self.header(pt_phdr)
            && self.phdr_offset()? != Some(h.virtual_address)
        {
            return Err(image_error::program_header_address);
        }
        Ok(())
    }

    fn initialized_metadata(self, h: program_header) -> Result<bool, image_error> {
        let files = range(h.file_offset, h.file_size)?;
        let virtual_bytes = range(h.virtual_address, h.file_size)?;
        for load in self.program_headers().filter(|h| h.kind == pt_load) {
            if range(load.file_offset, load.file_size)?.contains(files)
                && range(load.virtual_address, load.file_size)?.contains(virtual_bytes)
                && h.virtual_address - load.virtual_address == h.file_offset - load.file_offset
            {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn memory_metadata(self, h: program_header) -> Result<bool, image_error> {
        let bytes = range(h.virtual_address, h.memory_size)?;
        for load in self
            .program_headers()
            .filter(|h| h.kind == pt_load && h.flags & 2 != 0)
        {
            if range(load.virtual_address, load.memory_size)?.contains(bytes) {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn phdr_offset(self) -> Result<Option<u64>, image_error> {
        let mut address = None;
        for load in self.program_headers().filter(|h| h.kind == pt_load) {
            if range(load.file_offset, load.file_size)?.contains(self.table) {
                let derived = load
                    .virtual_address
                    .checked_add(self.table.start - load.file_offset)
                    .ok_or(image_error::program_header_address)?;
                if address.replace(derived).is_some() {
                    return Err(image_error::program_header_address);
                }
            }
        }
        Ok(address)
    }

    pub fn plan(self, load_bias: u64, role: image_role) -> Result<load_plan<'a>, image_error> {
        if (self.kind == elf_type::executable && load_bias != 0)
            || !load_bias.is_multiple_of(self.alignment)
        {
            return Err(image_error::load_bias);
        }
        for h in self.program_headers() {
            if !matches!(
                h.kind,
                pt_null
                    | pt_load
                    | pt_dynamic
                    | pt_interp
                    | pt_note
                    | pt_phdr
                    | pt_tls
                    | pt_gnu_eh_frame
                    | pt_gnu_stack
                    | pt_gnu_relro
                    | pt_gnu_property
            ) {
                return Err(image_error::unsupported_program_header(h.kind));
            }
            if h.kind == pt_load {
                let address = h
                    .virtual_address
                    .checked_add(load_bias)
                    .ok_or(image_error::virtual_range)?;
                let mapped = rounded(address, h.memory_size)?;
                if h.memory_size != 0 && (mapped.start < page_size || mapped.end > user_address_end)
                {
                    return Err(image_error::virtual_range);
                }
            }
            if matches!(
                h.kind,
                pt_tls | pt_dynamic | pt_phdr | pt_gnu_relro | pt_gnu_eh_frame | pt_gnu_property
            ) {
                let address = h
                    .virtual_address
                    .checked_add(load_bias)
                    .ok_or(image_error::virtual_range)?;
                let span = range(address, h.memory_size)?;
                if span.end > user_address_end || (!span.is_empty() && span.start < page_size) {
                    return Err(image_error::virtual_range);
                }
            }
        }
        let entry = if role == image_role::executable {
            let address = self
                .entry
                .checked_add(load_bias)
                .ok_or(image_error::entry)?;
            let mut executable = false;
            for h in self
                .program_headers()
                .filter(|h| h.kind == pt_load && h.flags & 1 != 0)
            {
                let span = range(h.virtual_address, h.file_size)?;
                executable |= self.entry >= span.start && self.entry < span.end;
            }
            if !executable {
                return Err(image_error::entry);
            }
            Some(address)
        } else {
            None
        };
        let phdr_address = self
            .phdr_offset()?
            .map(|v| {
                v.checked_add(load_bias)
                    .ok_or(image_error::program_header_address)
            })
            .transpose()?;
        Ok(load_plan {
            image: self,
            bias: load_bias,
            role,
            entry,
            phdr_address,
        })
    }
}

#[derive(Clone, Debug)]
pub struct program_headers<'a> {
    bytes: &'a [u8],
    index: usize,
}
impl Iterator for program_headers<'_> {
    type Item = program_header;
    fn next(&mut self) -> Option<Self::Item> {
        if self.index == self.bytes.len() {
            return None;
        }
        let bytes = self
            .bytes
            .get(self.index..self.index + program_header_size as usize)?;
        self.index += program_header_size as usize;
        Some(program_header {
            kind: u32_at(bytes, 0),
            flags: u32_at(bytes, 4),
            file_offset: u64_at(bytes, 8),
            virtual_address: u64_at(bytes, 16),
            physical_address: u64_at(bytes, 24),
            file_size: u64_at(bytes, 32),
            memory_size: u64_at(bytes, 40),
            alignment: u64_at(bytes, 48),
        })
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        let n = (self.bytes.len() - self.index) / program_header_size as usize;
        (n, Some(n))
    }
}
impl ExactSizeIterator for program_headers<'_> {}
impl core::iter::FusedIterator for program_headers<'_> {}

#[derive(Clone, Copy, Debug)]
pub struct load_plan<'a> {
    image: image<'a>,
    bias: u64,
    role: image_role,
    entry: Option<u64>,
    phdr_address: Option<u64>,
}
impl<'a> load_plan<'a> {
    pub const fn image(self) -> image<'a> {
        self.image
    }
    pub const fn source(self) -> &'a [u8] {
        self.image.source
    }
    pub const fn elf_type(self) -> elf_type {
        self.image.kind
    }
    pub const fn load_bias(self) -> u64 {
        self.bias
    }
    pub const fn role(self) -> image_role {
        self.role
    }
    pub const fn entry(self) -> Option<u64> {
        self.entry
    }
    /// None means the table is not initialized in a LOAD; never invent AT_PHDR.
    pub const fn phdr_address(self) -> Option<u64> {
        self.phdr_address
    }
    pub const fn program_header_count(self) -> usize {
        self.image.header_count
    }
    pub const fn mapped_pages(self) -> u64 {
        self.image.pages
    }
    pub fn program_headers(self) -> program_headers<'a> {
        self.image.program_headers()
    }
    pub fn interp(self) -> Option<&'a [u8]> {
        self.image.interp()
    }
    pub fn loads(self) -> load_segments<'a> {
        load_segments {
            headers: self.program_headers(),
            bias: self.bias,
        }
    }
    pub fn dynamic(self) -> Option<metadata<'a>> {
        self.metadata(pt_dynamic)
    }
    pub fn tls(self) -> Option<metadata<'a>> {
        self.metadata(pt_tls)
    }
    pub fn relro(self) -> Option<metadata<'a>> {
        self.metadata(pt_gnu_relro)
    }
    pub fn stack(self) -> Option<program_header> {
        self.image.header(pt_gnu_stack)
    }
    fn metadata(self, kind: u32) -> Option<metadata<'a>> {
        let header = self.image.header(kind)?;
        let start = header.virtual_address + self.bias;
        Some(metadata {
            header,
            bytes: &self.source()
                [header.file_offset as usize..(header.file_offset + header.file_size) as usize],
            memory: byte_range {
                start,
                end: start + header.memory_size,
            },
        })
    }
}

/// TLS memory size is a thread-local template size, not extra PT_LOAD storage.
/// RELRO is the exact requested span; actual protection belongs to the linker.
#[derive(Clone, Copy, Debug)]
pub struct metadata<'a> {
    header: program_header,
    bytes: &'a [u8],
    memory: byte_range,
}
impl<'a> metadata<'a> {
    pub const fn header(self) -> program_header {
        self.header
    }
    pub const fn bytes(self) -> &'a [u8] {
        self.bytes
    }
    pub const fn memory(self) -> byte_range {
        self.memory
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct load_segment {
    header: program_header,
    bias: u64,
}
impl load_segment {
    pub const fn header(self) -> program_header {
        self.header
    }
    pub const fn flags(self) -> u32 {
        self.header.flags
    }
    pub const fn readable(self) -> bool {
        self.header.flags & 4 != 0
    }
    pub const fn writable(self) -> bool {
        self.header.flags & 2 != 0
    }
    pub const fn executable(self) -> bool {
        self.header.flags & 1 != 0
    }
    pub fn file(self) -> byte_range {
        byte_range {
            start: self.header.file_offset,
            end: self.header.file_offset + self.header.file_size,
        }
    }
    pub fn memory(self) -> byte_range {
        let start = self.header.virtual_address + self.bias;
        byte_range {
            start,
            end: start + self.header.memory_size,
        }
    }
    pub fn initialized(self) -> byte_range {
        byte_range {
            start: self.memory().start,
            end: self.memory().start + self.header.file_size,
        }
    }
    pub fn zero_tail(self) -> byte_range {
        byte_range {
            start: self.initialized().end,
            end: self.memory().end,
        }
    }
    pub fn mapped(self) -> byte_range {
        let memory = self.memory();
        byte_range {
            start: memory.start & !(page_size - 1),
            end: if memory.is_empty() {
                memory.start & !(page_size - 1)
            } else {
                (memory.end + page_size - 1) & !(page_size - 1)
            },
        }
    }
    /// File copy includes the leading file-page prefix for a non-page-aligned
    /// segment. No bytes beyond p_filesz are exposed; remaining page slack is zero.
    pub fn copy_file(self) -> byte_range {
        if self.header.file_size == 0 {
            byte_range {
                start: self.header.file_offset,
                end: self.header.file_offset,
            }
        } else {
            byte_range {
                start: self.header.file_offset & !(page_size - 1),
                end: self.file().end,
            }
        }
    }
    pub fn copy_destination(self) -> byte_range {
        byte_range {
            start: self.mapped().start,
            end: if self.header.file_size == 0 {
                self.mapped().start
            } else {
                self.initialized().end
            },
        }
    }
    pub fn zero_fill(self) -> byte_range {
        byte_range {
            start: self.copy_destination().end,
            end: self.mapped().end,
        }
    }
}

#[derive(Clone, Debug)]
pub struct load_segments<'a> {
    headers: program_headers<'a>,
    bias: u64,
}
impl Iterator for load_segments<'_> {
    type Item = load_segment;
    fn next(&mut self) -> Option<Self::Item> {
        self.headers
            .by_ref()
            .find(|h| h.kind == pt_load)
            .map(|header| load_segment {
                header,
                bias: self.bias,
            })
    }
}
impl core::iter::FusedIterator for load_segments<'_> {}

fn singleton(kind: u32) -> bool {
    matches!(
        kind,
        pt_interp
            | pt_phdr
            | pt_dynamic
            | pt_tls
            | pt_gnu_stack
            | pt_gnu_relro
            | pt_gnu_eh_frame
            | pt_gnu_property
    )
}
fn range(start: u64, size: u64) -> Result<byte_range, image_error> {
    Ok(byte_range {
        start,
        end: start.checked_add(size).ok_or(image_error::virtual_range)?,
    })
}
fn rounded(start: u64, size: u64) -> Result<byte_range, image_error> {
    let exact = range(start, size)?;
    Ok(byte_range {
        start: start & !(page_size - 1),
        end: if size == 0 {
            start & !(page_size - 1)
        } else {
            exact
                .end
                .checked_add(page_size - 1)
                .ok_or(image_error::virtual_range)?
                & !(page_size - 1)
        },
    })
}
fn file_bytes(source: &[u8], range: byte_range) -> Result<&[u8], image_error> {
    if range.end > source.len() as u64 {
        return Err(image_error::file_range);
    }
    Ok(&source[range.start as usize..range.end as usize])
}
fn congruent(h: program_header, alignment: u64) -> Result<(), image_error> {
    if h.virtual_address % alignment != h.file_offset % alignment {
        Err(image_error::alignment)
    } else {
        Ok(())
    }
}
fn sections(source: &[u8]) -> Result<Option<section_table>, image_error> {
    let value = section_table {
        file_offset: u64_at(source, 40),
        entry_size: u16_at(source, 58),
        count: u16_at(source, 60),
        string_table_index: u16_at(source, 62),
    };
    if value.file_offset == 0 {
        if value.count != 0 || value.string_table_index != 0 || !matches!(value.entry_size, 0 | 64)
        {
            return Err(image_error::invalid_section_table);
        }
        return Ok(None);
    }
    if value.count == 0 || value.string_table_index == 0xffff {
        return Err(image_error::unsupported_extended_numbering);
    }
    if value.file_offset < 64 || value.entry_size != 64 || value.string_table_index >= value.count {
        return Err(image_error::invalid_section_table);
    }
    file_bytes(
        source,
        range(value.file_offset, u64::from(value.count) * 64)?,
    )?;
    Ok(Some(value))
}
// Each call is preceded by an exact fixed header/table extent check.
fn u16_at(bytes: &[u8], at: usize) -> u16 {
    u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap())
}
fn u32_at(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
}
fn u64_at(bytes: &[u8], at: usize) -> u64 {
    u64::from_le_bytes(bytes[at..at + 8].try_into().unwrap())
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use std::{vec, vec::Vec};

    const count: usize = 12;
    const bias: u64 = 0x400000;
    const budget: limits = limits {
        image_bytes: 1 << 20,
        program_headers: 32,
        load_segments: 8,
        mapped_pages: 256,
    };

    fn put16(bytes: &mut [u8], offset: usize, value: u16) {
        bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
    }
    fn put32(bytes: &mut [u8], offset: usize, value: u32) {
        bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    fn put64(bytes: &mut [u8], offset: usize, value: u64) {
        bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
    }
    fn set(bytes: &mut [u8], index: usize, h: program_header) {
        let at = 64 + index * 56;
        put32(bytes, at, h.kind);
        put32(bytes, at + 4, h.flags);
        for (offset, value) in [
            (8, h.file_offset),
            (16, h.virtual_address),
            (24, h.physical_address),
            (32, h.file_size),
            (40, h.memory_size),
            (48, h.alignment),
        ] {
            put64(bytes, at + offset, value);
        }
    }
    fn header(
        kind: u32,
        flags: u32,
        file: u64,
        address: u64,
        files: u64,
        memory: u64,
        alignment: u64,
    ) -> program_header {
        program_header {
            kind,
            flags,
            file_offset: file,
            virtual_address: address,
            physical_address: u64::MAX,
            file_size: files,
            memory_size: memory,
            alignment,
        }
    }
    fn fixture() -> Vec<u8> {
        let mut bytes = vec![0; 0x2680];
        bytes[..7].copy_from_slice(b"\x7fELF\x02\x01\x01");
        put16(&mut bytes, 16, 3);
        put16(&mut bytes, 18, 62);
        put32(&mut bytes, 20, 1);
        put64(&mut bytes, 24, 0x2040);
        put64(&mut bytes, 32, 64);
        put64(&mut bytes, 40, 0x2600);
        put16(&mut bytes, 52, 64);
        put16(&mut bytes, 54, 56);
        put16(&mut bytes, 56, count as u16);
        put16(&mut bytes, 58, 64);
        put16(&mut bytes, 60, 2);
        put16(&mut bytes, 62, 1);
        let headers = [
            header(pt_phdr, 4, 64, 64, count as u64 * 56, count as u64 * 56, 8),
            header(pt_interp, 4, 0x380, 0x380, 8, 8, 1),
            header(pt_load, 4, 0, 0, 0x400, 0x400, 4096),
            header(pt_load, 5, 0x1000, 0x2000, 0x200, 0x200, 4096),
            header(pt_load, 6, 0x2380, 0x5380, 0x180, 0x380, 4096),
            header(pt_dynamic, 6, 0x2400, 0x5400, 32, 32, 8),
            header(pt_tls, 4, 0x2380, 0x5380, 32, 96, 32),
            header(pt_gnu_relro, 4, 0x2380, 0x5380, 0x180, 0x280, 1),
            header(pt_gnu_stack, 6, 0, 0, 0, 0, 16),
            header(pt_gnu_eh_frame, 4, 0x300, 0x300, 16, 16, 4),
            header(pt_gnu_property, 4, 0x320, 0x320, 16, 16, 8),
            header(pt_note, 4, 0x340, 0x340, 16, 16, 4),
        ];
        for (i, h) in headers.into_iter().enumerate() {
            set(&mut bytes, i, h);
        }
        bytes[0x380..0x388].copy_from_slice(b"/ld.so\0\0");
        // The declared path includes exactly one terminating NUL.
        put64(&mut bytes, 64 + 56 + 32, 7);
        put64(&mut bytes, 64 + 56 + 40, 7);
        for (i, b) in bytes[0x2000..0x2500].iter_mut().enumerate() {
            *b = (i % 251) as u8;
        }
        bytes
    }
    fn parsed(bytes: &[u8]) -> image<'_> {
        image::parse(bytes, budget).unwrap()
    }
    fn planned(bytes: &[u8]) -> load_plan<'_> {
        parsed(bytes).plan(bias, image_role::executable).unwrap()
    }
    fn changed_header(bytes: &mut [u8], index: usize, edit: impl FnOnce(&mut program_header)) {
        let mut h = parsed(bytes).program_headers().nth(index).unwrap();
        edit(&mut h);
        set(bytes, index, h);
    }

    #[test]
    fn unchanged_source_and_non_aligned_load_have_exact_copy_zero_and_metadata_spans() {
        let bytes = fixture();
        let plan = planned(&bytes);
        assert_eq!(plan.source(), bytes);
        assert_eq!(plan.elf_type(), elf_type::shared);
        assert_eq!(plan.entry(), Some(bias + 0x2040));
        assert_eq!(plan.phdr_address(), Some(bias + 64));
        assert_eq!(plan.program_header_count(), count);
        assert_eq!(plan.mapped_pages(), 3);
        assert_eq!(plan.interp(), Some(&b"/ld.so"[..]));
        assert_eq!(plan.image().sections().unwrap().count, 2);
        let rw = plan.loads().nth(2).unwrap();
        assert_eq!(
            rw.file(),
            byte_range {
                start: 0x2380,
                end: 0x2500
            }
        );
        assert_eq!(
            rw.initialized(),
            byte_range {
                start: bias + 0x5380,
                end: bias + 0x5500
            }
        );
        assert_eq!(
            rw.zero_tail(),
            byte_range {
                start: bias + 0x5500,
                end: bias + 0x5700
            }
        );
        assert_eq!(
            rw.mapped(),
            byte_range {
                start: bias + 0x5000,
                end: bias + 0x6000
            }
        );
        assert_eq!(
            rw.copy_file(),
            byte_range {
                start: 0x2000,
                end: 0x2500
            }
        );
        assert_eq!(
            rw.copy_destination(),
            byte_range {
                start: bias + 0x5000,
                end: bias + 0x5500
            }
        );
        assert_eq!(
            rw.zero_fill(),
            byte_range {
                start: bias + 0x5500,
                end: bias + 0x6000
            }
        );
        let mut actual = [0xa5; 4096];
        actual[..rw.copy_file().len() as usize].copy_from_slice(
            &bytes[rw.copy_file().start() as usize..rw.copy_file().end() as usize],
        );
        actual[rw.copy_file().len() as usize..].fill(0);
        assert_eq!(&actual[..0x500], &bytes[0x2000..0x2500]);
        assert!(actual[0x500..].iter().all(|b| *b == 0));
        assert_eq!(
            plan.dynamic().unwrap().memory(),
            byte_range {
                start: bias + 0x5400,
                end: bias + 0x5420
            }
        );
        assert_eq!(plan.tls().unwrap().bytes().len(), 32);
        assert_eq!(plan.tls().unwrap().memory().len(), 96);
        assert_eq!(plan.relro().unwrap().memory().len(), 0x280);
        assert!(
            rw.writable(),
            "RELRO is protected by the linker after relocation"
        );
        assert_eq!(plan.stack().unwrap().flags, 6);
        assert_eq!(
            plan.program_headers().nth(2).unwrap().physical_address,
            u64::MAX
        );
    }

    #[test]
    fn every_resource_budget_and_shortened_source_is_checked() {
        let bytes = fixture();
        for (limits, error) in [
            (
                limits {
                    image_bytes: bytes.len() as u64 - 1,
                    ..budget
                },
                image_error::image_budget,
            ),
            (
                limits {
                    program_headers: count - 1,
                    ..budget
                },
                image_error::program_header_budget,
            ),
            (
                limits {
                    load_segments: 2,
                    ..budget
                },
                image_error::load_budget,
            ),
            (
                limits {
                    mapped_pages: 2,
                    ..budget
                },
                image_error::mapped_page_budget,
            ),
        ] {
            assert_eq!(image::parse(&bytes, limits).unwrap_err(), error);
        }
        for end in 0..bytes.len() {
            assert!(image::parse(&bytes[..end], budget).is_err(), "prefix {end}");
        }
    }

    #[test]
    fn headers_table_ranges_numbering_and_flags_fail_closed() {
        let original = fixture();
        for (offset, value) in [(4, 1), (5, 2), (6, 0), (7, 9), (8, 1), (15, 1)] {
            let mut bytes = original.clone();
            bytes[offset] = value;
            assert_eq!(
                image::parse(&bytes, budget).unwrap_err(),
                image_error::invalid_header
            );
        }
        for (offset, value) in [
            (16, 1),
            (18, 3),
            (52, 63),
            (54, 55),
            (56, 0),
            (58, 63),
            (62, 2),
        ] {
            let mut bytes = original.clone();
            put16(&mut bytes, offset, value);
            assert!(image::parse(&bytes, budget).is_err(), "field {offset}");
        }
        for offset in [32, 40] {
            let mut bytes = original.clone();
            put64(&mut bytes, offset, u64::MAX - 8);
            assert!(image::parse(&bytes, budget).is_err());
        }
        for (offset, value) in [(56, 0xffff), (60, 0), (62, 0xffff)] {
            let mut bytes = original.clone();
            put16(&mut bytes, offset, value);
            assert_eq!(
                image::parse(&bytes, budget).unwrap_err(),
                image_error::unsupported_extended_numbering
            );
        }
        let mut bytes = original;
        bytes[7] = 3;
        assert!(image::parse(&bytes, budget).is_ok());
    }

    #[test]
    fn load_overflow_alignment_permissions_order_and_page_aliases_reject() {
        let original = fixture();
        let mutations: [fn(&mut program_header); 9] = [
            |h| h.file_offset = u64::MAX,
            |h| h.virtual_address = u64::MAX - 4,
            |h| h.file_size = h.memory_size + 1,
            |h| h.alignment = 3,
            |h| h.virtual_address += 1,
            |h| h.flags = 7,
            |h| h.flags = 5 | 0x100,
            |h| h.flags = 2,
            |h| {
                h.virtual_address = 0;
                h.file_offset = 0;
            },
        ];
        for edit in mutations {
            let mut bytes = original.clone();
            changed_header(&mut bytes, 4, edit);
            assert!(image::parse(&bytes, budget).is_err());
        }
        for flags in [4, 5, 6] {
            let mut bytes = original.clone();
            changed_header(&mut bytes, 3, |h| {
                h.file_offset = 0x300;
                h.virtual_address = 0x300;
                h.file_size = 0x100;
                h.memory_size = 0x100;
                h.flags = flags;
            });
            assert_eq!(
                image::parse(&bytes, budget).unwrap_err(),
                image_error::overlapping_load_pages
            );
        }
        let mut bytes = original;
        changed_header(&mut bytes, 3, |h| {
            h.virtual_address = 0x6000;
        });
        assert_eq!(
            image::parse(&bytes, budget).unwrap_err(),
            image_error::load_order
        );
    }

    #[test]
    fn rebasing_preserves_load_alignment_and_low_user_geometry() {
        let bytes = fixture();
        let image = parsed(&bytes);
        assert_eq!(
            image.plan(0, image_role::executable).unwrap_err(),
            image_error::virtual_range
        );
        assert_eq!(
            image.plan(bias + 1, image_role::executable).unwrap_err(),
            image_error::load_bias
        );
        for base in [user_address_end - 4096, user_address_end, u64::MAX - 4095] {
            assert!(image.plan(base, image_role::executable).is_err());
        }
        let mut bytes = bytes;
        changed_header(&mut bytes, 2, |h| h.alignment = 2 * 1024 * 1024);
        let image = parsed(&bytes);
        assert_eq!(image.required_bias_alignment(), 2 * 1024 * 1024);
        assert!(image.plan(bias, image_role::executable).is_ok());
        assert_eq!(
            image.plan(bias + 4096, image_role::executable).unwrap_err(),
            image_error::load_bias
        );
    }

    #[test]
    fn executable_requires_initialized_entry_but_library_does_not_select_entry() {
        let original = fixture();
        for entry in [0, 0x5380, 0x5600, 0x2200, u64::MAX] {
            let mut bytes = original.clone();
            put64(&mut bytes, 24, entry);
            let image = parsed(&bytes);
            assert_eq!(
                image.plan(bias, image_role::executable).unwrap_err(),
                image_error::entry
            );
            assert_eq!(
                image.plan(bias, image_role::shared_object).unwrap().entry(),
                None
            );
        }
        let mut bytes = original;
        let headers: Vec<_> = parsed(&bytes).program_headers().collect();
        put16(&mut bytes, 16, 2);
        put64(&mut bytes, 24, bias + 0x2040);
        for (i, mut h) in headers.into_iter().enumerate() {
            if h.kind != pt_gnu_stack {
                h.virtual_address += bias;
            }
            set(&mut bytes, i, h);
        }
        let image = parsed(&bytes);
        assert_eq!(
            image.plan(0, image_role::executable).unwrap().entry(),
            Some(bias + 0x2040)
        );
        assert_eq!(
            image.plan(bias, image_role::executable).unwrap_err(),
            image_error::load_bias
        );
    }

    #[test]
    fn phdr_is_derived_even_without_header_but_never_from_bss_or_ambiguous_alias() {
        let mut bytes = fixture();
        changed_header(&mut bytes, 0, |h| h.kind = pt_null);
        assert_eq!(planned(&bytes).phdr_address(), Some(bias + 64));
        changed_header(&mut bytes, 9, |h| h.kind = pt_null);
        changed_header(&mut bytes, 10, |h| h.kind = pt_null);
        changed_header(&mut bytes, 2, |h| h.file_size = 64);
        assert_eq!(planned(&bytes).phdr_address(), None);
        let mut bytes = fixture();
        changed_header(&mut bytes, 0, |h| h.virtual_address += 8);
        assert!(image::parse(&bytes, budget).is_err());
        let mut bytes = fixture();
        changed_header(&mut bytes, 0, |h| h.file_size -= 1);
        assert_eq!(
            image::parse(&bytes, budget).unwrap_err(),
            image_error::program_header_address
        );
        let mut bytes = fixture();
        changed_header(&mut bytes, 2, |h| h.file_size = 0x2d0);
        assert_eq!(
            image::parse(&bytes, budget).unwrap_err(),
            image_error::metadata_not_loaded(pt_phdr)
        );
        let mut bytes = fixture();
        changed_header(&mut bytes, 3, |h| {
            h.file_offset = 0;
            h.file_size = 0x400;
            h.memory_size = 0x400;
        });
        assert_eq!(
            image::parse(&bytes, budget).unwrap_err(),
            image_error::program_header_address
        );
    }

    #[test]
    fn interpreter_is_borrowed_absolute_bytes_without_a_pathname_ceiling_or_normalization() {
        let original = fixture();
        for path in [
            b"relative\0".as_slice(),
            b"/a\0b\0",
            b"/unterminated",
            b"\0",
        ] {
            let mut bytes = original.clone();
            let at = bytes.len();
            bytes.extend_from_slice(path);
            changed_header(&mut bytes, 1, |h| {
                h.file_offset = at as u64;
                h.file_size = path.len() as u64;
                h.memory_size = h.file_size;
            });
            assert_eq!(
                image::parse(&bytes, budget).unwrap_err(),
                image_error::interpreter
            );
        }
        let mut bytes = original;
        let at = bytes.len();
        let mut path = b"/./../nonutf8-\xff//".to_vec();
        path.extend(core::iter::repeat_n(b'x', 8192));
        path.push(0);
        bytes.extend_from_slice(&path);
        changed_header(&mut bytes, 1, |h| {
            h.file_offset = at as u64;
            h.file_size = path.len() as u64;
            h.memory_size = h.file_size;
        });
        assert_eq!(parsed(&bytes).interp(), Some(&path[..path.len() - 1]));
    }

    #[test]
    fn singleton_and_loaded_metadata_cannot_forge_a_load_or_tls_template() {
        for kind in [
            pt_phdr,
            pt_interp,
            pt_dynamic,
            pt_tls,
            pt_gnu_relro,
            pt_gnu_stack,
            pt_gnu_eh_frame,
            pt_gnu_property,
        ] {
            let mut bytes = fixture();
            let h = parsed(&bytes).header(kind).unwrap();
            set(&mut bytes, 11, h);
            assert_eq!(
                image::parse(&bytes, budget).unwrap_err(),
                image_error::duplicate_header(kind)
            );
        }
        for index in [0, 5, 6, 7, 9, 10] {
            let mut bytes = fixture();
            changed_header(&mut bytes, index, |h| h.file_offset += 16);
            assert!(image::parse(&bytes, budget).is_err(), "metadata {index}");
        }
        let mut bytes = fixture();
        changed_header(&mut bytes, 7, |h| h.memory_size = 0x1000);
        assert_eq!(
            image::parse(&bytes, budget).unwrap_err(),
            image_error::metadata_not_loaded(pt_gnu_relro)
        );
        let mut bytes = fixture();
        changed_header(&mut bytes, 6, |h| h.memory_size = 31);
        assert!(image::parse(&bytes, budget).is_err());
        let mut bytes = fixture();
        changed_header(&mut bytes, 6, |h| {
            h.file_size = 0;
            h.memory_size = 0x5000;
        });
        let tls = planned(&bytes).tls().unwrap();
        assert!(tls.bytes().is_empty());
        assert_eq!(
            tls.memory().len(),
            0x5000,
            "TLS zero tail belongs to the thread template, not process BSS"
        );
        for index in [9, 10] {
            let mut bytes = fixture();
            changed_header(&mut bytes, index, |h| h.memory_size += 8);
            assert_eq!(
                image::parse(&bytes, budget).unwrap_err(),
                image_error::invalid_program_header
            );
        }
        let mut bytes = fixture();
        changed_header(&mut bytes, 8, |h| h.flags = 7);
        assert_eq!(
            image::parse(&bytes, budget).unwrap_err(),
            image_error::permissions
        );
    }

    #[test]
    fn unknown_semantics_are_visible_but_unplannable_and_null_fields_are_ignored() {
        for kind in [5, 8, 0x6000_0001, 0x7000_0000] {
            let mut bytes = fixture();
            changed_header(&mut bytes, 11, |h| h.kind = kind);
            let image = parsed(&bytes);
            assert_eq!(image.program_headers().nth(11).unwrap().kind, kind);
            assert_eq!(
                image.plan(bias, image_role::executable).unwrap_err(),
                image_error::unsupported_program_header(kind)
            );
        }
        let mut bytes = fixture();
        set(
            &mut bytes,
            11,
            header(pt_null, u32::MAX, u64::MAX, u64::MAX, u64::MAX, u64::MAX, 3),
        );
        let plan = planned(&bytes);
        assert_eq!(plan.image().header_bytes(11), None);
        assert_eq!(plan.image().header_bytes(usize::MAX), None);
        let mut headers = plan.program_headers();
        for _ in 0..count {
            assert!(headers.next().is_some());
        }
        assert_eq!(headers.len(), 0);
        assert_eq!(headers.next(), None);
        assert_eq!(headers.next(), None);
    }

    #[test]
    fn fileless_load_has_no_prefix_copy_and_entire_mapping_is_zero() {
        let mut bytes = fixture();
        for index in [5, 6, 7] {
            changed_header(&mut bytes, index, |h| h.kind = pt_null);
        }
        changed_header(&mut bytes, 4, |h| h.file_size = 0);
        let load = planned(&bytes).loads().nth(2).unwrap();
        assert!(load.copy_file().is_empty());
        assert!(load.copy_destination().is_empty());
        assert_eq!(load.zero_fill(), load.mapped());
        assert_eq!(load.zero_tail(), load.memory());
    }
}
