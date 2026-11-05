//! A native adapter for the portable file service. The trusted deployment
//! supplies the immutable libc source; this build declares a complete library
//! subtree. This remains a removable proof with explicit per-request budgets.

use super::*;
use anomalous_veil_protocol::file_service::{
    client_identity, descriptor_handle, file_error, file_metadata, file_service, file_snapshot,
    immutable_file_source, namespace, namespace_entry, publication, publication_result,
    source_error,
};
use anomalous_veil_protocol::stage_operation;
use anomalous_veil_protocol::{loader_mapping_policy, source_mapping, source_mapping_lifetime};

const context_operation: u64 = 24;
const context_size: usize = 80;
const request_budget: usize = 32;
const library_prefix: &[u8] = b"/lib/x86_64-linux-gnu";
const library_name: &[u8] = b"/lib/x86_64-linux-gnu/libc.so.6";
// This proof namespace's logical file identity is independent of the native
// source grant. These values and foreign ownership fields do not grant access.
const library_object: u64 = 1;
const library_version: u64 = 1;
const stat_size: usize = 144;

#[unsafe(no_mangle)]
#[unsafe(link_section = ".bss.file_service_report")]
static mut anomalous_linux_veil_file_service_report: [u8; 80] = [0; 80];

core::arch::global_asm!(
    ".section .text.linux_file_fault,\"ax\",@progbits",
    ".global anomalous_linux_file_stage_fault",
    ".type anomalous_linux_file_stage_fault,@function",
    "anomalous_linux_file_stage_fault:",
    "ud2",
    ".size anomalous_linux_file_stage_fault,.-anomalous_linux_file_stage_fault",
    ".global anomalous_linux_file_unknown_fault",
    ".type anomalous_linux_file_unknown_fault,@function",
    "anomalous_linux_file_unknown_fault:",
    "ud2",
    ".size anomalous_linux_file_unknown_fault,.-anomalous_linux_file_unknown_fault",
    options(att_syntax)
);

unsafe extern "C" {
    fn anomalous_linux_file_stage_fault() -> !;
    fn anomalous_linux_file_unknown_fault() -> !;
}

#[derive(Clone, Copy, Eq, PartialEq)]
struct caller_context {
    words: [u64; 10],
}

impl caller_context {
    fn query(sequence: u64) -> Self {
        let mut bytes = [0; context_size];
        let reply = call(
            context_operation,
            [
                sequence,
                bytes.as_mut_ptr() as u64,
                context_size as u64,
                0,
                0,
                0,
            ],
        );
        assert_eq!(
            (reply.status, reply.value, reply.auxiliary),
            (0, sequence, context_size as u64)
        );
        let mut words = [0; 10];
        for (word, bytes) in words.iter_mut().zip(bytes.chunks_exact(8)) {
            *word = u64::from_le_bytes(bytes.try_into().unwrap());
        }
        assert_eq!(words[0].to_le_bytes(), *b"ANFCTX01");
        assert_eq!(words[1], sequence);
        assert!(words[2..9].iter().all(|word| *word != 0));
        assert!(words[9] < sequence);
        Self { words }
    }

    fn owner(self) -> client_identity {
        client_identity::new(self.words[2], self.words[3]).expect("invalid authenticated owner")
    }

    fn identity(self) -> [u64; 7] {
        self.words[2..9].try_into().unwrap()
    }

    fn sequence(self) -> u64 {
        self.words[1]
    }
}

#[derive(Eq, PartialEq)]
struct native_source {
    binding: source_binding,
    identity: [u64; 7],
}

impl immutable_file_source for native_source {
    type read_context = caller_context;

    fn byte_length(&self) -> u64 {
        self.binding.length()
    }

    fn read_at(
        &self,
        context: &mut caller_context,
        offset: u64,
        destination: &mut [u8],
    ) -> Result<usize, source_error> {
        // The copied context is an authenticated observation, never authority
        // by itself. The source gate checks the sealed live caller again.
        let observed = caller_context::query(context.sequence());
        if observed != *context || observed.identity() != self.identity {
            return Err(source_error::unavailable);
        }
        let reply = call(
            source_read::source_read_operation,
            [
                context.sequence(),
                self.binding.grant(),
                offset,
                destination.as_mut_ptr() as u64,
                destination.len() as u64,
                0,
            ],
        );
        if (reply.status, reply.value, reply.auxiliary) == (0, destination.len() as u64, 0) {
            Ok(destination.len())
        } else {
            Err(source_error::failed)
        }
    }
}

fn write_report(words: [u64; 10]) {
    assert_eq!(
        ptr::addr_of!(anomalous_linux_veil_file_service_report) as u64,
        0x8010d0
    );
    for (index, word) in words.iter().enumerate() {
        for (byte, value) in word.to_le_bytes().iter().enumerate() {
            // SAFETY: Linker-bounded private BSS diagnostic. The kernel reads
            // this only after this task has entered a gate or fault handler.
            unsafe {
                ptr::write_volatile(
                    ptr::addr_of_mut!(anomalous_linux_veil_file_service_report)
                        .cast::<u8>()
                        .add(index * 8 + byte),
                    *value,
                )
            };
        }
    }
}

fn report(
    service: &file_service<'_, 2, native_source>,
    owner: client_identity,
    handle: Option<descriptor_handle>,
    mode: u64,
    active: u64,
    acknowledged: u64,
) -> [u64; 10] {
    [
        u64::from_le_bytes(*b"ANFSPR01"),
        mode,
        active,
        acknowledged,
        service.last_sequence(),
        service.open_count() as u64,
        handle.map_or(0, descriptor_handle::fd) as u64,
        handle.map_or(0, descriptor_handle::generation),
        handle
            .and_then(|handle| service.offset(owner, handle).ok())
            .unwrap_or(u64::MAX),
        u64::from(service.is_fenced()),
    ]
}

fn expected_fault(words: [u64; 10]) -> ! {
    write_report(words);
    // SAFETY: Distinct expected UD2 is accepted only in a matching kernel
    // scenario with independently retained stage, pending and cleanup state.
    assert!(
        (1..=7).contains(&words[1]),
        "invalid expected fault scenario"
    );
    // Scenarios1/3/5 stop after staging;2/4/6 fence an unknown file publication.
    // Scenario7 uses the second fault after an acknowledged mapping return.
    // A binary branch retains direct emitted fault transfers.
    if words[1] & 1 != 0 && words[1] != 7 {
        unsafe { anomalous_linux_file_stage_fault() }
    } else {
        unsafe { anomalous_linux_file_unknown_fault() }
    }
}

fn complete(sequence: u64, result: u64, token: u64, length: u64) -> publication_result {
    let reply = call(
        complete_operation,
        [
            sequence,
            result,
            completion_action::return_to_caller as u64,
            token,
            0,
            0,
        ],
    );
    // This exact gate is resumed only by a later sealed invocation. The local
    // file effect remains uncommitted for the entire scheduling interval.
    if (reply.status, reply.value, reply.auxiliary) == (0, sequence, length) {
        publication_result::committed
    } else if matches!(reply.status, 2..=4) && reply.value == 0 && reply.auxiliary == 0 {
        publication_result::rejected
    } else {
        publication_result::unknown
    }
}

fn capture_path(
    sequence: u64,
    pointer: u64,
    path: &mut [u8; loader_policy::pathname_read_budget],
) -> Result<usize, read_error> {
    for (index, output) in path.iter_mut().enumerate() {
        let address = pointer
            .checked_add(index as u64)
            .ok_or(read_error::inaccessible)?;
        *output = caller_byte(sequence, address)?;
        if *output == 0 {
            return Ok(index);
        }
    }
    Err(read_error::unsupported)
}

fn stage_probes(sequence: u64, destination: u64, source: u64, length: u64) {
    for (arguments, expected) in [
        ([0, 3, destination, length, source, 0], 4),
        ([sequence - 1, 3, destination, length, source, 0], 4),
        ([sequence, 0, destination, length, source, 0], 4),
        ([sequence, 3, destination, 0, source, 0], 4),
        ([sequence, 3, destination, 4097, source, 0], 4),
        ([sequence, 3, destination, length, source, 1], 4),
        ([sequence, 3, destination, length, u64::MAX, 0], 3),
        ([sequence, 3, u64::MAX, length, source, 0], 3),
    ] {
        let rejected = call(stage_operation, arguments);
        assert_eq!(
            (rejected.status, rejected.value, rejected.auxiliary),
            (expected, 0, 0)
        );
    }
}

fn completion_probes(sequence: u64, result: u64) {
    for arguments in [
        [sequence - 1, result, 0, sequence, 0, 0],
        [sequence, result, 0, 0, 0, 0],
        [sequence, result, 0, sequence, 1, 0],
        [sequence, result, 2, sequence, 0, 0],
    ] {
        let rejected = call(complete_operation, arguments);
        assert_eq!(
            (rejected.status, rejected.value, rejected.auxiliary),
            (4, 0, 0)
        );
    }
}

fn metadata_admission_probes(sequence: u64, source: u64) {
    // No delivery for this metadata request has been selected yet. An
    // inaccessible first destination and a completion without a stage must
    // remain inert, including the prior delivery's retained observations.
    let rejected = call(
        stage_operation,
        [sequence, 3, u64::MAX, stat_size as u64, source, 0],
    );
    assert_eq!(
        (rejected.status, rejected.value, rejected.auxiliary),
        (3, 0, 0)
    );
    let rejected = call(complete_operation, [sequence, 0, 0, sequence, 0, 0]);
    assert_eq!(
        (rejected.status, rejected.value, rejected.auxiliary),
        (4, 0, 0)
    );
}

struct file_delivery<'a> {
    sequence: u64,
    destination: u64,
    result: u64,
    stage_fault: bool,
    unknown_fault: bool,
    before: &'a [u64; 10],
    acknowledged: &'a mut u64,
}

impl file_delivery<'_> {
    fn publish(self, bytes: &[u8]) -> publication_result {
        if bytes.is_empty() {
            let reply = complete(self.sequence, self.result, 0, 0);
            if reply == publication_result::committed {
                *self.acknowledged = self.sequence;
            }
            return reply;
        }
        // Every delivery has its own journal, even when the syscall class
        // repeats. Qualify its rejected stages and completions independently.
        stage_probes(
            self.sequence,
            self.destination,
            bytes.as_ptr() as u64,
            bytes.len() as u64,
        );
        let staged = call(
            stage_operation,
            [
                self.sequence,
                3,
                self.destination,
                bytes.len() as u64,
                bytes.as_ptr() as u64,
                0,
            ],
        );
        if (staged.status, staged.value, staged.auxiliary) != (0, self.sequence, bytes.len() as u64)
        {
            return publication_result::unknown;
        }
        completion_probes(self.sequence, self.result);
        if self.stage_fault {
            expected_fault(*self.before);
        }
        let reply = complete(
            self.sequence,
            self.result,
            self.sequence,
            bytes.len() as u64,
        );
        if reply == publication_result::committed {
            *self.acknowledged = self.sequence;
            if self.unknown_fault {
                return publication_result::unknown;
            }
        }
        reply
    }
}

fn encode_stat(metadata: file_metadata, bytes: &mut [u8]) -> bool {
    if bytes.len() != stat_size || metadata.byte_length() > i64::MAX as u64 {
        return false;
    }
    // x86-64 Linux UAPI stat: explicit little-endian fields, including zero
    // timestamps, padding and reserved words. This is userspace proof policy,
    // not host metadata, a credential mapping or physical storage accounting.
    bytes.fill(0);
    for (offset, value) in [
        (0, 1u64),                                  // logical library volume
        (8, metadata.object()),                     // retained immutable file identity
        (16, 1),                                    // exactly one configured name
        (48, metadata.byte_length()),               // representable signed Linux size
        (56, 4096),                                 // preferred I/O size policy
        (64, metadata.byte_length().div_ceil(512)), // nonsparse logical blocks
    ] {
        bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
    }
    bytes[24..28].copy_from_slice(&0o100444u32.to_le_bytes());
    // uid/gid/rdev are the namespace's synthetic zero values. Linux stat has
    // no version field; the service still retains and publishes that identity.
    true
}

fn mapping_preparation_probes(request: source_mapping::source_mapping_request) {
    let arguments = request.arguments();
    for (index, value, status) in [
        (0, request.sequence() - 1, native_status::invalid),
        (1, request.grant() ^ 1, native_status::denied),
        (5, request.permissions() | 2, native_status::invalid),
        (2, request.base() + 1, native_status::invalid),
        (3, u64::MAX, native_status::invalid),
        (
            3,
            source_mapping::page_limit * source_mapping::page_size + 1,
            native_status::invalid,
        ),
        (
            4,
            request.populated_byte_length() ^ 1,
            native_status::invalid,
        ),
        (2, 0x6000_0000, native_status::invalid),
    ] {
        let mut invalid = arguments;
        invalid[index] = value;
        let reply = call(source_mapping::source_mapping_operation, invalid);
        assert_eq!(
            (reply.status, reply.value, reply.auxiliary),
            (status as u64, 0, 0)
        );
    }
    let prepared = call(source_mapping::source_mapping_operation, arguments);
    assert_eq!(
        (prepared.status, prepared.value, prepared.auxiliary),
        (
            native_status::ok as u64,
            request.data_pages(),
            request.virtual_pages()
        )
    );
    // The controller consumes this proof once, even though it cancels and
    // publishes nothing. A duplicate must preserve the first observation and
    // must not acquire another allocation. This remains a native result only.
    let replayed = call(source_mapping::source_mapping_operation, arguments);
    assert_eq!(
        (replayed.status, replayed.value, replayed.auxiliary),
        (native_status::invalid as u64, 0, 0)
    );
}

fn mapping_lifetime_reply(operation: u64, arguments: [u64; 6], expected: (u64, u64, u64)) {
    let reply = call(operation, arguments);
    assert_eq!((reply.status, reply.value, reply.auxiliary), expected);
}

fn mapping_completion_refusal(sequence: u64, result: u64) {
    mapping_lifetime_reply(
        complete_operation,
        [
            sequence,
            result,
            completion_action::return_to_caller as u64,
            0,
            0,
            0,
        ],
        (native_status::invalid as u64, 0, 0),
    );
}

fn mapping_lifetime_probes(
    request: source_mapping::source_mapping_request,
    context: &caller_context,
    binding: source_binding,
) -> source_mapping_lifetime::source_mapping_handle {
    use source_mapping_lifetime::{
        source_mapping_handle, source_mapping_inspect_operation, source_mapping_publish_operation,
        source_mapping_release_operation, source_mapping_retain_operation,
    };

    let sequence = request.sequence();
    assert_eq!(context.sequence(), sequence);
    assert_eq!(request.populated_byte_length(), binding.length());
    let invalid = (native_status::invalid as u64, 0, 0);
    let page_counts = (0, request.data_pages(), request.virtual_pages());
    // The exact Linux result is still premature while no retained owner,
    // publication or complete readback proof exists. Each refusal is inert.
    mapping_completion_refusal(sequence, request.base());
    let unallocated = source_mapping_handle::new(1, context.words[8])
        .expect("invalid mapping service incarnation");
    mapping_lifetime_reply(
        source_mapping_publish_operation,
        unallocated.arguments(sequence),
        invalid,
    );
    let mut stale = request.arguments();
    stale[0] = sequence - 1;
    mapping_lifetime_reply(source_mapping_retain_operation, stale, invalid);

    // These native replies leave the same foreign invocation suspended. The
    // real owner stays in the kernel across both intervening context queries.
    let retained = call(source_mapping_retain_operation, request.arguments());
    assert_eq!(retained.status, native_status::ok as u64);
    let handle = source_mapping_handle::new(retained.value, retained.auxiliary)
        .expect("invalid retained mapping handle");
    assert_eq!(handle.incarnation(), context.words[8]);
    assert!(caller_context::query(sequence) == *context);
    mapping_lifetime_reply(
        source_mapping_retain_operation,
        request.arguments(),
        invalid,
    );
    mapping_completion_refusal(sequence, request.base());

    let control = handle.arguments(sequence);
    let mut wrong_generation = control;
    wrong_generation[1] ^= 1;
    mapping_lifetime_reply(source_mapping_publish_operation, wrong_generation, invalid);
    mapping_lifetime_reply(source_mapping_publish_operation, control, page_counts);
    assert!(caller_context::query(sequence) == *context);
    mapping_completion_refusal(sequence, request.base());
    // A retirement request must not tear down a published owner before the
    // required inspection. Refuse it before any destructive terminal work.
    mapping_lifetime_reply(
        complete_operation,
        [
            sequence,
            (-38i64) as u64,
            completion_action::retire_caller as u64,
            0,
            0,
            0,
        ],
        invalid,
    );
    mapping_lifetime_reply(source_mapping_publish_operation, control, invalid);

    let mut wrong_incarnation = control;
    wrong_incarnation[2] ^= 1;
    mapping_lifetime_reply(source_mapping_inspect_operation, wrong_incarnation, invalid);
    mapping_lifetime_reply(
        source_mapping_inspect_operation,
        control,
        (0, binding.length(), binding.fnv64()),
    );
    mapping_lifetime_reply(source_mapping_release_operation, wrong_generation, invalid);
    mapping_completion_refusal(sequence, request.base() + source_mapping::page_size);
    mapping_completion_refusal(sequence - 1, request.base());
    // Leave the owner and published edge retained across the actual caller
    // activation. Terminal cleanup must withdraw and invalidate after use.
    handle
}

fn resumed_mapping_refusal_probes(
    mapping: source_mapping::source_mapping_request,
    handle: source_mapping_lifetime::source_mapping_handle,
    current_sequence: u64,
) {
    use source_mapping_lifetime::{
        source_mapping_inspect_operation, source_mapping_release_operation,
    };

    assert_eq!(current_sequence, mapping.sequence() + 1);
    // Reusing either the old sequence or the newly captured sequence cannot
    // grant control of an owner that has crossed a hardware execution interval.
    // The handle remains correlation data; no userspace call releases it here.
    for sequence in [mapping.sequence(), current_sequence] {
        for operation in [
            source_mapping_inspect_operation,
            source_mapping_release_operation,
        ] {
            mapping_lifetime_reply(
                operation,
                handle.arguments(sequence),
                (native_status::invalid as u64, 0, 0),
            );
        }
    }
    mapping_completion_refusal(mapping.sequence(), mapping.base());
}

pub(super) fn run(
    mut current: invocation,
    path: &mut [u8; loader_policy::pathname_read_budget],
    first_path_length: usize,
    mode: u64,
) -> ! {
    let initial = caller_context::query(current.sequence());
    assert_eq!(initial.words[9], 3);
    let owner = initial.owner();
    let source = native_source {
        binding: file_binding(),
        identity: initial.identity(),
    };
    let snapshot = file_snapshot::from_source(library_object, library_version, &source)
        .expect("invalid held file");
    let entries = [namespace_entry::new(library_name, snapshot).expect("invalid library entry")];
    let namespace =
        namespace::new(library_prefix, &entries).expect("invalid complete library scope");
    // This proof explicitly reserves integer0..2; it supplies no stdio service.
    let mut service = file_service::<2, native_source>::new(owner, initial.words[8], 3)
        .expect("invalid service identity");
    let mut handle = None;
    let mut acknowledged = initial.words[9];
    let mut context = initial;
    let mut first = true;
    for _ in 0..request_budget {
        assert_eq!(context.identity(), initial.identity());
        assert_eq!(context.words[9], acknowledged);
        let sequence = current.sequence();
        assert_eq!(context.sequence(), sequence);
        if sequence >= request_budget as u64 {
            write_report(report(
                &service,
                owner,
                handle,
                mode,
                sequence,
                acknowledged,
            ));
            retire(sequence);
        }
        let arguments = current.arguments();
        match current.number() {
            257 | 262
                if arguments[0] as u32 as i32 == -100
                    && if current.number() == 257 {
                        matches!(arguments[2] as u32, 0 | 0x80000)
                    } else {
                        arguments[3] as u32 == 0
                    } =>
            {
                let length = if first {
                    first_path_length
                } else {
                    match capture_path(sequence, arguments[1], path) {
                        Ok(length) => length,
                        Err(read_error::inaccessible) => {
                            return_result(sequence, (-14i64) as u64);
                            acknowledged = sequence;
                            current = request();
                            context = caller_context::query(current.sequence());
                            first = false;
                            continue;
                        }
                        Err(read_error::unsupported) => {
                            write_report(report(
                                &service,
                                owner,
                                handle,
                                mode,
                                sequence,
                                acknowledged,
                            ));
                            retire(sequence);
                        }
                    }
                };
                if current.number() == 262 {
                    if !loader_policy::metadata_path_is_absent(&namespace, &path[..length]) {
                        write_report(report(
                            &service,
                            owner,
                            handle,
                            mode,
                            sequence,
                            acknowledged,
                        ));
                        retire(sequence);
                    }
                    return_result(sequence, (-2i64) as u64);
                    acknowledged = sequence;
                } else {
                    let opened = service.open(
                        owner,
                        sequence,
                        &namespace,
                        &path[..length],
                        arguments[2] as u32 & 0x80000 != 0,
                        |event| {
                            if let publication::opened { descriptor } = event {
                                let outcome = complete(sequence, u64::from(descriptor), 0, 0);
                                if outcome == publication_result::committed {
                                    acknowledged = sequence;
                                }
                                outcome
                            } else {
                                publication_result::unknown
                            }
                        },
                    );
                    match opened {
                        Ok(opened) => handle = Some(opened),
                        Err(file_error::not_found) => {
                            return_result(sequence, (-2i64) as u64);
                            acknowledged = sequence;
                        }
                        Err(file_error::unsupported_path | file_error::out_of_scope) => {
                            write_report(report(
                                &service,
                                owner,
                                handle,
                                mode,
                                sequence,
                                acknowledged,
                            ));
                            retire(sequence);
                        }
                        Err(_) => panic!("file open publication failed"),
                    }
                }
            }
            0 | 17 if current.number() == 0 || arguments[3] <= i64::MAX as u64 => {
                let positional = current.number() == 17;
                let stage_fault = mode == if positional { 3 } else { 1 };
                let unknown_fault = mode == if positional { 4 } else { 2 };
                let selected = service
                    .handle_for_fd(owner, arguments[0] as u32)
                    .expect("read descriptor not opened");
                let before = report(
                    &service,
                    owner,
                    Some(selected),
                    mode,
                    sequence,
                    acknowledged,
                );
                let source_offset = if positional { arguments[3] } else { before[8] };
                fill_source_buffer(0);
                let outcome = {
                    // SAFETY: One native thread exclusively owns this BSS
                    // buffer. No other Rust reference exists; the provider's
                    // mutable borrow is passed to op23, then the callback only
                    // borrows immutable bytes until delayed acknowledgement.
                    let scratch = unsafe {
                        core::slice::from_raw_parts_mut(
                            source_buffer(),
                            source_read::source_chunk_limit,
                        )
                    };
                    let publish = |event: publication<'_>| {
                        let publication::read {
                            descriptor,
                            offset,
                            bytes,
                        } = event
                        else {
                            return publication_result::unknown;
                        };
                        if descriptor != selected.fd() || offset != source_offset {
                            return publication_result::unknown;
                        }
                        file_delivery {
                            sequence,
                            destination: arguments[1],
                            result: bytes.len() as u64,
                            stage_fault,
                            unknown_fault,
                            before: &before,
                            acknowledged: &mut acknowledged,
                        }
                        .publish(bytes)
                    };
                    if positional {
                        service.read_at_into(
                            owner,
                            sequence,
                            selected,
                            source_offset,
                            arguments[2],
                            source_read::source_chunk_limit,
                            &mut context,
                            scratch,
                            publish,
                        )
                    } else {
                        service.read_into(
                            owner,
                            sequence,
                            selected,
                            arguments[2],
                            source_read::source_chunk_limit,
                            &mut context,
                            scratch,
                            publish,
                        )
                    }
                };
                // The callback and scratch borrows have ended. Physical
                // zeroing here cannot alias a still-live publication slice.
                fill_source_buffer(0);
                check_source_buffer(0, 0);
                if unknown_fault && outcome == Err(file_error::publication_unknown) {
                    assert!(service.is_fenced() && service.open_count() == 0);
                    assert_eq!(service.last_sequence(), before[4]);
                    assert_eq!(acknowledged, sequence);
                    expected_fault(report(
                        &service,
                        owner,
                        Some(selected),
                        mode,
                        sequence,
                        acknowledged,
                    ));
                }
                outcome.expect("file read publication failed");
                if positional {
                    assert_eq!(service.offset(owner, selected), Ok(before[8]));
                    assert_eq!(service.last_sequence(), sequence);
                }
                handle = Some(selected);
            }
            5 => {
                let selected = service
                    .handle_for_fd(owner, arguments[0] as u32)
                    .expect("metadata descriptor not opened");
                let before = report(
                    &service,
                    owner,
                    Some(selected),
                    mode,
                    sequence,
                    acknowledged,
                );
                fill_source_buffer(0);
                let outcome = {
                    // SAFETY: The same exclusively owned 4 KiB BSS scratch is
                    // available after earlier callbacks end. This callback
                    // encodes only its first 144 bytes and retains that borrow
                    // through delayed completion; no source provider runs.
                    let scratch =
                        unsafe { core::slice::from_raw_parts_mut(source_buffer(), stat_size) };
                    service.metadata(owner, sequence, selected, |metadata| {
                        if metadata.object() != snapshot.object()
                            || metadata.version() != snapshot.version()
                            || metadata.byte_length() != snapshot.byte_length()
                        {
                            return publication_result::unknown;
                        }
                        if !encode_stat(metadata, scratch) {
                            return publication_result::rejected;
                        }
                        metadata_admission_probes(sequence, scratch.as_ptr() as u64);
                        file_delivery {
                            sequence,
                            destination: arguments[1],
                            result: 0,
                            stage_fault: mode == 5,
                            unknown_fault: mode == 6,
                            before: &before,
                            acknowledged: &mut acknowledged,
                        }
                        .publish(scratch)
                    })
                };
                // The callback no longer borrows encoded bytes, so volatile
                // clearing cannot alias a suspended publication slice.
                fill_source_buffer(0);
                check_source_buffer(0, 0);
                if mode == 6 && outcome == Err(file_error::publication_unknown) {
                    assert!(service.is_fenced() && service.open_count() == 0);
                    assert_eq!(service.last_sequence(), before[4]);
                    assert_eq!(acknowledged, sequence);
                    expected_fault(report(
                        &service,
                        owner,
                        Some(selected),
                        mode,
                        sequence,
                        acknowledged,
                    ));
                }
                outcome.expect("file metadata publication failed");
                assert_eq!(service.offset(owner, selected), Ok(before[8]));
                assert_eq!(service.last_sequence(), sequence);
                handle = Some(selected);
            }
            _ => {
                let before = report(&service, owner, handle, mode, sequence, acknowledged);
                write_report(before);
                if matches!(mode, 0 | 7)
                    && let Some(mapping) = handle.and_then(|selected| {
                        loader_mapping_policy::first_file_mapping(
                            current,
                            source.binding,
                            &service,
                            owner,
                            selected,
                            snapshot,
                        )
                    })
                {
                    mapping_preparation_probes(mapping);
                    assert!(caller_context::query(sequence) == context);
                    let mapping_handle = mapping_lifetime_probes(mapping, &context, source.binding);
                    assert!(service.file(owner, handle.unwrap()).ok() == Some(snapshot));
                    assert_eq!(
                        report(&service, owner, handle, mode, sequence, acknowledged),
                        before
                    );
                    assert_eq!(
                        complete(sequence, mapping.base(), 0, 0),
                        publication_result::committed
                    );
                    acknowledged = sequence;
                    current = request();
                    context = caller_context::query(current.sequence());
                    assert_eq!(context.identity(), initial.identity());
                    assert_eq!(context.words[9], acknowledged);
                    resumed_mapping_refusal_probes(mapping, mapping_handle, current.sequence());
                    // Capture this new boundary for the kernel's observation;
                    // do not execute any policy for its syscall or arguments.
                    assert!(service.file(owner, handle.unwrap()).ok() == Some(snapshot));
                    let after = report(
                        &service,
                        owner,
                        handle,
                        mode,
                        current.sequence(),
                        acknowledged,
                    );
                    assert_eq!(&after[4..], &before[4..]);
                    if mode == 7 {
                        expected_fault(after);
                    }
                    write_report(after);
                    retire(current.sequence());
                }
                retire(sequence);
            }
        }
        first = false;
        current = request();
        assert!(current.sequence() > sequence);
        context = caller_context::query(current.sequence());
    }
    write_report(report(
        &service,
        owner,
        handle,
        mode,
        current.sequence(),
        acknowledged,
    ));
    retire(current.sequence())
}
