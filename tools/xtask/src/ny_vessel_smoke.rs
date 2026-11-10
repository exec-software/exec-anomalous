//! Strict oracle for a Ny vessel booted on the rite ABI.
//!
//! The vessel's Ny half composes a message into the region the kernel installs
//! and returns 42; its Anomalous half submits the message as one console_write
//! and exits with that value. Both are checked here against the values the
//! build stage measured, so a vessel that sends something else, or exits with
//! something else, fails rather than being reported as whatever arrived.
//!
//! The vessel also reads. It asks for a line, the kernel delivers one, and the
//! third write is the answer Ny built around it -- so the byte counts here are
//! what pins that the delivered text reached Ny and came back out, since the
//! third count is only reachable by a vessel that received those bytes.
//!
//! And it fails. The same image is entered twice: once to do its work, once to
//! exhaust the region on purpose, because one run cannot be both and the first
//! run's evidence must not be given up to see the second. The failing run is
//! what shows that a Ny failure arrives as a rite at all -- without the
//! binding's own `__ny_host_fail` the vessel would spin in user mode with
//! nothing submitted, and only a timeout would notice.
use super::*;

pub(super) const feature: &str = "ny_vessel_test";

/// What the vessel writes. Three times: the message it composed, a second line
/// it sends only because the first rite returned the count it expected -- which
/// is the vessel acting on the ABI rather than crossing it again -- and an
/// answer composed around the line the kernel delivered to it.
/// Taken from the payload stage's statement of what the vessel says, so the
/// two cannot disagree.
const expected_first_bytes: u64 = ny_vessel_artifact::expected_first_message.len() as u64;
const expected_second_bytes: u64 = ny_vessel_artifact::expected_second_message.len() as u64;
const expected_third_bytes: u64 = ny_vessel_artifact::expected_third_message.len() as u64;
/// The line the kernel hands over, as a length. The vessel's image cannot
/// contain it, so the third write's count is the evidence it arrived.
const expected_delivered_bytes: u64 = ny_vessel_artifact::expected_delivered_message.len() as u64;
const expected_exit_code: u64 = ny_vessel_artifact::expected_exit_code as u64;
/// What the binding exits with when Ny fails inside it, stated in the
/// vessel's `entry.S` and outside the range of values Ny itself returns.
const expected_failure_exit_code: u64 = 70;
/// The runtime's failure message, as a length. Its bytes are upstream's prose
/// and this oracle does not pin them, but the length is a tripwire: a reworded
/// message is a change to what a vessel's failure looks like from outside, and
/// it should fail here and be looked at rather than pass unnoticed.
const expected_failure_message_bytes: u64 = 64;

const expected_events: [&str; 21] = [
    "boot.kernel.enter",
    "kernel.fault_path.ready",
    "boot.omen.accepted",
    "boot.memory.map",
    "kernel.memory.ready",
    "boot.display.ready",
    "boot.ready",
    "kernel.ny_vessel.ready",
    // The working run.
    "kernel.ny_vessel.task.entered",
    "kernel.ny_vessel.console.committed",
    "kernel.ny_vessel.console.committed",
    "kernel.ny_vessel.input.delivered",
    "kernel.ny_vessel.yielded",
    "kernel.ny_vessel.console.committed",
    "kernel.ny_vessel.exited",
    "kernel.ny_vessel.released",
    // The failing run, from the same image.
    "kernel.ny_vessel.task.entered",
    "kernel.ny_vessel.failure.reported",
    "kernel.ny_vessel.exited",
    "kernel.ny_vessel.released",
    "kernel.ny_vessel.passed",
];

pub(super) fn selected(features: &[&str]) -> bool {
    features.contains(&feature)
        && !features.iter().any(|value| {
            matches!(
                *value,
                "fault_test"
                    | "circle_test"
                    | "control_test"
                    | "rite_test"
                    | "memory_test"
                    | "interrupt_test"
                    | "supervision_test"
                    | "shell_vessel_test"
                    | "input_wait_test"
                    | "display_vessel_test"
                    | "supervised_renderer_test"
                    | "semantic_surface_transport_test"
                    | "semantic_surface_presentation_test"
                    | "ny_object_test"
                    | "desktop_dev"
            )
        })
}

pub(super) fn build() -> Result<built_image, String> {
    let image = build_image_with_context(&[feature], None)?;
    println!("Ny vessel image inspected: {}", image.path.display());
    Ok(image)
}

pub(super) fn smoke() -> Result<(), String> {
    let build = repository_root()?.join("build");
    let log = build.join("qemu_ny_vessel.log");
    remove_optional_file(&log)?;
    build_and_inspect_normal_image()?;
    let image = self::build()?;
    let mut command = qemu_command(&image.path)?;
    command
        .arg("-display")
        .arg("none")
        .arg("-monitor")
        .arg("none")
        .arg("-device")
        .arg("isa-debug-exit,iobase=0xf4,iosize=0x04");
    run_qemu_with_transcript(command, smoke_timeout, |status, transcript| {
        // Retain failures too: diagnostic evidence must survive a rejected
        // oracle.
        fs::write(&log, transcript)
            .map_err(|error| format!("retain ny-vessel transcript: {error}"))?;
        if status.code() != Some(qemu_success_status) {
            return Err(format!(
                "QEMU did not report ny-vessel success (process status: {:?})",
                status.code()
            ));
        }
        verify_records(transcript)
    })?;
    println!(
        "Ny vessel booted and issued its own rites: {expected_first_bytes} bytes it \
         composed in the host's region, then {expected_second_bytes} more after reading \
         the first rite's result, then {expected_delivered_bytes} delivered bytes it had \
         never contained answered with {expected_third_bytes}, and exited \
         {expected_exit_code}; then the same image exhausted its region on purpose, \
         spoke the runtime's {expected_failure_message_bytes}-byte failure as a rite, \
         and exited {expected_failure_exit_code} instead of spinning"
    );
    Ok(())
}

pub(super) fn verify_transcript(transcript: &str) -> Result<(), String> {
    verify_records(transcript)
}

/// The shared helpers cover strings and numbers; a flag this fixture states
/// must be exactly true, and must be a boolean rather than the string "true".
fn require_boolean(
    record: &structured_record,
    event: &str,
    field: &str,
    expected: bool,
) -> Result<(), String> {
    match record.get(field) {
        Some(json_scalar::boolean(value)) if *value == expected => Ok(()),
        Some(json_scalar::boolean(value)) => Err(format!(
            "event `{event}` field `{field}` is {value}, expected {expected}"
        )),
        _ => Err(format!(
            "event `{event}` has missing or non-boolean field `{field}`"
        )),
    }
}

fn verify_records(transcript: &str) -> Result<(), String> {
    let mut records = Vec::new();
    for (index, line) in transcript.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if !line.starts_with('{') {
            return Err(format!(
                "Ny vessel has unstructured serial line {}",
                index + 1
            ));
        }
        let record = json_parser::new(line)
            .parse_record()
            .map_err(|error| format!("malformed Ny vessel record {}: {error}", index + 1))?;
        let event = expected_events
            .get(records.len())
            .ok_or("extra Ny vessel lifecycle record")?;
        require_string(&record, event, "event", event)?;
        require_number(&record, event, "schema", 1)?;
        records.push(record);
    }
    if records.len() != expected_events.len() {
        return Err("missing Ny vessel lifecycle records".into());
    }

    // Readiness states the grant and the inspected image, so a transcript
    // cannot be satisfied by a different payload than the one built.
    let ready = &records[7];
    require_string(ready, expected_events[7], "arch", "x86_64")?;
    require_number(ready, expected_events[7], "vector", 243)?;
    // The shell minimum plus the semantic-session read and the yield the two
    // further operations need: 0b101111. No wider than the three operations
    // require.
    require_number(ready, expected_events[7], "grant", 47)?;
    require_number(ready, expected_events[7], "operations", 3)?;
    require_number(ready, expected_events[7], "entry", 0x0000_6000_0000_1000)?;
    let image_bytes = required_number(ready, expected_events[7], "image_bytes")?;
    if image_bytes == 0
        || image_bytes > ny_vessel_artifact::image_pages * ny_vessel_artifact::page_size
    {
        return Err("Ny vessel image must fit its code extent".into());
    }

    // Each run states the mapped profile: the RX code extent, the writable
    // extent, the stack, and absent guards on every side. Three mappings for
    // both runs: the second code page costs none, because the pair is mapped
    // as one extent.
    for (index, run) in [(8usize, "speaking"), (16usize, "failing")] {
        let entered = &records[index];
        let event = expected_events[index];
        require_string(entered, event, "origin", "user")?;
        require_string(entered, event, "run", run)?;
        require_number(entered, event, "mappings", 3)?;
        require_string(entered, event, "code_permissions", "user_rx")?;
        require_string(entered, event, "writable_permissions", "user_rw_nx")?;
        require_string(entered, event, "stack_permissions", "user_rw_nx")?;
        require_boolean(entered, event, "guards_absent", true)?;
    }

    // Three committed console writes, all issued from inside Ny. The first
    // carries the message Ny composed in the region the kernel installed; the
    // second is the one Ny only sends because the first returned its count;
    // the third is the answer it composed around the delivered line, so it is
    // both in the region and carrying the delivery.
    for (index, sequence, bytes, from_region, carries_delivery) in [
        (9usize, 1u64, expected_first_bytes, true, false),
        (10usize, 2u64, expected_second_bytes, false, false),
        (13usize, 5u64, expected_third_bytes, true, true),
    ] {
        let console = &records[index];
        let event = expected_events[index];
        require_number(console, event, "sequence", sequence)?;
        require_string(console, event, "status", "ok")?;
        require_number(console, event, "bytes", bytes)?;
        require_string(console, event, "composed_by", "ny")?;
        require_string(console, event, "issued_by", "ny")?;
        require_boolean(console, event, "from_region", from_region)?;
        require_boolean(console, event, "carries_delivery", carries_delivery)?;
    }

    // The delivery Ny asked for. `waited` must be false: the vessel's request
    // asks to wait, and this fixture answers immediately, so a transcript
    // claiming a wait would be claiming a path nothing here exercises.
    let delivered = &records[11];
    require_number(delivered, expected_events[11], "sequence", 3)?;
    require_string(delivered, expected_events[11], "status", "ok")?;
    require_number(
        delivered,
        expected_events[11],
        "bytes",
        expected_delivered_bytes,
    )?;
    require_string(delivered, expected_events[11], "origin", "human_text")?;
    require_number(delivered, expected_events[11], "delivery_sequence", 1)?;
    require_boolean(delivered, expected_events[11], "waited", false)?;
    require_string(delivered, expected_events[11], "read_by", "ny")?;

    // The yield. What the vessel cannot observe is the point: its address
    // space was left and resumed while this rite was answered, and the third
    // write below is composed from a pointer it was holding beforehand. No
    // peer ran -- the transcript says so rather than letting a reader assume
    // a scheduler handed the processor to something else.
    let yielded = &records[12];
    require_number(yielded, expected_events[12], "sequence", 4)?;
    require_string(yielded, expected_events[12], "status", "ok")?;
    require_number(yielded, expected_events[12], "address_space_round_trips", 1)?;
    require_boolean(yielded, expected_events[12], "resumed_in_ny", true)?;
    require_boolean(yielded, expected_events[12], "peer_ran", false)?;

    // The working run's exit carries what Ny returned, through the vessel's
    // own cell.
    let exited = &records[14];
    require_number(exited, expected_events[14], "sequence", 6)?;
    require_string(exited, expected_events[14], "status", "ok")?;
    require_string(exited, expected_events[14], "run", "speaking")?;
    require_number(exited, expected_events[14], "code", expected_exit_code)?;
    require_boolean(exited, expected_events[14], "returned_from_ny", true)?;

    // The failing run. Ny exhausted the region deliberately, the runtime's
    // failure hook reached the binding, and the binding spoke the message and
    // exited -- rather than the vessel spinning with nothing submitted, which
    // is what the shipped weak hook would have left it doing.
    let reported = &records[17];
    require_number(reported, expected_events[17], "sequence", 1)?;
    require_string(reported, expected_events[17], "status", "ok")?;
    require_number(
        reported,
        expected_events[17],
        "bytes",
        expected_failure_message_bytes,
    )?;
    require_string(reported, expected_events[17], "composed_by", "runtime")?;
    require_string(reported, expected_events[17], "issued_by", "binding")?;
    require_boolean(reported, expected_events[17], "from_region", false)?;
    require_boolean(reported, expected_events[17], "names_the_cause", true)?;

    // Ny never returned on this run, and the transcript says so: the code is
    // the binding's, not a value Ny chose.
    let failed_exit = &records[18];
    require_number(failed_exit, expected_events[18], "sequence", 2)?;
    require_string(failed_exit, expected_events[18], "status", "ok")?;
    require_string(failed_exit, expected_events[18], "run", "failing")?;
    require_number(
        failed_exit,
        expected_events[18],
        "code",
        expected_failure_exit_code,
    )?;
    require_boolean(failed_exit, expected_events[18], "returned_from_ny", false)?;

    // Release accounting, after each run: every frame returned, nothing of the
    // vessel left. The failing run is held to the same standard as the
    // working one -- a vessel that failed inside Ny still owes its frames
    // back, and the region it exhausted still has to be wiped.
    for index in [15usize, 19usize] {
        let released = &records[index];
        let event = expected_events[index];
        let free = required_number(released, event, "free_frames")?;
        let initial = required_number(released, event, "initial_free_frames")?;
        if free != initial {
            return Err(format!(
                "Ny vessel did not restore its frames after {event}: {free} of {initial}"
            ));
        }
        for field in [
            "data_owned_frames",
            "table_owned_frames",
            "active_circles",
            "active_tasks",
            "active_mappings",
        ] {
            require_number(released, event, field, 0)?;
        }
        for field in ["channel_retired", "region_wiped", "transition_stack_wiped"] {
            require_boolean(released, event, field, true)?;
        }
    }

    let passed = &records[20];
    require_number(passed, expected_events[20], "runs", 2)?;
    require_number(passed, expected_events[20], "calls", 8)?;
    require_number(passed, expected_events[20], "commits", 8)?;
    require_number(passed, expected_events[20], "console_writes", 4)?;
    require_number(passed, expected_events[20], "deliveries", 1)?;
    require_number(passed, expected_events[20], "yields", 1)?;
    require_number(passed, expected_events[20], "ny_failures", 1)?;
    require_string(passed, expected_events[20], "issued_by", "ny")?;
    require_boolean(passed, expected_events[20], "terminal", true)?;
    Ok(())
}
