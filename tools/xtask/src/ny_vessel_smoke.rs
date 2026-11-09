//! Strict oracle for a Ny vessel booted on the rite ABI.
//!
//! The vessel's Ny half composes a message into the region the kernel installs
//! and returns 42; its Anomalous half submits the message as one console_write
//! and exits with that value. Both are checked here against the values the
//! build stage measured, so a vessel that sends something else, or exits with
//! something else, fails rather than being reported as whatever arrived.
use super::*;

pub(super) const feature: &str = "ny_vessel_test";

/// What the vessel composes, measured by running its pre-gate sequence on the
/// build host against the same object and archive.
const expected_console_bytes: u64 = 38;
const expected_exit_code: u64 = 42;

const expected_events: [&str; 13] = [
    "boot.kernel.enter",
    "kernel.fault_path.ready",
    "boot.omen.accepted",
    "boot.memory.map",
    "kernel.memory.ready",
    "boot.display.ready",
    "boot.ready",
    "kernel.ny_vessel.ready",
    "kernel.ny_vessel.task.entered",
    "kernel.ny_vessel.console.committed",
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
        "Ny vessel booted, composed {expected_console_bytes} bytes in the host's region, \
         committed them through one console_write, and exited {expected_exit_code}"
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
    require_number(ready, expected_events[7], "grant", 7)?;
    require_number(ready, expected_events[7], "entry", 0x0000_6000_0000_1000)?;
    let image_bytes = required_number(ready, expected_events[7], "image_bytes")?;
    if image_bytes == 0 || image_bytes > 4096 {
        return Err("Ny vessel image must occupy part of one page".into());
    }

    // Entry states the mapped profile: one RX page, the writable extent, the
    // stack, and absent guards on every side.
    let entered = &records[8];
    require_string(entered, expected_events[8], "origin", "user")?;
    require_number(entered, expected_events[8], "mappings", 3)?;
    require_string(entered, expected_events[8], "code_permissions", "user_rx")?;
    require_string(
        entered,
        expected_events[8],
        "writable_permissions",
        "user_rw_nx",
    )?;
    require_string(
        entered,
        expected_events[8],
        "stack_permissions",
        "user_rw_nx",
    )?;
    require_boolean(entered, expected_events[8], "guards_absent", true)?;

    // The committed console write: Ny composed it, it came out of the region,
    // and it is exactly the measured length.
    let console = &records[9];
    require_number(console, expected_events[9], "sequence", 1)?;
    require_string(console, expected_events[9], "status", "ok")?;
    require_number(console, expected_events[9], "bytes", expected_console_bytes)?;
    require_string(console, expected_events[9], "composed_by", "ny")?;
    require_boolean(console, expected_events[9], "from_region", true)?;

    // The exit carries what Ny returned, through the vessel's own cell.
    let exited = &records[10];
    require_number(exited, expected_events[10], "sequence", 2)?;
    require_string(exited, expected_events[10], "status", "ok")?;
    require_number(exited, expected_events[10], "code", expected_exit_code)?;
    require_boolean(exited, expected_events[10], "returned_from_ny", true)?;

    // Release accounting: every frame returned, nothing of the vessel left.
    let released = &records[11];
    let free = required_number(released, expected_events[11], "free_frames")?;
    let initial = required_number(released, expected_events[11], "initial_free_frames")?;
    if free != initial {
        return Err(format!(
            "Ny vessel did not restore its frames: {free} of {initial}"
        ));
    }
    for field in [
        "data_owned_frames",
        "table_owned_frames",
        "active_circles",
        "active_tasks",
        "active_mappings",
    ] {
        require_number(released, expected_events[11], field, 0)?;
    }
    for field in ["channel_retired", "region_wiped", "transition_stack_wiped"] {
        require_boolean(released, expected_events[11], field, true)?;
    }

    let passed = &records[12];
    require_number(passed, expected_events[12], "calls", 2)?;
    require_number(passed, expected_events[12], "commits", 2)?;
    require_boolean(passed, expected_events[12], "terminal", true)?;
    Ok(())
}
