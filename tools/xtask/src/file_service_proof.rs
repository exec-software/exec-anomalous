//! Host-only qualification of the portable read-only file service.
//! Input bytes are borrowed immutable data; this command never executes ELF,
//! provisions guest RAM, authenticates a real task or publishes a Linux reply.

use std::ffi::OsString;
use std::fmt::Write as _;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};

use anomalous_veil_protocol::file_service::{
    client_identity, descriptor_handle, file_error, file_service, file_snapshot, namespace,
    namespace_entry, publication, publication_result,
};

mod source;

const input_limit: usize = 8 * 1024 * 1024;
const read_trace_limit: usize = input_limit / 4096 + input_limit / 997 + 16;
const namespace_prefix: &[u8] = b"/lib/x86_64-linux-gnu";
const libc_path: &[u8] = b"/lib/x86_64-linux-gnu/libc.so.6";
const omitted_candidate: &[u8] = b"/lib/x86_64-linux-gnu/glibc-hwcaps/x86-64-v2/libc.so.6";
const file_object: u64 = 0x4c49_4243;
const file_version: u64 = 1;
const owner_task: u64 = 71;
const requested_count: u64 = 65536;

struct proof_result {
    report: String,
    owner_a: Vec<u8>,
    owner_b: Vec<u8>,
}

#[derive(Clone, Copy)]
struct read_request {
    label: &'static str,
    owner: client_identity,
    sequence: u64,
    handle: descriptor_handle,
    count: u64,
    budget: usize,
    outcome: publication_result,
}

struct read_trace {
    request: read_request,
    before: u64,
    proposed: usize,
    delivered: Option<usize>,
    after: Option<u64>,
}

impl read_trace {
    fn json(&self) -> String {
        let request = self.request;
        let outcome = match request.outcome {
            publication_result::committed => "committed",
            publication_result::rejected => "rejected",
            publication_result::unknown => "unknown",
        };
        format!(
            "{{\"owner\":{},\"task\":{},\"owner_generation\":{},\"service_incarnation\":{},\"descriptor\":{},\"handle_generation\":{},\"sequence\":{},\"offset_before\":{},\"requested\":{},\"budget\":{},\"proposed_bytes\":{},\"delivered_bytes\":{},\"offset_after\":{},\"outcome\":{},\"callback_bytes_checked\":true}}",
            quoted(request.label),
            request.owner.task(),
            request.owner.generation(),
            request.handle.service_incarnation(),
            request.handle.fd(),
            request.handle.generation(),
            request.sequence,
            self.before,
            request.count,
            request.budget,
            self.proposed,
            self.delivered
                .map_or_else(|| "null".into(), |value| value.to_string()),
            self.after
                .map_or_else(|| "null".into(), |value| value.to_string()),
            quoted(outcome)
        )
    }
}

fn control_publication(
    value: publication<'_>,
    descriptor: u32,
    opening: bool,
) -> publication_result {
    let correct = match value {
        publication::opened { descriptor: actual } => opening && actual == descriptor,
        publication::closed { descriptor: actual } => !opening && actual == descriptor,
        publication::read { .. } => false,
    };
    if correct {
        publication_result::committed
    } else {
        publication_result::rejected
    }
}

fn read_step(
    service: &mut file_service<'_, 2>,
    input: &[u8],
    delivered: &mut Vec<u8>,
    trace: &mut Vec<read_trace>,
    request: read_request,
) -> Result<usize, String> {
    require(
        trace.len() < read_trace_limit,
        "read publication trace exhausted its host proof budget",
    )?;
    let before = core(
        service.offset(request.owner, request.handle),
        "offset before read",
    )?;
    let previous_sequence = service.last_sequence();
    let begin = usize::try_from(before).map_err(|_| "file offset does not fit host")?;
    let remaining = input
        .len()
        .checked_sub(begin)
        .ok_or("offset escaped held file")?;
    let length = remaining
        .min(request.budget)
        .min(usize::try_from(request.count).unwrap_or(usize::MAX));
    let expected = input
        .get(begin..begin + length)
        .ok_or("read escaped held file")?;
    let mut observed = false;
    let result = service.read(
        request.owner,
        request.sequence,
        request.handle,
        request.count,
        request.budget,
        |publication| {
            if let publication::read {
                descriptor,
                offset,
                bytes,
            } = publication
                && descriptor == request.handle.fd()
                && offset == before
                && bytes == expected
            {
                observed = true;
                if request.outcome == publication_result::committed {
                    delivered.extend_from_slice(bytes);
                }
                return request.outcome;
            }
            publication_result::rejected
        },
    );
    require(
        observed,
        "read callback did not expose the exact held bytes and offset",
    )?;
    let (published, after) = match request.outcome {
        publication_result::committed => {
            require(
                core(result, "committed read")? == length,
                "committed read returned wrong length",
            )?;
            let after = core(
                service.offset(request.owner, request.handle),
                "offset after read",
            )?;
            require(
                after == before + length as u64 && service.last_sequence() == request.sequence,
                "committed read advanced incorrect offset or freshness",
            )?;
            (Some(length), Some(after))
        }
        publication_result::rejected => {
            expect_error(result, file_error::publication_rejected)?;
            require(
                core(
                    service.offset(request.owner, request.handle),
                    "rejected read offset",
                )? == before
                    && service.last_sequence() == previous_sequence,
                "rejected read changed offset or freshness",
            )?;
            (Some(0), Some(before))
        }
        publication_result::unknown => {
            expect_error(result, file_error::publication_unknown)?;
            require(
                service.is_fenced() && service.open_count() == 0,
                "unknown read did not fence and revoke descriptors",
            )?;
            expect_error(
                service.offset(request.owner, request.handle),
                file_error::fenced,
            )?;
            expect_error(
                service.handle_for_fd(request.owner, request.handle.fd()),
                file_error::fenced,
            )?;
            (None, None)
        }
    };
    trace.push(read_trace {
        request,
        before,
        proposed: length,
        delivered: published,
        after,
    });
    Ok(length)
}

fn stream_to_end(
    service: &mut file_service<'_, 2>,
    input: &[u8],
    delivered: &mut Vec<u8>,
    trace: &mut Vec<read_trace>,
    mut request: read_request,
) -> Result<u64, String> {
    while core(
        service.offset(request.owner, request.handle),
        "stream position",
    )? < input.len() as u64
    {
        require(
            read_step(service, input, delivered, trace, request)? > 0,
            "stream made no progress before EOF",
        )?;
        request.sequence = request
            .sequence
            .checked_add(1)
            .ok_or("stream sequence exhausted")?;
    }
    require(
        read_step(service, input, delivered, trace, request)? == 0,
        "EOF returned data",
    )?;
    require(
        delivered == input,
        "committed callback bytes do not reconstruct the held input",
    )?;
    Ok(request.sequence)
}

fn prove(input: &[u8], digest: &str, path: &Path) -> Result<proof_result, String> {
    require(
        !input.is_empty() && input.len() <= input_limit,
        "proof input escaped nonempty bound",
    )?;
    let owner_a = core(client_identity::new(owner_task, 1), "owner A")?;
    let owner_b = core(client_identity::new(owner_task, 2), "owner B")?;
    let file = core(
        file_snapshot::new(file_object, file_version, input),
        "held file",
    )?;
    let entries = [core(
        namespace_entry::new(libc_path, file),
        "namespace entry",
    )?];
    let scope = core(
        namespace::new(namespace_prefix, &entries),
        "closed library namespace",
    )?;
    let mut a = core(file_service::<2>::new(owner_a, 101, 3), "service A")?;
    let mut b = core(file_service::<2>::new(owner_b, 202, 3), "service B")?;
    let mut forbidden_callbacks = 0;
    for (name, expected) in [
        (omitted_candidate, file_error::not_found),
        (
            b"/lib/x86_64-linux-gnu-other/libc.so.6".as_slice(),
            file_error::out_of_scope,
        ),
        (
            b"/lib/x86_64-linux-gnu/../libc.so.6".as_slice(),
            file_error::unsupported_path,
        ),
    ] {
        expect_error(
            a.open(owner_a, 1, &scope, name, true, |_| {
                forbidden_callbacks += 1;
                publication_result::rejected
            }),
            expected,
        )?;
    }
    require(
        a.open_count() == 0 && a.last_sequence() == 0,
        "namespace refusal mutated the service",
    )?;
    let handle_a = core(
        a.open(owner_a, 1, &scope, libc_path, true, |value| {
            control_publication(value, 3, true)
        }),
        "open A",
    )?;
    let handle_b = core(
        b.open(owner_b, 1, &scope, libc_path, true, |value| {
            control_publication(value, 3, true)
        }),
        "open B",
    )?;
    require(
        handle_a.fd() == 3 && handle_b.fd() == 3 && handle_a != handle_b,
        "two owner incarnations did not receive separately bound handles",
    )?;
    require(
        core(a.handle_for_fd(owner_a, 3), "lookup A")? == handle_a
            && core(b.handle_for_fd(owner_b, 3), "lookup B")? == handle_b
            && core(a.close_on_exec(owner_a, handle_a), "A close-on-exec")?
            && core(b.close_on_exec(owner_b, handle_b), "B close-on-exec")?,
        "descriptor policy lookup changed",
    )?;
    let held = core(a.file(owner_a, handle_a), "A pinned file")?;
    require(
        held.object() == file_object && held.version() == file_version && held.bytes() == input,
        "descriptor does not retain exact file identity/version/bytes",
    )?;
    for (owner, handle, expected) in [
        (owner_b, handle_a, file_error::wrong_owner),
        (owner_a, handle_b, file_error::stale_handle),
    ] {
        expect_error(
            a.read(owner, 2, handle, 1, 4096, |_| {
                forbidden_callbacks += 1;
                publication_result::rejected
            }),
            expected,
        )?;
    }
    expect_error(
        a.read(owner_a, 2, handle_a, 1, 0, |_| {
            forbidden_callbacks += 1;
            publication_result::rejected
        }),
        file_error::invalid_budget,
    )?;
    let mut first = Vec::with_capacity(input.len());
    let mut second = Vec::with_capacity(input.len());
    let mut trace = Vec::new();
    let initial = read_request {
        label: "a",
        owner: owner_a,
        sequence: 2,
        handle: handle_a,
        count: 0,
        budget: 0,
        outcome: publication_result::committed,
    };
    read_step(&mut a, input, &mut first, &mut trace, initial)?;
    let prefix = read_request {
        sequence: 3,
        count: 1,
        budget: 4096,
        ..initial
    };
    read_step(
        &mut a,
        input,
        &mut first,
        &mut trace,
        read_request {
            outcome: publication_result::rejected,
            ..prefix
        },
    )?;
    require(first.is_empty(), "rejected callback delivered bytes")?;
    read_step(&mut a, input, &mut first, &mut trace, prefix)?;
    require(
        core(a.offset(owner_a, handle_a), "A independent position")? == 1
            && core(b.offset(owner_b, handle_b), "B independent position")? == 0,
        "one caller changed another caller's offset",
    )?;
    expect_error(
        a.read(owner_a, 3, handle_a, 1, 4096, |_| {
            forbidden_callbacks += 1;
            publication_result::rejected
        }),
        file_error::stale_invocation,
    )?;
    let a_end = stream_to_end(
        &mut a,
        input,
        &mut first,
        &mut trace,
        read_request {
            sequence: 4,
            count: requested_count,
            ..prefix
        },
    )?;
    let b_end = stream_to_end(
        &mut b,
        input,
        &mut second,
        &mut trace,
        read_request {
            label: "b",
            owner: owner_b,
            sequence: 2,
            handle: handle_b,
            count: requested_count,
            budget: 997,
            outcome: publication_result::committed,
        },
    )?;
    expect_error(
        a.close(owner_a, a_end + 1, handle_a, |value| {
            if control_publication(value, 3, false) == publication_result::committed {
                publication_result::rejected
            } else {
                forbidden_callbacks += 1;
                publication_result::rejected
            }
        }),
        file_error::publication_rejected,
    )?;
    require(
        a.open_count() == 1
            && a.last_sequence() == a_end
            && core(a.offset(owner_a, handle_a), "rejected close offset")? == input.len() as u64,
        "rejected close mutated committed state",
    )?;
    core(
        a.close(owner_a, a_end + 1, handle_a, |value| {
            control_publication(value, 3, false)
        }),
        "close A",
    )?;
    expect_error(a.offset(owner_a, handle_a), file_error::stale_handle)?;
    expect_error(a.handle_for_fd(owner_a, 3), file_error::bad_descriptor)?;
    let reopened = core(
        a.open(owner_a, a_end + 2, &scope, libc_path, true, |value| {
            control_publication(value, 3, true)
        }),
        "reopen A",
    )?;
    require(
        reopened.fd() == handle_a.fd()
            && reopened.generation() > handle_a.generation()
            && reopened.service_incarnation() == handle_a.service_incarnation()
            && core(a.offset(owner_a, reopened), "reopened offset")? == 0,
        "reopened descriptor was not fresh at offset zero",
    )?;
    expect_error(
        a.read(owner_a, a_end + 3, handle_a, 1, 4096, |_| {
            forbidden_callbacks += 1;
            publication_result::rejected
        }),
        file_error::stale_handle,
    )?;
    core(
        a.close(owner_a, a_end + 3, reopened, |value| {
            control_publication(value, 3, false)
        }),
        "close reopened A",
    )?;
    core(
        b.close(owner_b, b_end + 1, handle_b, |value| {
            control_publication(value, 3, false)
        }),
        "close B",
    )?;
    require(
        a.open_count() == 0 && b.open_count() == 0,
        "completed streams retained open descriptors",
    )?;

    let mut uncertain = core(file_service::<2>::new(owner_a, 303, 3), "uncertain service")?;
    let uncertain_handle = core(
        uncertain.open(owner_a, 1, &scope, libc_path, true, |value| {
            control_publication(value, 3, true)
        }),
        "open uncertain service",
    )?;
    read_step(
        &mut uncertain,
        input,
        &mut Vec::new(),
        &mut trace,
        read_request {
            label: "uncertain",
            owner: owner_a,
            sequence: 2,
            handle: uncertain_handle,
            count: 1,
            budget: 4096,
            outcome: publication_result::unknown,
        },
    )?;
    expect_error(
        uncertain.open(owner_a, 3, &scope, libc_path, true, |_| {
            forbidden_callbacks += 1;
            publication_result::rejected
        }),
        file_error::fenced,
    )?;
    require(
        forbidden_callbacks == 0,
        "validation failure unexpectedly reached publication",
    )?;
    let first_digest = sha256(&first)?;
    let second_digest = sha256(&second)?;
    require(
        first_digest == digest && second_digest == digest,
        "reconstructed stream SHA-256 differs",
    )?;
    let committed = trace
        .iter()
        .filter(|entry| entry.request.outcome == publication_result::committed)
        .count();
    let zero_reads = trace
        .iter()
        .filter(|entry| {
            entry.delivered == Some(0) && entry.request.outcome == publication_result::committed
        })
        .count();
    let short_reads = trace
        .iter()
        .filter(|entry| {
            entry
                .delivered
                .is_some_and(|length| length > 0 && (length as u64) < entry.request.count)
        })
        .count();
    let traces = trace
        .iter()
        .map(read_trace::json)
        .collect::<Vec<_>>()
        .join(",");
    let report = format!(
        "{{\"schema\":1,\"scope\":\"host-only-portable-read-only-file-service\",\"passed\":true,\"guest_executed\":false,\"source_path\":{},\"source_path_hex\":{},\"source_path_role\":\"Supplied held-input path label; retained bytes and the configured digest pin the snapshot.\",\"source_bytes\":{},\"configured_sha256\":{},\"input_sha256\":{},\"file_object\":{},\"file_version\":{},\"input_budget\":{},\"read_trace_budget\":{},\"namespace\":{{\"kind\":\"explicit-closed-subtree\",\"prefix_hex\":{},\"present_path_hex\":{},\"omitted_candidate_hex\":{},\"host_namespace_inferred\":false}},\"owners\":[{{\"label\":\"a\",\"task\":71,\"generation\":1,\"service_incarnation\":101}},{{\"label\":\"b\",\"task\":71,\"generation\":2,\"service_incarnation\":202}},{{\"label\":\"uncertain\",\"task\":71,\"generation\":1,\"service_incarnation\":303}}],\"streams\":[{{\"owner\":\"a\",\"path\":\"owner-a.bin\",\"bytes\":{},\"sha256\":{}}},{{\"owner\":\"b\",\"path\":\"owner-b.bin\",\"bytes\":{},\"sha256\":{}}}],\"first_descriptor\":3,\"table_capacity\":2,\"initial_handle_generation_a\":{},\"initial_handle_generation_b\":{},\"reopened_handle_generation_a\":{},\"a_stream_final_sequence\":{},\"b_stream_final_sequence\":{},\"a_close_sequence\":{},\"a_reopen_sequence\":{},\"a_final_close_sequence\":{},\"b_close_sequence\":{},\"read_publications\":{},\"committed_read_publications\":{},\"rejected_read_publications\":1,\"unknown_read_publications\":1,\"committed_empty_reads\":{},\"committed_nonempty_short_reads\":{},\"independent_offsets\":true,\"rejected_read_unchanged\":true,\"rejected_close_unchanged\":true,\"fresh_reopen\":true,\"final_open_counts\":[0,0,0],\"unknown_fenced\":true,\"negative_checks\":[\"declared_scoped_miss\",\"outside_scope_not_absence\",\"noncanonical_path\",\"wrong_owner\",\"cross_service_handle\",\"zero_budget\",\"stale_invocation\",\"closed_handle\",\"closed_descriptor\",\"stale_reopened_handle\",\"fenced_service\"],\"rejected_validation_publications\":0,\"reads\":[{}],\"callback_contract\":\"Trusted synchronous host embedding: committed means exact bytes copied to the retained per-owner stream before success; rejected copies nothing; unknown deliberately simulates uncertain publication and fences the whole service. Callback fields are matched to the held input and expected offset before any modeled publication.\",\"limits\":[\"Caller identities and service incarnations are trusted fixture configuration, not authenticated guest authority.\",\"Namespace is an explicit supplied guest-model configuration; no host absence or dependency search is inferred.\",\"CLOEXEC is retained policy only; no exec lifecycle is implemented.\",\"No ELF execution, guest provisioning, syscall adapter, kernel source read, physical caller copyout, library mapping, libc startup or Antimatter execution is qualified.\"]}}\n",
        quoted(&path.to_string_lossy()),
        quoted(&hex(path.as_os_str().as_encoded_bytes())),
        input.len(),
        quoted(digest),
        quoted(digest),
        file_object,
        file_version,
        input_limit,
        read_trace_limit,
        quoted(&hex(namespace_prefix)),
        quoted(&hex(libc_path)),
        quoted(&hex(omitted_candidate)),
        first.len(),
        quoted(&first_digest),
        second.len(),
        quoted(&second_digest),
        handle_a.generation(),
        handle_b.generation(),
        reopened.generation(),
        a_end,
        b_end,
        a_end + 1,
        a_end + 2,
        a_end + 3,
        b_end + 1,
        trace.len(),
        committed,
        zero_reads,
        short_reads,
        traces
    );
    Ok(proof_result {
        report,
        owner_a: first,
        owner_b: second,
    })
}

pub(super) fn qualify(arguments: Vec<OsString>) -> Result<(), String> {
    if arguments.len() != 3 {
        return Err("qualify-read-only-files requires PATH SHA256 NEW_OUTPUT_DIRECTORY".into());
    }
    let expected = arguments[1]
        .to_str()
        .ok_or("expected SHA-256 must be lowercase ASCII hexadecimal")?;
    validate_digest(expected)?;
    let path = Path::new(&arguments[0]);
    let output = Path::new(&arguments[2]);
    let bytes = read_bounded(path, input_limit)?;
    let digest = sha256(&bytes)?;
    if digest != expected {
        return Err("held file differs from configured SHA-256; no evidence published".into());
    }
    let proof = prove(&bytes, &digest, path)?;
    let source_proof = source::prove(&bytes, &digest)?;
    publish(output, &bytes, &proof, Some(&source_proof))?;
    println!(
        "Read-only file service host proof retained {} input bytes and two complete delivered streams in {} (no guest execution)",
        bytes.len(),
        output.display()
    );
    Ok(())
}

fn require(condition: bool, message: &str) -> Result<(), String> {
    condition.then_some(()).ok_or_else(|| message.into())
}

fn core<T>(result: Result<T, file_error>, operation: &str) -> Result<T, String> {
    result.map_err(|error| format!("read-only file proof {operation}: {error:?}"))
}

fn expect_error<T>(result: Result<T, file_error>, expected: file_error) -> Result<(), String> {
    match result {
        Err(actual) if actual == expected => Ok(()),
        Err(actual) => Err(format!("expected {expected:?}, received {actual:?}")),
        Ok(_) => Err(format!(
            "operation unexpectedly succeeded instead of {expected:?}"
        )),
    }
}

fn validate_digest(value: &str) -> Result<(), String> {
    require(
        value.len() == 64
            && value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
        "expected SHA-256 must contain exactly 64 lowercase hexadecimal digits",
    )
}

fn read_bounded(path: &Path, limit: usize) -> Result<Vec<u8>, String> {
    // Reject known nonregular paths before opening them (notably FIFOs).
    // Digest verification, not a pathname or metadata timestamp, pins the
    // complete bytes subsequently read from this one held file descriptor.
    let initial = fs::metadata(path).map_err(|error| format!("stat input: {error}"))?;
    if !initial.is_file() || initial.len() == 0 || initial.len() > limit as u64 {
        return Err("file proof requires a nonempty regular file within its byte budget".into());
    }
    let file = File::open(path).map_err(|error| format!("open input: {error}"))?;
    let metadata = file.metadata().map_err(|error| error.to_string())?;
    if !metadata.is_file() || metadata.len() == 0 || metadata.len() > limit as u64 {
        return Err("held input is not a nonempty bounded regular file".into());
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("read held input: {error}"))?;
    require(
        !bytes.is_empty() && bytes.len() <= limit,
        "held input escaped its nonempty byte budget",
    )?;
    Ok(bytes)
}

fn sha256(bytes: &[u8]) -> Result<String, String> {
    let mut child = Command::new("sha256sum")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("start held-byte SHA-256: {error}"))?;
    let written = child
        .stdin
        .take()
        .ok_or("missing hash stdin")?
        .write_all(bytes);
    let output = child
        .wait_with_output()
        .map_err(|error| error.to_string())?;
    written.map_err(|error| error.to_string())?;
    require(
        output.status.success() && output.stderr.is_empty(),
        "held-byte SHA-256 failed",
    )?;
    parse_hash_output(&output.stdout)
}

fn parse_hash_output(output: &[u8]) -> Result<String, String> {
    require(
        output.len() == 68 && output.get(64..) == Some(b"  -\n"),
        "unexpected SHA-256 output format",
    )?;
    let digest = std::str::from_utf8(&output[..64]).map_err(|_| "SHA-256 output is not ASCII")?;
    validate_digest(digest)?;
    Ok(digest.into())
}

fn quoted(value: &str) -> String {
    let mut out = String::from("\"");
    for character in value.chars() {
        match character {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{00}'..='\u{1f}' => write!(out, "\\u{:04x}", character as u32).unwrap(),
            _ => out.push(character),
        }
    }
    out.push('"');
    out
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn publish(
    output: &Path,
    input: &[u8],
    proof: &proof_result,
    source_proof: Option<&source::proof_result>,
) -> Result<(), String> {
    fs::create_dir(output).map_err(|error| format!("create new proof directory: {error}"))?;
    // A write failure leaves incomplete evidence for inspection. An existing
    // directory is never reused, removed or treated as this attempt's output.
    // The report is last, after every byte stream has been synchronized.
    let mut files = vec![
        ("input.bin", input),
        ("owner-a.bin", proof.owner_a.as_slice()),
        ("owner-b.bin", proof.owner_b.as_slice()),
    ];
    if let Some(source) = source_proof {
        files.extend(source.files());
    }
    files.push(("report.json", proof.report.as_bytes()));
    for (name, bytes) in files {
        let mut file = File::create_new(output.join(name))
            .map_err(|error| format!("create {name}: {error}"))?;
        file.write_all(bytes)
            .and_then(|()| file.sync_all())
            .map_err(|error| format!("write {name}: {error}"))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static next_directory: AtomicU64 = AtomicU64::new(0);

    struct test_directory(PathBuf);

    impl test_directory {
        fn new() -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "anomalous-file-proof-{}-{nonce}-{}",
                std::process::id(),
                next_directory.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for test_directory {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }

    #[test]
    fn digest_and_hash_output_require_exact_canonical_values() {
        let valid = "0123456789abcdef".repeat(4);
        validate_digest(&valid).unwrap();
        assert_eq!(
            parse_hash_output(format!("{valid}  -\n").as_bytes()).unwrap(),
            valid
        );
        for invalid in [
            String::new(),
            valid[..63].to_string(),
            format!("{valid}0"),
            valid.to_uppercase(),
            format!("g{}", &valid[1..]),
            format!("é{}", &valid[2..]),
            format!(" {valid}"),
        ] {
            assert!(validate_digest(&invalid).is_err());
        }
        for output in [
            valid.clone(),
            format!("{valid} -\n"),
            format!("{valid}  filename\n"),
            format!("{valid}  -\r\n"),
            format!("{valid}  -\nextra"),
            format!("{}  -\n", valid.to_uppercase()),
        ] {
            assert!(parse_hash_output(output.as_bytes()).is_err());
        }
    }

    #[test]
    fn held_input_rejects_nonregular_empty_and_oversized_data() {
        let directory = test_directory::new();
        let path = directory.0.join("input.bin");
        fs::write(&path, [0, 0xff, 0x80, 17]).unwrap();
        assert_eq!(read_bounded(&path, 4).unwrap(), [0, 0xff, 0x80, 17]);
        assert!(read_bounded(&path, 3).is_err());
        assert!(read_bounded(&directory.0, 4).is_err());
        assert!(read_bounded(&directory.0.join("missing"), 4).is_err());
        fs::write(&path, []).unwrap();
        assert!(read_bounded(&path, 4).is_err());
    }

    #[test]
    fn report_strings_preserve_text_and_escape_every_control_byte() {
        assert_eq!(
            quoted("a\"b\\c\n\t\0é"),
            "\"a\\\"b\\\\c\\u000a\\u0009\\u0000é\""
        );
        for byte in 0u8..=31 {
            assert_eq!(
                quoted(&char::from(byte).to_string()),
                format!("\"\\u{byte:04x}\"")
            );
        }
        assert_eq!(hex(&[0, 0xff, 0x80, b'/']), "00ff802f");
    }

    #[test]
    fn bad_command_inputs_publish_no_evidence() {
        assert!(qualify(Vec::new()).is_err());
        assert!(qualify(vec!["a".into(), "b".into(), "c".into(), "d".into()]).is_err());
        let directory = test_directory::new();
        let input = directory.0.join("input");
        let output = directory.0.join("proof");
        fs::write(&input, b"not the configured pin").unwrap();
        assert!(
            qualify(vec![
                input.clone().into(),
                "invalid".into(),
                output.clone().into()
            ])
            .is_err()
        );
        assert!(!output.exists());
        assert!(
            qualify(vec![
                input.into(),
                "0".repeat(64).into(),
                output.clone().into()
            ])
            .is_err()
        );
        assert!(!output.exists());
    }

    #[test]
    fn pinned_command_refuses_existing_output_without_changing_its_contents() {
        let directory = test_directory::new();
        let input = directory.0.join("input");
        let output = directory.0.join("proof");
        let bytes = [0, 255, 128, 17];
        fs::write(&input, bytes).unwrap();
        fs::create_dir(&output).unwrap();
        fs::write(output.join("input.bin"), b"existing evidence").unwrap();
        fs::write(output.join("report.json"), b"existing report").unwrap();
        assert!(
            qualify(vec![
                input.into(),
                sha256(&bytes).unwrap().into(),
                output.clone().into()
            ])
            .is_err()
        );
        assert_eq!(
            fs::read(output.join("input.bin")).unwrap(),
            b"existing evidence"
        );
        assert_eq!(
            fs::read(output.join("report.json")).unwrap(),
            b"existing report"
        );
        assert_eq!(fs::read_dir(output).unwrap().count(), 2);
    }

    #[test]
    fn publication_retains_all_streams_and_never_overwrites_a_directory() {
        let directory = test_directory::new();
        let output = directory.0.join("proof");
        let proof = proof_result {
            report: "{\"schema\":1}\n".into(),
            owner_a: vec![0, 255],
            owner_b: vec![0, 255],
        };
        publish(&output, &[0, 255], &proof, None).unwrap();
        for name in ["input.bin", "owner-a.bin", "owner-b.bin"] {
            assert_eq!(fs::read(output.join(name)).unwrap(), [0, 255]);
        }
        assert_eq!(
            fs::read_to_string(output.join("report.json")).unwrap(),
            proof.report
        );
        assert!(publish(&output, &[1], &proof, None).is_err());
        assert_eq!(fs::read(output.join("input.bin")).unwrap(), [0, 255]);
    }

    #[test]
    fn host_adapter_reconstructs_binary_input_at_chunk_boundaries() {
        for length in [1, 996, 997, 998, 4095, 4096, 4097, 8193] {
            let input = (0..length)
                .map(|index| (index % 256) as u8)
                .collect::<Vec<_>>();
            let digest = sha256(&input).unwrap();
            let proof = prove(&input, &digest, Path::new("binary-input")).unwrap();
            assert_eq!(proof.owner_a, input);
            assert_eq!(proof.owner_b, input);
            verify_read_report(&proof.report, length, &digest);
        }
    }

    fn record_number(record: &crate::structured_record, key: &str) -> usize {
        match record.get(key).unwrap() {
            crate::json_scalar::number(value) => value.parse().unwrap(),
            other => panic!("expected number for {key}, received {other:?}"),
        }
    }

    fn verify_read_report(report: &str, length: usize, digest: &str) {
        // The existing strict serial parser intentionally supports only flat
        // scalar records. Reuse it for the top-level scalar prefix and every
        // read record, including duplicate-field and trailing-data rejection.
        let (prefix, _) = report.split_once(",\"namespace\":").unwrap();
        let header = crate::json_parser::new(&format!("{prefix}}}"))
            .parse_record()
            .unwrap();
        assert_eq!(
            header.get("passed"),
            Some(&crate::json_scalar::boolean(true))
        );
        assert_eq!(
            header.get("guest_executed"),
            Some(&crate::json_scalar::boolean(false))
        );
        assert_eq!(
            header.get("input_sha256"),
            Some(&crate::json_scalar::string(digest.into()))
        );
        assert_eq!(record_number(&header, "source_bytes"), length);
        assert_eq!(record_number(&header, "input_budget"), input_limit);
        assert_eq!(
            record_number(&header, "read_trace_budget"),
            read_trace_limit
        );

        let (_, after_reads) = report.split_once("\"reads\":[").unwrap();
        let (reads, _) = after_reads.split_once("],\"callback_contract\":").unwrap();
        let body = reads.strip_prefix('{').unwrap().strip_suffix('}').unwrap();
        let mut offsets = [0; 3];
        let mut sequences = [1; 3];
        let mut delivered = [0; 3];
        let mut counts = [0; 3];
        let mut publications = 0;
        let mut empty = 0;
        for fields in body.split("},{") {
            let record = crate::json_parser::new(&format!("{{{fields}}}"))
                .parse_record()
                .unwrap();
            assert_eq!(record.len(), 15);
            let index = match record.get("owner").unwrap() {
                crate::json_scalar::string(label) if label == "a" => 0,
                crate::json_scalar::string(label) if label == "b" => 1,
                crate::json_scalar::string(label) if label == "uncertain" => 2,
                other => panic!("unexpected owner {other:?}"),
            };
            assert_eq!(record_number(&record, "task"), 71);
            assert_eq!(record_number(&record, "owner_generation"), [1, 2, 1][index]);
            assert_eq!(
                record_number(&record, "service_incarnation"),
                [101, 202, 303][index]
            );
            assert_eq!(record_number(&record, "descriptor"), 3);
            assert!(record_number(&record, "handle_generation") > 0);
            assert_eq!(
                record.get("callback_bytes_checked"),
                Some(&crate::json_scalar::boolean(true))
            );
            assert_eq!(record_number(&record, "offset_before"), offsets[index]);
            let sequence = record_number(&record, "sequence");
            assert_eq!(sequence, sequences[index] + 1);
            let proposed = (length - offsets[index])
                .min(record_number(&record, "requested"))
                .min(record_number(&record, "budget"));
            assert_eq!(record_number(&record, "proposed_bytes"), proposed);
            match record.get("outcome").unwrap() {
                crate::json_scalar::string(outcome) if outcome == "committed" => {
                    counts[0] += 1;
                    empty += usize::from(proposed == 0);
                    assert_eq!(record_number(&record, "delivered_bytes"), proposed);
                    delivered[index] += proposed;
                    offsets[index] += proposed;
                    sequences[index] = sequence;
                    assert_eq!(record_number(&record, "offset_after"), offsets[index]);
                }
                crate::json_scalar::string(outcome) if outcome == "rejected" => {
                    counts[1] += 1;
                    assert_eq!(index, 0);
                    assert_eq!(sequence, 3);
                    assert_eq!(record_number(&record, "delivered_bytes"), 0);
                    assert_eq!(record_number(&record, "offset_after"), offsets[index]);
                }
                crate::json_scalar::string(outcome) if outcome == "unknown" => {
                    counts[2] += 1;
                    assert_eq!(index, 2);
                    assert_eq!(
                        record.get("delivered_bytes"),
                        Some(&crate::json_scalar::null)
                    );
                    assert_eq!(record.get("offset_after"), Some(&crate::json_scalar::null));
                }
                other => panic!("unexpected publication outcome {other:?}"),
            }
            publications += 1;
        }
        let chunk_reads = (length - 1).div_ceil(4096) + length.div_ceil(997);
        assert_eq!(publications, chunk_reads + 6);
        assert_eq!(counts, [chunk_reads + 4, 1, 1]);
        assert_eq!(empty, 3);
        assert_eq!(delivered, [length, length, 0]);
        assert_eq!(offsets, [length, length, 0]);
    }
}
