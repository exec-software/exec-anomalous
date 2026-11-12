use crate::startup::user_end;

/// Launch data for a task that *carries a program it did not compile*.
///
/// The existing [`crate::startup_descriptor`] describes a first-party native
/// application: a fixed arena, a role, a scenario and a short document. That is
/// the right shape for a program whose behaviour is settled when it is built.
///
/// An interpreter is not that. The same image runs any edition, so what it will
/// do is decided by the edition it is handed, and the launch has to say which
/// edition, where its output goes, and — the part that matters for ADR 0093 —
/// **the most authority this task may ever grant to what it runs**.
///
/// A separate descriptor rather than a wider `startup_descriptor`: that one is
/// 128 bytes with invariants already in the field, and every validator of it
/// would have to change for the benefit of one new consumer. Nothing here
/// weakens it.
///
/// As with every descriptor in this crate, these values *describe* a launch.
/// They do not grant authority or map memory; the kernel's live binding for
/// each opaque handle remains authoritative.
pub const program_startup_size: usize = 256;

/// Arena bounds for a program-bearing task.
///
/// The fixed 32 KiB of a native application cannot hold an interpreter's
/// working memory — edition storage, values, text and a call stack — so this is
/// a range rather than a constant. It is still a range: a task that may ask for
/// any amount is a task whose footprint nobody has reasoned about.
pub const minimum_arena_bytes: u64 = 64 * 1024;
pub const maximum_arena_bytes: u64 = 64 * 1024 * 1024;

/// The largest edition this contract admits.
///
/// Sized from real editions rather than a round number: the largest built so
/// far is a scanner at roughly 469 KiB, and an order of magnitude above that
/// leaves room to grow without admitting an arbitrarily large one.
pub const maximum_edition_bytes: u64 = 16 * 1024 * 1024;

const argument_capacity: usize = 191;

/// Service families a program-bearing task may be granted.
///
/// This is the **ceiling**, not a request. ADR 0093 has the edition carry its
/// own declared surface, derived from its artifact; what actually runs is the
/// intersection of that and this. An interpreter admitted with only
/// `console_write` cannot be talked into reading a file by the edition it
/// loads, however that edition is written.
pub mod service {
    pub const console_write: u64 = 1 << 0;
    pub const file_read: u64 = 1 << 1;
    pub const clock_read: u64 = 1 << 2;

    /// Every family this contract knows. A bit outside it is refused rather
    /// than ignored, so a descriptor written against a newer vocabulary fails
    /// loudly on an older runtime instead of silently losing a grant.
    pub const known: u64 = console_write | file_read | clock_read;
}

/// A handle a launch actually granted, which cannot be written down any other
/// way.
///
/// Returning a bare `u64` would make the launch-time check advisory: code
/// inside the task could name any number it liked and the type system would
/// not object. On every mainstream system a console handle is an integer in a
/// table, and guessing, inheriting or reusing one is a well-worn way in.
///
/// So the number leaves this module only inside a value that nothing outside
/// can construct. Holding one *is* the proof that the launch granted it. The
/// kernel's live binding remains authoritative -- this cannot make a forged
/// handle work -- but it makes a forged handle something you cannot casually
/// express, and the grant travels with the handle rather than beside it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct granted_handle {
    value: u64,
    family: u64,
}

impl granted_handle {
    /// The raw value, for the one place that must speak to the kernel.
    ///
    /// Deliberately the only way out, and deliberately not `From`/`Into`: a
    /// conversion that reads as incidental is one that spreads.
    pub const fn raw(&self) -> u64 {
        self.value
    }

    /// Which family this handle was granted for, so a caller cannot pass a
    /// console handle to a file service and have it look well-typed.
    pub const fn family(&self) -> u64 {
        self.family
    }
}

/// The header before the argument block: eight words and eight reserved
/// bytes. With the length byte that follows, the argument capacity is exactly
/// what remains of the published size.
const header_size: usize = 64;
const arguments_offset: usize = header_size + 1;
const magic: [u8; 4] = *b"ANPG";
/// Format version, then the published size little-endian. A reader checks the
/// size it was compiled against rather than trusting a length it was handed.
const version: [u8; 4] = [1, 0, 0, 1];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum program_startup_error {
    /// The bytes are not a program launch: wrong size, magic, version, or a
    /// reserved or unused byte that is not zero.
    encoding,
    arena,
    edition,
    console,
    services,
    arguments,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct program_descriptor {
    arena_base: u64,
    arena_length: u64,
    edition_handle: u64,
    edition_length: u64,
    console_handle: u64,
    granted_services: u64,
    arguments: [u8; argument_capacity],
    arguments_length: usize,
}

impl program_descriptor {
    /// Arguments are NUL-separated printable ASCII, as a command line is.
    ///
    /// Printable-only for the same reason the native application's document is:
    /// a launch value reaches logs, traces and a person's screen, and a control
    /// byte in any of those is a problem nobody asked for. An empty argument
    /// list is ordinary — most programs take none.
    pub fn try_new(
        arena_base: u64,
        arena_length: u64,
        edition_handle: u64,
        edition_length: u64,
        console_handle: u64,
        granted_services: u64,
        arguments: &[u8],
    ) -> Result<Self, program_startup_error> {
        if arena_base == 0
            || !arena_base.is_multiple_of(4096)
            || !(minimum_arena_bytes..=maximum_arena_bytes).contains(&arena_length)
            || !arena_length.is_multiple_of(4096)
            || arena_base
                .checked_add(arena_length)
                .is_none_or(|end| end > user_end)
        {
            return Err(program_startup_error::arena);
        }
        if edition_handle == 0 || edition_length == 0 || edition_length > maximum_edition_bytes {
            return Err(program_startup_error::edition);
        }
        if granted_services & !service::known != 0 {
            return Err(program_startup_error::services);
        }
        // A handle and the grant that justifies it travel together. A console
        // handle without the grant is authority nobody authorised; the grant
        // without a handle is a promise that cannot be kept, and a task that
        // discovered the second at its first write would already be running.
        let writes_console = granted_services & service::console_write != 0;
        if writes_console == (console_handle == 0) {
            return Err(program_startup_error::console);
        }
        if console_handle != 0 && console_handle == edition_handle {
            return Err(program_startup_error::console);
        }
        if arguments.len() > argument_capacity
            || arguments
                .iter()
                .any(|byte| *byte != 0 && !(b' '..=b'~').contains(byte))
        {
            return Err(program_startup_error::arguments);
        }
        let mut stored = [0u8; argument_capacity];
        stored[..arguments.len()].copy_from_slice(arguments);
        Ok(Self {
            arena_base,
            arena_length,
            edition_handle,
            edition_length,
            console_handle,
            granted_services,
            arguments: stored,
            arguments_length: arguments.len(),
        })
    }

    pub const fn arena_base(&self) -> u64 {
        self.arena_base
    }
    pub const fn arena_length(&self) -> u64 {
        self.arena_length
    }
    pub const fn edition_handle(&self) -> u64 {
        self.edition_handle
    }
    pub const fn edition_length(&self) -> u64 {
        self.edition_length
    }
    /// The one retained edition, when this launch grants bounded file reads.
    /// This handle names no other file and conveys no namespace authority.
    /// The descriptor remains launch data; the kernel's live binding decides
    /// whether a request may actually read the retained source.
    pub const fn edition(&self) -> Option<granted_handle> {
        if self.granted_services & service::file_read != 0 {
            Some(granted_handle {
                value: self.edition_handle,
                family: service::file_read,
            })
        } else {
            None
        }
    }
    /// The console, when this launch granted one.
    ///
    /// `None` is not an error: a program that only computes is a legitimate
    /// launch, and asking it to carry a console it may not use would be the
    /// ambient authority this contract exists to avoid.
    pub const fn console(&self) -> Option<granted_handle> {
        if self.granted_services & service::console_write != 0 && self.console_handle != 0 {
            Some(granted_handle {
                value: self.console_handle,
                family: service::console_write,
            })
        } else {
            None
        }
    }
    pub const fn granted_services(&self) -> u64 {
        self.granted_services
    }
    pub fn arguments(&self) -> &[u8] {
        &self.arguments[..self.arguments_length]
    }

    /// Read a launch a kernel wrote.
    ///
    /// Every bound is re-checked by handing the decoded words to `try_new`:
    /// the wire form settles the layout and nothing else, so a descriptor that
    /// arrives over the boundary cannot be weaker than one built in process.
    /// `ANRT` is left alone -- a program-bearing launch is a different
    /// contract, not a wider version of the native application's, and sharing
    /// a magic would let a reader accept the wrong one.
    ///
    /// The reserved bytes must be zero. They are the room a later field will
    /// occupy, and admitting a non-zero reserved byte now is what makes adding
    /// that field a silent change in meaning rather than a refusal.
    pub fn decode(bytes: &[u8]) -> Result<Self, program_startup_error> {
        if bytes.len() != program_startup_size
            || bytes[..4] != magic
            || bytes[4..8] != version
            || bytes[56..header_size].iter().any(|byte| *byte != 0)
            || bytes[header_size] as usize > argument_capacity
        {
            return Err(program_startup_error::encoding);
        }
        let length = usize::from(bytes[header_size]);
        // The unused tail of the argument block is zero, so one launch cannot
        // carry bytes a reader never looks at.
        if bytes[arguments_offset + length..]
            .iter()
            .any(|byte| *byte != 0)
        {
            return Err(program_startup_error::encoding);
        }
        let word =
            |offset: usize| u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap());
        Self::try_new(
            word(8),
            word(16),
            word(24),
            word(32),
            word(40),
            word(48),
            &bytes[arguments_offset..arguments_offset + length],
        )
    }

    /// Write a launch for a task to read. Canonical: the same descriptor
    /// always produces the same bytes, so a launch can be compared.
    #[must_use]
    pub fn encode(&self) -> [u8; program_startup_size] {
        let mut bytes = [0; program_startup_size];
        bytes[..4].copy_from_slice(&magic);
        bytes[4..8].copy_from_slice(&version);
        for (offset, value) in [
            (8, self.arena_base),
            (16, self.arena_length),
            (24, self.edition_handle),
            (32, self.edition_length),
            (40, self.console_handle),
            (48, self.granted_services),
        ] {
            bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
        }
        bytes[header_size] = self.arguments_length as u8;
        bytes[arguments_offset..arguments_offset + self.arguments_length]
            .copy_from_slice(self.arguments());
        bytes
    }

    /// Whether this launch admits a family at all.
    ///
    /// The question an interpreter asks before honouring an edition's request,
    /// and the whole of ADR 0093's intersection on this side of the boundary.
    pub const fn grants(&self, family: u64) -> bool {
        family != 0 && family & !service::known == 0 && self.granted_services & family == family
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const base: u64 = 0x1000_0000;

    fn valid() -> program_descriptor {
        program_descriptor::try_new(
            base,
            minimum_arena_bytes,
            7,
            334,
            9,
            service::console_write,
            b"scanner-cli\0--quiet",
        )
        .expect("a launch inside every bound is accepted")
    }

    #[test]
    fn a_launch_inside_every_bound_is_accepted() {
        let descriptor = valid();
        assert_eq!(descriptor.arena_length(), minimum_arena_bytes);
        assert_eq!(descriptor.edition_length(), 334);
        assert_eq!(descriptor.arguments(), b"scanner-cli\0--quiet");
    }

    #[test]
    fn the_arena_is_a_bounded_range_not_a_fixed_size() {
        // Unlike a native application's fixed 32 KiB, an interpreter sizes its
        // own working memory -- within limits that have been reasoned about.
        for length in [minimum_arena_bytes, 1 << 20, maximum_arena_bytes] {
            assert!(
                program_descriptor::try_new(base, length, 7, 334, 9, service::console_write, b"")
                    .is_ok(),
                "{length} is inside the range"
            );
        }
        for length in [
            0,
            4096,
            minimum_arena_bytes - 4096,
            maximum_arena_bytes + 4096,
        ] {
            assert_eq!(
                program_descriptor::try_new(base, length, 7, 334, 9, service::console_write, b""),
                Err(program_startup_error::arena),
                "{length} is outside it"
            );
        }
        // Still page-aligned at both ends.
        assert_eq!(
            program_descriptor::try_new(
                base + 1,
                minimum_arena_bytes,
                7,
                334,
                9,
                service::console_write,
                b""
            ),
            Err(program_startup_error::arena)
        );
        assert_eq!(
            program_descriptor::try_new(
                base,
                minimum_arena_bytes + 1,
                7,
                334,
                9,
                service::console_write,
                b""
            ),
            Err(program_startup_error::arena)
        );
    }

    #[test]
    fn an_edition_must_exist_and_be_bounded() {
        for (handle, length) in [(0, 334), (7, 0), (7, maximum_edition_bytes + 1)] {
            assert_eq!(
                program_descriptor::try_new(
                    base,
                    minimum_arena_bytes,
                    handle,
                    length,
                    9,
                    service::console_write,
                    b""
                ),
                Err(program_startup_error::edition)
            );
        }
    }

    #[test]
    fn a_handle_and_the_grant_that_justifies_it_travel_together() {
        // A console handle with no grant is authority nobody authorised.
        assert_eq!(
            program_descriptor::try_new(base, minimum_arena_bytes, 7, 334, 9, 0, b""),
            Err(program_startup_error::console)
        );
        // A grant with no handle is a promise that cannot be kept, and the task
        // would only discover it at its first write, already running.
        assert_eq!(
            program_descriptor::try_new(
                base,
                minimum_arena_bytes,
                7,
                334,
                0,
                service::console_write,
                b""
            ),
            Err(program_startup_error::console)
        );
        // Granting nothing at all is a legitimate launch: a program that only
        // computes needs no services.
        assert!(program_descriptor::try_new(base, minimum_arena_bytes, 7, 334, 0, 0, b"").is_ok());
    }

    #[test]
    fn the_edition_and_the_console_are_not_the_same_handle() {
        assert_eq!(
            program_descriptor::try_new(
                base,
                minimum_arena_bytes,
                7,
                334,
                7,
                service::console_write,
                b""
            ),
            Err(program_startup_error::console)
        );
    }

    #[test]
    fn an_unknown_service_bit_is_refused_rather_than_ignored() {
        // A descriptor written against a newer vocabulary must fail loudly on
        // an older runtime, not quietly lose the grant it was counting on.
        assert_eq!(
            program_descriptor::try_new(
                base,
                minimum_arena_bytes,
                7,
                334,
                9,
                service::known | 1 << 63,
                b""
            ),
            Err(program_startup_error::services)
        );
    }

    #[test]
    fn a_granted_handle_is_the_only_way_to_name_one() {
        let descriptor = valid();
        let console = descriptor.console().expect("console_write was granted");
        assert_eq!(console.raw(), 9);
        assert_eq!(console.family(), service::console_write);

        // Granting nothing yields nothing to call with, rather than a number
        // the task could use anyway.
        let bare = program_descriptor::try_new(base, minimum_arena_bytes, 7, 334, 0, 0, b"")
            .expect("a program that only computes is a legitimate launch");
        assert!(bare.console().is_none());
    }

    #[test]
    fn the_ceiling_is_what_an_edition_is_measured_against() {
        let descriptor = program_descriptor::try_new(
            base,
            minimum_arena_bytes,
            7,
            334,
            9,
            service::console_write | service::clock_read,
            b"",
        )
        .expect("two families granted");
        assert!(descriptor.grants(service::console_write));
        assert!(descriptor.grants(service::clock_read));
        // The one not granted stays refused however the edition asks.
        assert!(!descriptor.grants(service::file_read));
        // And asking for more than was granted is refused as a whole, rather
        // than silently succeeding for the part that was.
        assert!(!descriptor.grants(service::console_write | service::file_read));
        assert!(!descriptor.grants(0));
        assert!(!descriptor.grants(1 << 63));
    }

    #[test]
    fn arguments_are_bounded_printable_text() {
        assert_eq!(
            program_descriptor::try_new(
                base,
                minimum_arena_bytes,
                7,
                334,
                9,
                service::console_write,
                &[b'a'; argument_capacity + 1]
            ),
            Err(program_startup_error::arguments)
        );
        // A NUL separates arguments; anything else outside printable ASCII is
        // refused, because a launch value reaches logs and screens.
        assert_eq!(
            program_descriptor::try_new(
                base,
                minimum_arena_bytes,
                7,
                334,
                9,
                service::console_write,
                b"ok\x07bell"
            ),
            Err(program_startup_error::arguments)
        );
        assert!(
            program_descriptor::try_new(
                base,
                minimum_arena_bytes,
                7,
                334,
                9,
                service::console_write,
                b""
            )
            .is_ok()
        );
    }

    #[test]
    fn a_launch_round_trips_through_its_wire_form() {
        let descriptor = valid();
        let bytes = descriptor.encode();
        assert_eq!(bytes.len(), program_startup_size);
        assert_eq!(&bytes[..4], b"ANPG", "a program launch is not an ANRT one");
        let read = program_descriptor::decode(&bytes).expect("canonical bytes decode");
        assert_eq!(read, descriptor);
        assert_eq!(read.encode(), bytes, "encoding is canonical");
        // The grant survives the boundary, which is the whole point: a ceiling
        // that cannot be delivered is not a ceiling.
        assert!(read.grants(service::console_write));
        assert!(read.console().is_some());
        assert_eq!(read.arguments(), b"scanner-cli\0--quiet");
    }

    #[test]
    fn a_launch_with_no_console_round_trips_as_one() {
        let quiet = program_descriptor::try_new(base, minimum_arena_bytes, 7, 334, 0, 0, b"")
            .expect("a program that only computes is a legitimate launch");
        let read = program_descriptor::decode(&quiet.encode()).expect("decodes");
        assert_eq!(read, quiet);
        assert!(read.console().is_none());
        assert!(read.arguments().is_empty());
    }

    #[test]
    fn every_truncation_and_reserved_encoding_is_rejected() {
        let bytes = valid().encode();
        // A short read is not a launch. Nothing shorter may be accepted by
        // reading only the part that arrived.
        for length in 0..program_startup_size {
            assert_eq!(
                program_descriptor::decode(&bytes[..length]),
                Err(program_startup_error::encoding),
                "a {length}-byte launch must be refused"
            );
        }
        let mut longer = [0u8; program_startup_size + 1];
        longer[..program_startup_size].copy_from_slice(&bytes);
        assert_eq!(
            program_descriptor::decode(&longer),
            Err(program_startup_error::encoding)
        );

        for offset in 0..8 {
            let mut wrong = bytes;
            wrong[offset] ^= 0xff;
            assert_eq!(
                program_descriptor::decode(&wrong),
                Err(program_startup_error::encoding),
                "a changed magic or version byte at {offset} must be refused"
            );
        }

        // Reserved bytes are the room a later field will take. Admitting a
        // non-zero one now would make adding that field a silent change in
        // meaning instead of a refusal.
        for offset in 56..64 {
            let mut used = bytes;
            used[offset] = 1;
            assert_eq!(
                program_descriptor::decode(&used),
                Err(program_startup_error::encoding),
                "a reserved byte in use at {offset} must be refused"
            );
        }

        // The unused tail of the argument block carries nothing, so one launch
        // cannot smuggle bytes a reader never looks at.
        let mut smuggled = bytes;
        smuggled[program_startup_size - 1] = b'x';
        assert_eq!(
            program_descriptor::decode(&smuggled),
            Err(program_startup_error::encoding)
        );

        let mut overlong = bytes;
        overlong[64] = argument_capacity as u8 + 1;
        assert_eq!(
            program_descriptor::decode(&overlong),
            Err(program_startup_error::encoding)
        );
    }

    #[test]
    fn the_wire_form_cannot_state_what_try_new_refuses() {
        // decode delegates every bound to try_new, so a descriptor that
        // arrives over the boundary is exactly as checked as one built here.
        let mut unpaired = valid().encode();
        unpaired[40..48].copy_from_slice(&0u64.to_le_bytes());
        assert_eq!(
            program_descriptor::decode(&unpaired),
            Err(program_startup_error::console),
            "a console grant without its handle must not survive decoding"
        );

        let mut unknown = valid().encode();
        unknown[48..56].copy_from_slice(&(service::known + 1).to_le_bytes());
        assert_eq!(
            program_descriptor::decode(&unknown),
            Err(program_startup_error::services),
            "a newer vocabulary must fail loudly on an older runtime"
        );

        let mut unaligned = valid().encode();
        unaligned[8..16].copy_from_slice(&(base + 1).to_le_bytes());
        assert_eq!(
            program_descriptor::decode(&unaligned),
            Err(program_startup_error::arena)
        );

        let mut aliased = valid().encode();
        aliased[40..48].copy_from_slice(&7u64.to_le_bytes());
        assert_eq!(
            program_descriptor::decode(&aliased),
            Err(program_startup_error::console),
            "the edition and the console must stay separate handles"
        );

        let mut controlled = valid().encode();
        controlled[arguments_offset] = 0x1b;
        assert_eq!(
            program_descriptor::decode(&controlled),
            Err(program_startup_error::arguments)
        );
    }

    #[test]
    fn the_argument_block_is_exactly_what_the_size_leaves() {
        assert_eq!(arguments_offset + argument_capacity, program_startup_size);
        let full = [b'a'; argument_capacity];
        let descriptor = program_descriptor::try_new(
            base,
            minimum_arena_bytes,
            7,
            334,
            9,
            service::console_write,
            &full,
        )
        .expect("a full argument block is admissible");
        let read = program_descriptor::decode(&descriptor.encode()).expect("decodes");
        assert_eq!(read.arguments(), &full[..]);
    }

    #[test]
    fn the_descriptor_fits_the_size_this_contract_publishes() {
        assert!(core::mem::size_of::<program_descriptor>() <= program_startup_size);
    }

    #[test]
    fn edition_metadata_requires_an_explicit_file_read_grant() {
        let ungranted = valid();
        assert_eq!(ungranted.edition_handle(), 7);
        assert_eq!(ungranted.edition(), None);
        let granted = program_descriptor::try_new(
            base,
            minimum_arena_bytes,
            7,
            4097,
            9,
            service::console_write | service::file_read,
            b"",
        )
        .unwrap();
        let decoded = program_descriptor::decode(&granted.encode()).unwrap();
        let edition = decoded.edition().unwrap();
        assert_eq!(edition.raw(), 7);
        assert_eq!(edition.family(), service::file_read);
        assert_ne!(edition, decoded.console().unwrap());
        assert_eq!(granted.encode().len(), program_startup_size);
    }
}
