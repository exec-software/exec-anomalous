//! Host fixture for bounded immutable providers and acknowledgement publication.
//! Context checks below are trusted test bindings, never native authentication.

use super::*;
use anomalous_veil_protocol::file_service::{immutable_file_source, source_error};

const chunk_limit: usize = 4096;
const operation_limit: usize = read_trace_limit + 64;

pub(super) struct proof_result {
    report: String,
    owner_a: Vec<u8>,
    owner_b: Vec<u8>,
    backend: String,
    operations: String,
}

impl proof_result {
    pub(super) fn files(&self) -> [(&'static str, &[u8]); 5] {
        [
            ("source-owner-a.bin", &self.owner_a),
            ("source-owner-b.bin", &self.owner_b),
            ("source-backend.jsonl", self.backend.as_bytes()),
            ("source-operations.jsonl", self.operations.as_bytes()),
            ("source-report.json", self.report.as_bytes()),
        ]
    }
}

#[derive(Debug, Eq, PartialEq)]
struct held_source<'a> {
    binding: u64,
    bytes: &'a [u8],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct context_binding {
    owner: client_identity,
    service: u64,
    sequence: u64,
    source: u64,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum behavior {
    exact,
    dirty_failure,
    short,
    overreported,
}

#[derive(Clone, Copy)]
enum context_fault {
    none,
    owner,
    service,
    sequence,
    source,
}

struct read_context {
    expected: context_binding,
    supplied: context_binding,
    behavior: behavior,
    operation: usize,
    case: &'static str,
    backend: Vec<String>,
}

impl immutable_file_source for held_source<'_> {
    type read_context = read_context;

    fn byte_length(&self) -> u64 {
        self.bytes.len() as u64
    }

    fn read_at(
        &self,
        context: &mut read_context,
        offset: u64,
        destination: &mut [u8],
    ) -> Result<usize, source_error> {
        assert!(context.backend.len() < operation_limit);
        assert!(destination.iter().all(|byte| *byte == 0));
        let result =
            if context.supplied != context.expected || context.expected.source != self.binding {
                Err(source_error::unavailable)
            } else if let Some(bytes) = usize::try_from(offset).ok().and_then(|start| {
                start
                    .checked_add(destination.len())
                    .and_then(|end| self.bytes.get(start..end))
            }) {
                match context.behavior {
                    behavior::dirty_failure => {
                        let prefix = destination.len().min(3);
                        destination[..prefix].fill(0xa5);
                        Err(source_error::failed)
                    }
                    behavior::short => {
                        destination.copy_from_slice(bytes);
                        Ok(destination.len() - 1)
                    }
                    behavior::overreported => {
                        destination.copy_from_slice(bytes);
                        Ok(destination.len() + 1)
                    }
                    behavior::exact => {
                        destination.copy_from_slice(bytes);
                        Ok(destination.len())
                    }
                }
            } else {
                Err(source_error::invalid_extent)
            };
        let (delivered, error) = match result {
            Ok(length) => (length.to_string(), "none"),
            Err(source_error::unavailable) => ("null".into(), "unavailable"),
            Err(source_error::changed) => ("null".into(), "changed"),
            Err(source_error::invalid_extent) => ("null".into(), "invalid_extent"),
            Err(source_error::failed) => ("null".into(), "failed"),
        };
        let actual = context.supplied;
        let expected = context.expected;
        context.backend.push(format!(
            "{{\"index\":{},\"operation_index\":{},\"case\":{},\"source_binding\":{},\"source_length\":{},\"offset\":{},\"requested\":{},\"delivered\":{},\"error\":{},\"write_hex\":{},\"context_task\":{},\"context_generation\":{},\"context_service\":{},\"context_sequence\":{},\"context_source\":{},\"expected_task\":{},\"expected_generation\":{},\"expected_service\":{},\"expected_sequence\":{},\"expected_source\":{}}}",
            context.backend.len(), context.operation, quoted(context.case), self.binding,
            self.bytes.len(), offset, destination.len(), delivered, quoted(error),
            quoted(&hex(destination)), actual.owner.task(), actual.owner.generation(),
            actual.service, actual.sequence, actual.source, expected.owner.task(),
            expected.owner.generation(), expected.service, expected.sequence, expected.source
        ));
        result
    }
}

struct trace {
    rows: Vec<String>,
    context: read_context,
}

#[derive(Clone, Copy)]
struct service_owner {
    label: &'static str,
    identity: client_identity,
    incarnation: u64,
}

type service<'a> = file_service<'a, 2, held_source<'a>>;

fn optional(value: Option<u64>) -> String {
    value.map_or_else(|| "null".into(), |number| number.to_string())
}

fn error_name(error: file_error) -> &'static str {
    match error {
        file_error::source_failure(source_error::failed) => "source_failed",
        file_error::source_failure(source_error::unavailable) => "source_unavailable",
        file_error::source_length_mismatch => "source_length_mismatch",
        file_error::publication_rejected => "publication_rejected",
        file_error::publication_unknown => "publication_unknown",
        file_error::fenced => "fenced",
        _ => "unexpected",
    }
}

#[allow(clippy::too_many_arguments)]
fn read(
    service: &mut service<'_>,
    owner: service_owner,
    handle: descriptor_handle,
    sequence: u64,
    count: u64,
    budget: usize,
    case: &'static str,
    behavior: behavior,
    fault: context_fault,
    outcome: publication_result,
    expected_error: Option<file_error>,
    delivered: &mut Vec<u8>,
    trace: &mut trace,
) -> Result<usize, String> {
    require(
        trace.rows.len() < operation_limit,
        "source operation budget exhausted",
    )?;
    let before = service.offset(owner.identity, handle).ok();
    let previous_sequence = service.last_sequence();
    let previous_count = service.open_count();
    let file = service.file(owner.identity, handle).ok();
    let expected_length = file.map_or(0, |file| {
        (file.byte_length() - before.unwrap())
            .min(count)
            .min(budget as u64) as usize
    });
    let source_binding = file.map_or(0, |file| file.source().binding);
    let source_version = file.map_or(0, |file| file.version());
    let binding = context_binding {
        owner: owner.identity,
        service: owner.incarnation,
        sequence,
        source: source_binding,
    };
    trace.context.expected = binding;
    trace.context.supplied = binding;
    match fault {
        context_fault::none => (),
        context_fault::owner => {
            trace.context.supplied.owner = core(
                client_identity::new(owner.identity.task(), owner.identity.generation() + 1),
                "wrong source context owner",
            )?;
        }
        context_fault::service => trace.context.supplied.service += 1,
        context_fault::sequence => trace.context.supplied.sequence -= 1,
        context_fault::source => trace.context.supplied.source += 1,
    }
    trace.context.operation = trace.rows.len();
    trace.context.case = case;
    trace.context.behavior = behavior;
    let backend_first = trace.context.backend.len();
    let mut scratch = [0xa5; chunk_limit];
    let mut publications = 0;
    let mut publication_bytes = String::new();
    let mut publication_matches = false;
    let result = service.read_into(
        owner.identity,
        sequence,
        handle,
        count,
        budget,
        &mut trace.context,
        &mut scratch,
        |value| {
            publications += 1;
            if let publication::read {
                descriptor,
                offset,
                bytes,
            } = value
            {
                publication_bytes = hex(bytes);
                if let Some(file) = file {
                    let start = before.unwrap() as usize;
                    publication_matches = descriptor == handle.fd()
                        && offset == before.unwrap()
                        && bytes == &file.source().bytes[start..start + expected_length];
                }
                if publication_matches {
                    if outcome == publication_result::committed {
                        delivered.extend_from_slice(bytes);
                    }
                    return outcome;
                }
            }
            publication_result::rejected
        },
    );
    require(
        scratch == [0; chunk_limit],
        "bounded read retained scratch bytes",
    )?;
    let backend_count = trace.context.backend.len() - backend_first;
    let result_name;
    let returned_length;
    if let Some(expected) = expected_error {
        expect_error(result, expected)?;
        result_name = error_name(expected);
        returned_length = None;
        if matches!(expected, file_error::publication_unknown) {
            require(
                service.is_fenced()
                    && service.open_count() == 0
                    && service.last_sequence() == previous_sequence,
                "unknown read was reused or committed local freshness",
            )?;
        } else {
            require(
                service.offset(owner.identity, handle).ok() == before
                    && service.last_sequence() == previous_sequence
                    && service.open_count() == previous_count,
                "rejected bounded read changed committed state",
            )?;
        }
    } else {
        require(
            core(result, "bounded read")? == expected_length,
            "wrong read length",
        )?;
        require(
            service.offset(owner.identity, handle).ok()
                == before.map(|v| v + expected_length as u64)
                && service.last_sequence() == sequence
                && service.open_count() == previous_count,
            "bounded read committed wrong offset or sequence",
        )?;
        result_name = "ok";
        returned_length = Some(expected_length as u64);
    }
    let expects_publication = expected_error.is_none()
        || matches!(
            expected_error,
            Some(file_error::publication_rejected | file_error::publication_unknown)
        );
    require(
        publications == usize::from(expects_publication)
            && (!expects_publication || publication_matches),
        "source failure published bytes or publication did not match retained source",
    )?;
    require(
        backend_count
            == usize::from(expected_length != 0 && expected_error != Some(file_error::fenced)),
        "zero/EOF validation or bounded read invoked wrong backend count",
    )?;
    let outcome_name = if publications == 0 {
        "none"
    } else {
        match outcome {
            publication_result::committed => "committed",
            publication_result::rejected => "rejected",
            publication_result::unknown => "unknown",
        }
    };
    trace.rows.push(format!(
        "{{\"index\":{},\"case\":{},\"kind\":\"read\",\"owner\":{},\"task\":{},\"owner_generation\":{},\"service_incarnation\":{},\"sequence\":{},\"fd\":{},\"handle_generation\":{},\"source_binding\":{},\"source_version\":{},\"offset_before\":{},\"offset_after\":{},\"last_sequence_before\":{},\"last_sequence_after\":{},\"requested\":{},\"budget\":{},\"scratch_length\":{},\"backend_first\":{},\"backend_count\":{},\"publication_count\":{},\"publication_outcome\":{},\"publication_bytes_hex\":{},\"result\":{},\"returned_length\":{},\"open_count_before\":{},\"open_count_after\":{},\"fenced\":{},\"scratch_cleared\":true}}",
        trace.rows.len(), quoted(case), quoted(owner.label), owner.identity.task(),
        owner.identity.generation(), owner.incarnation, sequence, handle.fd(), handle.generation(),
        source_binding, source_version, optional(before), optional(service.offset(owner.identity, handle).ok()),
        previous_sequence, optional((!service.is_fenced()).then_some(service.last_sequence())),
        count, budget, scratch.len(), backend_first, backend_count, publications, quoted(outcome_name),
        quoted(&publication_bytes), quoted(result_name), optional(returned_length), previous_count,
        service.open_count(), service.is_fenced()
    ));
    Ok(expected_length)
}

#[allow(clippy::too_many_arguments)]
fn control<'a>(
    service: &mut service<'a>,
    owner: service_owner,
    scope: &namespace<'a, held_source<'a>>,
    sequence: u64,
    closing: Option<descriptor_handle>,
    case: &'static str,
    trace: &mut trace,
) -> Result<descriptor_handle, String> {
    require(
        trace.rows.len() < operation_limit,
        "source operation budget exhausted",
    )?;
    let previous_sequence = service.last_sequence();
    let previous_count = service.open_count();
    let mut publications = 0;
    let mut actual_fd = None;
    let handle = if let Some(handle) = closing {
        core(
            service.close(owner.identity, sequence, handle, |event| {
                publications += 1;
                if let publication::closed { descriptor } = event {
                    actual_fd = Some(descriptor);
                    if descriptor == handle.fd() {
                        return publication_result::committed;
                    }
                }
                publication_result::rejected
            }),
            "bounded close",
        )?;
        handle
    } else {
        core(
            service.open(owner.identity, sequence, scope, libc_path, true, |event| {
                publications += 1;
                if let publication::opened { descriptor } = event {
                    actual_fd = Some(descriptor);
                    return publication_result::committed;
                }
                publication_result::rejected
            }),
            "bounded open",
        )?
    };
    require(
        publications == 1 && actual_fd == Some(handle.fd()),
        "bad control publication",
    )?;
    require(
        service.last_sequence() == sequence,
        "control sequence did not commit",
    )?;
    let file = if closing.is_none() {
        Some(core(service.file(owner.identity, handle), "opened file")?)
    } else {
        None
    };
    trace.rows.push(format!(
        "{{\"index\":{},\"case\":{},\"kind\":{},\"owner\":{},\"task\":{},\"owner_generation\":{},\"service_incarnation\":{},\"sequence\":{},\"fd\":{},\"handle_generation\":{},\"source_binding\":{},\"source_version\":{},\"offset_before\":null,\"offset_after\":{},\"last_sequence_before\":{},\"last_sequence_after\":{},\"requested\":0,\"budget\":0,\"scratch_length\":0,\"backend_first\":{},\"backend_count\":0,\"publication_count\":1,\"publication_outcome\":\"committed\",\"publication_bytes_hex\":\"\",\"result\":\"ok\",\"returned_length\":null,\"open_count_before\":{},\"open_count_after\":{},\"fenced\":false,\"scratch_cleared\":true}}",
        trace.rows.len(), quoted(case), quoted(if closing.is_some() { "close" } else { "open" }),
        quoted(owner.label), owner.identity.task(), owner.identity.generation(), owner.incarnation,
        sequence, handle.fd(), handle.generation(), file.map_or(0, |f| f.source().binding),
        file.map_or(0, |f| f.version()), optional(closing.is_none().then_some(0)),
        previous_sequence, sequence, trace.context.backend.len(), previous_count, service.open_count()
    ));
    Ok(handle)
}

pub(super) fn prove(input: &[u8], digest: &str) -> Result<proof_result, String> {
    require(
        !input.is_empty() && input.len() <= input_limit,
        "source proof input bound",
    )?;
    let owners = [
        service_owner {
            label: "a",
            identity: core(client_identity::new(71, 1), "source owner A")?,
            incarnation: 401,
        },
        service_owner {
            label: "b",
            identity: core(client_identity::new(71, 2), "source owner B")?,
            incarnation: 402,
        },
        service_owner {
            label: "uncertain",
            identity: core(client_identity::new(71, 1), "source uncertain owner")?,
            incarnation: 403,
        },
    ];
    let source = held_source {
        binding: 1,
        bytes: input,
    };
    let replacement = held_source {
        binding: 2,
        bytes: b"replacement bytes\0\xff",
    };
    let entries = [core(
        namespace_entry::new(
            libc_path,
            core(
                file_snapshot::from_source(file_object, 1, &source),
                "source file",
            )?,
        ),
        "source entry",
    )?];
    let replacements = [core(
        namespace_entry::new(
            libc_path,
            core(
                file_snapshot::from_source(file_object, 2, &replacement),
                "replacement file",
            )?,
        ),
        "replacement entry",
    )?];
    let scope = core(
        namespace::new(namespace_prefix, &entries),
        "source namespace",
    )?;
    let replaced = core(
        namespace::new(namespace_prefix, &replacements),
        "replacement namespace",
    )?;
    let binding = context_binding {
        owner: owners[0].identity,
        service: 401,
        sequence: 1,
        source: 1,
    };
    let mut trace = trace {
        rows: Vec::new(),
        context: read_context {
            expected: binding,
            supplied: binding,
            behavior: behavior::exact,
            operation: 0,
            case: "initial",
            backend: Vec::new(),
        },
    };
    let mut a = core(service::new(owners[0].identity, 401, 3), "source service A")?;
    let mut b = core(service::new(owners[1].identity, 402, 3), "source service B")?;
    let mut uncertain = core(
        service::new(owners[2].identity, 403, 3),
        "source uncertain service",
    )?;
    let handle_a = control(&mut a, owners[0], &scope, 1, None, "open_a", &mut trace)?;
    let handle_b = control(&mut b, owners[1], &scope, 1, None, "open_b", &mut trace)?;
    let replaced_handle = control(
        &mut a,
        owners[0],
        &replaced,
        2,
        None,
        "replacement_open",
        &mut trace,
    )?;
    let mut probe = Vec::new();
    read(
        &mut a,
        owners[0],
        replaced_handle,
        3,
        requested_count,
        chunk_limit,
        "replacement_read",
        behavior::exact,
        context_fault::none,
        publication_result::committed,
        None,
        &mut probe,
        &mut trace,
    )?;
    require(
        probe == replacement.bytes,
        "replacement namespace read wrong bytes",
    )?;
    require(
        core(a.file(owners[0].identity, handle_a), "retained snapshot")?.source() == &source,
        "replacement changed old descriptor snapshot",
    )?;
    control(
        &mut a,
        owners[0],
        &replaced,
        4,
        Some(replaced_handle),
        "replacement_close",
        &mut trace,
    )?;
    let mut first = Vec::new();
    let mut second = Vec::new();
    read(
        &mut a,
        owners[0],
        handle_a,
        5,
        0,
        0,
        "zero_count",
        behavior::exact,
        context_fault::none,
        publication_result::committed,
        None,
        &mut first,
        &mut trace,
    )?;
    for (case, mode, fault, error) in [
        (
            "dirty_prefix_failure",
            behavior::dirty_failure,
            context_fault::none,
            file_error::source_failure(source_error::failed),
        ),
        (
            "short_source",
            behavior::short,
            context_fault::none,
            file_error::source_length_mismatch,
        ),
        (
            "overreported_source",
            behavior::overreported,
            context_fault::none,
            file_error::source_length_mismatch,
        ),
        (
            "wrong_context_owner",
            behavior::exact,
            context_fault::owner,
            file_error::source_failure(source_error::unavailable),
        ),
        (
            "wrong_context_service",
            behavior::exact,
            context_fault::service,
            file_error::source_failure(source_error::unavailable),
        ),
        (
            "stale_context_sequence",
            behavior::exact,
            context_fault::sequence,
            file_error::source_failure(source_error::unavailable),
        ),
        (
            "wrong_context_source",
            behavior::exact,
            context_fault::source,
            file_error::source_failure(source_error::unavailable),
        ),
    ] {
        read(
            &mut a,
            owners[0],
            handle_a,
            6,
            17,
            chunk_limit,
            case,
            mode,
            fault,
            publication_result::committed,
            Some(error),
            &mut first,
            &mut trace,
        )?;
    }
    read(
        &mut a,
        owners[0],
        handle_a,
        6,
        17,
        chunk_limit,
        "rejected_publication",
        behavior::exact,
        context_fault::none,
        publication_result::rejected,
        Some(file_error::publication_rejected),
        &mut first,
        &mut trace,
    )?;
    read(
        &mut a,
        owners[0],
        handle_a,
        6,
        17,
        chunk_limit,
        "exact_rejected_retry",
        behavior::exact,
        context_fault::none,
        publication_result::committed,
        None,
        &mut first,
        &mut trace,
    )?;
    require(
        core(b.offset(owners[1].identity, handle_b), "independent offset")? == 0,
        "A moved B offset",
    )?;
    let mut next_a = 7;
    while core(a.offset(owners[0].identity, handle_a), "source A position")? < input.len() as u64 {
        read(
            &mut a,
            owners[0],
            handle_a,
            next_a,
            requested_count,
            chunk_limit,
            "stream_a",
            behavior::exact,
            context_fault::none,
            publication_result::committed,
            None,
            &mut first,
            &mut trace,
        )?;
        next_a += 1;
    }
    read(
        &mut a,
        owners[0],
        handle_a,
        next_a,
        requested_count,
        chunk_limit,
        "eof_a",
        behavior::exact,
        context_fault::none,
        publication_result::committed,
        None,
        &mut first,
        &mut trace,
    )?;
    let mut next_b = 2;
    while core(b.offset(owners[1].identity, handle_b), "source B position")? < input.len() as u64 {
        read(
            &mut b,
            owners[1],
            handle_b,
            next_b,
            requested_count,
            997,
            "stream_b",
            behavior::exact,
            context_fault::none,
            publication_result::committed,
            None,
            &mut second,
            &mut trace,
        )?;
        next_b += 1;
    }
    read(
        &mut b,
        owners[1],
        handle_b,
        next_b,
        requested_count,
        997,
        "eof_b",
        behavior::exact,
        context_fault::none,
        publication_result::committed,
        None,
        &mut second,
        &mut trace,
    )?;
    require(
        first == input && second == input,
        "bounded source did not reconstruct exact input",
    )?;
    control(
        &mut a,
        owners[0],
        &scope,
        next_a + 1,
        Some(handle_a),
        "close_a",
        &mut trace,
    )?;
    control(
        &mut b,
        owners[1],
        &scope,
        next_b + 1,
        Some(handle_b),
        "close_b",
        &mut trace,
    )?;
    let uncertain_handle = control(
        &mut uncertain,
        owners[2],
        &scope,
        1,
        None,
        "uncertain_open",
        &mut trace,
    )?;
    read(
        &mut uncertain,
        owners[2],
        uncertain_handle,
        2,
        17,
        chunk_limit,
        "unknown_publication",
        behavior::exact,
        context_fault::none,
        publication_result::unknown,
        Some(file_error::publication_unknown),
        &mut probe,
        &mut trace,
    )?;
    read(
        &mut uncertain,
        owners[2],
        uncertain_handle,
        3,
        17,
        chunk_limit,
        "fenced_reuse",
        behavior::exact,
        context_fault::none,
        publication_result::committed,
        Some(file_error::fenced),
        &mut probe,
        &mut trace,
    )?;
    let first_digest = sha256(&first)?;
    let second_digest = sha256(&second)?;
    require(
        first_digest == digest && second_digest == digest,
        "source stream digest mismatch",
    )?;
    require(
        a.open_count() == 0 && b.open_count() == 0 && uncertain.open_count() == 0,
        "source descriptors remain",
    )?;
    let report = format!(
        "{{\"schema\":1,\"scope\":\"host-only-bounded-file-source\",\"passed\":true,\"guest_executed\":false,\"source_bytes\":{},\"input_sha256\":{},\"input_path\":\"input.bin\",\"source_binding\":1,\"file_object\":{},\"file_version\":1,\"chunk_limit\":4096,\"first_descriptor\":3,\"table_capacity\":2,\"input_budget\":{},\"operation_budget\":{},\"operations_path\":\"source-operations.jsonl\",\"backend_path\":\"source-backend.jsonl\",\"operation_count\":{},\"backend_count\":{},\"owners\":[{{\"label\":\"a\",\"task\":71,\"generation\":1,\"service_incarnation\":401}},{{\"label\":\"b\",\"task\":71,\"generation\":2,\"service_incarnation\":402}},{{\"label\":\"uncertain\",\"task\":71,\"generation\":1,\"service_incarnation\":403}}],\"streams\":[{{\"owner\":\"a\",\"path\":\"source-owner-a.bin\",\"bytes\":{},\"sha256\":{}}},{{\"owner\":\"b\",\"path\":\"source-owner-b.bin\",\"bytes\":{},\"sha256\":{}}}],\"namespace\":{{\"prefix_hex\":{},\"present_path_hex\":{},\"replacement_binding\":2,\"replacement_version\":2,\"replacement_bytes_hex\":{}}},\"checks\":[\"independent_offsets\",\"retained_snapshot_after_namespace_replacement\",\"dirty_prefix_failure_no_publication\",\"short_and_overreported_source_no_publication\",\"wrong_and_stale_context_rejected\",\"exact_rejected_retry\",\"zero_and_eof_skip_backend\",\"unknown_fences\",\"scratch_zero_after_every_attempt\"],\"limits\":[\"All context identities are trusted host fixture bindings, not authenticated guest authority.\",\"Publication is a synchronous host callback, not a native delayed acknowledgement or Linux reply.\",\"No native source provisioning, caller copyout, dependency containment, library mapping or ELF execution is qualified.\"]}}\n",
        input.len(),
        quoted(digest),
        file_object,
        input_limit,
        operation_limit,
        trace.rows.len(),
        trace.context.backend.len(),
        first.len(),
        quoted(&first_digest),
        second.len(),
        quoted(&second_digest),
        quoted(&hex(namespace_prefix)),
        quoted(&hex(libc_path)),
        quoted(&hex(replacement.bytes))
    );
    Ok(proof_result {
        report,
        owner_a: first,
        owner_b: second,
        backend: trace.context.backend.join("\n") + "\n",
        operations: trace.rows.join("\n") + "\n",
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_fixture_reconstructs_binary_input_across_both_transfer_boundaries() {
        for length in [1, 17, 18, 996, 997, 998, 4095, 4096, 4097, 8193] {
            let bytes = (0..length).map(|i| (i % 256) as u8).collect::<Vec<_>>();
            let proof = prove(&bytes, &sha256(&bytes).unwrap()).unwrap();
            assert_eq!(proof.owner_a, bytes);
            assert_eq!(proof.owner_b, bytes);
            assert!(
                proof
                    .operations
                    .contains("\"case\":\"dirty_prefix_failure\"")
            );
            assert!(proof.operations.contains("\"case\":\"fenced_reuse\""));
        }
    }
}
