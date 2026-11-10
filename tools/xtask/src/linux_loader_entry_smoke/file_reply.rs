//! Independent physical oracle for eight clean bounded file-service lifetimes.
//! Path classification and descriptor arithmetic are derived here from the
//! declared fixture, not from userspace's service implementation or counters.

use super::*;

const boundary_limit: usize = 32;
const line_limit: usize = 32768;
const library_prefix: &[u8] = b"/lib/x86_64-linux-gnu";
const library_path: &[u8] = b"/lib/x86_64-linux-gnu/libc.so.6";
const delivery_event: &str = "kernel.linux_loader_entry_test.file_delivery";
const service_event: &str = "kernel.linux_loader_entry_test.file_service";
const mapping_event: &str = "kernel.linux_loader_entry_test.mapping_cancel";
const mapping_lifetime_event: &str = "kernel.linux_loader_entry_test.mapping_lifetime";
const containment_event: &str = "kernel.linux_loader_entry_test.containment";

fn check(condition: bool, reason: &str) -> Result<(), String> {
    condition.then_some(()).ok_or_else(|| reason.into())
}

fn boolean(record: &structured_record, event: &str, field: &str) -> Result<bool, String> {
    match record.get(field) {
        Some(json_scalar::boolean(value)) => Ok(*value),
        _ => Err(format!("{event}: {field} must be boolean")),
    }
}

fn decode_hex(
    record: &structured_record,
    event: &str,
    field: &str,
    length: usize,
) -> Result<Vec<u8>, String> {
    let text = required_string(record, event, field)?;
    check(
        length <= 4096 && text.len() == length * 2,
        "byte witness has wrong bounded length",
    )?;
    let mut bytes = Vec::with_capacity(length);
    let nibble = |byte| match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        _ => Err("byte witness is not canonical lowercase hex".to_string()),
    };
    for pair in text.as_bytes().chunks_exact(2) {
        bytes.push(nibble(pair[0])? * 16 + nibble(pair[1])?);
    }
    Ok(bytes)
}

fn event(record: &structured_record) -> Result<&str, String> {
    required_string(record, "loader", "event")
}

fn next(
    records: &[structured_record],
    cursor: &mut usize,
    expected: &str,
    scenario: u64,
) -> Result<structured_record, String> {
    let mut record = records
        .get(*cursor)
        .ok_or("incomplete file-reply transcript")?
        .clone();
    require_string(&record, expected, "event", expected)?;
    require_number(&record, expected, "schema", 1)?;
    require_string(&record, expected, "level", "info")?;
    require_number(&record, expected, "scenario", scenario)?;
    record.remove("scenario");
    *cursor += 1;
    Ok(record)
}

fn mapped_extent(
    plan: &[u64; plan_word_count],
    address: u64,
    length: usize,
    writable: bool,
) -> Result<(), String> {
    let end = address
        .checked_add(length as u64)
        .ok_or("caller witness address overflow")?;
    check(length > 0, "empty caller mapping witness")?;
    let mut covered = address;
    let mut regions = plan[16..43]
        .chunks_exact(3)
        .filter(|r| r[1] != 0)
        .collect::<Vec<_>>();
    regions.sort_by_key(|r| r[0]);
    for region in regions {
        let bound = region[1]
            .checked_mul(4096)
            .and_then(|n| region[0].checked_add(n))
            .ok_or("caller region overflow")?;
        if region[0] < end && address < bound {
            check(
                region[0] <= covered && bound > covered && (!writable || region[2] == 3),
                "caller witness is not covered by its admitted mapping permissions",
            )?;
            covered = bound.min(end);
        }
    }
    check(covered == end, "caller witness escaped owned mappings")
}

fn canonical(path: &[u8]) -> bool {
    path.first() == Some(&b'/')
        && !path.contains(&0)
        && path[1..]
            .split(|b| *b == b'/')
            .all(|p| !p.is_empty() && p != b"." && p != b"..")
}

fn scoped_missing(number: u64, path: &[u8]) -> bool {
    // Linux directory metadata probes may carry one terminal slash. Keep the
    // complete raw pathname as evidence, and apply this translation only to
    // newfstatat; openat does not gain directory traversal semantics here.
    let path = if number == 262 {
        path.strip_suffix(b"/").unwrap_or(path)
    } else {
        path
    };
    canonical(path)
        && path.starts_with(library_prefix)
        && path.get(library_prefix.len()) == Some(&b'/')
        && path != library_path
        && !path.starts_with(&[library_path, b"/"].concat())
}

fn pathname(
    record: &structured_record,
    boundary: &structured_record,
    plan: &[u64; plan_word_count],
    source: &[u8],
    sequence: u64,
) -> Result<Vec<u8>, String> {
    let current_event = event(record)?;
    let length = required_number(record, current_event, "copy_bytes")?;
    if length == 0 {
        for name in ["copy_sequence", "copy_address"] {
            require_number(record, current_event, name, 0)?;
        }
        require_string(record, current_event, "copy_hex", "")?;
        return Ok(Vec::new());
    }
    check(length <= 256, "pathname escaped per-invocation work budget")?;
    let number = required_number(boundary, events[9], "number")?;
    let argument = if number == 21 { "arg0" } else { "arg1" };
    let address = required_number(boundary, events[9], argument)?;
    require_number(record, current_event, "copy_sequence", sequence)?;
    require_number(record, current_event, "copy_address", address)?;
    let mut bytes = decode_hex(record, current_event, "copy_hex", length as usize)?;
    check(
        bytes.last() == Some(&0) && !bytes[..bytes.len() - 1].contains(&0),
        "pathname does not end at its first NUL",
    )?;
    mapped_extent(plan, address, bytes.len(), false)?;
    if sequence == 2 || sequence == 3 {
        // The initial constant strings also have independent initialized-source
        // witnesses. Later loader-built paths are actual mutable caller bytes.
        verify_copy(record, boundary, plan, source, sequence)?;
    }
    bytes.pop();
    Ok(bytes)
}

fn boundary_header(
    record: &structured_record,
    sequence: u64,
    returned: bool,
    plan: &[u64; plan_word_count],
    source: &[u8],
) -> Result<(), String> {
    require_control_header(
        record,
        events[9],
        &[
            "sequence",
            "number",
            "arg0",
            "arg1",
            "arg2",
            "arg3",
            "arg4",
            "arg5",
            "instruction_pointer",
            "syscall_instruction",
            "stack_pointer",
            "rcx",
            "r11",
            "fs_base",
            "result_bits",
            "returned_to_caller",
            "linux_exit_observed",
            "syscall_bytes_verified",
        ],
    )?;
    for name in [
        "sequence",
        "number",
        "arg0",
        "arg1",
        "arg2",
        "arg3",
        "arg4",
        "arg5",
        "instruction_pointer",
        "syscall_instruction",
        "stack_pointer",
        "rcx",
        "r11",
        "fs_base",
        "result_bits",
    ] {
        required_number(record, events[9], name)?;
    }
    require_number(record, events[9], "sequence", sequence)?;
    require_number(record, events[9], "fs_base", 0)?;
    require_bool(record, events[9], "returned_to_caller", returned)?;
    require_bool(record, events[9], "linux_exit_observed", false)?;
    require_bool(record, events[9], "syscall_bytes_verified", true)?;
    verify_boundary_hardware(record, plan, source)
}

struct flow {
    boundaries: Vec<structured_record>,
    open_sequence: u64,
    read_length: usize,
    deliveries: Vec<delivery>,
    copy_bytes: u64,
    scoped_misses: usize,
}

struct delivery {
    sequence: u64,
    length: usize,
    offset: usize,
    committed: bool,
    metadata: bool,
}

fn unknown_scenario(scenario: u64) -> bool {
    matches!(scenario, 2 | 4 | 6)
}

fn staged_scenario(scenario: u64) -> bool {
    matches!(scenario, 1 | 3 | 5)
}

fn mapping_scenario(scenario: u64) -> bool {
    matches!(scenario, 0 | 7)
}

impl flow {
    fn raw_calls(&self) -> u64 {
        self.boundaries.len() as u64
    }
}

pub(super) fn verify(
    transcript: &str,
    plan: &[u64; plan_word_count],
    source: &[u8],
    file: &[u8],
    faults: [u64; 2],
) -> Result<run_evidence, String> {
    check(
        transcript.len() <= transcript_byte_limit,
        "file-reply transcript exceeds512KiB proof budget",
    )?;
    let mut records = Vec::new();
    for line in transcript.lines().filter(|line| !line.trim().is_empty()) {
        check(
            line.len() <= line_limit && records.len() < 256,
            "file-reply record budget exhausted",
        )?;
        records.push(json_parser::new(line.trim()).parse_record()?);
    }
    check(records.len() >= 8, "file-reply boot transcript incomplete")?;
    for (record, name) in records[..8].iter().zip(events) {
        require_string(record, name, "event", name)?;
    }
    validate_scheduler_boot_records(&records[..8])?;
    check(
        source.len() as u64 == plan[2] && linux_loader_entry_artifact::fnv64(source) == plan[3],
        "oracle source differs from retained plan",
    )?;
    check(
        !file.is_empty() && file.len() <= linux_loader_entry_artifact::file_source_limit,
        "oracle libc exceeds configured source budget",
    )?;
    let mut cursor = 8;
    let mut incarnations = Vec::new();
    for scenario in 0..8 {
        let ready = next(&records, &mut cursor, events[8], scenario)?;
        let incarnation = verify_ready(&ready, plan, file)?;
        check(
            !incarnations.contains(&incarnation),
            "service incarnation reused across clean lifetimes",
        )?;
        incarnations.push(incarnation);
        let flow = verify_flow(&records, &mut cursor, scenario, plan, source, file)?;
        let full_source = next(&records, &mut cursor, events[17], scenario)?;
        let source_attempts = verify_file_source(&full_source, file)?;
        for delivered in &flow.deliveries {
            let record = next(&records, &mut cursor, delivery_event, scenario)?;
            verify_delivery(&record, &flow, delivered, plan, file)?;
        }
        let service = next(&records, &mut cursor, service_event, scenario)?;
        verify_service(&service, &flow, scenario)?;
        let mapping = next(&records, &mut cursor, mapping_event, scenario)?;
        verify_mapping_cancel(&mapping, &flow, file, scenario)?;
        let mapping_lifetime = next(&records, &mut cursor, mapping_lifetime_event, scenario)?;
        verify_mapping_lifetime(&mapping_lifetime, &flow, file, scenario)?;
        let containment = next(&records, &mut cursor, containment_event, scenario)?;
        verify_containment(&containment, &flow, scenario, faults)?;
        let passed = next(&records, &mut cursor, events[18], scenario)?;
        verify_passed(&passed, &flow, source_attempts, scenario)?;
    }
    check(
        cursor == records.len(),
        "file-reply transcript has unexpected trailing records",
    )?;
    Ok(run_evidence {
        total_frames: required_number(&records[4], events[4], "total_frames")?,
        free_frames: required_number(&records[4], events[4], "free_frames")?,
        unavailable_frames: required_number(&records[4], events[4], "unavailable_frames")?,
    })
}

fn verify_ready(
    record: &structured_record,
    plan: &[u64; plan_word_count],
    file: &[u8],
) -> Result<u64, String> {
    let numbers = [
        ("tasks", 3),
        ("regions", 9),
        ("user_pages", linux_loader_entry_artifact::mapped_page_total),
        ("source_bytes", plan[2]),
        ("source_fnv64", plan[3]),
        ("file_source_bytes", file.len() as u64),
        (
            "file_source_fnv64",
            linux_loader_entry_artifact::fnv64(file),
        ),
        ("file_source_pages", file.len().div_ceil(4096) as u64),
        ("initial_rip", plan[4]),
        ("initial_rsp", plan[5]),
        ("main_entry", plan[12]),
        ("main_phdr", plan[13]),
        ("interpreter_bias", plan[14]),
        ("break_start", plan[15]),
    ];
    let identities = [
        "incarnation",
        "caller_task",
        "caller_generation",
        "veil_task",
        "veil_generation",
    ];
    let fields = numbers
        .iter()
        .map(|(key, _)| *key)
        .chain(identities)
        .chain(["source_kind", "policy", "cpu"])
        .collect::<Vec<_>>();
    require_control_header(record, events[8], &fields)?;
    for (key, value) in numbers {
        require_number(record, events[8], key, value)?;
    }
    for key in identities {
        check(
            required_number(record, events[8], key)? != 0,
            "ready contains an empty task/service identity",
        )?;
    }
    check(
        required_number(record, events[8], "caller_task")?
            != required_number(record, events[8], "veil_task")?,
        "caller and veil identities alias",
    )?;
    require_string(record, events[8], "source_kind", "privileged_fw_cfg")?;
    require_string(record, events[8], "policy", "userspace")?;
    check(
        ["GenuineIntel", "AuthenticAMD", "HygonGenuine"]
            .contains(&required_string(record, events[8], "cpu")?),
        "unqualified file-reply CPU vendor",
    )?;
    required_number(record, events[8], "incarnation")
}

fn path_shape(boundary: &structured_record) -> Result<bool, String> {
    let number = required_number(boundary, events[9], "number")?;
    if required_number(boundary, events[9], "arg0")? as u32 as i32 != -100 {
        return Ok(false);
    }
    Ok(match number {
        257 => matches!(
            required_number(boundary, events[9], "arg2")? as u32,
            0 | 0x80000
        ),
        262 => required_number(boundary, events[9], "arg3")? as u32 == 0,
        _ => false,
    })
}

/// The length of the one mmap the guest loader issues for the configured
/// library: that image's mapped extent. It is the single host-dependent
/// number in this oracle, so it is named here and used by the live check and
/// by both synthetic fixtures -- re-pinned with the hashes in
/// `linux_loader_entry_artifact`, and this value is libc6 2.39-0ubuntu8.9 on
/// the build host. The page span follows from it.
const library_mmap_length: u64 = 2_174_352;

fn mapping_boundary(flow: &flow) -> Result<&structured_record, String> {
    flow.boundaries
        .iter()
        .find(|boundary| required_number(boundary, events[9], "sequence").ok() == Some(11))
        .ok_or_else(|| "mapping lacks its sequence11 creation boundary".into())
}

fn verify_mapping_return(boundary: &structured_record) -> Result<(), String> {
    require_number(boundary, events[9], "sequence", 11)?;
    require_number(boundary, events[9], "number", 9)?;
    require_number(boundary, events[9], "result_bits", 0x1_0000_0000)?;
    require_bool(boundary, events[9], "returned_to_caller", true)?;
    for (index, expected) in [0, library_mmap_length, 1, 0x802, 3, 0]
        .into_iter()
        .enumerate()
    {
        require_number(boundary, events[9], &format!("arg{index}"), expected)?;
    }
    Ok(())
}

fn verify_flow(
    records: &[structured_record],
    cursor: &mut usize,
    scenario: u64,
    plan: &[u64; plan_word_count],
    source: &[u8],
    file: &[u8],
) -> Result<flow, String> {
    let mut flow = flow {
        boundaries: Vec::new(),
        open_sequence: 0,
        read_length: 0,
        deliveries: Vec::new(),
        copy_bytes: 0,
        scoped_misses: 0,
    };
    for sequence in 1..=boundary_limit as u64 {
        let boundary = next(records, cursor, events[9], scenario)?;
        let returned = boolean(&boundary, events[9], "returned_to_caller")?;
        boundary_header(&boundary, sequence, returned, plan, source)?;
        let number = required_number(&boundary, events[9], "number")?;
        let result = required_number(&boundary, events[9], "result_bits")?;
        let copied;
        if returned {
            check(
                !mapping_scenario(scenario) || sequence <= 11,
                "post-mapping observation executed another Linux syscall policy",
            )?;
            let reply = next(records, cursor, events[10], scenario)?;
            require_control_header(
                &reply,
                events[10],
                &[
                    "sequence",
                    "result_bits",
                    "frame_before_hex",
                    "frame_after_hex",
                    "committed",
                    "copy_sequence",
                    "copy_address",
                    "copy_bytes",
                    "copy_hex",
                ],
            )?;
            require_number(&reply, events[10], "sequence", sequence)?;
            require_number(&reply, events[10], "result_bits", result)?;
            require_bool(&reply, events[10], "committed", true)?;
            let before = reply_frame(&reply, "frame_before_hex")?;
            let after = reply_frame(&reply, "frame_after_hex")?;
            verify_frame_boundary(&before, &boundary)?;
            check(
                after[0] == result && before[1..] == after[1..],
                "file reply changed caller frame outside RAX",
            )?;
            copied = pathname(&reply, &boundary, plan, source, sequence)?;
            flow.copy_bytes += required_number(&reply, events[10], "copy_bytes")?;
            if sequence <= 3 {
                verify_boundary(&boundary, plan, source, sequence)?;
                verify_reply(&reply, &boundary, plan, source, sequence)?;
            } else if mapping_scenario(scenario) && sequence == 11 {
                check(
                    flow.deliveries.len() == 4
                        && flow.deliveries.last().map(|delivery| delivery.sequence) == Some(10)
                        && copied.is_empty(),
                    "first mmap lacks completed file evidence or copied an unexpected pathname",
                )?;
                verify_mapping_return(&boundary)?;
            } else if path_shape(&boundary)? {
                check(
                    !copied.is_empty(),
                    "path reply lacks complete captured pathname",
                )?;
                if number == 257 && copied == library_path {
                    check(
                        flow.open_sequence == 0 && result == 3,
                        "first libc open did not allocate reserved fd3",
                    )?;
                    flow.open_sequence = sequence;
                } else {
                    check(
                        scoped_missing(number, &copied) && result == (-2i64) as u64,
                        "ENOENT was not derived from the explicit complete namespace",
                    )?;
                    flow.scoped_misses += 1;
                }
            } else if matches!(number, 0 | 17 | 5) {
                check(
                    copied.is_empty(),
                    "file read unexpectedly captured a pathname",
                )?;
                append_delivery(&mut flow, &boundary, sequence, result, true, file)?;
            } else {
                return Err("unqualified syscall was returned to the Linux caller".into());
            }
        } else {
            check(
                sequence > 4 && sequence < boundary_limit as u64,
                "terminal before useful file progress or at observation exhaustion",
            )?;
            let terminal = next(records, cursor, events[16], scenario)?;
            require_control_header(
                &terminal,
                events[16],
                &[
                    "sequence",
                    "frame_before_hex",
                    "frame_after_hex",
                    "copy_sequence",
                    "copy_address",
                    "copy_bytes",
                    "copy_hex",
                    "copy_attempts",
                    "copy_successes",
                    "copy_rejections",
                    "pending_preserved",
                ],
            )?;
            require_number(&terminal, events[16], "sequence", sequence)?;
            require_bool(&terminal, events[16], "pending_preserved", true)?;
            let before = reply_frame(&terminal, "frame_before_hex")?;
            let after = reply_frame(&terminal, "frame_after_hex")?;
            verify_frame_boundary(&before, &boundary)?;
            check(
                before == after,
                "terminal operation changed its suspended caller frame",
            )?;
            copied = pathname(&terminal, &boundary, plan, source, sequence)?;
            let length = required_number(&terminal, events[16], "copy_bytes")?;
            flow.copy_bytes += length;
            require_number(&terminal, events[16], "copy_successes", length)?;
            require_number(&terminal, events[16], "copy_attempts", length)?;
            require_number(&terminal, events[16], "copy_rejections", 0)?;
            require_number(
                &boundary,
                events[9],
                "result_bits",
                if scenario == 0 { (-38i64) as u64 } else { 0 },
            )?;
            check(
                flow.open_sequence > 0 && flow.scoped_misses > 0,
                "file proof lacks actual scoped miss and successful open",
            )?;
            if staged_scenario(scenario) {
                check(
                    number == [0, 17, 5][(scenario as usize - 1) / 2] && copied.is_empty(),
                    "staged fault is not the selected pending file operation",
                )?;
                append_delivery(&mut flow, &boundary, sequence, 0, false, file)?;
            } else if mapping_scenario(scenario) {
                require_number(&boundary, events[9], "sequence", 12)?;
                verify_mapping_return(mapping_boundary(&flow)?)?;
                check(
                    copied.is_empty(),
                    "post-mapping observation processed a pathname before retirement",
                )?;
                // The next actual syscall is evidence, not a guessed policy.
                // Its typed register capture and unchanged frame were checked
                // above; even a normally supported syscall must retire here.
            } else {
                let delivered = flow
                    .deliveries
                    .last()
                    .ok_or("terminal lacks file delivery")?;
                check(
                    sequence == delivered.sequence + 1,
                    "accepted read did not reach exactly the next real caller boundary",
                )?;
                check(
                    copied.is_empty(),
                    "unknown local outcome processed the later request before containment",
                )?;
            }
            check(
                flow.deliveries.len()
                    == match scenario {
                        0 | 7 => 4,
                        1 | 2 => 1,
                        3 | 4 => 2,
                        _ => 3,
                    },
                "file proof lacks its required sequential, positional, metadata and repeated positional deliveries",
            )?;
            flow.boundaries.push(boundary);
            return Ok(flow);
        }
        flow.boundaries.push(boundary);
    }
    Err("file-reply boundary budget exhausted before terminal evidence".into())
}

fn append_delivery(
    flow: &mut flow,
    boundary: &structured_record,
    sequence: u64,
    result: u64,
    committed: bool,
    file: &[u8],
) -> Result<(), String> {
    let number = required_number(boundary, events[9], "number")?;
    check(
        flow.open_sequence != 0
            && flow.deliveries.len() < 4
            && number == [0, 17, 5, 17][flow.deliveries.len()],
        "file delivery lacks opened file or violates read/pread/metadata/pread observation order",
    )?;
    check(
        required_number(boundary, events[9], "arg0")? as u32 == 3,
        "file read used another descriptor",
    )?;
    let metadata = number == 5;
    let offset = if number != 17 {
        0
    } else {
        let offset = required_number(boundary, events[9], "arg3")?;
        check(
            offset <= i64::MAX as u64,
            "pread used a negative signed offset",
        )?;
        offset
    };
    let length = if metadata {
        144
    } else {
        required_number(boundary, events[9], "arg2")?
            .min(4096)
            .min((file.len() as u64).saturating_sub(offset))
    };
    check(
        length > 0 && (!committed || result == if metadata { 0 } else { length }),
        "file delivery has incorrect Linux result or no nonempty progress",
    )?;
    if number == 0 {
        flow.read_length = length as usize;
    } else {
        check(
            sequence == flow.deliveries.last().unwrap().sequence + 1,
            "file delivery did not follow the prior actual completed operation",
        )?;
    }
    flow.deliveries.push(delivery {
        sequence,
        length: length as usize,
        offset: offset as usize,
        committed,
        metadata,
    });
    Ok(())
}

fn verify_delivery(
    record: &structured_record,
    flow: &flow,
    delivered: &delivery,
    plan: &[u64; plan_word_count],
    file: &[u8],
) -> Result<(), String> {
    require_control_header(
        record,
        delivery_event,
        &[
            "sequence",
            "address",
            "length",
            "source_offset",
            "stage_token",
            "stage_bytes_hex",
            "before_hex",
            "after_hex",
            "committed",
            "stage_rejections",
            "completion_rejections",
            "source_reads",
            "source_bytes",
            "rejections_preserved",
            "scratch_cleared",
            "stage_cleared",
        ],
    )?;
    let read = &flow.boundaries[delivered.sequence as usize - 1];
    let address = required_number(read, events[9], "arg1")?;
    for (key, value) in [
        ("sequence", delivered.sequence),
        ("address", address),
        ("length", delivered.length as u64),
        ("source_offset", delivered.offset as u64),
        ("stage_token", delivered.sequence),
        ("stage_rejections", 8),
        ("completion_rejections", 4),
        ("source_reads", u64::from(!delivered.metadata)),
        (
            "source_bytes",
            if delivered.metadata {
                0
            } else {
                delivered.length as u64
            },
        ),
    ] {
        require_number(record, delivery_event, key, value)?;
    }
    mapped_extent(plan, address, delivered.length, true)?;
    let staged = decode_hex(record, delivery_event, "stage_bytes_hex", delivered.length)?;
    let before = decode_hex(record, delivery_event, "before_hex", delivered.length)?;
    let after = decode_hex(record, delivery_event, "after_hex", delivered.length)?;
    if delivered.metadata {
        verify_metadata(&staged, file.len())?;
    } else {
        check(
            staged == file[delivered.offset..delivered.offset + delivered.length],
            "staged bytes differ from the pinned libc source at the independently derived read offset",
        )?;
    }
    require_bool(record, delivery_event, "committed", delivered.committed)?;
    check(
        after == if delivered.committed { staged } else { before },
        "actual caller bytes disagree with the known publication outcome",
    )?;
    for key in ["rejections_preserved", "scratch_cleared", "stage_cleared"] {
        require_bool(record, delivery_event, key, true)?;
    }
    Ok(())
}

// Decode the documented synthetic userspace policy independently of the native
// encoder. Every byte of the Linux x86-64 stat layout, including padding, is
// checked; no host stat fields or production serialization helpers are used.
fn verify_metadata(bytes: &[u8], file_length: usize) -> Result<(), String> {
    check(
        bytes.len() == 144,
        "fstat metadata must cover all 144 bytes",
    )?;
    let word = |offset| u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap());
    let half = |offset| u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap());
    for (offset, expected, field) in [
        (0, 1, "device"),
        (8, 1, "inode"),
        (16, 1, "link count"),
        (40, 0, "special device"),
        (48, file_length as u64, "snapshot size"),
        (56, 4096, "block size hint"),
        (
            64,
            (file_length as u64).div_ceil(512),
            "512-byte block count",
        ),
        (72, 0, "access seconds"),
        (80, 0, "access nanoseconds"),
        (88, 0, "modification seconds"),
        (96, 0, "modification nanoseconds"),
        (104, 0, "change seconds"),
        (112, 0, "change nanoseconds"),
        (120, 0, "reserved word 0"),
        (128, 0, "reserved word 1"),
        (136, 0, "reserved word 2"),
    ] {
        check(
            word(offset) == expected,
            &format!("fstat metadata {field} differs from policy"),
        )?;
    }
    for (offset, expected, field) in [
        (24, 0o100444, "mode"),
        (28, 0, "uid"),
        (32, 0, "gid"),
        (36, 0, "padding"),
    ] {
        check(
            half(offset) == expected,
            &format!("fstat metadata {field} differs from policy"),
        )?;
    }
    Ok(())
}

fn verify_service(record: &structured_record, flow: &flow, scenario: u64) -> Result<(), String> {
    require_control_header(
        record,
        service_event,
        &[
            "active_request_sequence",
            "acknowledged_sequence",
            "service_last_sequence",
            "open_count",
            "fd",
            "handle_generation",
            "offset",
            "fenced",
            "report_hex",
        ],
    )?;
    let active = flow.raw_calls() - u64::from(unknown_scenario(scenario));
    let acknowledged = flow.raw_calls() - 1;
    let last = acknowledged - u64::from(unknown_scenario(scenario) || mapping_scenario(scenario));
    let count = if unknown_scenario(scenario) { 0 } else { 1 };
    let offset = if unknown_scenario(scenario) {
        u64::MAX
    } else if scenario == 1 {
        0
    } else {
        flow.read_length as u64
    };
    for (key, value) in [
        ("active_request_sequence", active),
        ("acknowledged_sequence", acknowledged),
        ("service_last_sequence", last),
        ("open_count", count),
        ("fd", 3),
        ("handle_generation", 1),
        ("offset", offset),
    ] {
        require_number(record, service_event, key, value)?;
    }
    require_bool(record, service_event, "fenced", unknown_scenario(scenario))?;
    let expected = [
        u64::from_le_bytes(*b"ANFSPR01"),
        scenario,
        active,
        acknowledged,
        last,
        count,
        3,
        1,
        offset,
        u64::from(unknown_scenario(scenario)),
    ]
    .into_iter()
    .flat_map(u64::to_le_bytes)
    .collect::<Vec<_>>();
    check(
        decode_hex(record, service_event, "report_hex", 80)? == expected,
        "userspace diagnostic differs from observed transaction and acknowledgement state",
    )
}

fn verify_mapping_cancel(
    record: &structured_record,
    flow: &flow,
    file: &[u8],
    scenario: u64,
) -> Result<(), String> {
    let numeric_fields = [
        "sequence",
        "preparations",
        "rejections",
        "base",
        "byte_length",
        "virtual_pages",
        "source_bytes",
        "source_fnv64",
        "destination_fnv64",
        "zero_bytes",
        "populated_pages",
        "absent_pages",
        "table_frames",
        "caller_tree_fnv64",
        "cleanup_retries",
    ];
    let boolean_fields = [
        "unpublished",
        "cancelled",
        "accounting_restored",
        "caller_preserved",
        "scratch_cleared",
    ];
    let fields = numeric_fields
        .into_iter()
        .chain(boolean_fields)
        .collect::<Vec<_>>();
    require_control_header(record, mapping_event, &fields)?;
    for field in boolean_fields {
        require_bool(
            record,
            mapping_event,
            field,
            field != "cancelled" || mapping_scenario(scenario),
        )?;
    }
    if !mapping_scenario(scenario) {
        for field in numeric_fields {
            require_number(record, mapping_event, field, 0)?;
        }
        return Ok(());
    }
    let boundary = mapping_boundary(flow)?;
    let sequence = required_number(boundary, events[9], "sequence")?;
    let byte_length = required_number(boundary, events[9], "arg1")?;
    let rounded = byte_length
        .checked_add(4095)
        .map(|length| length & !4095)
        .filter(|length| *length != 0 && *length <= 2048 * 4096)
        .ok_or("mapping request exceeds checked page budget")?;
    let source_bytes = (file.len() as u64).min(rounded);
    let populated_pages = source_bytes.div_ceil(4096);
    let virtual_pages = rounded / 4096;
    let source_fnv64 = linux_loader_entry_artifact::fnv64(&file[..source_bytes as usize]);
    // At this independently fixed, 1-GiB-aligned proof base, the detached tree
    // has one root, one PDPT and one PD plus one leaf table per populated 2MiB.
    // Absent virtual tail pages must not allocate backing or extra leaf tables.
    for (field, expected) in [
        ("sequence", sequence),
        ("preparations", 1),
        ("rejections", 9),
        ("base", 0x1_0000_0000),
        ("byte_length", byte_length),
        ("virtual_pages", virtual_pages),
        ("source_bytes", source_bytes),
        ("source_fnv64", source_fnv64),
        ("destination_fnv64", source_fnv64),
        ("zero_bytes", populated_pages * 4096 - source_bytes),
        ("populated_pages", populated_pages),
        ("absent_pages", virtual_pages - populated_pages),
        ("table_frames", 3 + populated_pages.div_ceil(512)),
        ("cleanup_retries", 0),
    ] {
        require_number(record, mapping_event, field, expected)?;
    }
    check(
        required_number(record, mapping_event, "caller_tree_fnv64")? != 0,
        "mapping caller_tree_fnv64 witness is empty",
    )
}

fn verify_mapping_lifetime(
    record: &structured_record,
    flow: &flow,
    file: &[u8],
    scenario: u64,
) -> Result<(), String> {
    let numeric_fields = [
        "sequence",
        "generation",
        "retentions",
        "publishes",
        "inspections",
        "releases",
        "rejections",
        "base",
        "byte_length",
        "virtual_pages",
        "populated_pages",
        "table_frames",
        "source_bytes",
        "source_fnv64",
        "destination_fnv64",
        "zero_bytes",
        "absent_pages",
        "next_sequence",
        "resume_rejections",
    ];
    let phase_fields = [
        "published",
        "withdrawn",
        "released",
        "hardware_admitted",
        "caller_resumed",
        "terminal_release",
    ];
    let preservation_fields = [
        "original_copy_preserved",
        "writes_refused",
        "tail_refused",
        "neighbors_refused",
        "accounting_restored",
        "circle_restored",
        "caller_preserved",
        "scratch_cleared",
    ];
    let fields = numeric_fields
        .into_iter()
        .chain(phase_fields)
        .chain(preservation_fields)
        .collect::<Vec<_>>();
    require_control_header(record, mapping_lifetime_event, &fields)?;
    for field in phase_fields {
        require_bool(
            record,
            mapping_lifetime_event,
            field,
            mapping_scenario(scenario),
        )?;
    }
    for field in preservation_fields {
        require_bool(record, mapping_lifetime_event, field, true)?;
    }
    if !mapping_scenario(scenario) {
        for field in numeric_fields {
            require_number(record, mapping_lifetime_event, field, 0)?;
        }
        return Ok(());
    }
    let boundary = mapping_boundary(flow)?;
    let sequence = required_number(boundary, events[9], "sequence")?;
    let byte_length = required_number(boundary, events[9], "arg1")?;
    let rounded = byte_length
        .checked_add(4095)
        .map(|length| length & !4095)
        .filter(|length| *length != 0 && *length <= 2048 * 4096)
        .ok_or("mapping lifetime request exceeds checked page budget")?;
    let source_bytes = (file.len() as u64).min(rounded);
    let populated_pages = source_bytes.div_ceil(4096);
    let virtual_pages = rounded / 4096;
    let source_fnv64 = linux_loader_entry_artifact::fnv64(&file[..source_bytes as usize]);
    // Derive the retained mapping's geometry and contents from the committed
    // creation request and the actual fixture, independently of cancellation and
    // lifetime counters. Publication must neither fill the absent tail nor
    // replace any of the caller's original copy regions.
    for (field, expected) in [
        ("sequence", sequence),
        ("generation", 1),
        ("retentions", 1),
        ("publishes", 1),
        ("inspections", 1),
        ("releases", 1),
        ("rejections", 11),
        ("base", 0x1_0000_0000),
        ("byte_length", byte_length),
        ("virtual_pages", virtual_pages),
        ("populated_pages", populated_pages),
        ("table_frames", 3 + populated_pages.div_ceil(512)),
        ("source_bytes", source_bytes),
        ("source_fnv64", source_fnv64),
        ("destination_fnv64", source_fnv64),
        ("zero_bytes", populated_pages * 4096 - source_bytes),
        ("absent_pages", virtual_pages - populated_pages),
        ("next_sequence", 12),
        ("resume_rejections", 7),
    ] {
        require_number(record, mapping_lifetime_event, field, expected)?;
    }
    Ok(())
}

fn verify_containment(
    record: &structured_record,
    flow: &flow,
    scenario: u64,
    faults: [u64; 2],
) -> Result<(), String> {
    require_control_header(
        record,
        containment_event,
        &[
            "fault_vector",
            "fault_instruction_pointer",
            "pending_sequence",
            "last_committed_sequence",
            "expected_fault",
            "peer_survived",
        ],
    )?;
    require_number(
        record,
        containment_event,
        "fault_vector",
        if scenario == 0 { 0 } else { 6 },
    )?;
    require_number(
        record,
        containment_event,
        "fault_instruction_pointer",
        if scenario == 0 {
            0
        } else {
            faults[if staged_scenario(scenario) { 0 } else { 1 }]
        },
    )?;
    require_number(
        record,
        containment_event,
        "pending_sequence",
        flow.raw_calls(),
    )?;
    require_number(
        record,
        containment_event,
        "last_committed_sequence",
        flow.raw_calls() - 1,
    )?;
    require_bool(record, containment_event, "expected_fault", scenario != 0)?;
    require_bool(record, containment_event, "peer_survived", true)
}

fn verify_passed(
    record: &structured_record,
    flow: &flow,
    source_attempts: u64,
    scenario: u64,
) -> Result<(), String> {
    let flags = [
        "copy_scratch_cleared",
        "copy_journal_cleared",
        "stage_cleared",
        "native_buffer_cleared",
        "source_cleared",
        "tls_cleared",
        "cpu_restored",
        "resources_restored",
        "request_cleared",
    ];
    let raw = flow.raw_calls();
    let deliveries = flow.deliveries.len() as u64;
    let source_reads = flow
        .deliveries
        .iter()
        .filter(|delivery| !delivery.metadata)
        .count() as u64;
    let queries = raw - 3 - u64::from(unknown_scenario(scenario))
        + source_reads
        + 3 * u64::from(mapping_scenario(scenario));
    let requests = raw - u64::from(unknown_scenario(scenario));
    let completions = raw - u64::from(scenario != 0);
    // Three startup/peer gates; one op16 per fetched invocation; one op20 per
    // return/retirement. Each delivery adds nine stage calls and four rejected
    // completions. Only data reads add a provider call and provider context;
    // metadata publication has no source access. Its first operation also probes
    // a bad destination and an unstaged completion before admitting a journal;
    // these two rejected gates do not belong to its later 8/4 journal counts.
    // The full source stream is separate. The normal mapping interval adds ten
    // cancellation gates, fourteen retained-mapping gates, seven rejected mapping
    // completions and three context rechecks already counted above. Its accepted
    // mmap completion and newly captured request belong to the ordinary counts.
    let metadata_probes = 2 * flow
        .deliveries
        .iter()
        .filter(|delivery| delivery.metadata)
        .count() as u64;
    let calls = 3
        + requests
        + completions
        + flow.copy_bytes
        + 18
        + source_attempts
        + queries
        + 13 * deliveries
        + source_reads
        + metadata_probes
        + 31 * u64::from(mapping_scenario(scenario));
    let numbers = [
        ("raw_calls", raw),
        ("caller_returns", raw - 1),
        ("native_calls", calls),
        ("retired_tasks", 3),
        ("copy_attempts", flow.copy_bytes + 18),
        ("copy_successes", flow.copy_bytes),
        ("copy_rejections", 18),
        ("context_queries", queries),
        ("allocator_service_borrows", 2),
        ("allocator_nested_denials", 1),
    ];
    let fields = numbers
        .iter()
        .map(|(key, _)| *key)
        .chain(flags)
        .collect::<Vec<_>>();
    require_control_header(record, events[18], &fields)?;
    for (key, value) in numbers {
        require_number(record, events[18], key, value)?;
    }
    for key in flags {
        require_bool(record, events[18], key, true)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    /// The page span of the library mapping, which only the synthetic
    /// fixtures below need: the live oracle reads the span from the record it
    /// is checking.
    const library_mmap_pages: u64 = super::library_mmap_length.div_ceil(4096);

    use super::*;

    const faults: [u64; 2] = [0x400500, 0x400502];
    const fn patterned_file() -> [u8; 8193] {
        let mut bytes = [0; 8193];
        let mut index = 0;
        while index < bytes.len() {
            bytes[index] = (index * 37 + index / 256) as u8;
            index += 1;
        }
        bytes
    }
    const test_file_source: &[u8] = &patterned_file();

    fn number(record: &mut structured_record, name: &str, value: u64) {
        record.insert(name.into(), json_scalar::number(value.to_string()));
    }

    fn string(record: &mut structured_record, name: &str, value: &str) {
        record.insert(name.into(), json_scalar::string(value.into()));
    }

    fn flag(record: &mut structured_record, name: &str, value: bool) {
        record.insert(name.into(), json_scalar::boolean(value));
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }

    fn frame_hex(frame: &[u64; 22]) -> String {
        hex(&frame
            .iter()
            .flat_map(|w| w.to_le_bytes())
            .collect::<Vec<_>>())
    }

    fn record(event: &str, scenario: u64) -> structured_record {
        let mut record = structured_record::new();
        number(&mut record, "schema", 1);
        number(&mut record, "scenario", scenario);
        string(&mut record, "event", event);
        string(&mut record, "level", "info");
        record
    }

    fn render(record: &structured_record) -> String {
        format!(
            "{{{}}}",
            record
                .iter()
                .map(|(key, value)| {
                    let value = match value {
                        json_scalar::number(value) => value.clone(),
                        json_scalar::string(value) => format!("{value:?}"),
                        json_scalar::boolean(value) => value.to_string(),
                        json_scalar::null => "null".into(),
                    };
                    format!("{key:?}:{value}")
                })
                .collect::<Vec<_>>()
                .join(",")
        )
    }

    fn transcript(records: &[structured_record]) -> String {
        records.iter().map(render).collect::<Vec<_>>().join("\n")
    }

    #[allow(clippy::too_many_arguments)]
    fn operation(
        records: &mut Vec<structured_record>,
        scenario: u64,
        sequence: u64,
        syscall: u64,
        arguments: [u64; 6],
        result: u64,
        returned: bool,
        path: Option<&[u8]>,
    ) {
        let mut boundary = record(events[9], scenario);
        for (key, value) in [
            ("sequence", sequence),
            ("number", syscall),
            ("instruction_pointer", 0x6000100f),
            ("syscall_instruction", 0x6000100d),
            ("stack_pointer", stack_end - 1200),
            ("rcx", 0x6000100f),
            ("r11", 582),
            ("fs_base", 0),
            ("result_bits", result),
        ] {
            number(&mut boundary, key, value);
        }
        for (index, value) in arguments.iter().enumerate() {
            number(&mut boundary, &format!("arg{index}"), *value);
        }
        flag(&mut boundary, "returned_to_caller", returned);
        flag(&mut boundary, "linux_exit_observed", false);
        flag(&mut boundary, "syscall_bytes_verified", true);
        records.push(boundary);
        let mut frame = core::array::from_fn(|index| 0xdead0000 + index as u64);
        for (index, value) in [
            (0, syscall),
            (2, 0x6000100f),
            (3, arguments[2]),
            (5, arguments[1]),
            (6, arguments[0]),
            (7, arguments[4]),
            (8, arguments[5]),
            (9, arguments[3]),
            (10, 582),
            (15, 0x100),
            (16, 0),
            (17, 0x6000100f),
            (18, 0x33),
            (19, 582),
            (20, stack_end - 1200),
            (21, 0x2b),
        ] {
            frame[index] = value;
        }
        let mut reply = record(if returned { events[10] } else { events[16] }, scenario);
        number(&mut reply, "sequence", sequence);
        string(&mut reply, "frame_before_hex", &frame_hex(&frame));
        if returned {
            frame[0] = result;
        }
        string(&mut reply, "frame_after_hex", &frame_hex(&frame));
        let path_bytes = path.map(|path| [path, &[0]].concat()).unwrap_or_default();
        number(
            &mut reply,
            "copy_sequence",
            if path.is_some() { sequence } else { 0 },
        );
        number(
            &mut reply,
            "copy_address",
            if path.is_some() { arguments[1] } else { 0 },
        );
        number(&mut reply, "copy_bytes", path_bytes.len() as u64);
        string(&mut reply, "copy_hex", &hex(&path_bytes));
        if returned {
            number(&mut reply, "result_bits", result);
            flag(&mut reply, "committed", true);
        } else {
            number(&mut reply, "copy_attempts", path_bytes.len() as u64);
            number(&mut reply, "copy_successes", path_bytes.len() as u64);
            number(&mut reply, "copy_rejections", 0);
            flag(&mut reply, "pending_preserved", true);
        }
        records.push(reply);
    }

    fn metadata_fixture(file_length: usize) -> [u8; 144] {
        let mut bytes = [0; 144];
        // Fixture values are deliberately independent of the production encoder.
        for (offset, value) in [
            (0, 1u64),
            (8, 1),
            (16, 1),
            (48, file_length as u64),
            (56, 4096),
            (64, (file_length as u64).div_ceil(512)),
        ] {
            bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
        }
        bytes[24..28].copy_from_slice(&0o100444u32.to_le_bytes());
        bytes
    }

    fn fixture(
        length: usize,
        reverse_misses: bool,
        trailing_directory_slash: bool,
    ) -> (Vec<structured_record>, [u64; plan_word_count], Vec<u8>) {
        fixture_with_pread(length, reverse_misses, trailing_directory_slash, 64, 840, 9)
    }

    fn fixture_with_pread(
        length: usize,
        reverse_misses: bool,
        trailing_directory_slash: bool,
        pread_offset: usize,
        pread_count: u64,
        mapping_number: u64,
    ) -> (Vec<structured_record>, [u64; plan_word_count], Vec<u8>) {
        let (old, plan, source) = super::super::tests::fixture();
        let file_hash = linux_loader_entry_artifact::fnv64(test_file_source);
        let pread_length = pread_count
            .min(4096)
            .min(test_file_source.len().saturating_sub(pread_offset) as u64)
            as usize;
        let old = old
            .lines()
            .map(|line| json_parser::new(line).parse_record().unwrap())
            .collect::<Vec<_>>();
        let mut records = old[..8].to_vec();
        let missing_file = b"/lib/x86_64-linux-gnu/glibc-hwcaps/variant/libc.so.6";
        let missing_directory: &[u8] = if trailing_directory_slash {
            b"/lib/x86_64-linux-gnu/glibc-hwcaps/variant/"
        } else {
            b"/lib/x86_64-linux-gnu/glibc-hwcaps/variant"
        };
        let copied = 36
            + missing_file.len() as u64
            + 1
            + missing_directory.len() as u64
            + 1
            + library_path.len() as u64
            + 1;
        for scenario in 0..8 {
            let mut ready = old[8].clone();
            for (key, value) in [
                ("scenario", scenario),
                ("incarnation", 401 + scenario),
                ("caller_task", 71),
                ("caller_generation", scenario + 1),
                ("veil_task", 73),
                ("veil_generation", scenario + 1),
            ] {
                number(&mut ready, key, value);
            }
            number(&mut ready, "file_source_fnv64", file_hash);
            records.push(ready);
            for existing in &old[9..15] {
                let mut value = existing.clone();
                number(&mut value, "scenario", scenario);
                records.push(value);
            }
            let path_address = stack_end - 512;
            let candidates: [(u64, &[u8]); 2] = if reverse_misses {
                [(262, missing_directory), (257, missing_file)]
            } else {
                [(257, missing_file), (262, missing_directory)]
            };
            for (index, (syscall, path)) in candidates.into_iter().enumerate() {
                operation(
                    &mut records,
                    scenario,
                    index as u64 + 4,
                    syscall,
                    [
                        0xffff_ff9c,
                        path_address,
                        if syscall == 257 {
                            0x80000
                        } else {
                            stack_end - 256
                        },
                        0,
                        17,
                        19,
                    ],
                    (-2i64) as u64,
                    true,
                    Some(path),
                );
            }
            operation(
                &mut records,
                scenario,
                6,
                257,
                [0xffff_ff9c, path_address, 0x80000, 0, 17, 19],
                3,
                true,
                Some(library_path),
            );
            operation(
                &mut records,
                scenario,
                7,
                0,
                [3, stack_end - 8192, length as u64, 0, 17, 19],
                if scenario == 1 { 0 } else { length as u64 },
                scenario != 1,
                None,
            );
            if scenario != 1 {
                operation(
                    &mut records,
                    scenario,
                    8,
                    17,
                    [
                        3,
                        stack_end - 4096,
                        pread_count,
                        pread_offset as u64,
                        17,
                        19,
                    ],
                    if scenario == 2 || scenario == 3 {
                        0
                    } else {
                        pread_length as u64
                    },
                    scenario != 2 && scenario != 3,
                    None,
                );
            }
            if matches!(scenario, 0 | 4 | 5 | 6 | 7) {
                operation(
                    &mut records,
                    scenario,
                    9,
                    5,
                    [3, stack_end - 1024, 0xfeed, 0xbeef, 17, 19],
                    0,
                    mapping_scenario(scenario) || scenario == 6,
                    None,
                );
            }
            if mapping_scenario(scenario) || scenario == 6 {
                operation(
                    &mut records,
                    scenario,
                    10,
                    17,
                    [
                        3,
                        stack_end - 12288,
                        pread_count,
                        pread_offset as u64,
                        17,
                        19,
                    ],
                    if mapping_scenario(scenario) {
                        pread_length as u64
                    } else {
                        0
                    },
                    mapping_scenario(scenario),
                    None,
                );
            }
            if mapping_scenario(scenario) {
                // Only syscall9 with the observed full mmap shape may return.
                // The parameter retains alternate syscalls for negative tests.
                operation(
                    &mut records,
                    scenario,
                    11,
                    mapping_number,
                    [0, library_mmap_length, 1, 0x802, 3, 0],
                    0x1_0000_0000,
                    true,
                    None,
                );
                // Opaque synthetic next request, deliberately not a prediction
                // of the next syscall produced by the physical interpreter.
                operation(
                    &mut records,
                    scenario,
                    12,
                    u64::MAX,
                    [91, 92, 93, 94, 95, 96],
                    if scenario == 0 { (-38i64) as u64 } else { 0 },
                    false,
                    None,
                );
            }
            let mut full_source = old[17].clone();
            number(&mut full_source, "scenario", scenario);
            for field in ["source_fnv64", "delivered_fnv64", "guest_fnv64"] {
                number(&mut full_source, field, file_hash);
            }
            records.push(full_source);
            let delivery_count = match scenario {
                0 | 7 => 4,
                1 | 2 => 1,
                3 | 4 => 2,
                _ => 3,
            };
            let metadata = metadata_fixture(test_file_source.len());
            for index in 0..delivery_count {
                let (address, delivered_length, offset) = if index == 0 {
                    (stack_end - 8192, length, 0)
                } else if index == 1 {
                    (stack_end - 4096, pread_length, pread_offset)
                } else if index == 2 {
                    (stack_end - 1024, 144, 0)
                } else {
                    (stack_end - 12288, pread_length, pread_offset)
                };
                let committed = !(staged_scenario(scenario) && index == delivery_count - 1);
                let mut delivery = record(delivery_event, scenario);
                for (key, value) in [
                    ("sequence", 7 + index),
                    ("address", address),
                    ("length", delivered_length as u64),
                    ("source_offset", offset as u64),
                    ("stage_token", 7 + index),
                    ("stage_rejections", 8),
                    ("completion_rejections", 4),
                    ("source_reads", u64::from(index != 2)),
                    (
                        "source_bytes",
                        if index == 2 {
                            0
                        } else {
                            delivered_length as u64
                        },
                    ),
                ] {
                    number(&mut delivery, key, value);
                }
                let staged = if index == 2 {
                    &metadata[..]
                } else {
                    &test_file_source[offset..offset + delivered_length]
                };
                string(&mut delivery, "stage_bytes_hex", &hex(staged));
                string(
                    &mut delivery,
                    "before_hex",
                    &hex(&vec![0xa5; delivered_length]),
                );
                string(
                    &mut delivery,
                    "after_hex",
                    &if committed {
                        hex(staged)
                    } else {
                        hex(&vec![0xa5; delivered_length])
                    },
                );
                flag(&mut delivery, "committed", committed);
                for name in ["rejections_preserved", "scratch_cleared", "stage_cleared"] {
                    flag(&mut delivery, name, true);
                }
                records.push(delivery);
            }
            let mut service = record(service_event, scenario);
            let raw = [12, 7, 8, 8, 9, 9, 10, 12][scenario as usize];
            let active = raw - u64::from(unknown_scenario(scenario));
            let ack = raw - 1;
            let last = ack - u64::from(unknown_scenario(scenario) || mapping_scenario(scenario));
            let count = if unknown_scenario(scenario) { 0 } else { 1 };
            let offset = if unknown_scenario(scenario) {
                u64::MAX
            } else if scenario == 1 {
                0
            } else {
                length as u64
            };
            for (key, value) in [
                ("active_request_sequence", active),
                ("acknowledged_sequence", ack),
                ("service_last_sequence", last),
                ("open_count", count),
                ("fd", 3),
                ("handle_generation", 1),
                ("offset", offset),
            ] {
                number(&mut service, key, value);
            }
            flag(&mut service, "fenced", unknown_scenario(scenario));
            let report = [
                u64::from_le_bytes(*b"ANFSPR01"),
                scenario,
                active,
                ack,
                last,
                count,
                3,
                1,
                offset,
                u64::from(unknown_scenario(scenario)),
            ]
            .into_iter()
            .flat_map(u64::to_le_bytes)
            .collect::<Vec<_>>();
            string(&mut service, "report_hex", &hex(&report));
            records.push(service);
            let mut mapping = record(mapping_event, scenario);
            // These fixture values do not call the production mapping decoder
            // or oracle. The 8193-byte patterned file occupies three pages;
            // the observed mmap requests the library's page span and leaves
            // the rest of it as an absent tail.
            for (field, value) in [
                ("sequence", 11),
                ("preparations", 1),
                ("rejections", 9),
                ("base", 0x1_0000_0000),
                ("byte_length", library_mmap_length),
                ("virtual_pages", library_mmap_pages),
                ("source_bytes", 8193),
                ("source_fnv64", file_hash),
                ("destination_fnv64", file_hash),
                ("zero_bytes", 4095),
                ("populated_pages", 3),
                ("absent_pages", library_mmap_pages - 3),
                ("table_frames", 4),
                ("caller_tree_fnv64", 0x1234_5678_90ab_cdef),
                ("cleanup_retries", 0),
            ] {
                number(
                    &mut mapping,
                    field,
                    if mapping_scenario(scenario) { value } else { 0 },
                );
            }
            flag(&mut mapping, "cancelled", mapping_scenario(scenario));
            for field in [
                "unpublished",
                "accounting_restored",
                "caller_preserved",
                "scratch_cleared",
            ] {
                flag(&mut mapping, field, true);
            }
            records.push(mapping);
            let mut mapping_lifetime = record(mapping_lifetime_event, scenario);
            // Independent literal geometry for the same 8193-byte fixture;
            // the retained tree must preserve the complete reservation.
            for (field, value) in [
                ("sequence", 11),
                ("generation", 1),
                ("retentions", 1),
                ("publishes", 1),
                ("inspections", 1),
                ("releases", 1),
                ("rejections", 11),
                ("base", 0x1_0000_0000),
                ("byte_length", library_mmap_length),
                ("virtual_pages", library_mmap_pages),
                ("populated_pages", 3),
                ("table_frames", 4),
                ("source_bytes", 8193),
                ("source_fnv64", file_hash),
                ("destination_fnv64", file_hash),
                ("zero_bytes", 4095),
                ("absent_pages", library_mmap_pages - 3),
                ("next_sequence", 12),
                ("resume_rejections", 7),
            ] {
                number(
                    &mut mapping_lifetime,
                    field,
                    if mapping_scenario(scenario) { value } else { 0 },
                );
            }
            for field in [
                "published",
                "withdrawn",
                "released",
                "hardware_admitted",
                "caller_resumed",
                "terminal_release",
            ] {
                flag(&mut mapping_lifetime, field, mapping_scenario(scenario));
            }
            for field in [
                "original_copy_preserved",
                "writes_refused",
                "tail_refused",
                "neighbors_refused",
                "accounting_restored",
                "circle_restored",
                "caller_preserved",
                "scratch_cleared",
            ] {
                flag(&mut mapping_lifetime, field, true);
            }
            records.push(mapping_lifetime);
            let mut containment = record(containment_event, scenario);
            for (key, value) in [
                ("fault_vector", if scenario == 0 { 0 } else { 6 }),
                (
                    "fault_instruction_pointer",
                    if scenario == 0 {
                        0
                    } else {
                        faults[if staged_scenario(scenario) { 0 } else { 1 }]
                    },
                ),
                ("pending_sequence", raw),
                ("last_committed_sequence", raw - 1),
            ] {
                number(&mut containment, key, value);
            }
            flag(&mut containment, "expected_fault", scenario != 0);
            flag(&mut containment, "peer_survived", true);
            records.push(containment);
            let mut passed = record(events[18], scenario);
            for (key, value) in [
                ("raw_calls", raw),
                ("caller_returns", raw - 1),
                (
                    "native_calls",
                    [161, 66, 67, 84, 85, 102, 103, 160][scenario as usize] + copied,
                ),
                ("retired_tasks", 3),
                ("copy_attempts", copied + 18),
                ("copy_successes", copied),
                ("copy_rejections", 18),
                (
                    "context_queries",
                    [15, 5, 5, 7, 7, 8, 8, 15][scenario as usize],
                ),
                ("allocator_service_borrows", 2),
                ("allocator_nested_denials", 1),
            ] {
                number(&mut passed, key, value);
            }
            for name in [
                "copy_scratch_cleared",
                "copy_journal_cleared",
                "stage_cleared",
                "native_buffer_cleared",
                "source_cleared",
                "tls_cleared",
                "cpu_restored",
                "resources_restored",
                "request_cleared",
            ] {
                flag(&mut passed, name, true);
            }
            records.push(passed);
        }
        (records, plan, source)
    }

    fn verify_fixture(
        records: &[structured_record],
        plan: &[u64; plan_word_count],
        source: &[u8],
    ) -> Result<run_evidence, String> {
        verify(&transcript(records), plan, source, test_file_source, faults)
    }

    #[test]
    fn qualifies_eight_lifetimes_with_actual_read_sizes_and_variable_missing_path_order() {
        for length in [1, 17, 832, 4096] {
            for reverse in [false, true] {
                for trailing_slash in [false, true] {
                    let (records, plan, source) = fixture(length, reverse, trailing_slash);
                    assert_eq!(records.len(), 234);
                    verify_fixture(&records, &plan, &source).unwrap();
                }
            }
        }
    }

    #[test]
    fn requires_exact_allocator_borrow_evidence_in_every_lifetime() {
        let (records, plan, source) = fixture(17, false, true);
        verify_fixture(&records, &plan, &source).unwrap();
        for scenario in 0..8 {
            let index = records
                .iter()
                .position(|record| {
                    event(record).unwrap() == events[18]
                        && required_number(record, events[18], "scenario").unwrap() == scenario
                })
                .unwrap();
            for (field, expected) in [
                ("allocator_service_borrows", 2),
                ("allocator_nested_denials", 1),
            ] {
                let mutations = [
                    None,
                    Some(json_scalar::number("0".into())),
                    Some(json_scalar::number((expected - 1u64).to_string())),
                    Some(json_scalar::number((expected + 1u64).to_string())),
                    Some(json_scalar::number(u64::MAX.to_string())),
                    Some(json_scalar::string(expected.to_string())),
                    Some(json_scalar::boolean(true)),
                    Some(json_scalar::boolean(false)),
                    Some(json_scalar::null),
                ];
                for (mutation, value) in mutations.into_iter().enumerate() {
                    let mut changed = records.clone();
                    if let Some(value) = value {
                        changed[index].insert(field.into(), value);
                    } else {
                        changed[index].remove(field);
                    }
                    let reason = verify_fixture(&changed, &plan, &source)
                        .err()
                        .expect("invalid allocator lifetime evidence qualified");
                    assert!(
                        reason.contains(field),
                        "scenario{scenario}:{field}:mutation{mutation}:{reason}"
                    );
                }
            }
        }
    }

    #[test]
    fn requires_typed_mapping_cancellation_evidence_in_every_lifetime() {
        let (records, plan, source) = fixture(17, false, true);
        verify_fixture(&records, &plan, &source).unwrap();
        for scenario in 0..8 {
            let index = records
                .iter()
                .position(|record| {
                    event(record).unwrap() == mapping_event
                        && required_number(record, mapping_event, "scenario").unwrap() == scenario
                })
                .unwrap();
            for (field, expected) in &records[index] {
                let mutations = match expected {
                    json_scalar::number(value) => {
                        let value = value.parse::<u64>().unwrap();
                        let incorrect =
                            if field == "caller_tree_fnv64" && mapping_scenario(scenario) {
                                0
                            } else {
                                value ^ 1
                            };
                        vec![
                            None,
                            Some(json_scalar::number(incorrect.to_string())),
                            Some(json_scalar::string(value.to_string())),
                            Some(json_scalar::boolean(value != 0)),
                            Some(json_scalar::null),
                        ]
                    }
                    json_scalar::boolean(value) => vec![
                        None,
                        Some(json_scalar::boolean(!value)),
                        Some(json_scalar::number(u64::from(*value).to_string())),
                        Some(json_scalar::string(value.to_string())),
                        Some(json_scalar::null),
                    ],
                    _ => continue,
                };
                for value in mutations {
                    let mut changed = records.clone();
                    if let Some(value) = value {
                        changed[index].insert(field.clone(), value);
                    } else {
                        changed[index].remove(field);
                    }
                    assert!(
                        verify_fixture(&changed, &plan, &source).is_err(),
                        "scenario{scenario}:{field}"
                    );
                }
            }
        }
    }

    #[test]
    fn requires_typed_mapping_lifetime_evidence_in_every_lifetime() {
        let (records, plan, source) = fixture(17, false, true);
        verify_fixture(&records, &plan, &source).unwrap();
        for scenario in 0..8 {
            let index = records
                .iter()
                .position(|record| {
                    event(record).unwrap() == mapping_lifetime_event
                        && required_number(record, mapping_lifetime_event, "scenario").unwrap()
                            == scenario
                })
                .unwrap();
            for (field, expected) in &records[index] {
                let mutations = match expected {
                    json_scalar::number(value) => {
                        let value = value.parse::<u64>().unwrap();
                        vec![
                            None,
                            Some(json_scalar::number((value ^ 1).to_string())),
                            Some(json_scalar::string(value.to_string())),
                            Some(json_scalar::boolean(value != 0)),
                            Some(json_scalar::null),
                        ]
                    }
                    json_scalar::boolean(value) => vec![
                        None,
                        Some(json_scalar::boolean(!value)),
                        Some(json_scalar::number(u64::from(*value).to_string())),
                        Some(json_scalar::string(value.to_string())),
                        Some(json_scalar::null),
                    ],
                    _ => continue,
                };
                for value in mutations {
                    let mut changed = records.clone();
                    if let Some(value) = value {
                        changed[index].insert(field.clone(), value);
                    } else {
                        changed[index].remove(field);
                    }
                    assert!(
                        verify_fixture(&changed, &plan, &source).is_err(),
                        "scenario{scenario}:{field}"
                    );
                }
            }
        }
    }

    #[test]
    fn rejects_mapping_lifetime_replay_and_self_consistent_substitutions() {
        let (records, plan, source) = fixture(17, false, true);
        let index = records
            .iter()
            .position(|record| event(record).unwrap() == mapping_lifetime_event)
            .unwrap();
        for (field, value) in [
            ("sequence", 12),
            ("generation", 2),
            ("retentions", 2),
            ("publishes", 2),
            ("inspections", 2),
            ("releases", 2),
            ("rejections", 10),
            ("next_sequence", 11),
            ("resume_rejections", 5),
            ("base", 0x6000_0000),
            ("byte_length", 545 * 4096),
            ("table_frames", 5),
        ] {
            let mut changed = records.clone();
            number(&mut changed[index], field, value);
            let reason = verify_fixture(&changed, &plan, &source)
                .err()
                .expect("substituted mapping lifetime evidence qualified");
            assert!(reason.contains(field), "{field}:{value}:{reason}");
        }
        let mut changed = records.clone();
        let hash =
            required_number(&changed[index], mapping_lifetime_event, "source_fnv64").unwrap() ^ 1;
        number(&mut changed[index], "source_fnv64", hash);
        number(&mut changed[index], "destination_fnv64", hash);
        assert!(verify_fixture(&changed, &plan, &source).is_err());
        // Matching source/destination hashes do not authenticate a shortened
        // source, and internally consistent page counts do not authenticate a
        // shortened reservation or a newly populated tail.
        for substitution in [
            vec![
                ("source_bytes", 8192),
                (
                    "source_fnv64",
                    linux_loader_entry_artifact::fnv64(&test_file_source[..8192]),
                ),
                (
                    "destination_fnv64",
                    linux_loader_entry_artifact::fnv64(&test_file_source[..8192]),
                ),
                ("populated_pages", 2),
                ("zero_bytes", 0),
                ("absent_pages", 543),
            ],
            vec![
                ("byte_length", 8193),
                ("virtual_pages", 3),
                ("absent_pages", 0),
            ],
            vec![
                ("populated_pages", 545),
                ("zero_bytes", 545 * 4096 - 8193),
                ("absent_pages", 0),
                ("table_frames", 5),
            ],
        ] {
            let mut changed = records.clone();
            for (field, value) in substitution {
                number(&mut changed[index], field, value);
            }
            assert!(verify_fixture(&changed, &plan, &source).is_err());
        }
    }

    #[test]
    fn mapping_lifetime_requires_one_record_between_cancellation_and_containment() {
        let (records, plan, source) = fixture(17, false, true);
        for (index, record) in records.iter().enumerate() {
            if event(record).unwrap() != mapping_lifetime_event {
                continue;
            }
            assert_eq!(event(&records[index - 1]).unwrap(), mapping_event);
            assert_eq!(event(&records[index + 1]).unwrap(), containment_event);
            let mut changed = records.clone();
            changed.remove(index);
            assert!(verify_fixture(&changed, &plan, &source).is_err());
            let mut changed = records.clone();
            changed.insert(index, record.clone());
            assert!(verify_fixture(&changed, &plan, &source).is_err());
            for neighbor in [index - 1, index + 1] {
                let mut changed = records.clone();
                changed.swap(index, neighbor);
                assert!(verify_fixture(&changed, &plan, &source).is_err());
            }
        }
    }

    #[test]
    fn rejects_mapping_geometry_hash_replay_and_cleanup_substitutions() {
        let (records, plan, source) = fixture(17, false, true);
        let index = records
            .iter()
            .position(|record| event(record).unwrap() == mapping_event)
            .unwrap();
        for (field, value) in [
            ("preparations", 0),
            ("preparations", 2),
            ("rejections", 0),
            ("rejections", 8),
            ("rejections", 10),
            ("sequence", 10),
            ("base", 0x6000_0000),
            ("byte_length", 545 * 4096),
            ("virtual_pages", 3),
            ("source_bytes", 8192),
            ("source_fnv64", 0),
            ("destination_fnv64", 0),
            ("zero_bytes", 0),
            ("populated_pages", 545),
            ("absent_pages", 0),
            ("table_frames", 5),
            ("caller_tree_fnv64", 0),
            ("cleanup_retries", 1),
        ] {
            let mut changed = records.clone();
            number(&mut changed[index], field, value);
            let reason = verify_fixture(&changed, &plan, &source)
                .err()
                .expect("substituted mapping evidence qualified");
            assert!(reason.contains(field), "{field}:{value}:{reason}");
        }
        let mut changed = records.clone();
        let hash = required_number(&changed[index], mapping_event, "source_fnv64").unwrap() ^ 1;
        number(&mut changed[index], "source_fnv64", hash);
        number(&mut changed[index], "destination_fnv64", hash);
        assert!(verify_fixture(&changed, &plan, &source).is_err());
        // A self-consistent but shorter mapping cannot replace the complete
        // reservation requested by the committed creation invocation.
        let mut changed = records.clone();
        for (field, value) in [
            ("byte_length", 8193),
            ("virtual_pages", 3),
            ("absent_pages", 0),
        ] {
            number(&mut changed[index], field, value);
        }
        assert!(verify_fixture(&changed, &plan, &source).is_err());
    }

    #[test]
    fn first_mapping_requires_exact_shape_and_its_successful_linux_return() {
        let (records, plan, source) = fixture(17, false, true);
        let index = records
            .iter()
            .position(|record| {
                event(record).unwrap() == events[9]
                    && required_number(record, events[9], "scenario").unwrap() == 0
                    && required_number(record, events[9], "sequence").unwrap() == 11
            })
            .unwrap();
        let arguments = [0, 2_231_696, 1, 0x802, 3, 0];
        for (argument, value) in [
            (0, 0x1_0000_0000),
            (1, 0),
            (1, 2_231_697),
            (1, u64::MAX),
            (2, 3),
            (2, (1 << 32) | 1),
            (3, 2),
            (3, 0x812),
            (4, 4),
            (4, (1 << 32) | 3),
            (5, 4096),
        ] {
            let mut altered = arguments;
            altered[argument] = value;
            let mut operation_records = Vec::new();
            operation(
                &mut operation_records,
                0,
                11,
                9,
                altered,
                0x1_0000_0000,
                true,
                None,
            );
            let mut changed = records.clone();
            changed[index..index + 2].clone_from_slice(&operation_records);
            assert!(
                verify_fixture(&changed, &plan, &source).is_err(),
                "arg{argument}"
            );
        }
        for syscall in [10, 1024] {
            let (changed, plan, source) = fixture_with_pread(17, false, true, 64, 840, syscall);
            assert!(verify_fixture(&changed, &plan, &source).is_err());
        }
        for (result, returned) in [
            (0, true),
            (0x1_0000_1000, true),
            ((-38i64) as u64, true),
            (u64::MAX, true),
            ((-38i64) as u64, false),
        ] {
            let mut operation_records = Vec::new();
            operation(
                &mut operation_records,
                0,
                11,
                9,
                arguments,
                result,
                returned,
                None,
            );
            let mut changed = records.clone();
            changed[index..index + 2].clone_from_slice(&operation_records);
            assert!(
                verify_fixture(&changed, &plan, &source).is_err(),
                "result{result}:returned{returned}"
            );
        }
    }

    #[test]
    fn mapping_hash_covers_only_the_populated_prefix_of_a_longer_source() {
        let (records, _, _) = fixture(17, false, true);
        let mut mapping = records
            .iter()
            .find(|record| event(record).unwrap() == mapping_event)
            .unwrap()
            .clone();
        mapping.remove("scenario");
        let terminal = records
            .iter()
            .find(|record| {
                event(record).unwrap() == events[9]
                    && required_number(record, events[9], "scenario").unwrap() == 0
                    && required_number(record, events[9], "sequence").unwrap() == 11
            })
            .unwrap()
            .clone();
        let flow = flow {
            boundaries: vec![terminal],
            open_sequence: 0,
            read_length: 0,
            deliveries: Vec::new(),
            copy_bytes: 0,
            scoped_misses: 0,
        };
        // A source one byte longer than the mapping's whole extent, so the
        // hash is required to cover the extent and stop. The extent follows
        // the library's own page span rather than repeating it.
        let rounded = library_mmap_pages as usize * 4096;
        let mut file = vec![0x5a; rounded + 1];
        file[rounded] = 0xa5;
        let prefix_hash = linux_loader_entry_artifact::fnv64(&file[..rounded]);
        for (field, value) in [
            ("source_bytes", rounded as u64),
            ("source_fnv64", prefix_hash),
            ("destination_fnv64", prefix_hash),
            ("zero_bytes", 0),
            ("populated_pages", library_mmap_pages),
            ("absent_pages", 0),
            ("table_frames", 5),
        ] {
            number(&mut mapping, field, value);
        }
        verify_mapping_cancel(&mapping, &flow, &file, 0).unwrap();
        let whole_hash = linux_loader_entry_artifact::fnv64(&file);
        assert_ne!(whole_hash, prefix_hash);
        number(&mut mapping, "source_fnv64", whole_hash);
        number(&mut mapping, "destination_fnv64", whole_hash);
        assert!(verify_mapping_cancel(&mapping, &flow, &file, 0).is_err());
    }

    #[test]
    fn derives_positional_lengths_and_offsets_before_exact_first_mmap() {
        for (offset, count) in [(0, 1), (64, 840), (8192, 512), (7000, u64::MAX)] {
            let (records, plan, source) = fixture_with_pread(832, false, true, offset, count, 9);
            verify_fixture(&records, &plan, &source).unwrap();
        }
    }

    #[test]
    fn rejects_non_mapping_syscall_as_the_mapping_return() {
        for syscall in [0, 17, 5, 257, 262] {
            let (records, plan, source) = fixture_with_pread(832, false, true, 64, 840, syscall);
            let reason = verify_fixture(&records, &plan, &source)
                .err()
                .expect("non-mapping operation became the mapping return");
            assert!(reason.contains("number"), "{reason}");
        }
    }

    #[test]
    fn next_request_is_typed_and_preserved_without_guessing_its_syscall_policy() {
        let (records, plan, source) = fixture(832, false, true);
        let index = records
            .iter()
            .position(|record| {
                event(record).unwrap() == events[9]
                    && required_number(record, events[9], "scenario").unwrap() == 0
                    && required_number(record, events[9], "sequence").unwrap() == 12
            })
            .unwrap();
        for syscall in [0, 5, 9, 17, 21, 257, 262, u64::MAX] {
            let mut operation_records = Vec::new();
            operation(
                &mut operation_records,
                0,
                12,
                syscall,
                [0xffff_ff9c, stack_end - 512, 0, 0, 0, u64::MAX],
                (-38i64) as u64,
                false,
                None,
            );
            let mut changed = records.clone();
            changed[index..index + 2].clone_from_slice(&operation_records);
            verify_fixture(&changed, &plan, &source).unwrap();
            // Even a self-consistent reply at this boundary is beyond scope.
            let mut operation_records = Vec::new();
            operation(
                &mut operation_records,
                0,
                12,
                syscall,
                [0xffff_ff9c, stack_end - 512, 0, 0, 0, u64::MAX],
                0,
                true,
                None,
            );
            changed[index..index + 2].clone_from_slice(&operation_records);
            let reason = verify_fixture(&changed, &plan, &source)
                .err()
                .expect("post-mapping request unexpectedly returned");
            assert!(reason.contains("another Linux syscall policy"), "{reason}");
        }
        for field in ["number", "arg0", "arg1", "arg2", "arg3", "arg4", "arg5"] {
            for replacement in [None, Some(json_scalar::string("0".into()))] {
                let mut changed = records.clone();
                if let Some(replacement) = replacement {
                    changed[index].insert(field.into(), replacement);
                } else {
                    changed[index].remove(field);
                }
                assert!(verify_fixture(&changed, &plan, &source).is_err());
            }
        }
        let mut changed = records.clone();
        let mut frame = reply_frame(&changed[index + 1], "frame_after_hex").unwrap();
        frame[0] ^= 1;
        string(
            &mut changed[index + 1],
            "frame_after_hex",
            &frame_hex(&frame),
        );
        assert!(verify_fixture(&changed, &plan, &source).is_err());
    }

    #[test]
    fn creation_and_terminal_sequences_cannot_be_swapped_reused_or_omitted() {
        let (records, plan, source) = fixture(832, false, true);
        let index = records
            .iter()
            .position(|record| {
                event(record).unwrap() == events[9]
                    && required_number(record, events[9], "scenario").unwrap() == 0
                    && required_number(record, events[9], "sequence").unwrap() == 11
            })
            .unwrap();
        let mut changed = records.clone();
        changed[index..index + 4].rotate_left(2);
        assert!(verify_fixture(&changed, &plan, &source).is_err());
        for (offset, sequence) in [(0, 12), (2, 11), (2, 13)] {
            let mut changed = records.clone();
            for part in 0..2 {
                number(&mut changed[index + offset + part], "sequence", sequence);
            }
            assert!(verify_fixture(&changed, &plan, &source).is_err());
        }
        for offset in [0, 2] {
            let mut changed = records.clone();
            drop(changed.drain(index + offset..index + offset + 2));
            assert!(verify_fixture(&changed, &plan, &source).is_err());
        }
    }

    #[test]
    fn resumed_lifetime_requires_its_new_request_return_and_native_counts() {
        let (records, plan, source) = fixture(832, false, true);
        for scenario in [0, 7] {
            let index = records
                .iter()
                .position(|record| {
                    event(record).unwrap() == events[18]
                        && required_number(record, events[18], "scenario").unwrap() == scenario
                })
                .unwrap();
            for field in [
                "raw_calls",
                "caller_returns",
                "native_calls",
                "context_queries",
            ] {
                let expected = required_number(&records[index], events[18], field).unwrap();
                for value in [expected - 1, expected + 1] {
                    let mut changed = records.clone();
                    number(&mut changed[index], field, value);
                    let reason = verify_fixture(&changed, &plan, &source)
                        .err()
                        .expect("changed resume accounting qualified");
                    assert!(reason.contains(field), "{field}:{value}:{reason}");
                }
            }
        }
    }

    #[test]
    fn mapping_acknowledgement_does_not_advance_the_file_watermark() {
        let (records, plan, source) = fixture(832, false, true);
        for scenario in [0, 7] {
            let index = records
                .iter()
                .position(|record| {
                    event(record).unwrap() == service_event
                        && required_number(record, service_event, "scenario").unwrap() == scenario
                })
                .unwrap();
            for (field, word, value) in [
                ("active_request_sequence", 2, 11),
                ("acknowledged_sequence", 3, 10),
                ("acknowledged_sequence", 3, 12),
                ("service_last_sequence", 4, 11),
                ("service_last_sequence", 4, 9),
                ("open_count", 5, 0),
                ("offset", 8, u64::MAX),
            ] {
                let mut changed = records.clone();
                number(&mut changed[index], field, value);
                let mut report =
                    decode_hex(&changed[index], service_event, "report_hex", 80).unwrap();
                report[word * 8..word * 8 + 8].copy_from_slice(&value.to_le_bytes());
                string(&mut changed[index], "report_hex", &hex(&report));
                let reason = verify_fixture(&changed, &plan, &source)
                    .err()
                    .expect("stale acknowledgement or changed file state qualified");
                assert!(reason.contains(field), "{field}:{value}:{reason}");
            }
            let mut changed = records.clone();
            flag(&mut changed[index], "fenced", true);
            let mut report = decode_hex(&changed[index], service_event, "report_hex", 80).unwrap();
            report[72..80].copy_from_slice(&1u64.to_le_bytes());
            string(&mut changed[index], "report_hex", &hex(&report));
            assert!(verify_fixture(&changed, &plan, &source).is_err());
        }
    }

    #[test]
    fn post_resume_fault_requires_exact_instruction_and_new_pending_caller() {
        let (records, plan, source) = fixture(832, false, true);
        let containment = records
            .iter()
            .position(|record| {
                event(record).unwrap() == containment_event
                    && required_number(record, containment_event, "scenario").unwrap() == 7
            })
            .unwrap();
        for (field, value) in [
            ("fault_vector", 0),
            ("fault_vector", 14),
            ("fault_instruction_pointer", faults[0]),
            ("fault_instruction_pointer", faults[1] + 1),
            ("pending_sequence", 11),
            ("last_committed_sequence", 10),
        ] {
            let mut changed = records.clone();
            number(&mut changed[containment], field, value);
            let reason = verify_fixture(&changed, &plan, &source)
                .err()
                .expect("wrong post-resume fault qualified");
            assert!(reason.contains(field), "{field}:{value}:{reason}");
        }
        for field in ["expected_fault", "peer_survived"] {
            let mut changed = records.clone();
            flag(&mut changed[containment], field, false);
            assert!(verify_fixture(&changed, &plan, &source).is_err());
        }
        let terminal = records
            .iter()
            .position(|record| {
                event(record).unwrap() == events[16]
                    && required_number(record, events[16], "scenario").unwrap() == 7
            })
            .unwrap();
        let mut changed = records.clone();
        let mut frame = reply_frame(&changed[terminal], "frame_after_hex").unwrap();
        frame[0] ^= 1;
        string(
            &mut changed[terminal],
            "frame_after_hex",
            &frame_hex(&frame),
        );
        let reason = verify_fixture(&changed, &plan, &source)
            .err()
            .expect("post-resume fault changed the caller frame");
        assert!(reason.contains("suspended caller frame"), "{reason}");
    }

    #[test]
    fn rejects_pread_bytes_from_sequential_offset_and_reused_read_journal() {
        let (records, plan, source) = fixture(832, false, true);
        for (scenario, sequence) in [(0, 8), (0, 10), (3, 8), (4, 8), (5, 8), (6, 8)] {
            let index = records
                .iter()
                .position(|r| {
                    event(r).unwrap() == delivery_event
                        && required_number(r, delivery_event, "scenario").unwrap() == scenario
                        && required_number(r, delivery_event, "sequence").unwrap() == sequence
                })
                .unwrap();
            let mut changed = records.clone();
            let wrong = hex(&test_file_source[832..832 + 840]);
            string(&mut changed[index], "stage_bytes_hex", &wrong);
            if scenario != 3 {
                string(&mut changed[index], "after_hex", &wrong);
            }
            let reason = verify_fixture(&changed, &plan, &source)
                .err()
                .expect("sequential offset substituted for positional source");
            assert!(
                reason.contains("independently derived read offset"),
                "{reason}"
            );

            let mut changed = records.clone();
            changed[index] = changed[index - 1].clone();
            assert!(verify_fixture(&changed, &plan, &source).is_err());
        }
    }

    #[test]
    fn derives_repeated_pread_from_its_own_request_and_preserves_prior_journals() {
        let (mut records, plan, source) = fixture(832, false, true);
        let boundary = records
            .iter()
            .position(|r| {
                event(r).unwrap() == events[9]
                    && required_number(r, events[9], "scenario").unwrap() == 0
                    && required_number(r, events[9], "sequence").unwrap() == 10
            })
            .unwrap();
        let mut changed_operation = Vec::new();
        operation(
            &mut changed_operation,
            0,
            10,
            17,
            [3, stack_end - 16384, 73, 2317, 17, 19],
            73,
            true,
            None,
        );
        for (index, record) in changed_operation.into_iter().enumerate() {
            records[boundary + index] = record;
        }
        let delivery = records
            .iter()
            .position(|r| {
                event(r).unwrap() == delivery_event
                    && required_number(r, delivery_event, "scenario").unwrap() == 0
                    && required_number(r, delivery_event, "sequence").unwrap() == 10
            })
            .unwrap();
        for (field, value) in [
            ("address", stack_end - 16384),
            ("length", 73),
            ("source_offset", 2317),
            ("source_bytes", 73),
        ] {
            number(&mut records[delivery], field, value);
        }
        let bytes = hex(&test_file_source[2317..2317 + 73]);
        string(&mut records[delivery], "stage_bytes_hex", &bytes);
        string(&mut records[delivery], "after_hex", &bytes);
        string(&mut records[delivery], "before_hex", &"a5".repeat(73));
        verify_fixture(&records, &plan, &source).unwrap();

        let mut changed = records.clone();
        let reused = hex(&test_file_source[64..64 + 73]);
        string(&mut changed[delivery], "stage_bytes_hex", &reused);
        string(&mut changed[delivery], "after_hex", &reused);
        let reason = verify_fixture(&changed, &plan, &source)
            .err()
            .expect("repeated pread reused the earlier request's source offset");
        assert!(
            reason.contains("independently derived read offset"),
            "{reason}"
        );
        let mut changed = records.clone();
        changed[delivery - 2] = changed[delivery].clone();
        assert!(verify_fixture(&changed, &plan, &source).is_err());
    }

    #[test]
    fn refuses_empty_positional_progress_as_the_milestone() {
        let (records, plan, source) = fixture_with_pread(832, false, true, 64, 0, 9);
        assert!(verify_fixture(&records, &plan, &source).is_err());
        let (records, plan, source) =
            fixture_with_pread(832, false, true, test_file_source.len(), 840, 9);
        assert!(verify_fixture(&records, &plan, &source).is_err());
    }

    #[test]
    fn metadata_decoder_covers_every_byte_including_padding_and_reserved_words() {
        for length in [0, 1, 511, 512, 513, 8193, 2_190_608] {
            let bytes = metadata_fixture(length);
            verify_metadata(&bytes, length).unwrap();
            for index in 0..144 {
                let mut changed = bytes;
                changed[index] ^= 1;
                assert!(
                    verify_metadata(&changed, length).is_err(),
                    "length{length}:byte{index}"
                );
            }
            assert!(verify_metadata(&bytes[..143], length).is_err());
            let mut oversized = bytes.to_vec();
            oversized.push(0);
            assert!(verify_metadata(&oversized, length).is_err());
        }
    }

    #[test]
    fn rejects_fstat_policy_corruption_even_when_actual_copy_matches_the_stage() {
        let (records, plan, source) = fixture(832, false, true);
        for scenario in [0, 5, 6] {
            let index = records
                .iter()
                .position(|r| {
                    event(r).unwrap() == delivery_event
                        && required_number(r, delivery_event, "scenario").unwrap() == scenario
                        && required_number(r, delivery_event, "sequence").unwrap() == 9
                })
                .unwrap();
            for offset in [
                0, 8, 16, 24, 28, 32, 36, 40, 48, 56, 64, 72, 80, 88, 96, 104, 112, 120, 128, 136,
            ] {
                let mut changed = records.clone();
                let mut metadata = metadata_fixture(test_file_source.len());
                metadata[offset] ^= 1;
                string(&mut changed[index], "stage_bytes_hex", &hex(&metadata));
                if scenario != 5 {
                    string(&mut changed[index], "after_hex", &hex(&metadata));
                }
                let reason = verify_fixture(&changed, &plan, &source)
                    .err()
                    .expect("invalid metadata policy qualified");
                assert!(
                    reason.contains("fstat metadata"),
                    "{scenario}:{offset}:{reason}"
                );
            }
            let mut changed = records.clone();
            let wrong = if scenario == 5 {
                metadata_fixture(test_file_source.len()).to_vec()
            } else {
                vec![0xa5; 144]
            };
            string(&mut changed[index], "after_hex", &hex(&wrong));
            assert!(verify_fixture(&changed, &plan, &source).is_err());
        }
    }

    #[test]
    fn distinguishes_fstat_zero_result_from_its_144_byte_copyout() {
        let (records, plan, source) = fixture(832, false, true);
        let mut changed = records.clone();
        for record in &mut changed {
            if required_number(record, "test", "scenario").ok() != Some(0)
                || required_number(record, "test", "sequence").ok() != Some(9)
            {
                continue;
            }
            if event(record).unwrap() == events[9] {
                number(record, "result_bits", 144);
            } else if event(record).unwrap() == events[10] {
                number(record, "result_bits", 144);
                let mut frame = reply_frame(record, "frame_after_hex").unwrap();
                frame[0] = 144;
                string(record, "frame_after_hex", &frame_hex(&frame));
            }
        }
        let reason = verify_fixture(&changed, &plan, &source)
            .err()
            .expect("copyout length returned as fstat result");
        assert!(reason.contains("incorrect Linux result"), "{reason}");
    }

    #[test]
    fn requires_all_ordered_records_and_all_clean_lifetimes() {
        let (records, plan, source) = fixture(17, false, true);
        for index in 8..records.len() {
            let mut changed = records.clone();
            changed.remove(index);
            assert!(
                verify_fixture(&changed, &plan, &source).is_err(),
                "missing{index}"
            );
            let mut changed = records.clone();
            changed.insert(index, records[index].clone());
            assert!(
                verify_fixture(&changed, &plan, &source).is_err(),
                "duplicate{index}"
            );
            if index + 1 < records.len() {
                let mut changed = records.clone();
                changed.swap(index, index + 1);
                assert!(
                    verify_fixture(&changed, &plan, &source).is_err(),
                    "reorder{index}"
                );
            }
        }
        let mut changed = records.clone();
        let ready = changed
            .iter_mut()
            .filter(|r| event(r).unwrap() == events[8])
            .nth(1)
            .unwrap();
        number(ready, "incarnation", 401);
        assert!(verify_fixture(&changed, &plan, &source).is_err());
    }

    #[test]
    fn rejects_missing_unknown_duplicate_and_wrong_typed_fields() {
        let (records, plan, source) = fixture(17, false, true);
        let mut seen = std::collections::BTreeSet::new();
        for index in 8..records.len() {
            if !seen.insert(event(&records[index]).unwrap().to_owned()) {
                continue;
            }
            for field in records[index].keys() {
                for absent in [false, true] {
                    let mut changed = records.clone();
                    if absent {
                        changed[index].remove(field);
                    } else {
                        changed[index].insert(field.clone(), json_scalar::null);
                    }
                    assert!(
                        verify_fixture(&changed, &plan, &source).is_err(),
                        "{index}:{field}"
                    );
                }
            }
            let mut changed = records.clone();
            flag(&mut changed[index], "unexpected", true);
            assert!(verify_fixture(&changed, &plan, &source).is_err());
            let mut lines = records.iter().map(render).collect::<Vec<_>>();
            lines[index] = lines[index].replacen('{', "{\"schema\":1,", 1);
            assert!(verify(&lines.join("\n"), &plan, &source, test_file_source, faults).is_err());
        }
    }

    #[test]
    fn rejects_false_absence_positive_metadata_and_path_scope_confusion() {
        for wrong in [
            b"/lib/x86_64-linux-gnu-other/no".as_slice(),
            library_path,
            b"/lib/x86_64-linux-gnu/libc.so.6/",
            b"/lib/x86_64-linux-gnu/libc.so.6/child",
            b"/lib/x86_64-linux-gnu/libc.so.6/child/",
            b"/lib/x86_64-linux-gnu/../no",
            b"/lib/x86_64-linux-gnu/../no/",
            b"/lib/x86_64-linux-gnu/./no/",
            b"/lib/x86_64-linux-gnu//no/",
            b"/lib/x86_64-linux-gnu/no//",
            b"/lib/x86_64-linux-gnu",
            b"/lib/x86_64-linux-gnu/",
        ] {
            for sequence in [4, 5] {
                let (mut records, plan, source) = fixture(17, false, true);
                let reply = records
                    .iter_mut()
                    .find(|r| {
                        event(r).unwrap() == events[10]
                            && required_number(r, events[10], "sequence").unwrap() == sequence
                    })
                    .unwrap();
                let bytes = [wrong, &[0]].concat();
                number(reply, "copy_bytes", bytes.len() as u64);
                string(reply, "copy_hex", &hex(&bytes));
                let reason = verify_fixture(&records, &plan, &source)
                    .err()
                    .expect("invalid namespace miss qualified");
                assert!(
                    reason.contains("explicit complete namespace")
                        || reason.contains("first libc open"),
                    "wrong rejection for sequence{sequence}: {reason}"
                );
            }
        }
        assert!(!scoped_missing(257, b"/lib/x86_64-linux-gnu/missing/"));
    }

    #[test]
    fn rejects_actual_caller_corruption_and_unknown_local_state_reuse() {
        let (records, plan, source) = fixture(17, false, true);
        for (scenario, event_name, field, value) in [
            (0, delivery_event, "after_hex", "a5".repeat(17)),
            (1, delivery_event, "after_hex", hex(&test_file_source[..17])),
            (2, delivery_event, "stage_bytes_hex", "a5".repeat(17)),
            (2, service_event, "report_hex", "00".repeat(80)),
        ] {
            let mut changed = records.clone();
            let selected = changed
                .iter_mut()
                .find(|r| {
                    event(r).unwrap() == event_name
                        && required_number(r, event_name, "scenario").unwrap() == scenario
                })
                .unwrap();
            string(selected, field, &value);
            assert!(verify_fixture(&changed, &plan, &source).is_err());
        }
        for event_name in [delivery_event, service_event, containment_event, events[18]] {
            for index in records
                .iter()
                .enumerate()
                .filter_map(|(i, r)| (event(r).unwrap() == event_name).then_some(i))
            {
                for (key, value) in &records[index] {
                    let mut changed = records.clone();
                    match value {
                        json_scalar::number(value) => {
                            number(&mut changed[index], key, value.parse::<u64>().unwrap() ^ 1)
                        }
                        json_scalar::boolean(value) => flag(&mut changed[index], key, !value),
                        _ => continue,
                    }
                    assert!(
                        verify_fixture(&changed, &plan, &source).is_err(),
                        "{index}:{key}"
                    );
                }
            }
        }
    }

    #[test]
    fn rejects_reply_frame_corruption_and_uninitialized_syscall_sites() {
        let (records, plan, source) = fixture(17, false, true);
        for sequence in [7, 8, 9, 10] {
            let index = records
                .iter()
                .position(|r| {
                    event(r).unwrap() == events[10]
                        && required_number(r, events[10], "scenario").unwrap() == 0
                        && required_number(r, events[10], "sequence").unwrap() == sequence
                })
                .unwrap();
            let frame = reply_frame(&records[index], "frame_after_hex").unwrap();
            for word in 0..22 {
                let mut changed = records.clone();
                let mut corrupt = frame;
                corrupt[word] ^= 1;
                string(&mut changed[index], "frame_after_hex", &frame_hex(&corrupt));
                assert!(verify_fixture(&changed, &plan, &source).is_err());
            }
        }
        let mut corrupt = source.clone();
        corrupt[13] = 0x90;
        let mut changed_plan = plan;
        changed_plan[3] = linux_loader_entry_artifact::fnv64(&corrupt);
        let mut changed = records.clone();
        for ready in changed
            .iter_mut()
            .filter(|r| event(r).unwrap() == events[8])
        {
            number(ready, "source_fnv64", changed_plan[3]);
        }
        assert!(verify_fixture(&changed, &changed_plan, &corrupt).is_err());
    }
}
