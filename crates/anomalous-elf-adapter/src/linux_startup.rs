//! Inert Linux x86-64 initial-stack templates, without allocation or authority.
//!
//! The caller supplies already validated main-image facts, truthful auxiliary
//! facts and random bytes, and a byte buffer corresponding exactly to its owned
//! stack range. This module checks encodability, not mappings, ownership, entropy,
//! credentials, CPU features, executable safety or Linux runtime compatibility.
//! In particular, `AT_ENTRY` describes the main program even when initial RIP
//! will enter an interpreter. Register setup and interpreter admission are
//! separate responsibilities. No return-address slot is put on this stack.
//!
//! Layout follows the x86-64 psABI's Initial Process Stack and Linux's
//! `create_elf_tables`: argc, argv/NULL, envp/NULL, auxiliary pairs/AT_NULL,
//! followed by copied strings and the caller's sixteen random bytes.
//! References:
//! <https://gitlab.com/x86-psABIs/x86-64-ABI/-/blob/master/x86-64-ABI/low-level-sys-info.tex>
//! <https://github.com/torvalds/linux/blob/master/fs/binfmt_elf.c>
//! <https://github.com/torvalds/linux/blob/master/include/uapi/linux/auxvec.h>

const user_limit: u64 = 0x0000_8000_0000_0000;
const core_auxiliary_count: usize = 8;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum startup_error {
    stack_range,
    buffer_length,
    noncanonical_address,
    image_facts,
    interior_nul,
    auxiliary_reserved,
    auxiliary_duplicate,
    auxiliary_kind,
    auxiliary_value,
    arithmetic_overflow,
    insufficient_space,
}

/// Raw main-image facts, not parser output or proof of executable admission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct image_facts {
    /// Main program's complete, mapped program-header table after relocation.
    pub program_headers: u64,
    pub program_header_entry_size: u16,
    pub program_header_count: u16,
    /// Main program's relocated entry, not the interpreter's entry point.
    pub entry: u64,
    /// Interpreter load bias; `None` encodes `AT_BASE=0`.
    pub interpreter_base: Option<u64>,
    /// Explicit native page size; this x86-64 profile requires 4096.
    pub page_size: u64,
}

/// Numeric low-canonical user extent. `end` is exclusive, not a pointer grant.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct stack_range {
    pub base: u64,
    pub end: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum auxiliary_value {
    scalar(u64),
    /// A preexisting guest mapping, whose lifetime the caller must retain.
    /// Platform strings and vDSO contents are not copied or checked here.
    guest_address(u64),
}
impl auxiliary_value {
    const fn word(self) -> u64 {
        match self {
            Self::scalar(value) | Self::guest_address(value) => value,
        }
    }
}

/// One explicit Linux auxiliary fact. Known pointer tags require
/// `guest_address`; known scalar tags require `scalar`. Unsupported tags fail
/// rather than permitting a pointer to masquerade as unchecked scalar data.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct auxiliary_entry {
    pub kind: u64,
    pub value: auxiliary_value,
}

#[derive(Clone, Copy, Debug)]
pub struct startup_context<'a> {
    pub image: image_facts,
    pub stack: stack_range,
    /// No fixed count or string-length ceiling; the output buffer is the budget.
    pub argv: &'a [&'a [u8]],
    pub envp: &'a [&'a [u8]],
    /// No credentials, clock rate, secure mode, capability bits or vDSO defaults
    /// are synthesized. The caller supplies any such truthful facts explicitly.
    pub auxiliary: &'a [auxiliary_entry],
    /// Exactly the supplied bytes are copied. This builder supplies no entropy.
    pub random: &'a [u8; 16],
    /// Copied independently of argv[0], with no path-length policy.
    pub execfn: &'a [u8],
}

/// Inert addresses into the supplied stack template, not live process authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct startup_layout {
    pub stack_pointer: u64,
    pub argc: u64,
    pub argv_address: u64,
    pub envp_address: u64,
    pub auxiliary_address: u64,
    pub random_address: u64,
    pub execfn_address: u64,
    /// Occupied suffix, including alignment padding. Bytes below it are untouched.
    pub used_bytes: usize,
}

struct prepared_layout {
    result: startup_layout,
    stack_offset: usize,
    strings_offset: usize,
    random_offset: usize,
}

/// Build a complete little-endian Linux x86-64 process stack in a caller-owned
/// buffer. All validation and checks for size and address arithmetic precede the
/// first write; every returned error leaves the entire buffer unchanged.
/// Successful construction zeros padding in the occupied suffix only.
///
/// Core auxiliary entries are `AT_PHDR`, `AT_PHENT`, `AT_PHNUM`, `AT_PAGESZ`,
/// `AT_BASE`, `AT_ENTRY`, `AT_RANDOM`, and `AT_EXECFN`. Explicit extra facts retain
/// caller order. A single `(AT_NULL, 0)` pair terminates the vector.
pub fn build(
    context: &startup_context<'_>,
    buffer: &mut [u8],
) -> Result<startup_layout, startup_error> {
    let prepared = preflight(context, buffer.len())?;
    let result = prepared.result;
    // From this point, every offset and complete write extent is within the
    // preflighted suffix. No fallible validation remains after mutation begins.
    buffer[prepared.stack_offset..].fill(0);
    let mut words = prepared.stack_offset;
    let mut strings = prepared.strings_offset;
    put_word(buffer, &mut words, result.argc);
    for value in context.argv {
        put_word(buffer, &mut words, context.stack.base + strings as u64);
        put_string(buffer, &mut strings, value);
    }
    put_word(buffer, &mut words, 0);
    for value in context.envp {
        put_word(buffer, &mut words, context.stack.base + strings as u64);
        put_string(buffer, &mut strings, value);
    }
    put_word(buffer, &mut words, 0);
    put_string(buffer, &mut strings, context.execfn);
    buffer[prepared.random_offset..].copy_from_slice(context.random);
    for (kind, value) in [
        (3, context.image.program_headers),
        (4, u64::from(context.image.program_header_entry_size)),
        (5, u64::from(context.image.program_header_count)),
        (6, context.image.page_size),
        (7, context.image.interpreter_base.unwrap_or(0)),
        (9, context.image.entry),
        (25, result.random_address),
        (31, result.execfn_address),
    ] {
        put_word(buffer, &mut words, kind);
        put_word(buffer, &mut words, value);
    }
    for entry in context.auxiliary {
        put_word(buffer, &mut words, entry.kind);
        put_word(buffer, &mut words, entry.value.word());
    }
    put_word(buffer, &mut words, 0);
    put_word(buffer, &mut words, 0);
    Ok(result)
}

fn preflight(
    context: &startup_context<'_>,
    buffer_length: usize,
) -> Result<prepared_layout, startup_error> {
    let stack = context.stack;
    if stack.base == 0 || stack.end <= stack.base {
        return Err(startup_error::stack_range);
    }
    if !user_address(stack.base) || stack.end > user_limit {
        return Err(startup_error::noncanonical_address);
    }
    let length = u64::try_from(buffer_length).map_err(|_| startup_error::arithmetic_overflow)?;
    if stack.end - stack.base != length {
        return Err(startup_error::buffer_length);
    }
    validate_image(context.image)?;
    let table_size = table_bytes(
        context.argv.len(),
        context.envp.len(),
        context.auxiliary.len(),
    )?;
    if table_size > buffer_length {
        return Err(startup_error::insufficient_space);
    }
    validate_auxiliary(context.auxiliary)?;
    let argv_bytes = string_list_bytes(context.argv)?;
    let envp_bytes = string_list_bytes(context.envp)?;
    let execfn_bytes = string_bytes(context.execfn)?;
    let string_size = checked_add(checked_add(argv_bytes, envp_bytes)?, execfn_bytes)?;
    let data_size = checked_add(string_size, 16)?;
    let strings_offset = buffer_length
        .checked_sub(data_size)
        .ok_or(startup_error::insufficient_space)?;
    let unaligned_offset = strings_offset
        .checked_sub(table_size)
        .ok_or(startup_error::insufficient_space)?;
    let unaligned_address = guest_address(stack.base, unaligned_offset)?;
    let stack_pointer = unaligned_address & !15;
    let stack_offset = usize::try_from(
        stack_pointer
            .checked_sub(stack.base)
            .ok_or(startup_error::insufficient_space)?,
    )
    .map_err(|_| startup_error::arithmetic_overflow)?;
    let argv_offset = checked_add(stack_offset, 8)?;
    let envp_offset = checked_add(argv_offset, pointer_bytes(context.argv.len())?)?;
    let auxiliary_offset = checked_add(envp_offset, pointer_bytes(context.envp.len())?)?;
    let execfn_offset = checked_add(strings_offset, checked_add(argv_bytes, envp_bytes)?)?;
    let random_offset = checked_add(execfn_offset, execfn_bytes)?;
    Ok(prepared_layout {
        result: startup_layout {
            stack_pointer,
            argc: u64::try_from(context.argv.len())
                .map_err(|_| startup_error::arithmetic_overflow)?,
            argv_address: guest_address(stack.base, argv_offset)?,
            envp_address: guest_address(stack.base, envp_offset)?,
            auxiliary_address: guest_address(stack.base, auxiliary_offset)?,
            random_address: guest_address(stack.base, random_offset)?,
            execfn_address: guest_address(stack.base, execfn_offset)?,
            used_bytes: buffer_length - stack_offset,
        },
        stack_offset,
        strings_offset,
        random_offset,
    })
}

fn user_address(value: u64) -> bool {
    value != 0 && value < user_limit
}

fn validate_image(image: image_facts) -> Result<(), startup_error> {
    if image.program_header_entry_size != 56
        || image.program_header_count == 0
        || image.page_size != 4096
    {
        return Err(startup_error::image_facts);
    }
    let header_end = image
        .program_headers
        .checked_add(u64::from(image.program_header_count) * 56)
        .ok_or(startup_error::arithmetic_overflow)?;
    if !user_address(image.program_headers)
        || !user_address(header_end - 1)
        || !user_address(image.entry)
        || image
            .interpreter_base
            .is_some_and(|base| base >= user_limit)
    {
        return Err(startup_error::noncanonical_address);
    }
    Ok(())
}

fn validate_auxiliary(entries: &[auxiliary_entry]) -> Result<(), startup_error> {
    for (index, entry) in entries.iter().enumerate() {
        if matches!(entry.kind, 0 | 3..=7 | 9 | 25 | 31) {
            return Err(startup_error::auxiliary_reserved);
        }
        if entries[..index]
            .iter()
            .any(|earlier| earlier.kind == entry.kind)
        {
            return Err(startup_error::auxiliary_duplicate);
        }
        match (entry.kind, entry.value) {
            // AT_PLATFORM, AT_BASE_PLATFORM, AT_SYSINFO, AT_SYSINFO_EHDR.
            (15 | 24 | 32 | 33, auxiliary_value::guest_address(value)) => {
                if !user_address(value) {
                    return Err(startup_error::noncanonical_address);
                }
            }
            (15 | 24 | 32 | 33, _) => return Err(startup_error::auxiliary_value),
            // Linux scalar facts, including cache geometry and signal size.
            (
                1 | 2 | 8 | 10..=14 | 16 | 17 | 23 | 26..=30 | 34..=37 | 40..=47 | 51,
                auxiliary_value::scalar(value),
            ) => {
                if entry.kind == 23 && value > 1 {
                    return Err(startup_error::auxiliary_value);
                }
            }
            (1 | 2 | 8 | 10..=14 | 16 | 17 | 23 | 26..=30 | 34..=37 | 40..=47 | 51, _) => {
                return Err(startup_error::auxiliary_value);
            }
            _ => return Err(startup_error::auxiliary_kind),
        }
    }
    Ok(())
}

fn checked_add(left: usize, right: usize) -> Result<usize, startup_error> {
    left.checked_add(right)
        .ok_or(startup_error::arithmetic_overflow)
}

fn pointer_bytes(count: usize) -> Result<usize, startup_error> {
    checked_add(count, 1)?
        .checked_mul(8)
        .ok_or(startup_error::arithmetic_overflow)
}

fn table_bytes(argc: usize, envc: usize, extra_auxiliary: usize) -> Result<usize, startup_error> {
    let pairs = checked_add(extra_auxiliary, core_auxiliary_count + 1)?;
    let auxiliary = pairs
        .checked_mul(16)
        .ok_or(startup_error::arithmetic_overflow)?;
    checked_add(
        checked_add(checked_add(8, pointer_bytes(argc)?)?, pointer_bytes(envc)?)?,
        auxiliary,
    )
}

fn string_bytes(value: &[u8]) -> Result<usize, startup_error> {
    if value.contains(&0) {
        return Err(startup_error::interior_nul);
    }
    checked_add(value.len(), 1)
}

fn string_list_bytes(values: &[&[u8]]) -> Result<usize, startup_error> {
    values
        .iter()
        .try_fold(0, |size, value| checked_add(size, string_bytes(value)?))
}

fn guest_address(base: u64, offset: usize) -> Result<u64, startup_error> {
    let offset = u64::try_from(offset).map_err(|_| startup_error::arithmetic_overflow)?;
    let address = base
        .checked_add(offset)
        .ok_or(startup_error::arithmetic_overflow)?;
    if user_address(address) {
        Ok(address)
    } else {
        Err(startup_error::noncanonical_address)
    }
}

fn put_word(buffer: &mut [u8], cursor: &mut usize, value: u64) {
    let end = *cursor + 8;
    buffer[*cursor..end].copy_from_slice(&value.to_le_bytes());
    *cursor = end;
}

fn put_string(buffer: &mut [u8], cursor: &mut usize, value: &[u8]) {
    let end = *cursor + value.len();
    buffer[*cursor..end].copy_from_slice(value);
    // The complete suffix, including each terminator, was already zeroed.
    *cursor = end + 1;
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use std::vec;

    const random_bytes: [u8; 16] = [
        0, 17, 34, 51, 68, 85, 102, 119, 136, 153, 170, 187, 204, 221, 238, 255,
    ];

    fn context<'a>(
        length: usize,
        argv: &'a [&'a [u8]],
        envp: &'a [&'a [u8]],
    ) -> startup_context<'a> {
        startup_context {
            image: image_facts {
                program_headers: 0x400040,
                program_header_entry_size: 56,
                program_header_count: 13,
                entry: 0x402310,
                interpreter_base: Some(0x7f00_0000_0000),
                page_size: 4096,
            },
            stack: stack_range {
                base: 0x7fff_ffff_0000,
                end: 0x7fff_ffff_0000 + length as u64,
            },
            argv,
            envp,
            auxiliary: &[],
            random: &random_bytes,
            execfn: b"/usr/bin/true",
        }
    }

    fn word(bytes: &[u8], base: u64, address: u64) -> u64 {
        let offset = usize::try_from(address - base).unwrap();
        u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap())
    }

    fn string(bytes: &[u8], base: u64, address: u64) -> &[u8] {
        let offset = usize::try_from(address - base).unwrap();
        let tail = &bytes[offset..];
        &tail[..tail.iter().position(|byte| *byte == 0).unwrap()]
    }

    fn unchanged(context: &startup_context<'_>, error: startup_error, length: usize) {
        let mut bytes = vec![0xa7; length];
        assert_eq!(build(context, &mut bytes), Err(error));
        assert!(bytes.iter().all(|byte| *byte == 0xa7));
    }

    #[test]
    fn exact_linux_words_pointers_strings_random_and_explicit_facts() {
        let argv: &[&[u8]] = &[b"alias", b"", b"--example"];
        let envp: &[&[u8]] = &[b"LANG=C", b"X=\xff"];
        let extra = [
            auxiliary_entry {
                kind: 11,
                value: auxiliary_value::scalar(1234),
            },
            auxiliary_entry {
                kind: 16,
                value: auxiliary_value::scalar(0x1234),
            },
            auxiliary_entry {
                kind: 23,
                value: auxiliary_value::scalar(0),
            },
            auxiliary_entry {
                kind: 33,
                value: auxiliary_value::guest_address(0x7000_0000),
            },
        ];
        let mut context = context(1024, argv, envp);
        context.auxiliary = &extra;
        let mut guarded = [0xa7; 1056];
        let result = build(&context, &mut guarded[16..1040]).unwrap();
        assert_eq!(guarded[..16], [0xa7; 16]);
        assert_eq!(guarded[1040..], [0xa7; 16]);
        let bytes = &guarded[16..1040];
        let base = context.stack.base;
        assert_eq!(result.stack_pointer % 16, 0);
        assert_eq!(word(bytes, base, result.stack_pointer), 3);
        assert_eq!(result.argc, 3);
        assert_eq!(result.argv_address, result.stack_pointer + 8);
        for (index, expected) in argv.iter().enumerate() {
            assert_eq!(
                string(
                    bytes,
                    base,
                    word(bytes, base, result.argv_address + index as u64 * 8)
                ),
                *expected
            );
        }
        assert_eq!(word(bytes, base, result.argv_address + 24), 0);
        assert_eq!(result.envp_address, result.argv_address + 32);
        for (index, expected) in envp.iter().enumerate() {
            assert_eq!(
                string(
                    bytes,
                    base,
                    word(bytes, base, result.envp_address + index as u64 * 8)
                ),
                *expected
            );
        }
        assert_eq!(word(bytes, base, result.envp_address + 16), 0);
        assert_eq!(result.auxiliary_address, result.envp_address + 24);
        let expected = [
            (3, 0x400040),
            (4, 56),
            (5, 13),
            (6, 4096),
            (7, 0x7f00_0000_0000),
            (9, 0x402310),
            (25, result.random_address),
            (31, result.execfn_address),
            (11, 1234),
            (16, 0x1234),
            (23, 0),
            (33, 0x7000_0000),
            (0, 0),
        ];
        for (index, (kind, value)) in expected.into_iter().enumerate() {
            let address = result.auxiliary_address + index as u64 * 16;
            assert_eq!(word(bytes, base, address), kind);
            assert_eq!(word(bytes, base, address + 8), value);
        }
        assert_eq!(string(bytes, base, result.execfn_address), b"/usr/bin/true");
        let offset = (result.random_address - base) as usize;
        assert_eq!(&bytes[offset..offset + 16], &random_bytes);
        assert!(
            bytes[..1024 - result.used_bytes]
                .iter()
                .all(|byte| *byte == 0xa7)
        );
    }

    #[test]
    fn all_stack_alignments_and_empty_vectors_have_no_return_slot_or_defaults() {
        for skew in 0..16 {
            let mut context = context(512, &[], &[]);
            context.stack.base += skew;
            context.stack.end += skew;
            context.image.interpreter_base = None;
            let mut bytes = [0xa7; 512];
            let result = build(&context, &mut bytes).unwrap();
            assert_eq!(result.stack_pointer % 16, 0);
            assert_eq!(word(&bytes, context.stack.base, result.stack_pointer), 0);
            assert_eq!(word(&bytes, context.stack.base, result.argv_address), 0);
            assert_eq!(word(&bytes, context.stack.base, result.envp_address), 0);
            // Eight core entries only; no invented UID/HWCAP/CLKTCK/etc.
            assert_eq!(
                word(
                    &bytes,
                    context.stack.base,
                    result.auxiliary_address + 4 * 16 + 8
                ),
                0
            );
            assert_eq!(
                word(
                    &bytes,
                    context.stack.base,
                    result.auxiliary_address + 8 * 16
                ),
                0
            );
            assert_eq!(
                word(
                    &bytes,
                    context.stack.base,
                    result.auxiliary_address + 8 * 16 + 8
                ),
                0
            );
        }
    }

    #[test]
    fn rejects_nul_in_every_string_class_without_writing() {
        let bad: &[&[u8]] = &[b"good", b"bad\0tail"];
        unchanged(&context(512, bad, &[]), startup_error::interior_nul, 512);
        unchanged(&context(512, &[], bad), startup_error::interior_nul, 512);
        let mut context = context(512, &[], &[]);
        context.execfn = b"/bad\0path";
        unchanged(&context, startup_error::interior_nul, 512);
    }

    #[test]
    fn rejects_duplicate_null_and_all_core_overrides_atomically() {
        for kind in [0, 3, 4, 5, 6, 7, 9, 25, 31] {
            let entries = [auxiliary_entry {
                kind,
                value: auxiliary_value::scalar(0),
            }];
            let mut context = context(512, &[], &[]);
            context.auxiliary = &entries;
            unchanged(&context, startup_error::auxiliary_reserved, 512);
        }
        let entries = [
            auxiliary_entry {
                kind: 11,
                value: auxiliary_value::scalar(1000),
            },
            auxiliary_entry {
                kind: 11,
                value: auxiliary_value::scalar(1001),
            },
        ];
        let mut context = context(512, &[], &[]);
        context.auxiliary = &entries;
        unchanged(&context, startup_error::auxiliary_duplicate, 512);
    }

    #[test]
    fn auxiliary_pointer_types_and_addresses_cannot_be_forged_as_scalar() {
        for kind in [15, 24, 32, 33] {
            for (value, error) in [
                (
                    auxiliary_value::scalar(0x1000),
                    startup_error::auxiliary_value,
                ),
                (
                    auxiliary_value::guest_address(0),
                    startup_error::noncanonical_address,
                ),
                (
                    auxiliary_value::guest_address(user_limit),
                    startup_error::noncanonical_address,
                ),
                (
                    auxiliary_value::guest_address(u64::MAX),
                    startup_error::noncanonical_address,
                ),
            ] {
                let entries = [auxiliary_entry { kind, value }];
                let mut context = context(512, &[], &[]);
                context.auxiliary = &entries;
                unchanged(&context, error, 512);
            }
        }
        for (kind, value, error) in [
            (
                11,
                auxiliary_value::guest_address(0x1000),
                startup_error::auxiliary_value,
            ),
            (
                23,
                auxiliary_value::scalar(2),
                startup_error::auxiliary_value,
            ),
            (
                u64::MAX,
                auxiliary_value::scalar(0),
                startup_error::auxiliary_kind,
            ),
        ] {
            let entries = [auxiliary_entry { kind, value }];
            let mut context = context(512, &[], &[]);
            context.auxiliary = &entries;
            unchanged(&context, error, 512);
        }
    }

    #[test]
    fn malformed_ranges_images_and_overflow_preserve_canaries() {
        let original = context(512, &[], &[]);
        for stack in [
            stack_range { base: 0, end: 512 },
            stack_range { base: 20, end: 20 },
            stack_range { base: 20, end: 19 },
        ] {
            let mut context = original;
            context.stack = stack;
            unchanged(&context, startup_error::stack_range, 512);
        }
        let mut context = original;
        context.stack.end += 1;
        unchanged(&context, startup_error::buffer_length, 512);
        context.stack = stack_range {
            base: user_limit - 256,
            end: user_limit + 256,
        };
        unchanged(&context, startup_error::noncanonical_address, 512);
        context = original;
        context.image.program_headers = u64::MAX - 1;
        unchanged(&context, startup_error::arithmetic_overflow, 512);
        for bad_address in [0, user_limit, u64::MAX] {
            context = original;
            context.image.entry = bad_address;
            unchanged(&context, startup_error::noncanonical_address, 512);
        }
        context = original;
        context.image.program_headers = user_limit - 100;
        unchanged(&context, startup_error::noncanonical_address, 512);
        context = original;
        context.image.interpreter_base = Some(user_limit);
        unchanged(&context, startup_error::noncanonical_address, 512);
        for (entry_size, count, page_size) in
            [(55, 1, 4096), (56, 0, 4096), (56, 1, 0), (56, 1, 8192)]
        {
            context = original;
            context.image.program_header_entry_size = entry_size;
            context.image.program_header_count = count;
            context.image.page_size = page_size;
            unchanged(&context, startup_error::image_facts, 512);
        }
        assert_eq!(
            table_bytes(usize::MAX, 0, 0),
            Err(startup_error::arithmetic_overflow)
        );
        assert_eq!(
            table_bytes(0, usize::MAX, 0),
            Err(startup_error::arithmetic_overflow)
        );
        assert_eq!(
            table_bytes(0, 0, usize::MAX),
            Err(startup_error::arithmetic_overflow)
        );
        assert_eq!(
            pointer_bytes(usize::MAX / 8),
            Err(startup_error::arithmetic_overflow)
        );
    }

    #[test]
    fn resource_budget_includes_alignment_and_never_partially_writes() {
        let argv: &[&[u8]] = &[b"/usr/bin/true"];
        let first = context(512, argv, &[]);
        let mut bytes = [0xa7; 512];
        let result = build(&first, &mut bytes).unwrap();
        let mut exact = first;
        exact.stack.base = result.stack_pointer;
        let mut storage = vec![0xa7; result.used_bytes];
        assert_eq!(
            build(&exact, &mut storage).unwrap().stack_pointer,
            result.stack_pointer
        );
        exact.stack.base += 1;
        unchanged(
            &exact,
            startup_error::insufficient_space,
            result.used_bytes - 1,
        );
        unchanged(
            &context(32, argv, &[]),
            startup_error::insufficient_space,
            32,
        );
    }

    #[test]
    fn exclusive_canonical_end_allows_last_header_and_stack_bytes() {
        let mut context = context(512, &[], &[]);
        context.stack = stack_range {
            base: user_limit - 512,
            end: user_limit,
        };
        context.image.program_headers = user_limit - 56;
        context.image.program_header_count = 1;
        context.image.entry = user_limit - 1;
        let mut bytes = [0xa7; 512];
        let result = build(&context, &mut bytes).unwrap();
        assert_eq!(result.random_address, user_limit - 16);
        assert_eq!(&bytes[496..], &random_bytes);
        assert_eq!(
            word(&bytes, context.stack.base, result.auxiliary_address + 8),
            user_limit - 56
        );
        context.image.program_headers += 1;
        unchanged(&context, startup_error::noncanonical_address, 512);
    }

    #[test]
    fn long_paths_and_arguments_are_limited_only_by_the_supplied_buffer() {
        let path = vec![b'x'; 16_385];
        let argv: &[&[u8]] = &[&path, b""];
        let mut context = context(40_000, argv, &[]);
        context.stack = stack_range {
            base: 0x7000_0000,
            end: 0x7000_0000 + 40_000,
        };
        context.execfn = &path;
        let mut bytes = vec![0xa7; 40_000];
        let result = build(&context, &mut bytes).unwrap();
        assert_eq!(
            string(&bytes, context.stack.base, result.execfn_address),
            path
        );
        assert_eq!(
            string(
                &bytes,
                context.stack.base,
                word(&bytes, context.stack.base, result.argv_address)
            ),
            path
        );
        let random_offset = (result.random_address - context.stack.base) as usize;
        assert_eq!(&bytes[random_offset..random_offset + 16], &random_bytes);
        context.stack.end = context.stack.base + 1024;
        unchanged(&context, startup_error::insufficient_space, 1024);
    }
}
