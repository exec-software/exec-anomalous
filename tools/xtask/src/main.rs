#![allow(non_camel_case_types, non_upper_case_globals)]

use std::collections::BTreeMap;
use std::env;
use std::ffi::{OsStr, OsString};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, ExitCode, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use anomalous_desktop_transport::{
    retained_host_transaction_fixture, surface_snapshot_v1_byte_count,
};

mod application_profile_smoke;
mod desktop_dev;
mod dispatcher_smoke;
mod elf_application_artifact;
mod elf_application_smoke;
mod elf_document_artifact;
mod elf_document_smoke;
mod elf_relaunch_artifact;
mod elf_relaunch_smoke;
mod file_service_proof;
mod linux_application_artifact;
mod linux_application_smoke;
mod linux_loader_entry_artifact;
mod linux_loader_entry_smoke;
mod linux_process;
mod linux_tls_artifact;
mod linux_tls_smoke;
mod mixed_workspace;
mod native_application_artifact;
mod native_application_smoke;
mod native_document_artifact;
mod native_document_smoke;
mod native_durable_artifact;
mod native_durable_failure_artifact;
mod native_durable_failure_smoke;
mod native_durable_smoke;
mod native_editor_workspace;
mod native_interactive_smoke;
mod native_manifest_artifact;
mod native_manifest_smoke;
mod native_program_artifact;
mod native_program_smoke;
mod native_relaunch_artifact;
mod native_relaunch_smoke;
mod native_repeated_smoke;
mod native_runtime_artifact;
mod native_runtime_smoke;
mod ny_object_artifact;
mod ny_object_smoke;
mod ny_vessel_artifact;
mod ny_vessel_smoke;
mod owned_dispatcher_smoke;
mod owned_multi_region_smoke;
mod owned_source_smoke;
mod owned_tls_smoke;
mod owned_workspace;
mod owned_workspace_cpu;
mod program_manifest;
mod shared_veil_smoke;
mod supervised_renderer_oracle;
mod veil_application_artifact;
mod veil_application_smoke;
mod workspace_dev;

const sector_size: u64 = 512;
const disk_size: u64 = 16 * 1024 * 1024;
const manifest_lba: u64 = 1;
const stage_2_lba: u64 = 2;
const manifest_magic: [u8; 8] = *b"THRESHLD";
const manifest_version: u32 = 1;
const manifest_size: usize = 96;
const manifest_hash_offset: usize = 16;
const manifest_hashed_offset: usize = 20;
const manifest_sector_size: usize = sector_size as usize;
const stage_2_load_address: u32 = 0x0001_0000;
const maximum_stage_2_sectors: u32 = 127;
const kernel_staging_address: u32 = 0x0003_0000;
const kernel_staging_capacity: u32 = 0x0006_0000;
const kernel_load_address: u32 = 0x0010_0000;
const maximum_kernel_memory_size: u32 = 0x0400_0000;
const fnv_offset_basis: u32 = 0x811c_9dc5;
const fnv_prime: u32 = 0x0100_0193;
const ready_event: &str = r#""event":"boot.ready""#;
const fault_path_ready_event: &str = r#""event":"kernel.fault_path.ready""#;
const fault_test_passed_event: &str =
    r#""event":"kernel.fault_test.passed","level":"info","vector":6,"kind":"invalid_opcode""#;
const bootstrap_shell_ready_event: &str = r#""event":"bootstrap_shell.ready""#;
const bootstrap_shell_version_event: &str = r#""event":"bootstrap_shell.command.completed","level":"info","sequence":1,"kind":"version","status":"ok""#;
const bootstrap_shell_echo_event: &str = r#""event":"bootstrap_shell.command.completed","level":"info","sequence":2,"kind":"echo","status":"ok""#;
const bootstrap_shell_unknown_event: &str = r#""event":"bootstrap_shell.command.completed","level":"info","sequence":3,"kind":"unknown","status":"unknown_command""#;
const bootstrap_shell_final_prompt_event: &str =
    r#""event":"bootstrap_shell.prompt","level":"debug","after_sequence":3"#;
const privacy_probe: &str = "privateprobe";
const qemu_success_status: i32 = 33;
const smoke_timeout: Duration = Duration::from_secs(15);
const circle_event_prefix: &str = "kernel.circle_test.";
const circle_failed_event: &str = "kernel.circle_test.failed";
const circle_after_denial_event: &str = "kernel.circle_test.after_denial";
const nested_exception_event: &str = "kernel.exception.nested";
const circle_forbidden_symbols: &[&str] = &[
    "circle_kernel_canary",
    "x86_64_circle_generation_1_start",
    "x86_64_circle_generation_2_start",
    "x86_64_circle_generation_3_start",
    "x86_64_circle_report_entry",
    "x86_64_circle_stack_probe_start",
    "x86_64_enter_circle",
    "x86_64_resume_circle",
];
const circle_forbidden_binary_strings: &[&[u8]] =
    &[b"kernel.circle_test.", &[0x16; 8], &[0x26; 8], &[0x36; 8]];
const circle_page_size: u64 = 4096;
const circle_user_code_address: u64 = 0x0000_4000_0000_1000;
const circle_user_stack_address: u64 = 0x0000_4000_0000_5000;
const circle_user_stack_top: u64 = 0x0000_4000_0000_6000;
const circle_user_data_selector: u64 = 0x002b;
const circle_user_code_selector: u64 = 0x0033;
const circle_sanitized_flags: u64 = 0x0002;
const circle_fault_resume_flag: u64 = 1 << 16;
const circle_generation_markers: [u64; 3] = [
    0x1616_1616_1616_1616,
    0x2626_2626_2626_2626,
    0x3636_3636_3636_3636,
];
const control_event_prefix: &str = "kernel.control_test.";
const control_failed_event: &str = "kernel.control_test.failed";
const control_forbidden_symbols: &[&str] = &[
    "x86_64_control_gate_entry",
    "x86_64_control_producer_start",
    "x86_64_control_producer_probe_return",
    "x86_64_control_producer_send_return",
    "x86_64_control_producer_complete_return",
    "x86_64_control_receiver_start",
    "x86_64_control_receiver_probe_return",
    "x86_64_control_receiver_first_receive_return",
    "x86_64_control_receiver_second_receive_return",
    "x86_64_control_receiver_empty_return",
    "x86_64_control_receiver_complete_return",
    "x86_64_enter_control_task",
    "x86_64_resume_control_task",
];
const control_private_payload: [u64; 2] = [0x9ad4_c1e7_6b20_8f35, 0x53b8_f02d_a691_4ce7];
const control_forbidden_binary_strings: &[&[u8]] = &[
    b"kernel.control_test.",
    &control_private_payload[0].to_le_bytes(),
    &control_private_payload[1].to_le_bytes(),
    b"human_fixture",
    b"machine_fixture",
    b"receiver_fixture",
    b"human_role_fixture",
    b"machine_role_fixture",
    b"receiver_role_fixture",
];
const expected_control_serial_events: &[&str] = &[
    "boot.kernel.enter",
    "kernel.fault_path.ready",
    "boot.omen.accepted",
    "boot.memory.map",
    "kernel.memory.ready",
    "boot.display.ready",
    "boot.ready",
    "kernel.control_test.ready",
    "kernel.control_test.receiver.ready",
    "kernel.control_test.producer.entered",
    "kernel.control_test.authority.denied",
    "kernel.control_test.message.sent",
    "kernel.control_test.producer.exited",
    "kernel.control_test.producer.withdrawn",
    "kernel.control_test.producer.entered",
    "kernel.control_test.authority.denied",
    "kernel.control_test.message.sent",
    "kernel.control_test.producer.exited",
    "kernel.control_test.producer.withdrawn",
    "kernel.control_test.receiver.entered",
    "kernel.control_test.authority.denied",
    "kernel.control_test.message.received",
    "kernel.control_test.message.received",
    "kernel.control_test.queue.empty",
    "kernel.control_test.lineage.revoked",
    "kernel.control_test.receiver.exited",
    "kernel.control_test.released",
    "kernel.control_test.passed",
];
const control_private_fields: &[&str] = &[
    "address",
    "buffer",
    "content",
    "handle",
    "marker",
    "payload",
    "physical",
    "register",
    "sigil_ref",
];
const rite_event_prefix: &str = "kernel.rite_test.";
const rite_failed_event: &str = "kernel.rite_test.failed";
const rite_private_sentinel_text: &str = "NOX7Q9M4R2X6P8ZK";
const rite_private_sentinel: [u8; 16] = *b"NOX7Q9M4R2X6P8ZK";
const rite_forbidden_symbols: &[&str] = &[
    "run_rite_test",
    "x86_64_rite_gate_entry",
    "x86_64_rite_gate_dispatch",
    "x86_64_rite_test_user_start",
    "x86_64_rite_test_user_end",
    "x86_64_enter_rite_task",
    "x86_64_resume_rite_task",
];
const rite_forbidden_binary_strings: &[&[u8]] = &[
    b"kernel.rite_test.",
    &rite_private_sentinel,
    b"NOX7Q9M4",
    b"R2X6P8ZK",
];
const rite_private_fields: &[&str] = &[
    "address",
    "body",
    "content",
    "exit_body",
    "input",
    "payload",
    "pointer",
    "private",
    "register",
    "secret",
    "text",
    "trace",
];
const expected_rite_serial_events: &[&str] = &[
    "boot.kernel.enter",
    "kernel.fault_path.ready",
    "boot.omen.accepted",
    "boot.memory.map",
    "kernel.memory.ready",
    "boot.display.ready",
    "boot.ready",
    "kernel.rite_test.ready",
    "kernel.rite_test.task.entered",
    "kernel.rite_test.call.rejected",
    "kernel.rite_test.call.rejected",
    "kernel.rite_test.call.rejected",
    "kernel.rite_test.call.rejected",
    "kernel.rite_test.call.rejected",
    "kernel.rite_test.console.committed",
    "kernel.rite_test.input.empty",
    "kernel.rite_test.call.rejected",
    "kernel.rite_test.task.exited",
    "kernel.rite_test.released",
    "kernel.rite_test.passed",
];
const scheduler_event_prefix: &str = "kernel.scheduler_test.";
const scheduler_failed_event: &str = "kernel.scheduler_test.failed";
const scheduler_user_code_address: u64 = 0x0000_7000_0000_1000;
const scheduler_user_stack_top: u64 = 0x0000_7000_0000_6000;
const scheduler_user_data_selector: u64 = 0x002b;
const scheduler_user_code_selector: u64 = 0x0033;
const scheduler_user_flags: u64 = 0x0202;
const scheduler_fault_resume_flag: u64 = 1 << 16;
const scheduler_private_markers: [u64; 6] = [
    0x51a7_c0de_f17e_0001,
    0x51a7_c0de_f17e_0002,
    0x9ee7_c0de_ba5e_0001,
    0x9ee7_c0de_ba5e_0002,
    0xf017_aa55_c0de_0001,
    0x9ee7_aa55_c0de_0002,
];
const scheduler_forbidden_symbols: &[&str] = &[
    "run_scheduler_test",
    "x86_64_scheduler_gate_entry",
    "x86_64_scheduler_gate_dispatch",
    "x86_64_scheduler_faulting_start",
    "x86_64_scheduler_fault_ready_return",
    "x86_64_scheduler_fault_instruction",
    "x86_64_scheduler_fault_failed_instruction",
    "x86_64_scheduler_faulting_end",
    "x86_64_scheduler_peer_start",
    "x86_64_scheduler_peer_ready_return",
    "x86_64_scheduler_peer_completion_return",
    "x86_64_scheduler_peer_failed_instruction",
    "x86_64_scheduler_peer_end",
    "x86_64_scheduler_enter",
    "x86_64_scheduler_resume_kernel",
    "dispatch_timer_interrupt",
    "contain_expected_user_exception",
    "user_interrupt_frame_is_valid",
];
const scheduler_forbidden_binary_strings: &[&[u8]] = &[
    b"kernel.scheduler_test.",
    b"faulting_fixture",
    b"peer_fixture",
    &scheduler_private_markers[0].to_le_bytes(),
    &scheduler_private_markers[1].to_le_bytes(),
    &scheduler_private_markers[2].to_le_bytes(),
    &scheduler_private_markers[3].to_le_bytes(),
    &scheduler_private_markers[4].to_le_bytes(),
    &scheduler_private_markers[5].to_le_bytes(),
];
const scheduler_private_fields: &[&str] = &[
    "address", "pointer", "register", "canary", "marker", "payload", "private",
];
const expected_scheduler_serial_events: &[&str] = &[
    "boot.kernel.enter",
    "kernel.fault_path.ready",
    "boot.omen.accepted",
    "boot.memory.map",
    "kernel.memory.ready",
    "boot.display.ready",
    "boot.ready",
    "kernel.interrupts.ready",
    "kernel.scheduler_test.ready",
    "kernel.scheduler_test.task.ready",
    "kernel.scheduler_test.task.ready",
    "kernel.scheduler_test.task.dispatched",
    "kernel.scheduler_test.task.preempted",
    "kernel.scheduler_test.task.dispatched",
    "kernel.scheduler_test.task.preempted",
    "kernel.scheduler_test.task.resumed",
    "kernel.exception",
    "kernel.scheduler_test.task.faulted",
    "kernel.scheduler_test.task.resumed",
    "kernel.scheduler_test.task.completed",
    "kernel.scheduler_test.interrupts.masked",
    "kernel.scheduler_test.task.withdrawn",
    "kernel.scheduler_test.task.withdrawn",
    "kernel.scheduler_test.released",
    "kernel.scheduler_test.passed",
];
const supervision_event_prefix: &str = "kernel.supervision_test.";
const supervision_failed_event: &str = "kernel.supervision_test.failed";
const supervision_user_code_address: u64 = 0x0000_7000_0000_1000;
const supervision_user_stack_top: u64 = 0x0000_7000_0000_6000;
const supervision_user_data_selector: u64 = 0x002b;
const supervision_user_code_selector: u64 = 0x0033;
const supervision_user_flags: u64 = 0x0202;
const supervision_fault_resume_flag: u64 = 1 << 16;
const supervision_private_markers: [u64; 9] = [
    0xa110_ca11_0001_0001,
    0xa110_ca11_0001_0002,
    0xa110_a11d_0001_0001,
    0x5a9e_ca11_0001_0001,
    0x5a9e_ca11_0001_0002,
    0x5a9e_c017_0001_0001,
    0xa110_ca11_0002_0001,
    0xa110_ca11_0002_0002,
    0xa110_d09e_0002_0001,
];
const supervision_forbidden_symbols: &[&str] = &[
    "run_supervision_test",
    "x86_64_supervision_gate_entry",
    "x86_64_supervision_gate_dispatch",
    "x86_64_supervision_vessel_generation_1_start",
    "x86_64_supervision_vessel_generation_1_ready_return",
    "x86_64_supervision_vessel_generation_1_fault_instruction",
    "x86_64_supervision_vessel_generation_1_end",
    "x86_64_supervision_vessel_generation_2_start",
    "x86_64_supervision_vessel_generation_2_completion_return",
    "x86_64_supervision_vessel_generation_2_end",
    "x86_64_supervision_supervisor_start",
    "x86_64_supervision_supervisor_fault_return",
    "x86_64_supervision_supervisor_completion_return",
    "x86_64_supervision_supervisor_complete_return",
    "x86_64_supervision_supervisor_end",
    "x86_64_supervision_enter",
    "x86_64_supervision_resume_kernel",
    "dispatch_supervision_timer_interrupt",
    "contain_expected_supervision_exception",
    "supervision_user_interrupt_frame_is_valid",
];
const supervision_forbidden_binary_strings: &[&[u8]] = &[
    b"kernel.supervision_test.",
    b"vessel_fixture",
    b"supervisor_fixture",
    b"await_fault",
    b"restart_checkpoint",
    &supervision_private_markers[0].to_le_bytes(),
    &supervision_private_markers[1].to_le_bytes(),
    &supervision_private_markers[2].to_le_bytes(),
    &supervision_private_markers[3].to_le_bytes(),
    &supervision_private_markers[4].to_le_bytes(),
    &supervision_private_markers[5].to_le_bytes(),
    &supervision_private_markers[6].to_le_bytes(),
    &supervision_private_markers[7].to_le_bytes(),
    &supervision_private_markers[8].to_le_bytes(),
];
const supervision_private_fields: &[&str] = &[
    "address", "pointer", "register", "canary", "marker", "mailbox", "payload", "private", "secret",
];
const expected_supervision_serial_events: &[&str] = &[
    "boot.kernel.enter",
    "kernel.fault_path.ready",
    "boot.omen.accepted",
    "boot.memory.map",
    "kernel.memory.ready",
    "boot.display.ready",
    "boot.ready",
    "kernel.interrupts.ready",
    "kernel.supervision_test.ready",
    "kernel.supervision_test.vessel.generation.created",
    "kernel.supervision_test.supervisor.ready",
    "kernel.supervision_test.task.dispatched",
    "kernel.supervision_test.task.preempted",
    "kernel.supervision_test.task.dispatched",
    "kernel.supervision_test.supervisor.blocked",
    "kernel.supervision_test.interrupts.masked",
    "kernel.supervision_test.task.resumed",
    "kernel.exception",
    "kernel.supervision_test.vessel.generation.faulted",
    "kernel.supervision_test.task.resumed",
    "kernel.supervision_test.observation.consumed",
    "kernel.supervision_test.restart.requested",
    "kernel.supervision_test.vessel.generation.withdrawn",
    "kernel.supervision_test.vessel.generation.created",
    "kernel.supervision_test.task.dispatched",
    "kernel.supervision_test.vessel.generation.completed",
    "kernel.supervision_test.task.resumed",
    "kernel.supervision_test.observation.consumed",
    "kernel.supervision_test.supervisor.completed",
    "kernel.supervision_test.vessel.generation.withdrawn",
    "kernel.supervision_test.supervisor.withdrawn",
    "kernel.supervision_test.released",
    "kernel.supervision_test.passed",
];
const shell_vessel_event_prefix: &str = "kernel.shell_vessel_test.";
const shell_vessel_failed_event: &str = "kernel.shell_vessel_test.failed";
const shell_vessel_user_code_address: u64 = 0x0000_7000_0000_1000;
const shell_vessel_user_stack_top: u64 = 0x0000_7000_0000_6000;
const shell_vessel_user_data_selector: u64 = 0x002b;
const shell_vessel_user_code_selector: u64 = 0x0033;
const shell_vessel_user_flags: u64 = 0x0202;
const shell_vessel_fault_resume_flag: u64 = 1 << 16;
const shell_vessel_private_output: &[u8] = b"Anomalous ready.\n";
const shell_vessel_private_source: &[u8] = b"echo Anomalous ready.\n";
const shell_vessel_private_markers: [u64; 16] = [
    0x8f12_c6a4_7bd9_30e1,
    0x17e4_b9c2_6a5d_803f,
    0xa55a_c33c_96e1_7b4d,
    0x6d31_a8f2_c47b_950e,
    0xe20c_759a_13bf_486d,
    0x4b9e_d170_a62c_f835,
    0x91c4_7a2e_6315_b8d0,
    0x2f6d_a905_c37e_184b,
    0xa27d_183f_54c6_90e1,
    0x3b7e_ba16_d48f_295c,
    0xb38e_2940_65d7_a1f2,
    0x4c8f_cb27_e590_3a6d,
    0xc49f_3a51_76e8_b203,
    0x5da0_dc38_f6a1_4b7e,
    0xd5a0_4b62_87f9_c314,
    0x6eb1_ed49_07b2_5c8f,
];
const shell_vessel_forbidden_symbols: &[&str] = &[
    "run_shell_vessel_test",
    "x86_64_shell_vessel_rite_gate_entry",
    "x86_64_shell_vessel_rite_gate_dispatch",
    "x86_64_shell_vessel_rite_dispatch",
    "x86_64_shell_vessel_generation_1_start",
    "x86_64_shell_vessel_generation_1_console_return",
    "x86_64_shell_vessel_generation_1_input_return",
    "x86_64_shell_vessel_generation_1_ready_return",
    "x86_64_shell_vessel_generation_1_fault_instruction",
    "x86_64_shell_vessel_generation_1_end",
    "x86_64_shell_vessel_generation_2_start",
    "x86_64_shell_vessel_generation_2_console_return",
    "x86_64_shell_vessel_generation_2_input_return",
    "x86_64_shell_vessel_generation_2_exit_return",
    "x86_64_shell_vessel_generation_2_completion_return",
    "x86_64_shell_vessel_generation_2_end",
    "x86_64_shell_vessel_supervisor_start",
    "x86_64_shell_vessel_supervisor_fault_return",
    "x86_64_shell_vessel_supervisor_completion_return",
    "x86_64_shell_vessel_supervisor_complete_return",
    "x86_64_shell_vessel_supervisor_end",
    "x86_64_shell_vessel_enter",
    "x86_64_shell_vessel_resume_kernel",
    "dispatch_shell_vessel_timer_interrupt",
    "contain_expected_shell_vessel_exception",
    "shell_vessel_user_interrupt_frame_is_valid",
];
const shell_vessel_forbidden_binary_strings: &[&[u8]] = &[
    b"kernel.shell_vessel_test.",
    b"fixed_supervised_shell",
    b"shell_vessel_fixture",
    b"supervisor_fixture",
    b"await_fault",
    b"restart_checkpoint",
    shell_vessel_private_output,
    b"s ready.",
    shell_vessel_private_source,
    b"echo Ano",
    b"malous r",
    b"eady.\n",
    &shell_vessel_private_markers[0].to_le_bytes(),
    &shell_vessel_private_markers[1].to_le_bytes(),
    &shell_vessel_private_markers[2].to_le_bytes(),
    &shell_vessel_private_markers[3].to_le_bytes(),
    &shell_vessel_private_markers[4].to_le_bytes(),
    &shell_vessel_private_markers[5].to_le_bytes(),
    &shell_vessel_private_markers[6].to_le_bytes(),
    &shell_vessel_private_markers[7].to_le_bytes(),
    &shell_vessel_private_markers[8].to_le_bytes(),
    &shell_vessel_private_markers[9].to_le_bytes(),
    &shell_vessel_private_markers[10].to_le_bytes(),
    &shell_vessel_private_markers[11].to_le_bytes(),
    &shell_vessel_private_markers[12].to_le_bytes(),
    &shell_vessel_private_markers[13].to_le_bytes(),
    &shell_vessel_private_markers[14].to_le_bytes(),
    &shell_vessel_private_markers[15].to_le_bytes(),
];
const shell_vessel_private_fields: &[&str] = &[
    "address",
    "body",
    "buffer",
    "canary",
    "content",
    "exit_body",
    "handle",
    "input",
    "mailbox",
    "marker",
    "payload",
    "physical",
    "pointer",
    "private",
    "register",
    "secret",
    "sigil_ref",
    "text",
    "trace",
];
const expected_shell_vessel_serial_events: &[&str] = &[
    "boot.kernel.enter",
    "kernel.fault_path.ready",
    "boot.omen.accepted",
    "boot.memory.map",
    "kernel.memory.ready",
    "boot.display.ready",
    "boot.ready",
    "kernel.interrupts.ready",
    "kernel.shell_vessel_test.ready",
    "kernel.shell_vessel_test.vessel.generation.created",
    "kernel.shell_vessel_test.supervisor.ready",
    "kernel.shell_vessel_test.task.dispatched",
    "kernel.shell_vessel_test.rite.console.committed",
    "kernel.shell_vessel_test.rite.input.empty",
    "kernel.shell_vessel_test.task.preempted",
    "kernel.shell_vessel_test.task.dispatched",
    "kernel.shell_vessel_test.supervisor.blocked",
    "kernel.shell_vessel_test.interrupts.masked",
    "kernel.shell_vessel_test.task.resumed",
    "kernel.exception",
    "kernel.shell_vessel_test.vessel.generation.faulted",
    "kernel.shell_vessel_test.task.resumed",
    "kernel.shell_vessel_test.observation.consumed",
    "kernel.shell_vessel_test.restart.requested",
    "kernel.shell_vessel_test.vessel.generation.withdrawn",
    "kernel.shell_vessel_test.vessel.generation.created",
    "kernel.shell_vessel_test.task.dispatched",
    "kernel.shell_vessel_test.rite.console.committed",
    "kernel.shell_vessel_test.rite.input.empty",
    "kernel.shell_vessel_test.rite.task.exited",
    "kernel.shell_vessel_test.vessel.generation.completed",
    "kernel.shell_vessel_test.task.resumed",
    "kernel.shell_vessel_test.observation.consumed",
    "kernel.shell_vessel_test.supervisor.completed",
    "kernel.shell_vessel_test.vessel.generation.withdrawn",
    "kernel.shell_vessel_test.supervisor.withdrawn",
    "kernel.shell_vessel_test.released",
    "kernel.shell_vessel_test.passed",
];
const input_wait_event_prefix: &str = "kernel.input_wait_test.";
const input_wait_failed_event: &str = "kernel.input_wait_test.failed";
const input_wait_private_text: &[u8] = &[0x41, 0x0a, 0xe2, 0x98, 0x83];
const input_wait_trace_1: [u8; 16] = [
    0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x1b, 0x1c, 0x1d, 0x1e, 0x1f,
];
const input_wait_trace_2: [u8; 16] = [
    0x20, 0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27, 0x28, 0x29, 0x2a, 0x2b, 0x2c, 0x2d, 0x2e, 0x2f,
];
const input_wait_trace_3: [u8; 16] = [
    0x30, 0x31, 0x32, 0x33, 0x34, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3a, 0x3b, 0x3c, 0x3d, 0x3e, 0x3f,
];
const input_wait_forbidden_symbols: &[&str] = &[
    "run_input_wait_test",
    "x86_64_input_wait_rite_dispatch",
    "x86_64_input_wait_rite_gate_entry",
    "x86_64_input_wait_consumer_start",
    "x86_64_input_wait_consumer_immediate_return",
    "x86_64_input_wait_consumer_delivery_return",
    "x86_64_input_wait_consumer_interrupted_return",
    "x86_64_input_wait_consumer_completion_return",
    "x86_64_input_wait_consumer_end",
    "x86_64_input_wait_peer_start",
    "x86_64_input_wait_peer_publish_return",
    "x86_64_input_wait_peer_preempt_return",
    "x86_64_input_wait_peer_close_return",
    "x86_64_input_wait_peer_fault_instruction",
    "x86_64_input_wait_peer_end",
];
const input_wait_forbidden_binary_strings: &[&[u8]] = &[
    b"kernel.input_wait_test.",
    b"fixed_session_input",
    input_wait_private_text,
    &input_wait_trace_1,
    &input_wait_trace_2,
    &input_wait_trace_3,
];
const expected_input_wait_serial_events: &[&str] = &[
    "boot.kernel.enter",
    "kernel.fault_path.ready",
    "boot.omen.accepted",
    "boot.memory.map",
    "kernel.memory.ready",
    "boot.display.ready",
    "boot.ready",
    "kernel.interrupts.ready",
    "kernel.input_wait_test.ready",
    "kernel.input_wait_test.immediate.completed",
    "kernel.input_wait_test.wait.blocked",
    "kernel.input_wait_test.publication.woke",
    "kernel.input_wait_test.delivery.selected",
    "kernel.input_wait_test.wait.blocked_again",
    "kernel.input_wait_test.route.closed",
    "kernel.input_wait_test.fault.selected",
    "kernel.input_wait_test.interruption.completed",
    "kernel.input_wait_test.released",
    "kernel.input_wait_test.passed",
];
const memory_event_prefix: &str = "kernel.memory_test.";
const memory_failed_event: &str = "kernel.memory_test.failed";
const memory_frame_size: u64 = 4096;
const memory_address_limit: u64 = 4 * 1024 * 1024 * 1024;
const memory_total_frames: u64 = memory_address_limit / memory_frame_size;
const memory_test_virtual_address: u64 = 0x0000_5000_0000_0000;
const memory_test_marker_a: u64 = 0xa11c_a11c_a11c_a11c;
const memory_test_marker_b: u64 = 0xb22d_b22d_b22d_b22d;
const memory_test_data_frames: u64 = 2;
const memory_test_table_frames: u64 = 8;
const memory_test_root_frame_count: u64 = memory_test_table_frames / 2;
const memory_boot_omen_version: u64 = 2;
const memory_boot_drive: u64 = 0x80;
const memory_maximum_regions: u64 = 64;
const memory_control_abi: &str = "0.1";
const memory_test_forbidden_binary_strings: &[&[u8]] = &[
    b"kernel.memory_test.",
    &memory_test_marker_a.to_le_bytes(),
    &memory_test_marker_b.to_le_bytes(),
];
const interrupt_event_prefix: &str = "kernel.interrupt_test.";
const interrupt_ready_event: &str = r#""event":"kernel.interrupts.ready""#;
const interrupt_timer_event: &str = r#""event":"kernel.interrupt_test.timer.verified""#;
const interrupt_rearmed_event: &str = r#""event":"kernel.interrupt_test.loss.rearmed""#;
const interrupt_passed_event: &str = r#""event":"kernel.interrupt_test.passed""#;
const interrupt_forbidden_symbols: &[&str] = &[
    "run_interrupt_test",
    "interrupt_test_force_loss",
    "interrupt_test_entry",
    "force_ps2_capacity_loss_for_test",
    "read_external_interrupt_health",
    "run_interrupt_return_probe",
    "x86_64_timer_interrupt_count",
];
const interrupt_forbidden_binary_strings: &[&[u8]] = &[b"kernel.interrupt_test.", b"privateprobe"];
const interrupt_private_fields: &[&str] = &[
    "scan",
    "scan_byte",
    "scan_code",
    "key",
    "physical_key",
    "text",
    "content",
    "raw",
    "payload",
];
const interrupt_minimum_ticks: u64 = 8;
const interrupt_expected_transitions: u64 = 24;
const interrupt_expected_queue_capacity: u64 = 64;
const interrupt_expected_lost_count: u64 = 1;
const interrupt_expected_rearmed_generation: u64 = 2;
const interrupt_expected_post_rearm_transitions: u64 = 2;
const interrupt_post_rearm_probe: &str = "x";
const display_vessel_event_prefix: &str = "kernel.display_vessel_test.";
const display_vessel_failed_event: &str = "kernel.display_vessel_test.failed";
const display_vessel_passed_event: &str = "kernel.display_vessel_test.passed";
const display_vessel_fixture_marker: &[u8; 16] = b"ANOM-DISP-V1-X64";
const display_vessel_width: usize = 1024;
const display_vessel_height: usize = 768;
const display_vessel_channels: usize = 3;
const display_vessel_transcript_limit: usize = 64 * 1024;
const display_vessel_stderr_limit: usize = 64 * 1024;
const display_vessel_ppm_header: &[u8] = b"P6\n1024 768\n255\n";
const display_vessel_forbidden_symbols: &[&str] = &[
    "framebuffer_presenter",
    "prepared_framebuffer_patch",
    "run_display_vessel_test",
    "x86_64_display_vessel_enter_task",
    "x86_64_display_vessel_resume_task",
    "x86_64_display_vessel_gate_entry",
    "x86_64_display_vessel_gate_dispatch",
    "x86_64_display_vessel_user_start",
    "x86_64_display_vessel_user_end",
    "x86_64_display_vessel_denied_return",
    "x86_64_display_vessel_malformed_return",
    "x86_64_display_vessel_bounds_return",
    "x86_64_display_vessel_row_return",
    "x86_64_display_vessel_exit_return",
    "x86_64_display_vessel_feature_sentinel",
];
const display_vessel_required_symbols: &[&str] = &[
    "x86_64_display_vessel_enter_task",
    "x86_64_display_vessel_resume_task",
    "x86_64_display_vessel_gate_entry",
    "x86_64_display_vessel_gate_dispatch",
    "x86_64_display_vessel_user_start",
    "x86_64_display_vessel_user_end",
    "x86_64_display_vessel_denied_return",
    "x86_64_display_vessel_malformed_return",
    "x86_64_display_vessel_bounds_return",
    "x86_64_display_vessel_row_return",
    "x86_64_display_vessel_exit_return",
    "x86_64_display_vessel_feature_sentinel",
];
const display_vessel_forbidden_binary_strings: &[&[u8]] = &[
    b"kernel.display_vessel_test.",
    display_vessel_fixture_marker,
];
const display_vessel_private_fields: &[&str] = &[
    "address",
    "body",
    "buffer",
    "content",
    "handle",
    "marker",
    "payload",
    "physical",
    "pixel",
    "pointer",
    "screenshot",
    "trace",
];
const expected_display_vessel_serial_events: &[&str] = &[
    "boot.kernel.enter",
    "kernel.fault_path.ready",
    "boot.omen.accepted",
    "boot.memory.map",
    "kernel.memory.ready",
    "boot.display.ready",
    "boot.ready",
    "kernel.display_vessel_test.ready",
    "kernel.display_vessel_test.task.entered",
    "kernel.display_vessel_test.call.rejected",
    "kernel.display_vessel_test.call.rejected",
    "kernel.display_vessel_test.call.rejected",
    "kernel.display_vessel_test.frame.committed",
    "kernel.display_vessel_test.task.exited",
    "kernel.display_vessel_test.released",
    "kernel.display_vessel_test.passed",
];

const supervised_renderer_feature: &str = "supervised_renderer_test";
const semantic_surface_transport_feature: &str = "semantic_surface_transport_test";
const semantic_surface_presentation_feature: &str = "semantic_surface_presentation_test";
const supervised_renderer_all_kernel_features: &[&str] = &[
    "smoke_test",
    "fault_test",
    "circle_test",
    "control_test",
    "rite_test",
    "memory_test",
    "interrupt_test",
    "scheduler_test",
    "supervision_test",
    "shell_vessel_test",
    "input_wait_test",
    "display_vessel_test",
    supervised_renderer_feature,
];
const semantic_surface_transport_all_kernel_features: &[&str] = &[
    "smoke_test",
    "fault_test",
    "circle_test",
    "control_test",
    "rite_test",
    "memory_test",
    "interrupt_test",
    "scheduler_test",
    "supervision_test",
    "shell_vessel_test",
    "input_wait_test",
    "display_vessel_test",
    supervised_renderer_feature,
    semantic_surface_transport_feature,
];
const all_kernel_features: &[&str] = &[
    "smoke_test",
    "fault_test",
    "circle_test",
    "control_test",
    "rite_test",
    "memory_test",
    "interrupt_test",
    "scheduler_test",
    "supervision_test",
    "shell_vessel_test",
    "input_wait_test",
    "display_vessel_test",
    supervised_renderer_feature,
    semantic_surface_transport_feature,
    semantic_surface_presentation_feature,
    desktop_dev::feature,
    workspace_dev::feature,
    owned_workspace_cpu::feature,
    owned_workspace::feature,
    dispatcher_smoke::feature,
    owned_dispatcher_smoke::feature,
    owned_multi_region_smoke::feature,
    owned_source_smoke::feature,
    owned_tls_smoke::feature,
    application_profile_smoke::feature,
    mixed_workspace::feature,
    native_editor_workspace::feature,
    ny_object_smoke::feature,
    native_application_smoke::feature,
    native_runtime_smoke::feature,
    native_manifest_smoke::feature,
    native_program_smoke::feature,
    native_durable_smoke::feature,
    native_durable_failure_smoke::feature,
    native_repeated_smoke::feature,
    native_interactive_smoke::feature,
    native_document_smoke::feature,
    elf_document_smoke::feature,
    native_relaunch_smoke::feature,
    elf_relaunch_smoke::feature,
    elf_application_smoke::feature,
    linux_application_smoke::feature,
    veil_application_smoke::feature,
    shared_veil_smoke::feature,
    linux_tls_smoke::feature,
    linux_loader_entry_smoke::feature,
];
const semantic_surface_target_name: &str = "x86_64-anomalous-semantic-surface";
const semantic_surface_target_spec: &str = "targets/x86_64-anomalous-semantic-surface.json";
const semantic_surface_image_base: u64 = 0x0000_6000_0000_1000;
const semantic_surface_page_limit: u64 = 64;
const semantic_surface_renderer_mailbox: u64 = 0x0000_6000_0008_0000;
const semantic_surface_snapshot_size: usize = 528;
const semantic_surface_source_variant_environment: &str =
    "ANOMALOUS_SEMANTIC_SURFACE_SOURCE_VARIANT";
const semantic_surface_private_source_variants: &[(u8, usize)] = &[(0x35, 29), (0xa7, 71)];
const semantic_surface_event_prefix: &str = "kernel.semantic_surface_transport.";
const semantic_surface_expected_events: &[&str] = &[
    "boot.kernel.enter",
    "kernel.fault_path.ready",
    "boot.omen.accepted",
    "boot.memory.map",
    "kernel.memory.ready",
    "boot.display.ready",
    "boot.ready",
    "kernel.interrupts.ready",
    "kernel.semantic_surface_transport.ready",
    "kernel.semantic_surface_transport.source.released",
    "kernel.semantic_surface_transport.consumer.released",
    "kernel.semantic_surface_transport.released",
    "kernel.semantic_surface_transport.passed",
];
const semantic_presentation_event_prefix: &str = "kernel.semantic_surface_presentation_test.";
const semantic_presentation_failed_event: &str = "kernel.semantic_surface_presentation_test.failed";
const semantic_presentation_passed_event: &str = "kernel.semantic_surface_presentation_test.passed";
const semantic_presentation_transcript_limit: usize = 64 * 1024;
const semantic_presentation_stderr_limit: usize = 64 * 1024;
const semantic_presentation_expected_events: &[&str] = &[
    "boot.kernel.enter",
    "kernel.fault_path.ready",
    "boot.omen.accepted",
    "boot.memory.map",
    "kernel.memory.ready",
    "boot.display.ready",
    "boot.ready",
    "kernel.interrupts.ready",
    "kernel.semantic_surface_presentation_test.ready",
    "kernel.semantic_surface_presentation_test.source_released",
    "kernel.semantic_surface_presentation_test.render_acknowledged",
    "kernel.semantic_surface_presentation_test.prepared",
    "kernel.semantic_surface_presentation_test.presented",
    "kernel.semantic_surface_presentation_test.released",
    "kernel.semantic_surface_presentation_test.passed",
];
const semantic_presentation_private_fields: &[&str] = &[
    "address",
    "attempt",
    "body",
    "buffer",
    "content",
    "endpoint",
    "generation",
    "label",
    "payload",
    "pixel",
    "pointer",
    "private",
    "revision",
    "scene",
    "snapshot",
    "surface",
    "task",
    "trace",
];
const semantic_presentation_forbidden_binary_strings: &[&[u8]] = &[
    b"kernel.semantic_surface_presentation_test.",
    b"semantic_surface_presentation_test",
];
const semantic_presentation_required_binary_strings: &[&[u8]] = &[
    b"kernel.semantic_surface_presentation_test.ready",
    b"kernel.semantic_surface_presentation_test.passed",
];
const semantic_presentation_required_symbols: &[&str] = &[
    "x86_64_semantic_surface_presentation_rite_gate_entry",
    "x86_64_semantic_surface_presentation_supervisor_start",
    "x86_64_semantic_surface_presentation_supervisor_completion_return",
    "x86_64_semantic_surface_presentation_supervisor_complete_return",
    "x86_64_semantic_surface_presentation_supervisor_end",
];
const semantic_surface_forbidden_binary_strings: &[&[u8]] = &[
    b"kernel.semantic_surface_transport.",
    b"semantic_surface_transport_test",
];
// Semantic-surface transport deliberately reuses the scheduler frame and the
// enter/resume assembly from `scheduler_test`.  Precedence must therefore
// reject the former scheduler *fixture*, rather than the generic support
// module or its shared entry points.
const semantic_surface_preceded_fixture_symbols: &[&str] = &[
    "run_supervised_renderer_test",
    "run_display_vessel_test",
    "run_input_wait_test",
    "run_shell_vessel_test",
    "run_supervision_test",
    "run_scheduler_test",
    "run_interrupt_test",
    "run_memory_test",
    "run_rite_test",
    "run_control_test",
    "run_circle_test",
    "run_fault_test",
];
const semantic_surface_preceded_fixture_event_markers: &[&[u8]] = &[
    b"kernel.supervised_renderer_test.passed",
    b"kernel.display_vessel_test.passed",
    b"kernel.input_wait_test.passed",
    b"kernel.shell_vessel_test.passed",
    b"kernel.supervision_test.passed",
    b"kernel.scheduler_test.passed",
    b"kernel.interrupt_test.passed",
    b"kernel.memory_test.passed",
    b"kernel.rite_test.passed",
    b"kernel.control_test.passed",
    b"kernel.circle_test.passed",
    b"kernel.fault_test.passed",
];
const semantic_surface_source_symbols: &[&str] = &[
    "anomalous_semantic_surface_source_image_start",
    "anomalous_semantic_surface_source_image_payload_end",
    "anomalous_semantic_surface_source_image_end",
    "anomalous_semantic_surface_source_snapshot_start",
    "anomalous_semantic_surface_source_snapshot_end",
    "anomalous_semantic_surface_source_entry",
    "anomalous_semantic_surface_source_read_denied_return",
    "anomalous_semantic_surface_source_malformed_submit_return",
    "anomalous_semantic_surface_source_freshness_denied_return",
    "anomalous_semantic_surface_source_submit_return",
    "anomalous_semantic_surface_source_backpressured_return",
    "anomalous_semantic_surface_source_exit_return",
];
const semantic_surface_consumer_symbols: &[&str] = &[
    "anomalous_semantic_surface_consumer_image_start",
    "anomalous_semantic_surface_consumer_image_payload_end",
    "anomalous_semantic_surface_consumer_image_end",
    "anomalous_semantic_surface_consumer_entry",
    "anomalous_semantic_surface_consumer_submit_denied_return",
    "anomalous_semantic_surface_consumer_malformed_read_return",
    "anomalous_semantic_surface_consumer_delivery_return",
    "anomalous_semantic_surface_consumer_empty_return",
    "anomalous_semantic_surface_consumer_exit_return",
];
const semantic_surface_source_dependencies: &[&str] =
    &["anomalous-abi", "anomalous-semantic-surface-source"];
const semantic_surface_consumer_dependencies: &[&str] = &[
    "anomalous-abi",
    "anomalous-desktop-transport",
    "anomalous-semantic-surface-consumer",
];
const semantic_surface_renderer_symbols: &[&str] = &[
    "anomalous_semantic_surface_renderer_image_start",
    "anomalous_semantic_surface_renderer_image_payload_end",
    "anomalous_semantic_surface_renderer_image_end",
    "anomalous_semantic_surface_renderer_entry",
    "anomalous_semantic_surface_renderer_submit_denied_return",
    "anomalous_semantic_surface_renderer_delivery_return",
    "anomalous_semantic_surface_renderer_display_return",
    "anomalous_semantic_surface_renderer_exit_return",
    "anomalous_semantic_surface_renderer_failed",
];
const semantic_surface_renderer_dependencies: &[&str] = &[
    "anomalous-abi",
    "anomalous-desktop-renderer",
    "anomalous-desktop-transport",
    "anomalous-semantic-surface-renderer",
];

#[derive(Clone, Copy, Debug)]
struct semantic_surface_package {
    package: &'static str,
    role: &'static str,
    required_symbols: &'static [&'static str],
    wrapper_symbols: &'static [&'static str],
    dependencies: &'static [&'static str],
    snapshot: bool,
}

const semantic_surface_source_package: semantic_surface_package = semantic_surface_package {
    package: "anomalous-semantic-surface-source",
    role: "source",
    required_symbols: semantic_surface_source_symbols,
    wrapper_symbols: &[
        "x86_64_semantic_surface_source_entry",
        "x86_64_semantic_surface_source_read_denied_return",
        "x86_64_semantic_surface_source_malformed_submit_return",
        "x86_64_semantic_surface_source_freshness_denied_return",
        "x86_64_semantic_surface_source_submit_return",
        "x86_64_semantic_surface_source_backpressured_return",
        "x86_64_semantic_surface_source_exit_return",
    ],
    dependencies: semantic_surface_source_dependencies,
    snapshot: true,
};
const semantic_surface_consumer_package: semantic_surface_package = semantic_surface_package {
    package: "anomalous-semantic-surface-consumer",
    role: "consumer",
    required_symbols: semantic_surface_consumer_symbols,
    wrapper_symbols: &[
        "x86_64_semantic_surface_consumer_entry",
        "x86_64_semantic_surface_consumer_submit_denied_return",
        "x86_64_semantic_surface_consumer_malformed_read_return",
        "x86_64_semantic_surface_consumer_delivery_return",
        "x86_64_semantic_surface_consumer_empty_return",
        "x86_64_semantic_surface_consumer_exit_return",
    ],
    dependencies: semantic_surface_consumer_dependencies,
    snapshot: false,
};
const semantic_surface_renderer_package: semantic_surface_package = semantic_surface_package {
    package: "anomalous-semantic-surface-renderer",
    role: "renderer",
    required_symbols: semantic_surface_renderer_symbols,
    wrapper_symbols: &[
        "x86_64_semantic_surface_renderer_entry",
        "x86_64_semantic_surface_renderer_submit_denied_return",
        "x86_64_semantic_surface_renderer_delivery_return",
        "x86_64_semantic_surface_renderer_display_return",
        "x86_64_semantic_surface_renderer_exit_return",
        "x86_64_semantic_surface_renderer_failed",
    ],
    dependencies: semantic_surface_renderer_dependencies,
    snapshot: false,
};

#[derive(Clone, Debug)]
struct semantic_surface_artifact {
    package: &'static semantic_surface_package,
    binary: PathBuf,
    object: PathBuf,
    file_size: u64,
    offsets: Vec<u64>,
    snapshot: Option<Vec<u8>>,
    private_probe_out_directories: Vec<PathBuf>,
}

#[derive(Clone, Debug)]
struct semantic_surface_artifacts {
    source: semantic_surface_artifact,
    consumer: semantic_surface_artifact,
}
const supervised_renderer_package: &str = "anomalous-supervised-renderer";
const supervised_renderer_target_name: &str = "x86_64-anomalous-renderer";
const supervised_renderer_target_spec: &str = "targets/x86_64-anomalous-renderer.json";
const supervised_renderer_source_variant_environment: &str =
    "ANOMALOUS_RENDERER_PROTECTED_SOURCE_VARIANT";
const supervised_renderer_source_variant_first: &str = "first";
const supervised_renderer_source_variant_second: &str = "second";
const supervised_renderer_image_base: u64 = 0x0000_6000_0000_1000;
const supervised_renderer_generation_2_entry: u64 = supervised_renderer_image_base + 0x200;
const supervised_renderer_image_page_limit: u64 = 64;
const supervised_renderer_mailbox: u64 = 0x0000_6000_0008_0000;
const supervised_renderer_stack_base: u64 = 0x0000_6000_0009_0000;
const supervised_renderer_stack_top: u64 = supervised_renderer_stack_base + 4 * circle_page_size;
const supervised_renderer_capsule_size: usize = 416;
const supervised_renderer_capsule_magic: &[u8; 4] = b"ANR1";
const supervised_renderer_capsule_header: [u8; 16] = [
    0x41, 0x4e, 0x52, 0x31, 0x00, 0x04, 0x00, 0x03, 0x19, 0x3d, 0x06, 0x00, 0x00, 0x00, 0x00, 0x00,
];
const supervised_renderer_generation_1_canary_a: u64 = 0x72d1_5ca8_4e30_b691;
const supervised_renderer_generation_1_canary_b: u64 = 0xc438_9fe2_17a6_5bd0;
const supervised_renderer_generation_1_arm: u64 = 0xa671_3dc9_e254_8b0f;
const supervised_renderer_supervisor_value_a: u64 = 0x196b_f3a0_4d72_c85e;
const supervised_renderer_supervisor_value_b: u64 = 0xd4a7_0e5b_832c_61f9;
const supervised_renderer_supervisor_continuity: u64 = 0x5ec1_28a7_f40d_936b;
const supervised_renderer_glyph_fingerprint: [u8; 16] = [
    0x58, 0x8d, 0x26, 0x27, 0xb9, 0x45, 0x4b, 0x5f, 0x50, 0x39, 0xff, 0xaf, 0x7c, 0xc4, 0xe3, 0x53,
];
const supervised_renderer_private_source_sentinels: &[&[u8]] = &[
    b"tahovcjqxelszgnubipwdkryfmtahovcjqxel",
    b"xitepalwhsdozkvgrcnyjufqbmxitepalwhsd",
];
const supervised_renderer_forbidden_binary_strings: &[&[u8]] = &[
    &supervised_renderer_capsule_header,
    &supervised_renderer_generation_1_canary_a.to_le_bytes(),
    &supervised_renderer_generation_1_canary_b.to_le_bytes(),
    &supervised_renderer_generation_1_arm.to_le_bytes(),
    &supervised_renderer_supervisor_value_a.to_le_bytes(),
    &supervised_renderer_supervisor_value_b.to_le_bytes(),
    &supervised_renderer_supervisor_continuity.to_le_bytes(),
    &supervised_renderer_glyph_fingerprint,
    b"kernel.supervised_renderer_test.",
    b"initial_renderer_state_invalid",
];
const supervised_renderer_binary_name: &str = "supervised_renderer.bin";
const supervised_renderer_elf_name: &str = "supervised_renderer.elf";
const supervised_renderer_object_name: &str = "supervised_renderer_payload.o";
const supervised_renderer_payload_section: &str = ".rodata.supervised_renderer.payload";
const supervised_renderer_payload_start: &str = "x86_64_supervised_renderer_payload_start";
const supervised_renderer_payload_end: &str = "x86_64_supervised_renderer_payload_end";
const supervised_renderer_payload_size: &str = "x86_64_supervised_renderer_payload_size";
const supervised_renderer_event_prefix: &str = "kernel.supervised_renderer_test.";
const supervised_renderer_failed_event: &str = "kernel.supervised_renderer_test.failed";
const supervised_renderer_passed_event: &str =
    r#""event":"kernel.supervised_renderer_test.passed""#;
const supervised_renderer_transcript_limit: usize = 64 * 1024;
const supervised_renderer_stderr_limit: usize = 64 * 1024;
const supervised_renderer_private_fields: &[&str] = &[
    "address",
    "body",
    "buffer",
    "capsule",
    "content",
    "handle",
    "label",
    "marker",
    "payload",
    "physical",
    "pixel",
    "pointer",
    "protected",
    "screenshot",
    "trace",
];
const expected_supervised_renderer_serial_events: &[&str] = &[
    "boot.kernel.enter",
    "kernel.fault_path.ready",
    "boot.omen.accepted",
    "boot.memory.map",
    "kernel.memory.ready",
    "boot.display.ready",
    "boot.ready",
    "kernel.interrupts.ready",
    "kernel.supervised_renderer_test.ready",
    "kernel.supervised_renderer_test.renderer.entered",
    "kernel.supervised_renderer_test.renderer.preempted",
    "kernel.supervised_renderer_test.generation.staged",
    "kernel.supervised_renderer_test.generation.faulted",
    "kernel.supervised_renderer_test.generation.released",
    "kernel.supervised_renderer_test.renderer.restarted",
    "kernel.supervised_renderer_test.generation.staged",
    "kernel.supervised_renderer_test.renderer.exited",
    "kernel.supervised_renderer_test.supervisor.completed",
    "kernel.supervised_renderer_test.frame.committed",
    "kernel.supervised_renderer_test.released",
    "kernel.supervised_renderer_test.passed",
];
const expected_supervised_renderer_serial_fields: &[&[&str]] = &[
    &["schema", "event", "level", "arch"],
    &[
        "schema",
        "event",
        "level",
        "vectors",
        "tss",
        "double_fault_ist",
        "nmi_ist",
        "nmi_masked",
    ],
    &["schema", "event", "level", "version", "boot_drive"],
    &["schema", "event", "level", "regions", "usable_bytes"],
    &[
        "schema",
        "event",
        "level",
        "frame_size",
        "address_limit",
        "total_frames",
        "free_frames",
        "unavailable_frames",
        "data_owned_frames",
        "table_owned_frames",
    ],
    &[
        "schema", "event", "level", "width", "height", "pitch", "bpp",
    ],
    &["schema", "event", "level", "control_abi", "framebuffer"],
    &[
        "schema",
        "event",
        "level",
        "backend",
        "vector_base",
        "vector_count",
        "pit_input_hz",
        "pit_divisor",
        "timebase_generation",
        "initial_master_mask",
        "initial_slave_mask",
        "master_mask",
        "slave_mask",
        "input_available",
        "input_source_generation",
        "input_queue_capacity",
        "nmi_masked",
        "if_enabled",
    ],
    &[
        "schema",
        "event",
        "level",
        "arch",
        "grant",
        "image_pages",
        "mappings",
        "mailbox_pages",
        "stack_pages",
        "stage_frames",
        "supervisors",
        "renderer_limit",
        "restart_budget",
        "timer_budget",
        "nmi_masked",
    ],
    &[
        "schema",
        "event",
        "level",
        "generation",
        "origin",
        "image_pages",
        "mappings",
        "code_permissions",
        "mailbox_permissions",
        "stack_permissions",
        "guards_absent",
        "stage_generation",
        "sequence_start",
    ],
    &[
        "schema",
        "event",
        "level",
        "generation",
        "timer_interrupts",
        "timer_masked",
        "flags_normalized",
    ],
    &[
        "schema",
        "event",
        "level",
        "generation",
        "rows",
        "complete",
        "surface_unchanged",
    ],
    &[
        "schema",
        "event",
        "level",
        "generation",
        "vector",
        "kind",
        "contained",
        "supervisor_woken",
        "fault_site_verified",
        "saved_frame_verified",
    ],
    &[
        "schema",
        "event",
        "level",
        "generation",
        "stage_cleared",
        "channel_retired",
        "mappings_removed",
        "root_withdrawn",
        "privilege_stack_wiped",
        "mailbox_wiped",
        "image_wiped",
        "scratch_wiped",
        "task_destroyed",
        "circle_destroyed",
        "surface_unchanged",
    ],
    &[
        "schema",
        "event",
        "level",
        "generation",
        "fresh_task",
        "fresh_circle",
        "fresh_channel",
        "fresh_stage",
        "sequence_start",
    ],
    &[
        "schema",
        "event",
        "level",
        "generation",
        "rows",
        "complete",
        "surface_unchanged",
    ],
    &[
        "schema",
        "event",
        "level",
        "generation",
        "terminal",
        "replied",
        "supervisor_woken",
    ],
    &[
        "schema",
        "event",
        "level",
        "observations",
        "faults",
        "restarts",
        "completions",
        "success",
    ],
    &[
        "schema",
        "event",
        "level",
        "frames",
        "rows",
        "surface_withdrawn",
        "preflight_complete",
    ],
    &[
        "schema",
        "event",
        "level",
        "stages",
        "tasks",
        "circles",
        "channels",
        "free_frames",
        "data_owned_frames",
        "table_owned_frames",
        "active_leases",
        "bound_leases",
        "active_circles",
        "active_tasks",
        "active_mappings",
        "active_endpoints",
        "active_sigils",
        "active_bindings",
        "active_lineages",
        "active_supervisions",
        "supervision_children",
        "scheduler_depth",
        "attached_contexts",
        "active_rite_channels",
        "stage_cleared",
        "resources_restored",
        "interrupts_masked",
        "supervisor_retired",
        "channels_retired",
        "mailboxes_wiped",
        "stacks_wiped",
        "images_wiped",
        "scratch_wiped",
        "kernel_cr3_restored",
        "tss_rsp0_restored",
        "ts_restored",
    ],
    &[
        "schema",
        "event",
        "level",
        "generations",
        "timer_interrupts",
        "rows_generation_1",
        "rows_generation_2",
        "faults",
        "restarts",
        "terminal_exits",
        "frame_commits",
        "resources_restored",
    ],
];
const supervised_renderer_linked_symbols: &[supervised_renderer_linked_symbol] = &[
    supervised_renderer_linked_symbol {
        elf_name: "anomalous_supervised_renderer_generation_1_entry",
        wrapper_name: "x86_64_supervised_renderer_generation_1_entry",
    },
    supervised_renderer_linked_symbol {
        elf_name: "anomalous_supervised_renderer_generation_1_ready_return",
        wrapper_name: "x86_64_supervised_renderer_generation_1_ready_return",
    },
    supervised_renderer_linked_symbol {
        elf_name: "anomalous_supervised_renderer_generation_1_preemption_wait",
        wrapper_name: "x86_64_supervised_renderer_generation_1_preemption_wait",
    },
    supervised_renderer_linked_symbol {
        elf_name: "anomalous_supervised_renderer_generation_1_preemption_resume",
        wrapper_name: "x86_64_supervised_renderer_generation_1_preemption_resume",
    },
    supervised_renderer_linked_symbol {
        elf_name: "anomalous_supervised_renderer_generation_1_fault",
        wrapper_name: "x86_64_supervised_renderer_generation_1_fault",
    },
    supervised_renderer_linked_symbol {
        elf_name: "anomalous_supervised_renderer_generation_1_failed",
        wrapper_name: "x86_64_supervised_renderer_generation_1_failed",
    },
    supervised_renderer_linked_symbol {
        elf_name: "anomalous_supervised_renderer_generation_2_entry",
        wrapper_name: "x86_64_supervised_renderer_generation_2_entry",
    },
    supervised_renderer_linked_symbol {
        elf_name: "anomalous_supervised_renderer_generation_2_exit_return",
        wrapper_name: "x86_64_supervised_renderer_generation_2_exit_return",
    },
    supervised_renderer_linked_symbol {
        elf_name: "anomalous_supervised_renderer_generation_2_failed",
        wrapper_name: "x86_64_supervised_renderer_generation_2_failed",
    },
    supervised_renderer_linked_symbol {
        elf_name: "anomalous_supervised_renderer_submit_display_rite",
        wrapper_name: "x86_64_supervised_renderer_submit_display_rite",
    },
    supervised_renderer_linked_symbol {
        elf_name: "anomalous_supervised_renderer_display_rite_return",
        wrapper_name: "x86_64_supervised_renderer_display_rite_return",
    },
    supervised_renderer_linked_symbol {
        elf_name: "anomalous_supervised_renderer_submit_exit_rite",
        wrapper_name: "x86_64_supervised_renderer_submit_exit_rite",
    },
    supervised_renderer_linked_symbol {
        elf_name: "anomalous_supervised_renderer_task_exit_rite_return",
        wrapper_name: "x86_64_supervised_renderer_task_exit_rite_return",
    },
    supervised_renderer_linked_symbol {
        elf_name: "anomalous_supervised_renderer_glyph_fingerprint",
        wrapper_name: "x86_64_supervised_renderer_glyph_fingerprint",
    },
];
const supervised_renderer_required_symbols: &[&str] = &[
    "anomalous_supervised_renderer_image_start",
    "anomalous_supervised_renderer_image_payload_end",
    "anomalous_supervised_renderer_image_end",
    "anomalous_supervised_renderer_capsule_start",
    "anomalous_supervised_renderer_capsule_end",
    "anomalous_supervised_renderer_generation_1_entry",
    "anomalous_supervised_renderer_generation_1_ready_return",
    "anomalous_supervised_renderer_generation_1_preemption_wait",
    "anomalous_supervised_renderer_generation_1_preemption_resume",
    "anomalous_supervised_renderer_generation_1_fault",
    "anomalous_supervised_renderer_generation_1_failed",
    "anomalous_supervised_renderer_generation_2_entry",
    "anomalous_supervised_renderer_generation_2_exit_return",
    "anomalous_supervised_renderer_generation_2_failed",
    "anomalous_supervised_renderer_submit_display_rite",
    "anomalous_supervised_renderer_display_rite_return",
    "anomalous_supervised_renderer_submit_exit_rite",
    "anomalous_supervised_renderer_task_exit_rite_return",
    "anomalous_supervised_renderer_glyph_fingerprint",
];
const supervised_renderer_kernel_required_symbols: &[&str] = &[
    "x86_64_supervised_renderer_payload_start",
    "x86_64_supervised_renderer_payload_end",
    "x86_64_supervised_renderer_payload_size",
    "x86_64_supervised_renderer_generation_1_entry",
    "x86_64_supervised_renderer_generation_2_entry",
    "x86_64_supervised_renderer_rite_gate_entry",
    "x86_64_supervised_renderer_supervisor_start",
    "x86_64_supervised_renderer_supervisor_end",
    "x86_64_supervised_renderer_glyph_fingerprint",
];
const supervised_renderer_allowed_target_dependencies: &[&str] = &[
    "anomalous-abi",
    "anomalous-desktop-renderer",
    "anomalous-supervised-renderer",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct supervised_renderer_linked_symbol {
    elf_name: &'static str,
    wrapper_name: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct supervised_renderer_artifact {
    elf: PathBuf,
    binary: PathBuf,
    object: PathBuf,
    file_size: u64,
    image_pages: u64,
    generation_1_entry: u64,
    generation_2_entry: u64,
    generation_1_fault: u64,
    linked_symbol_offsets: Vec<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct supervised_renderer_inspection {
    file_size: u64,
    image_pages: u64,
    generation_1_entry: u64,
    generation_2_entry: u64,
    generation_1_fault: u64,
    linked_symbol_offsets: Vec<u64>,
    capsule: Vec<u8>,
}

struct built_image {
    path: PathBuf,
    supervised_renderer: Option<supervised_renderer_artifact>,
    semantic_surface: Option<semantic_surface_artifacts>,
}

struct semantic_presentation_host_run {
    transaction: Option<retained_host_transaction_fixture>,
    snapshot: [u8; surface_snapshot_v1_byte_count],
    protected_probes: Vec<Vec<u8>>,
}

struct semantic_presentation_build_inputs {
    first_snapshot: PathBuf,
    second_snapshot: PathBuf,
}

impl semantic_presentation_build_inputs {
    fn prepare(
        build: &Path,
        snapshot: &[u8; surface_snapshot_v1_byte_count],
    ) -> Result<Self, String> {
        let inputs = Self {
            first_snapshot: build.join("semantic_presentation_snapshot_first.bin"),
            second_snapshot: build.join("semantic_presentation_snapshot_second.bin"),
        };
        write_private_bytes(&inputs.first_snapshot, snapshot)?;
        write_private_bytes(&inputs.second_snapshot, snapshot)?;
        Ok(inputs)
    }

    fn paths(&self) -> (&Path, &Path) {
        (&self.first_snapshot, &self.second_snapshot)
    }

    fn remove_all(&self) -> Result<(), String> {
        remove_optional_file(&self.first_snapshot)?;
        remove_optional_file(&self.second_snapshot)
    }
}

impl Drop for semantic_presentation_build_inputs {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.first_snapshot);
        let _ = fs::remove_file(&self.second_snapshot);
    }
}

struct verified_semantic_presentation {
    _private: (),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum semantic_presentation_evidence_error {
    transcript,
    frame,
    host_binding,
}

struct semantic_presentation_capture_artifacts {
    serial_log: PathBuf,
    frame: PathBuf,
    serial_part: PathBuf,
    frame_part: PathBuf,
    published: bool,
}

impl semantic_presentation_capture_artifacts {
    fn prepare(build: &Path) -> Result<Self, String> {
        let artifacts = Self {
            serial_log: build.join("qemu_semantic_presentation.log"),
            frame: build.join("semantic_presentation.ppm"),
            serial_part: build.join("qemu_semantic_presentation.log.part"),
            frame_part: build.join("semantic_presentation.ppm.part"),
            published: false,
        };
        artifacts.remove_all()?;
        create_private_capture_file(&artifacts.serial_part)?;
        create_private_capture_file(&artifacts.frame_part)?;
        Ok(artifacts)
    }

    fn publish_exact(&mut self, transcript: &[u8], frame: &[u8]) -> Result<(), String> {
        write_private_bytes(&self.serial_part, transcript)?;
        write_private_bytes(&self.frame_part, frame)?;
        fs::rename(&self.serial_part, &self.serial_log)
            .and_then(|()| fs::rename(&self.frame_part, &self.frame))
            .map_err(|error| {
                format!("publish verified semantic-presentation artifacts: {error}")
            })?;
        self.published = true;
        Ok(())
    }

    fn remove_all(&self) -> Result<(), String> {
        for path in [
            &self.serial_part,
            &self.frame_part,
            &self.serial_log,
            &self.frame,
        ] {
            remove_optional_file(path)?;
        }
        Ok(())
    }
}

impl Drop for semantic_presentation_capture_artifacts {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.serial_part);
        let _ = fs::remove_file(&self.frame_part);
        if !self.published {
            let _ = fs::remove_file(&self.serial_log);
            let _ = fs::remove_file(&self.frame);
        }
    }
}

struct semantic_presentation_child {
    child: Child,
}

impl semantic_presentation_child {
    const fn new(child: Child) -> Self {
        Self { child }
    }

    fn child_mut(&mut self) -> &mut Child {
        &mut self.child
    }

    fn terminate_and_reap(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for semantic_presentation_child {
    fn drop(&mut self) {
        self.terminate_and_reap();
    }
}

impl semantic_presentation_host_run {
    fn prepare(build: &Path) -> Result<Self, String> {
        let seed = build.as_os_str().as_encoded_bytes();
        if seed.is_empty() {
            return Err("semantic-presentation protected-source seed is empty".into());
        }
        let first = derive_semantic_presentation_probe(seed, 0x4d, 37);
        let second = derive_semantic_presentation_probe(seed, 0xb3, 79);
        if first == second {
            return Err("semantic-presentation protected-source probes collided".into());
        }
        let first_text = std::str::from_utf8(&first)
            .map_err(|_| "semantic-presentation first protected probe is not UTF-8")?;
        let second_text = std::str::from_utf8(&second)
            .map_err(|_| "semantic-presentation second protected probe is not UTF-8")?;
        let retained = retained_host_transaction_fixture::try_new(first_text)
            .map_err(|error| format!("prepare retained host transaction: {}", error.code()))?;
        let comparison = retained_host_transaction_fixture::try_new(second_text)
            .map_err(|error| format!("prepare comparison host transaction: {}", error.code()))?;
        let retained_body = retained
            .try_encode_exact_body()
            .map_err(|error| format!("encode retained host transaction: {}", error.code()))?;
        let comparison_body = comparison
            .try_encode_exact_body()
            .map_err(|error| format!("encode comparison host transaction: {}", error.code()))?;
        if retained_body.as_bytes() != comparison_body.as_bytes()
            || !retained.revision_one_remains_public()
            || !comparison.revision_one_remains_public()
        {
            return Err(
                "semantic-presentation host transactions are not private and equivalent".into(),
            );
        }
        comparison
            .retire_without_publication()
            .map_err(|error| format!("retire comparison transaction: {}", error.code()))?;
        Ok(Self {
            transaction: Some(retained),
            snapshot: *retained_body.as_bytes(),
            protected_probes: vec![first, second],
        })
    }

    fn transaction_is_pending(&self) -> bool {
        self.transaction
            .as_ref()
            .is_some_and(retained_host_transaction_fixture::revision_one_remains_public)
    }

    fn complete(mut self, _evidence: verified_semantic_presentation) -> Result<(), String> {
        let transaction = self
            .transaction
            .take()
            .ok_or_else(|| "semantic-presentation host transaction is missing".to_string())?;
        let publication = transaction
            .complete_after_verified_presentation()
            .map_err(|error| format!("publish verified host transaction: {}", error.code()))?;
        if publication.revision() != 2 || publication.publication_sequence() != 2 {
            return Err("semantic-presentation host publication did not reach revision 2/2".into());
        }
        clear_semantic_presentation_probes(&mut self.protected_probes);
        Ok(())
    }
}

impl Drop for semantic_presentation_host_run {
    fn drop(&mut self) {
        if let Some(transaction) = self.transaction.take() {
            let _ = transaction.retire_without_publication();
        }
        clear_semantic_presentation_probes(&mut self.protected_probes);
        self.snapshot.fill(0);
    }
}

fn derive_semantic_presentation_probe(seed: &[u8], domain: u8, length: usize) -> Vec<u8> {
    (0..length)
        .map(|index| {
            b'a' + (seed[index % seed.len()]
                .wrapping_add(domain)
                .wrapping_add(index as u8)
                % 26)
        })
        .collect()
}

fn clear_semantic_presentation_probes(probes: &mut [Vec<u8>]) {
    for probe in probes {
        probe.fill(0);
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct elf64_program_header {
    kind: u32,
    flags: u32,
    offset: u64,
    virtual_address: u64,
    physical_address: u64,
    file_size: u64,
    memory_size: u64,
    alignment: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct elf64_section_header {
    name: String,
    kind: u32,
    flags: u64,
    address: u64,
    offset: u64,
    size: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct parsed_elf64 {
    kind: u16,
    machine: u16,
    entry: u64,
    program_headers: Vec<elf64_program_header>,
    section_headers: Vec<elf64_section_header>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct rgb8 {
    red: u8,
    green: u8,
    blue: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct display_fixture_rectangle {
    x: usize,
    y: usize,
    width: usize,
    height: usize,
    color: rgb8,
}

const display_background: rgb8 = rgb8 {
    red: 0xde,
    green: 0xe6,
    blue: 0xed,
};
const display_highlight: rgb8 = rgb8 {
    red: 0xfa,
    green: 0xfc,
    blue: 0xfe,
};
const display_shadow: rgb8 = rgb8 {
    red: 0xaa,
    green: 0xb8,
    blue: 0xc4,
};
const display_surface: rgb8 = rgb8 {
    red: 0xe0,
    green: 0xe8,
    blue: 0xef,
};
const display_border: rgb8 = rgb8 {
    red: 0x4e,
    green: 0x5b,
    blue: 0x6a,
};
const display_ink: rgb8 = rgb8 {
    red: 0x16,
    green: 0x1f,
    blue: 0x2b,
};
const display_muted: rgb8 = rgb8 {
    red: 0x3d,
    green: 0x4c,
    blue: 0x5c,
};
const display_accent: rgb8 = rgb8 {
    red: 0x5b,
    green: 0x46,
    blue: 0xf6,
};
const display_terminal: rgb8 = rgb8 {
    red: 0xeb,
    green: 0xf1,
    blue: 0xf7,
};
const display_fixture_rectangles: &[display_fixture_rectangle] = &[
    display_fixture_rectangle {
        x: 0,
        y: 0,
        width: 1024,
        height: 768,
        color: display_background,
    },
    display_fixture_rectangle {
        x: 0,
        y: 0,
        width: 1024,
        height: 72,
        color: display_surface,
    },
    display_fixture_rectangle {
        x: 0,
        y: 70,
        width: 1024,
        height: 2,
        color: display_border,
    },
    display_fixture_rectangle {
        x: 32,
        y: 24,
        width: 24,
        height: 24,
        color: display_accent,
    },
    display_fixture_rectangle {
        x: 72,
        y: 27,
        width: 152,
        height: 6,
        color: display_ink,
    },
    display_fixture_rectangle {
        x: 72,
        y: 39,
        width: 96,
        height: 4,
        color: display_muted,
    },
    display_fixture_rectangle {
        x: 55,
        y: 111,
        width: 240,
        height: 544,
        color: display_shadow,
    },
    display_fixture_rectangle {
        x: 45,
        y: 101,
        width: 240,
        height: 544,
        color: display_highlight,
    },
    display_fixture_rectangle {
        x: 49,
        y: 105,
        width: 232,
        height: 536,
        color: display_border,
    },
    display_fixture_rectangle {
        x: 51,
        y: 107,
        width: 228,
        height: 532,
        color: display_surface,
    },
    display_fixture_rectangle {
        x: 75,
        y: 137,
        width: 104,
        height: 6,
        color: display_ink,
    },
    display_fixture_rectangle {
        x: 67,
        y: 177,
        width: 196,
        height: 44,
        color: display_highlight,
    },
    display_fixture_rectangle {
        x: 67,
        y: 177,
        width: 5,
        height: 44,
        color: display_accent,
    },
    display_fixture_rectangle {
        x: 83,
        y: 192,
        width: 120,
        height: 6,
        color: display_ink,
    },
    display_fixture_rectangle {
        x: 75,
        y: 249,
        width: 112,
        height: 6,
        color: display_muted,
    },
    display_fixture_rectangle {
        x: 75,
        y: 285,
        width: 136,
        height: 6,
        color: display_muted,
    },
    display_fixture_rectangle {
        x: 75,
        y: 321,
        width: 96,
        height: 6,
        color: display_muted,
    },
    display_fixture_rectangle {
        x: 327,
        y: 111,
        width: 650,
        height: 544,
        color: display_shadow,
    },
    display_fixture_rectangle {
        x: 317,
        y: 101,
        width: 650,
        height: 544,
        color: display_highlight,
    },
    display_fixture_rectangle {
        x: 321,
        y: 105,
        width: 642,
        height: 536,
        color: display_border,
    },
    display_fixture_rectangle {
        x: 323,
        y: 107,
        width: 638,
        height: 532,
        color: display_surface,
    },
    display_fixture_rectangle {
        x: 355,
        y: 137,
        width: 196,
        height: 8,
        color: display_ink,
    },
    display_fixture_rectangle {
        x: 355,
        y: 157,
        width: 288,
        height: 5,
        color: display_muted,
    },
    display_fixture_rectangle {
        x: 355,
        y: 187,
        width: 574,
        height: 260,
        color: display_border,
    },
    display_fixture_rectangle {
        x: 359,
        y: 191,
        width: 566,
        height: 252,
        color: display_ink,
    },
    display_fixture_rectangle {
        x: 383,
        y: 219,
        width: 232,
        height: 5,
        color: display_terminal,
    },
    display_fixture_rectangle {
        x: 383,
        y: 243,
        width: 368,
        height: 5,
        color: display_terminal,
    },
    display_fixture_rectangle {
        x: 383,
        y: 267,
        width: 304,
        height: 5,
        color: display_terminal,
    },
    display_fixture_rectangle {
        x: 383,
        y: 291,
        width: 88,
        height: 5,
        color: display_accent,
    },
    display_fixture_rectangle {
        x: 479,
        y: 291,
        width: 248,
        height: 5,
        color: display_terminal,
    },
    display_fixture_rectangle {
        x: 367,
        y: 479,
        width: 330,
        height: 76,
        color: display_accent,
    },
    display_fixture_rectangle {
        x: 370,
        y: 482,
        width: 324,
        height: 70,
        color: display_surface,
    },
    display_fixture_rectangle {
        x: 373,
        y: 485,
        width: 318,
        height: 64,
        color: display_accent,
    },
    display_fixture_rectangle {
        x: 375,
        y: 487,
        width: 314,
        height: 60,
        color: display_highlight,
    },
    display_fixture_rectangle {
        x: 463,
        y: 513,
        width: 138,
        height: 8,
        color: display_ink,
    },
    display_fixture_rectangle {
        x: 355,
        y: 591,
        width: 240,
        height: 5,
        color: display_muted,
    },
    display_fixture_rectangle {
        x: 895,
        y: 591,
        width: 10,
        height: 10,
        color: display_accent,
    },
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct interrupt_run_evidence {
    timebase_generation: u64,
    start_tick: u64,
    end_tick: u64,
    timer_interrupts: u64,
    source_generation: u64,
    first_sequence: u64,
    last_sequence: u64,
    transitions: u64,
    lost_count: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum interrupt_observation {
    ready,
    timer_verified,
    input_verified,
    return_verified,
    loss_rearmed,
    loss_verified,
    masked,
    passed,
}

const expected_interrupt_observations: &[interrupt_observation] = &[
    interrupt_observation::ready,
    interrupt_observation::timer_verified,
    interrupt_observation::input_verified,
    interrupt_observation::return_verified,
    interrupt_observation::loss_rearmed,
    interrupt_observation::loss_verified,
    interrupt_observation::masked,
    interrupt_observation::passed,
];
const expected_interrupt_serial_events: &[&str] = &[
    "boot.kernel.enter",
    "kernel.fault_path.ready",
    "boot.omen.accepted",
    "boot.memory.map",
    "kernel.memory.ready",
    "boot.display.ready",
    "boot.ready",
    "kernel.interrupts.ready",
    "kernel.interrupt_test.timer.verified",
    "kernel.interrupt_test.input.verified",
    "kernel.interrupt_test.return.verified",
    "kernel.interrupt_test.loss.rearmed",
    "kernel.interrupt_test.loss.verified",
    "kernel.interrupt_test.masked",
    "kernel.interrupt_test.passed",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct memory_image_bounds {
    kernel_start: u64,
    kernel_end: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct memory_run_evidence {
    total_frames: u64,
    initial_free_frames: u64,
    unavailable_frames: u64,
    data_a: u64,
    data_b: u64,
    root_a: u64,
    root_b: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct circle_fault_addresses {
    generation_1_instruction_pointer: u64,
    generation_1_fault_address: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct scheduler_fault_address {
    instruction_pointer: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct supervision_fault_address {
    instruction_pointer: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct shell_vessel_fault_address {
    instruction_pointer: u64,
}

#[derive(Clone, Copy)]
struct image_extent {
    file_size: u32,
    sector_count: u32,
    hash: u32,
}

struct threshold_manifest {
    stage_2: image_extent,
    kernel: image_extent,
    kernel_memory_size: u32,
    kernel_entry_address: u32,
}

impl threshold_manifest {
    fn encode(&self) -> [u8; manifest_sector_size] {
        let mut bytes = [0u8; manifest_sector_size];
        bytes[0..8].copy_from_slice(&manifest_magic);
        write_u32(&mut bytes, 8, manifest_version);
        write_u32(&mut bytes, 12, manifest_size as u32);
        write_u32(&mut bytes, 20, 0);
        write_u64(&mut bytes, 24, stage_2_lba);
        write_u32(&mut bytes, 32, self.stage_2.file_size);
        write_u32(&mut bytes, 36, self.stage_2.sector_count);
        write_u32(&mut bytes, 40, stage_2_load_address);
        write_u32(&mut bytes, 44, stage_2_load_address);
        write_u32(&mut bytes, 48, self.stage_2.hash);
        write_u32(&mut bytes, 52, 0);

        let kernel_lba = stage_2_lba + u64::from(self.stage_2.sector_count);
        write_u64(&mut bytes, 56, kernel_lba);
        write_u32(&mut bytes, 64, self.kernel.file_size);
        write_u32(&mut bytes, 68, self.kernel_memory_size);
        write_u32(&mut bytes, 72, self.kernel.sector_count);
        write_u32(&mut bytes, 76, kernel_staging_address);
        write_u32(&mut bytes, 80, kernel_load_address);
        write_u32(&mut bytes, 84, self.kernel_entry_address);
        write_u32(&mut bytes, 88, self.kernel.hash);
        write_u32(&mut bytes, 92, 0);

        let manifest_hash = fnv_1a(&bytes[manifest_hashed_offset..manifest_size]);
        write_u32(&mut bytes, manifest_hash_offset, manifest_hash);
        bytes
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("xtask: {message}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    // These host-only inspectors never build, map, or execute foreign images.
    match env::args_os().nth(1).as_deref().and_then(OsStr::to_str) {
        Some("program-manifest-pack") => {
            let mut arguments = env::args_os().skip(2);
            let input = arguments
                .next()
                .ok_or("program-manifest-pack requires INPUT_NIL OUTPUT.re")?;
            let output = arguments
                .next()
                .ok_or("program-manifest-pack requires INPUT_NIL OUTPUT.re")?;
            reject_extra_arguments(arguments)?;
            return program_manifest::pack(Path::new(&input), Path::new(&output));
        }
        Some("program-manifest-inspect") => {
            let mut arguments = env::args_os().skip(2);
            let input = arguments
                .next()
                .ok_or("program-manifest-inspect requires PACKAGE CEILING")?;
            let ceiling = arguments
                .next()
                .ok_or("program-manifest-inspect requires PACKAGE CEILING")?;
            reject_extra_arguments(arguments)?;
            let ceiling = ceiling
                .to_str()
                .ok_or("ceiling must be none or console_write")?;
            return program_manifest::inspect(Path::new(&input), ceiling);
        }
        Some("linux-image-inspect") => {
            return linux_process::inspect_image(env::args_os().skip(2).collect());
        }
        Some("linux-process-inspect") => {
            return linux_process::inspect_process(env::args_os().skip(2).collect());
        }
        Some("qualify-read-only-files") => {
            return file_service_proof::qualify(env::args_os().skip(2).collect());
        }
        _ => {}
    }
    // Pure stdin/stdout conversion also runs inside a proof holding the build
    // lock. It neither builds nor writes workspace artifacts.
    if env::args_os().nth(1).as_deref() == Some(OsStr::new("native-interactive-render")) {
        reject_extra_arguments(env::args_os().skip(2))?;
        return native_interactive_smoke::render_observation();
    }
    if env::args_os().nth(1).as_deref() == Some(OsStr::new("native-editor-workspace-render")) {
        return native_editor_workspace::render_observation(env::args_os().skip(2).collect());
    }
    let workspace_guard = acquire_workspace_lock()?;
    let mut arguments = env::args_os().skip(1);
    match arguments.next().as_deref().and_then(OsStr::to_str) {
        Some("program-manifest-proof") => {
            reject_extra_arguments(arguments)?;
            let root = repository_root()?;
            program_manifest::proof(&root, &root.join("build"))
        }
        Some("image") => {
            reject_extra_arguments(arguments)?;
            let image = build_image(&[])?;
            println!("boot image: {}", image.display());
            Ok(())
        }
        Some("qemu") => {
            reject_extra_arguments(arguments)?;
            let image = build_image(&[])?;
            run_qemu(&image, false)
        }
        Some("smoke") => {
            reject_extra_arguments(arguments)?;
            let image = build_image(&["smoke_test"])?;
            run_qemu(&image, true)
        }
        Some("fault-smoke") => {
            reject_extra_arguments(arguments)?;
            let image = build_image(&["fault_test"])?;
            run_qemu_fault_smoke(&image)
        }
        Some("circle-smoke") => {
            reject_extra_arguments(arguments)?;
            build_and_inspect_normal_image()?;
            let image = build_image(&["circle_test"])?;
            let root = repository_root()?;
            let fault_addresses =
                resolve_circle_fault_addresses(&root, &root.join("build/kernel.elf"))?;
            run_qemu_circle_smoke(&image, fault_addresses)
        }
        Some("control-smoke") => {
            reject_extra_arguments(arguments)?;
            build_and_inspect_normal_image()?;
            let image = build_image(&["control_test"])?;
            run_qemu_control_smoke(&image)
        }
        Some("verify-control-transcript") => {
            let path = arguments
                .next()
                .map(PathBuf::from)
                .ok_or_else(|| "verify-control-transcript requires one log path".to_string())?;
            reject_extra_arguments(arguments)?;
            let transcript = fs::read_to_string(&path)
                .map_err(|error| format!("read {}: {error}", path.display()))?;
            verify_control_transcript(&transcript)?;
            println!(
                "control transcript proves the exact attributable sigil-gated IPC lifecycle: {}",
                path.display()
            );
            Ok(())
        }
        Some("rite-smoke") => {
            reject_extra_arguments(arguments)?;
            build_and_inspect_normal_image()?;
            let image = build_image(&["rite_test"])?;
            run_qemu_rite_smoke(&image)
        }
        Some("verify-rite-transcript") => {
            let path = arguments
                .next()
                .map(PathBuf::from)
                .ok_or_else(|| "verify-rite-transcript requires one log path".to_string())?;
            reject_extra_arguments(arguments)?;
            let transcript = fs::read_to_string(&path)
                .map_err(|error| format!("read {}: {error}", path.display()))?;
            verify_rite_transcript(&transcript)?;
            println!(
                "rite transcript proves the fixed x86-64 ring-3 rite lifecycle: {}",
                path.display()
            );
            Ok(())
        }
        Some("elf-relaunch-inspect") => {
            reject_extra_arguments(arguments)?;
            let root = repository_root()?;
            elf_relaunch_artifact::package_image(&root, &root.join("build")).map(|_| ())
        }
        Some("build-elf-relaunch" | "elf-relaunch-image") => {
            reject_extra_arguments(arguments)?;
            elf_relaunch_smoke::build().map(|_| ())
        }
        Some("smoke-elf-relaunch" | "elf-relaunch-smoke") => {
            reject_extra_arguments(arguments)?;
            elf_relaunch_smoke::smoke()
        }
        Some("verify-elf-relaunch-log" | "verify-elf-relaunch-transcript") => {
            let path = arguments
                .next()
                .ok_or("verify-elf-relaunch-log requires one log path")?;
            reject_extra_arguments(arguments)?;
            let transcript = fs::read_to_string(path).map_err(|error| error.to_string())?;
            elf_relaunch_smoke::verify_transcript(&transcript)
        }
        Some("native-durable-failure-build") => {
            reject_extra_arguments(arguments)?;
            native_durable_failure_smoke::build().map(|_| ())
        }
        Some("native-durable-failure-smoke") => {
            reject_extra_arguments(arguments)?;
            native_durable_failure_smoke::smoke()
        }
        Some("native-interactive-build") => {
            reject_extra_arguments(arguments)?;
            native_interactive_smoke::build().map(|_| ())
        }
        Some("native-interactive-smoke") => {
            reject_extra_arguments(arguments)?;
            native_interactive_smoke::smoke()
        }
        Some("native-repeated-build") => {
            reject_extra_arguments(arguments)?;
            native_repeated_smoke::build().map(|_| ())
        }
        Some("native-repeated-smoke") => {
            reject_extra_arguments(arguments)?;
            native_repeated_smoke::smoke()
        }
        Some("native-durable-build") => {
            reject_extra_arguments(arguments)?;
            native_durable_smoke::build().map(|_| ())
        }
        Some("native-durable-smoke") => {
            reject_extra_arguments(arguments)?;
            native_durable_smoke::smoke()
        }
        Some("native-relaunch-inspect") => {
            reject_extra_arguments(arguments)?;
            let root = repository_root()?;
            native_relaunch_artifact::package_image(&root, &root.join("build")).map(|_| ())
        }
        Some("build-native-relaunch" | "native-relaunch-image") => {
            reject_extra_arguments(arguments)?;
            native_relaunch_smoke::build().map(|_| ())
        }
        Some("smoke-native-relaunch" | "native-relaunch-smoke") => {
            reject_extra_arguments(arguments)?;
            native_relaunch_smoke::smoke()
        }
        Some("verify-native-relaunch-log" | "verify-native-relaunch-transcript") => {
            let path = arguments
                .next()
                .ok_or("verify-native-relaunch-log requires one log path")?;
            reject_extra_arguments(arguments)?;
            let transcript = fs::read_to_string(path).map_err(|error| error.to_string())?;
            native_relaunch_smoke::verify_transcript(&transcript)
        }
        Some("verify-native-relaunch-frame" | "verify-elf-relaunch-frame") => {
            let path = arguments
                .next()
                .ok_or("relaunch frame verification requires one canonical P6 path")?;
            reject_extra_arguments(arguments)?;
            if fs::metadata(&path)
                .map_err(|error| error.to_string())?
                .len()
                != (b"P6\n1024 768\n255\n".len() + 1024 * 768 * 3) as u64
            {
                return Err("relaunch frame must be a complete 1024x768 RGB P6 capture".into());
            }
            let bytes = fs::read(path).map_err(|error| error.to_string())?;
            native_relaunch_smoke::verify_frame(&bytes)
        }
        Some("elf-document-inspect") => {
            reject_extra_arguments(arguments)?;
            let root = repository_root()?;
            elf_document_artifact::package_image(&root, &root.join("build")).map(|_| ())
        }
        Some("build-elf-document" | "elf-document-image") => {
            reject_extra_arguments(arguments)?;
            elf_document_smoke::build().map(|_| ())
        }
        Some("smoke-elf-document" | "elf-document-smoke") => {
            reject_extra_arguments(arguments)?;
            elf_document_smoke::smoke()
        }
        Some("verify-elf-document-log" | "verify-elf-document-transcript") => {
            let path = arguments
                .next()
                .ok_or("verify-elf-document-log requires one log path")?;
            reject_extra_arguments(arguments)?;
            let transcript = fs::read_to_string(path).map_err(|error| error.to_string())?;
            elf_document_smoke::verify_transcript(&transcript)
        }
        Some("native-document-inspect") => {
            reject_extra_arguments(arguments)?;
            let root = repository_root()?;
            native_document_artifact::package_image(&root, &root.join("build")).map(|_| ())
        }
        Some("build-native-document" | "native-document-image") => {
            reject_extra_arguments(arguments)?;
            native_document_smoke::build().map(|_| ())
        }
        Some("smoke-native-document" | "native-document-smoke") => {
            reject_extra_arguments(arguments)?;
            native_document_smoke::smoke()
        }
        Some("verify-native-document-log" | "verify-native-document-transcript") => {
            let path = arguments
                .next()
                .ok_or("verify-native-document-log requires one log path")?;
            reject_extra_arguments(arguments)?;
            let transcript = fs::read_to_string(path).map_err(|error| error.to_string())?;
            native_document_smoke::verify_transcript(&transcript)
        }
        Some("native-runtime-inspect") => {
            reject_extra_arguments(arguments)?;
            let root = repository_root()?;
            native_runtime_artifact::package_image(&root, &root.join("build")).map(|_| ())
        }
        Some("build-native-runtime" | "native-runtime-image") => {
            reject_extra_arguments(arguments)?;
            native_runtime_smoke::build().map(|_| ())
        }
        Some("smoke-native-runtime" | "native-runtime-smoke") => {
            reject_extra_arguments(arguments)?;
            native_runtime_smoke::smoke()
        }
        Some("verify-native-runtime-log" | "verify-native-runtime-transcript") => {
            let path = arguments
                .next()
                .ok_or("verify-native-runtime-log requires one log path")?;
            reject_extra_arguments(arguments)?;
            let transcript = fs::read_to_string(path).map_err(|error| error.to_string())?;
            native_runtime_smoke::verify_transcript(&transcript)
        }
        Some("native-manifest-inspect") => {
            reject_extra_arguments(arguments)?;
            let root = repository_root()?;
            native_manifest_artifact::package_image(&root, &root.join("build")).map(|_| ())
        }
        Some("build-native-manifest" | "native-manifest-image") => {
            reject_extra_arguments(arguments)?;
            native_manifest_smoke::build().map(|_| ())
        }
        Some("smoke-native-manifest" | "native-manifest-smoke") => {
            reject_extra_arguments(arguments)?;
            native_manifest_smoke::smoke()
        }
        Some("verify-native-manifest-log" | "verify-native-manifest-transcript") => {
            let path = arguments
                .next()
                .ok_or("verify-native-manifest-log requires one log path")?;
            reject_extra_arguments(arguments)?;
            let transcript = fs::read_to_string(path).map_err(|error| error.to_string())?;
            native_manifest_smoke::verify_transcript(&transcript)
        }
        Some("native-program-inspect") => {
            reject_extra_arguments(arguments)?;
            let root = repository_root()?;
            native_program_artifact::package_image(&root, &root.join("build")).map(|_| ())
        }
        Some("build-native-program" | "native-program-image") => {
            reject_extra_arguments(arguments)?;
            native_program_smoke::build().map(|_| ())
        }
        Some("smoke-native-program" | "native-program-smoke") => {
            reject_extra_arguments(arguments)?;
            native_program_smoke::smoke()
        }
        Some("verify-native-program-log" | "verify-native-program-transcript") => {
            let path = arguments
                .next()
                .ok_or("verify-native-program-log requires one log path")?;
            reject_extra_arguments(arguments)?;
            let transcript = fs::read_to_string(path).map_err(|error| error.to_string())?;
            native_program_smoke::verify_transcript(&transcript)
        }
        Some("linux-loader-entry-inspect") => {
            reject_extra_arguments(arguments)?;
            let root = repository_root()?;
            linux_loader_entry_artifact::package_image(&root, &root.join("build")).map(|_| ())
        }
        Some("build-linux-loader-entry" | "linux-loader-entry-image") => {
            reject_extra_arguments(arguments)?;
            linux_loader_entry_smoke::build().map(|_| ())
        }
        Some("smoke-linux-loader-entry" | "linux-loader-entry-smoke") => {
            reject_extra_arguments(arguments)?;
            linux_loader_entry_smoke::smoke()
        }
        Some("verify-linux-loader-entry-log" | "verify-linux-loader-entry-transcript") => {
            let path = arguments
                .next()
                .map(PathBuf::from)
                .ok_or("verify-linux-loader-entry-transcript requires one log path")?;
            reject_extra_arguments(arguments)?;
            let transcript = read_bounded_optional_utf8_file(
                &path,
                linux_loader_entry_smoke::transcript_byte_limit,
            )?;
            linux_loader_entry_smoke::verify_transcript(&transcript)
        }
        Some("linux-tls-inspect") => {
            reject_extra_arguments(arguments)?;
            let root = repository_root()?;
            linux_tls_artifact::package_image(&root, &root.join("build")).map(|_| ())
        }
        Some("build-linux-tls" | "linux-tls-image") => {
            reject_extra_arguments(arguments)?;
            linux_tls_smoke::build().map(|_| ())
        }
        Some("smoke-linux-tls" | "linux-tls-smoke") => {
            reject_extra_arguments(arguments)?;
            linux_tls_smoke::smoke()
        }
        Some("verify-linux-tls-log" | "verify-linux-tls-transcript") => {
            let path = arguments
                .next()
                .ok_or("verify-linux-tls-transcript requires one log path")?;
            reject_extra_arguments(arguments)?;
            let transcript = fs::read_to_string(path).map_err(|error| error.to_string())?;
            linux_tls_smoke::verify_transcript(&transcript)
        }
        Some("shared-veil-inspect") => {
            reject_extra_arguments(arguments)?;
            let root = repository_root()?;
            veil_application_artifact::package_image(&root, &root.join("build")).map(|_| ())
        }
        Some("build-shared-veil" | "shared-veil-image") => {
            reject_extra_arguments(arguments)?;
            shared_veil_smoke::build().map(|_| ())
        }
        Some("smoke-shared-veil" | "shared-veil-smoke") => {
            reject_extra_arguments(arguments)?;
            shared_veil_smoke::smoke()
        }
        Some("verify-shared-veil-log" | "verify-shared-veil-transcript") => {
            let path = arguments
                .next()
                .ok_or("verify-shared-veil-log requires one log path")?;
            reject_extra_arguments(arguments)?;
            let transcript = fs::read_to_string(path).map_err(|error| error.to_string())?;
            shared_veil_smoke::verify_transcript(&transcript)
        }
        Some("veil-application-inspect") => {
            reject_extra_arguments(arguments)?;
            let root = repository_root()?;
            veil_application_artifact::package_image(&root, &root.join("build")).map(|_| ())
        }
        Some("build-veil-application" | "veil-application-image") => {
            reject_extra_arguments(arguments)?;
            veil_application_smoke::build().map(|_| ())
        }
        Some("smoke-veil-application" | "veil-application-smoke") => {
            reject_extra_arguments(arguments)?;
            veil_application_smoke::smoke()
        }
        Some("verify-veil-application-log" | "verify-veil-application-transcript") => {
            let path = arguments
                .next()
                .map(PathBuf::from)
                .ok_or("verify-veil-application-log requires one log path")?;
            reject_extra_arguments(arguments)?;
            let transcript = fs::read_to_string(path).map_err(|error| error.to_string())?;
            veil_application_smoke::verify_transcript(&transcript)
        }
        Some("linux-application-inspect") => {
            reject_extra_arguments(arguments)?;
            let root = repository_root()?;
            linux_application_artifact::package_image(&root, &root.join("build")).map(|_| ())
        }
        Some("build-linux-application" | "linux-application-image") => {
            reject_extra_arguments(arguments)?;
            linux_application_smoke::build().map(|_| ())
        }
        Some("smoke-linux-application" | "linux-application-smoke") => {
            reject_extra_arguments(arguments)?;
            linux_application_smoke::smoke()
        }
        Some("verify-linux-application-log" | "verify-linux-application-transcript") => {
            let path = arguments
                .next()
                .map(PathBuf::from)
                .ok_or("verify-linux-application-log requires one log path")?;
            reject_extra_arguments(arguments)?;
            let transcript = fs::read_to_string(path).map_err(|error| error.to_string())?;
            linux_application_smoke::verify_transcript(&transcript)
        }
        Some("elf-application-inspect") => {
            reject_extra_arguments(arguments)?;
            let root = repository_root()?;
            elf_application_artifact::package_image(&root, &root.join("build")).map(|_| ())
        }
        Some("elf-application-image") => {
            reject_extra_arguments(arguments)?;
            elf_application_smoke::build().map(|_| ())
        }
        Some("elf-application-smoke") => {
            reject_extra_arguments(arguments)?;
            elf_application_smoke::smoke()
        }
        Some("verify-elf-application-transcript") => {
            let path = arguments
                .next()
                .map(PathBuf::from)
                .ok_or("verify-elf-application-transcript requires one log path")?;
            reject_extra_arguments(arguments)?;
            let transcript = fs::read_to_string(path).map_err(|error| error.to_string())?;
            elf_application_smoke::verify_transcript(&transcript)
        }
        Some("native-application-inspect") => {
            reject_extra_arguments(arguments)?;
            let root = repository_root()?;
            native_application_artifact::package_image(&root, &root.join("build")).map(|_| ())
        }
        Some("native-application-image") => {
            reject_extra_arguments(arguments)?;
            native_application_smoke::build().map(|_| ())
        }
        Some("native-application-smoke") => {
            reject_extra_arguments(arguments)?;
            native_application_smoke::smoke()
        }
        Some("verify-native-application-transcript") => {
            let path = arguments
                .next()
                .map(PathBuf::from)
                .ok_or("verify-native-application-transcript requires one log path")?;
            reject_extra_arguments(arguments)?;
            let transcript = fs::read_to_string(path).map_err(|error| error.to_string())?;
            native_application_smoke::verify_transcript(&transcript)
        }
        Some("ny-object-inspect") => {
            reject_extra_arguments(arguments)?;
            let root = repository_root()?;
            ny_object_artifact::package_image(&root, &root.join("build")).map(|_| ())
        }
        Some("ny-vessel-inspect") => {
            reject_extra_arguments(arguments)?;
            let root = repository_root()?;
            ny_vessel_artifact::package_image(&root, &root.join("build")).map(|_| ())
        }
        Some("ny-vessel-image") => {
            reject_extra_arguments(arguments)?;
            ny_vessel_smoke::build().map(|_| ())
        }
        Some("ny-vessel-smoke") => {
            reject_extra_arguments(arguments)?;
            ny_vessel_smoke::smoke()
        }
        Some("verify-ny-vessel-transcript") => {
            let path = arguments
                .next()
                .map(PathBuf::from)
                .ok_or("verify-ny-vessel-transcript requires one log path")?;
            reject_extra_arguments(arguments)?;
            let transcript = fs::read_to_string(path).map_err(|error| error.to_string())?;
            ny_vessel_smoke::verify_transcript(&transcript)
        }
        Some("ny-object-image") => {
            reject_extra_arguments(arguments)?;
            ny_object_smoke::build().map(|_| ())
        }
        Some("ny-object-smoke") => {
            reject_extra_arguments(arguments)?;
            ny_object_smoke::smoke()
        }
        Some("verify-ny-object-transcript") => {
            let path = arguments
                .next()
                .map(PathBuf::from)
                .ok_or("verify-ny-object-transcript requires one log path")?;
            reject_extra_arguments(arguments)?;
            let transcript = fs::read_to_string(path).map_err(|error| error.to_string())?;
            ny_object_smoke::verify_transcript(&transcript)
        }
        Some("mixed-workspace-build" | "build-mixed-workspace") => {
            reject_extra_arguments(arguments)?;
            mixed_workspace::build().map(|_| ())
        }
        Some("native-editor-workspace-build") => {
            reject_extra_arguments(arguments)?;
            native_editor_workspace::build().map(|_| ())
        }
        Some("native-editor-workspace-smoke") => {
            reject_extra_arguments(arguments)?;
            native_editor_workspace::smoke()
        }
        Some("mixed-workspace-smoke" | "smoke-mixed-workspace") => {
            reject_extra_arguments(arguments)?;
            mixed_workspace::smoke()
        }
        Some("verify-mixed-workspace-log") => {
            let path = arguments
                .next()
                .map(PathBuf::from)
                .ok_or("verify-mixed-workspace-log requires one log path")?;
            reject_extra_arguments(arguments)?;
            let transcript = fs::read_to_string(path).map_err(|error| error.to_string())?;
            mixed_workspace::verify_transcript(&transcript).map(|_| ())
        }
        Some("application-profile-image") => {
            reject_extra_arguments(arguments)?;
            application_profile_smoke::build().map(|_| ())
        }
        Some("application-profile-smoke") => {
            reject_extra_arguments(arguments)?;
            application_profile_smoke::smoke()
        }
        Some("verify-application-profile-transcript") => {
            let path = arguments
                .next()
                .map(PathBuf::from)
                .ok_or("verify-application-profile-transcript requires one log path")?;
            reject_extra_arguments(arguments)?;
            let transcript = fs::read_to_string(path).map_err(|error| error.to_string())?;
            application_profile_smoke::verify_transcript(&transcript)
        }
        Some("owned-dispatcher-image") => {
            reject_extra_arguments(arguments)?;
            owned_dispatcher_smoke::build().map(|_| ())
        }
        Some("owned-multi-region-image") => {
            reject_extra_arguments(arguments)?;
            owned_multi_region_smoke::build().map(|_| ())
        }
        Some("build-owned-source") => {
            reject_extra_arguments(arguments)?;
            owned_source_smoke::build().map(|_| ())
        }
        Some("smoke-owned-source") => {
            reject_extra_arguments(arguments)?;
            owned_source_smoke::smoke()
        }
        Some("verify-owned-source-transcript") => {
            let path = arguments
                .next()
                .map(PathBuf::from)
                .ok_or("verify-owned-source-transcript requires one log path")?;
            reject_extra_arguments(arguments)?;
            let transcript =
                read_bounded_optional_utf8_file(&path, owned_source_smoke::transcript_byte_limit)?;
            owned_source_smoke::verify_transcript(&transcript)
        }
        Some("build-owned-tls") => {
            reject_extra_arguments(arguments)?;
            owned_tls_smoke::build().map(|_| ())
        }
        Some("smoke-owned-tls") => {
            reject_extra_arguments(arguments)?;
            owned_tls_smoke::smoke()
        }
        Some("verify-owned-tls-transcript") => {
            let path = arguments
                .next()
                .map(PathBuf::from)
                .ok_or("verify-owned-tls-transcript requires one log path")?;
            reject_extra_arguments(arguments)?;
            let transcript =
                read_bounded_optional_utf8_file(&path, owned_tls_smoke::transcript_byte_limit)?;
            owned_tls_smoke::verify_transcript(&transcript)
        }
        Some("owned-multi-region-smoke") => {
            reject_extra_arguments(arguments)?;
            owned_multi_region_smoke::smoke()
        }
        Some("verify-owned-multi-region-transcript") => {
            let path = arguments
                .next()
                .map(PathBuf::from)
                .ok_or("verify-owned-multi-region-transcript requires one log path")?;
            reject_extra_arguments(arguments)?;
            let transcript = fs::read_to_string(path).map_err(|error| error.to_string())?;
            owned_multi_region_smoke::verify_transcript(&transcript)
        }
        Some("owned-dispatcher-smoke") => {
            reject_extra_arguments(arguments)?;
            owned_dispatcher_smoke::smoke()
        }
        Some("verify-owned-dispatcher-transcript") => {
            let path = arguments
                .next()
                .map(PathBuf::from)
                .ok_or("verify-owned-dispatcher-transcript requires one log path")?;
            reject_extra_arguments(arguments)?;
            let transcript = fs::read_to_string(path).map_err(|error| error.to_string())?;
            owned_dispatcher_smoke::verify_transcript(&transcript)
        }
        Some("dispatcher-image") => {
            reject_extra_arguments(arguments)?;
            dispatcher_smoke::build().map(|_| ())
        }
        Some("dispatcher-smoke") => {
            reject_extra_arguments(arguments)?;
            dispatcher_smoke::smoke()
        }
        Some("verify-dispatcher-transcript") => {
            let path = arguments
                .next()
                .map(PathBuf::from)
                .ok_or("verify-dispatcher-transcript requires one log path")?;
            reject_extra_arguments(arguments)?;
            let transcript = fs::read_to_string(path).map_err(|e| e.to_string())?;
            dispatcher_smoke::verify_transcript(&transcript)
        }
        Some("scheduler-smoke") => {
            reject_extra_arguments(arguments)?;
            build_and_inspect_normal_image()?;
            let image = build_image(&["scheduler_test"])?;
            let root = repository_root()?;
            let fault_address =
                resolve_scheduler_fault_address(&root, &root.join("build/kernel.elf"))?;
            run_qemu_scheduler_smoke(&image, fault_address)
        }
        Some("verify-scheduler-transcript") => {
            let path = arguments
                .next()
                .map(PathBuf::from)
                .ok_or_else(|| "verify-scheduler-transcript requires one log path".to_string())?;
            reject_extra_arguments(arguments)?;
            let transcript = fs::read_to_string(&path)
                .map_err(|error| format!("read {}: {error}", path.display()))?;
            let root = repository_root()?;
            let fault_address =
                resolve_scheduler_fault_address(&root, &root.join("build/kernel.elf"))?;
            verify_scheduler_transcript(&transcript, fault_address)?;
            println!(
                "scheduler transcript proves bounded preemption and user-fault containment: {}",
                path.display()
            );
            Ok(())
        }
        Some("supervision-smoke") => {
            reject_extra_arguments(arguments)?;
            build_and_inspect_normal_image()?;
            let image = build_image(&["supervision_test"])?;
            let root = repository_root()?;
            let fault_address =
                resolve_supervision_fault_address(&root, &root.join("build/kernel.elf"))?;
            run_qemu_supervision_smoke(&image, fault_address)
        }
        Some("verify-supervision-transcript") => {
            let path = arguments
                .next()
                .map(PathBuf::from)
                .ok_or_else(|| "verify-supervision-transcript requires one log path".to_string())?;
            reject_extra_arguments(arguments)?;
            let transcript = fs::read_to_string(&path)
                .map_err(|error| format!("read {}: {error}", path.display()))?;
            let root = repository_root()?;
            let fault_address =
                resolve_supervision_fault_address(&root, &root.join("build/kernel.elf"))?;
            verify_supervision_transcript(&transcript, fault_address)?;
            println!(
                "supervision transcript proves one bounded vessel restart: {}",
                path.display()
            );
            Ok(())
        }
        Some("shell-vessel-smoke") => {
            reject_extra_arguments(arguments)?;
            build_and_inspect_normal_image()?;
            let image = build_image(&["shell_vessel_test"])?;
            let root = repository_root()?;
            let fault_address =
                resolve_shell_vessel_fault_address(&root, &root.join("build/kernel.elf"))?;
            run_qemu_shell_vessel_smoke(&image, fault_address)
        }
        Some("input-wait-smoke") => {
            reject_extra_arguments(arguments)?;
            build_and_inspect_normal_image()?;
            let image = build_image(&["input_wait_test"])?;
            run_qemu_input_wait_smoke(&image)
        }
        Some("display-vessel-smoke") => {
            reject_extra_arguments(arguments)?;
            build_and_inspect_normal_image()?;
            let image = build_image(&["display_vessel_test"])?;
            inspect_display_vessel_feature_image()?;
            run_qemu_display_vessel_smoke(&image)
        }
        Some("display-demo") => {
            reject_extra_arguments(arguments)?;
            let image = build_image(&["display_vessel_test"])?;
            inspect_display_vessel_feature_image()?;
            run_qemu_display_demo(&image)
        }
        Some("supervised-renderer-inspect") => {
            reject_extra_arguments(arguments)?;
            let artifact = build_and_package_supervised_renderer()?;
            println!(
                "supervised renderer inspected: {} bytes, {} pages, entries {:#018x}/{:#018x}, payload {}",
                artifact.file_size,
                artifact.image_pages,
                artifact.generation_1_entry,
                artifact.generation_2_entry,
                artifact.object.display()
            );
            Ok(())
        }
        Some("supervised-renderer-all-features-link") => {
            reject_extra_arguments(arguments)?;
            let image = build_image(supervised_renderer_all_kernel_features)?;
            println!(
                "all kernel features linked with supervised-renderer precedence: {}",
                image.display()
            );
            Ok(())
        }
        Some("supervised-renderer-smoke") => {
            reject_extra_arguments(arguments)?;
            remove_supervised_renderer_capture_artifacts()?;
            build_and_inspect_normal_image()?;
            let image = build_image_with_artifacts(&[supervised_renderer_feature])?;
            let image_pages = image
                .supervised_renderer
                .as_ref()
                .ok_or_else(|| "supervised renderer metadata is missing".to_string())?
                .image_pages;
            run_qemu_supervised_renderer_smoke(&image.path, image_pages)
        }
        Some("supervised-renderer-demo") => {
            reject_extra_arguments(arguments)?;
            let image = build_image(&[supervised_renderer_feature])?;
            run_qemu_supervised_renderer_demo(&image)
        }
        Some("semantic-surface-transport-inspect") => {
            reject_extra_arguments(arguments)?;
            let artifacts = build_and_package_semantic_surface_transport()?;
            println!(
                "semantic-surface transport inspected: source {} bytes; consumer {} bytes",
                artifacts.source.file_size, artifacts.consumer.file_size
            );
            Ok(())
        }
        Some("semantic-surface-transport-all-features-link") => {
            reject_extra_arguments(arguments)?;
            let image = build_image(semantic_surface_transport_all_kernel_features)?;
            println!(
                "all kernel features linked with semantic-surface transport precedence: {}",
                image.display()
            );
            Ok(())
        }
        Some("semantic-surface-transport-smoke") => {
            reject_extra_arguments(arguments)?;
            build_and_inspect_normal_image()?;
            let image = build_image(&[semantic_surface_transport_feature])?;
            run_qemu_semantic_surface_transport_smoke(&image)
        }
        Some("owned-workspace-cpu-build" | "owned-workspace-cpu-image") => {
            reject_extra_arguments(arguments)?;
            owned_workspace_cpu::build().map(|_| ())
        }
        Some("owned-workspace-cpu-inspect") => {
            reject_extra_arguments(arguments)?;
            let root = repository_root()?;
            owned_workspace_cpu::package_image(&root, &root.join("build")).map(|_| ())
        }
        Some("owned-workspace-cpu-smoke") => {
            reject_extra_arguments(arguments)?;
            owned_workspace_cpu::smoke()
        }
        Some("owned-workspace-build" | "owned-workspace-image") => {
            reject_extra_arguments(arguments)?;
            owned_workspace::build().map(|_| ())
        }
        Some("owned-workspace-inspect") => {
            reject_extra_arguments(arguments)?;
            let root = repository_root()?;
            owned_workspace::package_image(&root, &root.join("build")).map(|_| ())
        }
        Some("owned-workspace-smoke") => {
            reject_extra_arguments(arguments)?;
            owned_workspace::smoke()
        }
        Some("workspace-image") => {
            reject_extra_arguments(arguments)?;
            workspace_dev::build().map(|_| ())
        }
        Some("workspace-run") => {
            reject_extra_arguments(arguments)?;
            workspace_dev::run(workspace_guard)
        }
        Some("workspace-smoke") => {
            reject_extra_arguments(arguments)?;
            workspace_dev::smoke()
        }
        Some("workspace-editor-smoke") => {
            reject_extra_arguments(arguments)?;
            workspace_dev::editor_smoke()
        }
        Some("desktop-image") => {
            reject_extra_arguments(arguments)?;
            desktop_dev::build().map(|_| ())
        }
        Some("desktop-run") => {
            reject_extra_arguments(arguments)?;
            desktop_dev::run(workspace_guard)
        }
        Some("desktop-smoke") => {
            reject_extra_arguments(arguments)?;
            desktop_dev::smoke()
        }
        Some("semantic-presentation-inspect") => {
            reject_extra_arguments(arguments)?;
            let root = repository_root()?;
            let build = root.join("build");
            fs::create_dir_all(&build)
                .map_err(|error| format!("create {}: {error}", build.display()))?;
            let host = semantic_presentation_host_run::prepare(&build)?;
            let artifacts =
                build_and_package_semantic_surface_presentation_in(&root, &build, &host)?;
            println!(
                "semantic presentation payloads inspected: source {} bytes; renderer {} bytes",
                artifacts.source.file_size, artifacts.consumer.file_size
            );
            Ok(())
        }
        Some("semantic-presentation-all-features-link") => {
            reject_extra_arguments(arguments)?;
            let root = repository_root()?;
            let build = root.join("build");
            fs::create_dir_all(&build)
                .map_err(|error| format!("create {}: {error}", build.display()))?;
            let host = semantic_presentation_host_run::prepare(&build)?;
            let image = build_image_with_context(all_kernel_features, Some(&host))?;
            println!(
                "all kernel features linked with semantic-presentation precedence: {}",
                image.path.display()
            );
            Ok(())
        }
        Some("semantic-presentation-smoke") => {
            reject_extra_arguments(arguments)?;
            remove_semantic_presentation_capture_artifacts()?;
            build_and_inspect_normal_image()?;
            let root = repository_root()?;
            let build = root.join("build");
            let host = semantic_presentation_host_run::prepare(&build)?;
            let image = build_semantic_presentation_image(&host)?;
            let payloads = image
                .semantic_surface
                .as_ref()
                .ok_or_else(|| "semantic-presentation payload metadata is missing".to_string())?;
            let source_pages = payloads.source.file_size.div_ceil(circle_page_size);
            let renderer_pages = payloads.consumer.file_size.div_ceil(circle_page_size);
            run_qemu_semantic_presentation_smoke(&image.path, source_pages, renderer_pages, host)
        }
        Some("verify-shell-vessel-transcript") => {
            let path = arguments.next().map(PathBuf::from).ok_or_else(|| {
                "verify-shell-vessel-transcript requires one log path".to_string()
            })?;
            reject_extra_arguments(arguments)?;
            let root = repository_root()?;
            build_image(&["shell_vessel_test"])?;
            let fault_address =
                resolve_shell_vessel_fault_address(&root, &root.join("build/kernel.elf"))?;
            let transcript = fs::read_to_string(&path)
                .map_err(|error| format!("read {}: {error}", path.display()))?;
            verify_shell_vessel_transcript(&transcript, fault_address)?;
            println!(
                "shell-vessel transcript proves fixed supervised rite composition: {}",
                path.display()
            );
            Ok(())
        }
        Some("verify-input-wait-transcript") => {
            let path = arguments
                .next()
                .map(PathBuf::from)
                .ok_or_else(|| "verify-input-wait-transcript requires one log path".to_string())?;
            reject_extra_arguments(arguments)?;
            let transcript = fs::read_to_string(&path)
                .map_err(|error| format!("read {}: {error}", path.display()))?;
            verify_input_wait_transcript(&transcript)?;
            println!(
                "input-wait transcript proves exact semantic input wake and interruption: {}",
                path.display()
            );
            Ok(())
        }
        Some("verify-display-vessel-transcript") => {
            let path = arguments.next().map(PathBuf::from).ok_or_else(|| {
                "verify-display-vessel-transcript requires one log path".to_string()
            })?;
            reject_extra_arguments(arguments)?;
            let transcript = fs::read_to_string(&path)
                .map_err(|error| format!("read {}: {error}", path.display()))?;
            verify_display_vessel_transcript_redacted(&transcript)?;
            println!(
                "display-vessel transcript proves the bounded presentation lifecycle: {}",
                path.display()
            );
            Ok(())
        }
        Some("verify-display-vessel-frame") => {
            let path = arguments
                .next()
                .map(PathBuf::from)
                .ok_or_else(|| "verify-display-vessel-frame requires one PPM path".to_string())?;
            reject_extra_arguments(arguments)?;
            verify_display_vessel_frame_file_redacted(&path)?;
            println!(
                "display-vessel frame matches every public fixture pixel: {}",
                path.display()
            );
            Ok(())
        }
        Some("memory-smoke") => {
            reject_extra_arguments(arguments)?;
            build_and_inspect_normal_image()?;
            let image = build_image(&["memory_test"])?;
            let root = repository_root()?;
            let kernel_elf = root.join("build/kernel.elf");
            let bounds = memory_image_bounds {
                kernel_start: symbol_u64(&root, &kernel_elf, "kernel_image_start")?,
                kernel_end: symbol_u64(&root, &kernel_elf, "kernel_memory_end")?,
            };
            validate_memory_image_bounds(bounds)?;
            run_qemu_memory_smoke(&image, bounds)
        }
        Some("interrupt-smoke") => {
            reject_extra_arguments(arguments)?;
            build_and_inspect_normal_image()?;
            let image = build_image(&["interrupt_test"])?;
            run_qemu_interrupt_smoke(&image)
        }
        Some("normal-exclusion") => {
            reject_extra_arguments(arguments)?;
            build_and_inspect_normal_image()
        }
        Some("shell-smoke") => {
            reject_extra_arguments(arguments)?;
            let image = build_image(&[])?;
            run_qemu_shell_smoke(&image)
        }
        Some("help" | "-h" | "--help") | None => {
            print_help();
            Ok(())
        }
        Some(command) => Err(format!(
            "unknown command `{command}`; run `cargo xtask --help` for usage"
        )),
    }
}

struct workspace_lock {
    _file: File,
    #[cfg(not(unix))]
    path: PathBuf,
}

#[cfg(not(unix))]
impl Drop for workspace_lock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

#[cfg(unix)]
unsafe extern "C" {
    #[link_name = "flock"]
    fn system_flock(
        file_descriptor: std::ffi::c_int,
        operation: std::ffi::c_int,
    ) -> std::ffi::c_int;
}

fn acquire_workspace_lock() -> Result<workspace_lock, String> {
    acquire_workspace_lock_in(&repository_root()?.join("build"))
}

fn acquire_workspace_lock_in(build: &Path) -> Result<workspace_lock, String> {
    fs::create_dir_all(build).map_err(|error| format!("create {}: {error}", build.display()))?;
    let lock_path = build.join("xtask.lock");

    #[cfg(unix)]
    let lock = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(&lock_path)
        .map_err(|error| format!("open {}: {error}", lock_path.display()))?;
    #[cfg(unix)]
    {
        use std::os::fd::AsRawFd;

        const lock_exclusive: std::ffi::c_int = 2;
        const lock_nonblocking: std::ffi::c_int = 4;
        // SAFETY: `lock` owns one live descriptor for this call. The numeric
        // operation values are the Unix `LOCK_EX | LOCK_NB` ABI shared by the
        // supported Unix development hosts.
        if unsafe { system_flock(lock.as_raw_fd(), lock_exclusive | lock_nonblocking) } != 0 {
            return Err(
                "another Anomalous xtask owns the shared build artifacts; retry after it completes"
                    .into(),
            );
        }
        Ok(workspace_lock { _file: lock })
    }

    #[cfg(not(unix))]
    {
        let lock = OpenOptions::new()
            .create_new(true)
            .read(true)
            .write(true)
            .open(&lock_path)
            .map_err(|_| {
                "another Anomalous xtask may own the shared build artifacts; retry after it completes or remove a stale build/xtask.lock"
                    .to_string()
            })?;
        Ok(workspace_lock {
            _file: lock,
            path: lock_path,
        })
    }
}

fn build_image(kernel_features: &[&str]) -> Result<PathBuf, String> {
    build_image_with_artifacts(kernel_features).map(|image| image.path)
}

fn build_image_with_artifacts(kernel_features: &[&str]) -> Result<built_image, String> {
    build_image_with_context(kernel_features, None)
}

fn build_semantic_presentation_image(
    host: &semantic_presentation_host_run,
) -> Result<built_image, String> {
    build_image_with_context(&[semantic_surface_presentation_feature], Some(host))
}

fn build_image_with_context(
    kernel_features: &[&str],
    semantic_presentation_host: Option<&semantic_presentation_host_run>,
) -> Result<built_image, String> {
    let root = repository_root()?;
    let build = root.join("build");
    fs::create_dir_all(&build).map_err(|error| format!("create {}: {error}", build.display()))?;

    let has_semantic_presentation =
        kernel_features.contains(&semantic_surface_presentation_feature);
    let supervised_renderer = if kernel_features.contains(&supervised_renderer_feature)
        && !kernel_features.contains(&semantic_surface_transport_feature)
        && !has_semantic_presentation
    {
        Some(build_and_package_supervised_renderer_in(&root, &build)?)
    } else {
        None
    };
    let semantic_surface = if has_semantic_presentation {
        let host = semantic_presentation_host.ok_or_else(|| {
            "semantic-presentation image build requires its retained host transaction".to_string()
        })?;
        Some(build_and_package_semantic_surface_presentation_in(
            &root, &build, host,
        )?)
    } else if kernel_features.contains(&semantic_surface_transport_feature) {
        Some(build_and_package_semantic_surface_transport_in(
            &root, &build,
        )?)
    } else {
        None
    };
    let owned_workspace_payload = if owned_workspace_cpu::selected(kernel_features) {
        Some(owned_workspace_cpu::package_image(&root, &build)?)
    } else if owned_workspace::selected(kernel_features) {
        Some(owned_workspace::package_image(&root, &build)?)
    } else {
        None
    };
    let native_editor_workspace_payload = if native_editor_workspace::selected(kernel_features) {
        Some(native_editor_workspace::package_image(&root, &build)?)
    } else {
        None
    };
    let workspace_payload = if workspace_dev::selected(kernel_features) {
        Some(workspace_dev::package_image(&root, &build)?)
    } else {
        None
    };
    let linux_loader_entry_payload = if linux_loader_entry_smoke::selected(kernel_features) {
        Some(linux_loader_entry_artifact::package_image(&root, &build)?)
    } else {
        None
    };
    let veil_payload = if linux_tls_smoke::selected(kernel_features) {
        Some(linux_tls_artifact::package_image(&root, &build)?)
    } else if veil_application_smoke::selected(kernel_features)
        || shared_veil_smoke::selected(kernel_features)
    {
        Some(veil_application_artifact::package_image(&root, &build)?)
    } else {
        None
    };
    let linux_payload = if linux_application_smoke::selected(kernel_features) {
        Some(linux_application_artifact::package_image(&root, &build)?)
    } else {
        None
    };
    let elf_relaunch_payload = if elf_relaunch_smoke::selected(kernel_features) {
        Some(elf_relaunch_artifact::package_image(&root, &build)?)
    } else {
        None
    };
    let native_durable_failure_payload = if native_durable_failure_smoke::selected(kernel_features)
    {
        Some(native_durable_failure_artifact::package_image(
            &root, &build,
        )?)
    } else {
        None
    };
    let native_interactive_payload = if native_interactive_smoke::selected(kernel_features) {
        Some(native_durable_artifact::package_interactive_image(
            &root, &build,
        )?)
    } else {
        None
    };
    let native_repeated_payload = if native_repeated_smoke::selected(kernel_features) {
        Some(native_durable_artifact::package_repeated_image(
            &root, &build,
        )?)
    } else {
        None
    };
    let native_durable_payload = if native_durable_smoke::selected(kernel_features) {
        Some(native_durable_artifact::package_image(&root, &build)?)
    } else {
        None
    };
    let native_relaunch_payload = if native_relaunch_smoke::selected(kernel_features) {
        Some(native_relaunch_artifact::package_image(&root, &build)?)
    } else {
        None
    };
    let elf_document_payload = if elf_document_smoke::selected(kernel_features) {
        Some(elf_document_artifact::package_image(&root, &build)?)
    } else {
        None
    };
    let elf_payload = if elf_application_smoke::selected(kernel_features) {
        Some(elf_application_artifact::package_image(&root, &build)?)
    } else {
        None
    };
    let native_document_payload = if native_document_smoke::selected(kernel_features) {
        Some(native_document_artifact::package_image(&root, &build)?)
    } else {
        None
    };
    let native_runtime_payload = if native_runtime_smoke::selected(kernel_features) {
        Some(native_runtime_artifact::package_image(&root, &build)?)
    } else {
        None
    };
    let native_manifest_payload = if native_manifest_smoke::selected(kernel_features) {
        Some(native_manifest_artifact::package_image(&root, &build)?)
    } else {
        None
    };
    let native_program_payload = if native_program_smoke::selected(kernel_features) {
        Some(native_program_artifact::package_image(&root, &build)?)
    } else {
        None
    };
    let native_payload = if native_application_smoke::selected(kernel_features) {
        Some(native_application_artifact::package_image(&root, &build)?)
    } else {
        None
    };
    let ny_payload = if ny_object_smoke::selected(kernel_features) {
        Some(ny_object_artifact::package_image(&root, &build)?)
    } else {
        None
    };
    let ny_vessel_payload = if ny_vessel_smoke::selected(kernel_features) {
        Some(ny_vessel_artifact::package_image(&root, &build)?)
    } else {
        None
    };
    build_kernel(&root, kernel_features)?;
    let kernel_elf = link_kernel(
        &root,
        &build,
        supervised_renderer
            .as_ref()
            .map(|artifact| artifact.object.as_path()),
        semantic_surface.as_ref(),
        owned_workspace_payload
            .as_ref()
            .map(|artifact| artifact.object.as_path())
            .or_else(|| {
                workspace_payload
                    .as_ref()
                    .map(|artifact| artifact.object.as_path())
            }),
        linux_loader_entry_payload
            .as_ref()
            .map(|artifact| artifact.object.as_path())
            .or_else(|| {
                native_editor_workspace_payload
                    .as_ref()
                    .map(|artifact| artifact.object.as_path())
            })
            .or_else(|| {
                veil_payload
                    .as_ref()
                    .map(|artifact| artifact.object.as_path())
            })
            .or_else(|| {
                linux_payload
                    .as_ref()
                    .map(|artifact| artifact.object.as_path())
            })
            .or_else(|| {
                elf_relaunch_payload
                    .as_ref()
                    .map(|artifact| artifact.object.as_path())
            })
            .or_else(|| {
                native_durable_failure_payload
                    .as_ref()
                    .map(|artifact| artifact.object.as_path())
            })
            .or_else(|| {
                native_interactive_payload
                    .as_ref()
                    .map(|artifact| artifact.object.as_path())
            })
            .or_else(|| {
                native_repeated_payload
                    .as_ref()
                    .map(|artifact| artifact.object.as_path())
            })
            .or_else(|| {
                native_durable_payload
                    .as_ref()
                    .map(|artifact| artifact.object.as_path())
            })
            .or_else(|| {
                native_relaunch_payload
                    .as_ref()
                    .map(|artifact| artifact.object.as_path())
            })
            .or_else(|| {
                elf_document_payload
                    .as_ref()
                    .map(|artifact| artifact.object.as_path())
            })
            .or_else(|| {
                elf_payload
                    .as_ref()
                    .map(|artifact| artifact.object.as_path())
            })
            .or_else(|| {
                native_document_payload
                    .as_ref()
                    .map(|artifact| artifact.object.as_path())
            })
            .or_else(|| {
                native_manifest_payload
                    .as_ref()
                    .map(|artifact| artifact.object.as_path())
            })
            .or_else(|| {
                native_program_payload
                    .as_ref()
                    .map(|artifact| artifact.object.as_path())
            })
            .or_else(|| {
                native_runtime_payload
                    .as_ref()
                    .map(|artifact| artifact.object.as_path())
            })
            .or_else(|| {
                native_payload
                    .as_ref()
                    .map(|artifact| artifact.object.as_path())
            })
            .or_else(|| {
                ny_payload
                    .as_ref()
                    .map(|artifact| artifact.object.as_path())
            })
            .or_else(|| {
                ny_vessel_payload
                    .as_ref()
                    .map(|artifact| artifact.object.as_path())
            }),
        native_editor_workspace::selected(kernel_features)
            || shared_veil_smoke::selected(kernel_features)
            || linux_tls_smoke::selected(kernel_features)
            || linux_loader_entry_smoke::selected(kernel_features)
            || native_durable_failure_smoke::selected(kernel_features)
            || native_durable_smoke::selected(kernel_features)
            || native_repeated_smoke::selected(kernel_features)
            || native_interactive_smoke::selected(kernel_features)
            || native_runtime_smoke::selected(kernel_features)
            || native_manifest_smoke::selected(kernel_features)
            || native_program_smoke::selected(kernel_features)
            || native_document_smoke::selected(kernel_features)
            || elf_document_smoke::selected(kernel_features)
            || native_relaunch_smoke::selected(kernel_features)
            || elf_relaunch_smoke::selected(kernel_features),
    )?;
    if let Some(artifact) = linux_loader_entry_payload.as_ref() {
        linux_loader_entry_artifact::inspect_image(&root, &kernel_elf, artifact)?;
    }
    if let Some(artifact) = veil_payload.as_ref() {
        if linux_tls_smoke::selected(kernel_features) {
            linux_tls_artifact::inspect_linked(&root, &kernel_elf, artifact)?;
        } else {
            veil_application_artifact::inspect_linked(&root, &kernel_elf, artifact)?;
        }
    }
    if let Some(artifact) = native_editor_workspace_payload.as_ref() {
        native_editor_workspace::inspect_linked(&root, &kernel_elf, artifact)?;
    }
    if let Some(artifact) = linux_payload.as_ref() {
        linux_application_artifact::inspect_linked(&root, &kernel_elf, artifact)?;
    }
    if let Some(artifact) = elf_relaunch_payload.as_ref() {
        elf_relaunch_artifact::inspect_linked(&root, &kernel_elf, artifact)?;
    }
    if let Some(artifact) = native_durable_failure_payload.as_ref() {
        native_durable_failure_artifact::inspect_linked(&root, &kernel_elf, artifact)?;
    }
    if let Some(artifact) = native_interactive_payload.as_ref() {
        native_durable_artifact::inspect_linked(&root, &kernel_elf, artifact)?;
    }
    if let Some(artifact) = native_repeated_payload.as_ref() {
        native_durable_artifact::inspect_linked(&root, &kernel_elf, artifact)?;
    }
    if let Some(artifact) = native_durable_payload.as_ref() {
        native_durable_artifact::inspect_linked(&root, &kernel_elf, artifact)?;
    }
    if let Some(artifact) = native_relaunch_payload.as_ref() {
        native_relaunch_artifact::inspect_linked(&root, &kernel_elf, artifact)?;
    }
    if let Some(artifact) = elf_document_payload.as_ref() {
        elf_document_artifact::inspect_linked(&root, &kernel_elf, artifact)?;
    }
    if let Some(artifact) = elf_payload.as_ref() {
        elf_application_artifact::inspect_linked(&root, &kernel_elf, artifact)?;
    }
    if let Some(artifact) = native_document_payload.as_ref() {
        native_document_artifact::inspect_linked(&root, &kernel_elf, artifact)?;
    }
    if let Some(artifact) = native_manifest_payload.as_ref() {
        native_manifest_artifact::inspect_linked(&root, &kernel_elf, artifact)?;
    }
    if let Some(artifact) = native_program_payload.as_ref() {
        native_program_artifact::inspect_linked(&root, &kernel_elf, artifact)?;
    }
    if let Some(artifact) = native_runtime_payload.as_ref() {
        native_runtime_artifact::inspect_linked(&root, &kernel_elf, artifact)?;
    }
    if let Some(artifact) = native_payload.as_ref() {
        native_application_artifact::inspect_linked(&root, &kernel_elf, artifact)?;
    }
    if let Some(artifact) = ny_payload.as_ref() {
        ny_object_artifact::inspect_linked(&root, &kernel_elf, artifact)?;
    }
    if mixed_workspace::selected(kernel_features) {
        mixed_workspace::inspect_linked(&root, &kernel_elf)?;
    }
    if let Some(artifact) = owned_workspace_payload.as_ref() {
        owned_workspace::inspect_linked(&root, &kernel_elf, artifact)?;
    }
    if let Some(artifact) = workspace_payload.as_ref() {
        workspace_dev::inspect_linked(&root, &kernel_elf, artifact)?;
    }
    if let Some(renderer) = supervised_renderer.as_ref() {
        inspect_linked_supervised_renderer_payload(&root, &kernel_elf, renderer)?;
    }
    if let Some(artifacts) = semantic_surface.as_ref() {
        inspect_linked_semantic_surface_payload(&root, &kernel_elf, &artifacts.source)?;
        inspect_linked_semantic_surface_payload(&root, &kernel_elf, &artifacts.consumer)?;
    }
    let kernel = binary_from_elf(&root, &kernel_elf, &build.join("kernel.bin"))?;
    if supervised_renderer.is_some() {
        inspect_supervised_renderer_feature_image(&root, &kernel_elf, &kernel)?;
    }
    if desktop_dev::selected(kernel_features) {
        desktop_dev::inspect_image(&root, &kernel_elf, &kernel)?;
    }
    if has_semantic_presentation {
        inspect_semantic_presentation_feature_image(&root, &kernel_elf, &kernel)?;
    } else if semantic_surface.is_some() {
        inspect_semantic_surface_feature_image(&root, &kernel_elf, &kernel)?;
    }
    if let Some(host) = semantic_presentation_host {
        let elf_bytes = fs::read(&kernel_elf)
            .map_err(|error| format!("read {}: {error}", kernel_elf.display()))?;
        let flat_bytes =
            fs::read(&kernel).map_err(|error| format!("read {}: {error}", kernel.display()))?;
        reject_semantic_presentation_probes(
            &elf_bytes,
            "linked kernel ELF",
            &host.protected_probes,
        )?;
        reject_semantic_presentation_probes(
            &flat_bytes,
            "linked kernel image",
            &host.protected_probes,
        )?;
    }
    let kernel_extent = inspect_extent(&kernel, kernel_staging_capacity, "kernel")?;
    let kernel_entry = symbol_u32(&root, &kernel_elf, "kernel_main")?;
    let kernel_image_start = symbol_u32(&root, &kernel_elf, "kernel_image_start")?;
    let kernel_file_end = symbol_u32(&root, &kernel_elf, "kernel_file_end")?;
    let kernel_memory_end = symbol_u32(&root, &kernel_elf, "kernel_memory_end")?;
    if kernel_image_start != kernel_load_address
        || kernel_file_end.checked_sub(kernel_image_start) != Some(kernel_extent.file_size)
    {
        return Err("linked kernel file extent does not match its flat binary".into());
    }
    let kernel_memory_size = kernel_memory_end
        .checked_sub(kernel_image_start)
        .ok_or_else(|| "linked kernel memory extent is inverted".to_string())?;
    if kernel_memory_size < kernel_extent.file_size
        || kernel_memory_size > maximum_kernel_memory_size
        || !(kernel_image_start..kernel_file_end).contains(&kernel_entry)
    {
        return Err("linked kernel addresses violate the Threshold manifest contract".into());
    }

    let stage_2_elf = link_stage_2(&root, &build)?;
    let stage_2 = binary_from_elf(&root, &stage_2_elf, &build.join("threshold_stage_2.bin"))?;
    let stage_2_extent = inspect_extent(
        &stage_2,
        maximum_stage_2_sectors * sector_size as u32,
        "Threshold stage 2",
    )?;
    let threshold_entry = symbol_u32(&root, &stage_2_elf, "threshold_entry")?;
    let threshold_image_start = symbol_u32(&root, &stage_2_elf, "threshold_image_start")?;
    let threshold_file_end = symbol_u32(&root, &stage_2_elf, "threshold_file_end")?;
    if threshold_entry != stage_2_load_address
        || threshold_image_start != stage_2_load_address
        || threshold_file_end.checked_sub(threshold_image_start) != Some(stage_2_extent.file_size)
    {
        return Err("linked stage-2 extent does not match its flat binary".into());
    }

    let manifest = threshold_manifest {
        stage_2: stage_2_extent,
        kernel: kernel_extent,
        kernel_memory_size,
        kernel_entry_address: kernel_entry,
    }
    .encode();
    let manifest_path = build.join("threshold_manifest.bin");
    fs::write(&manifest_path, manifest)
        .map_err(|error| format!("write {}: {error}", manifest_path.display()))?;

    let stage_1 = build_stage_1(&root, &build)?;
    let image = build.join(if linux_loader_entry_smoke::selected(kernel_features) {
        "linux-loader-entry-test.img"
    } else if linux_tls_smoke::selected(kernel_features) {
        "linux-tls-test.img"
    } else if shared_veil_smoke::selected(kernel_features) {
        "shared-veil-test.img"
    } else if veil_application_smoke::selected(kernel_features) {
        "veil-application-test.img"
    } else if linux_application_smoke::selected(kernel_features) {
        "linux-application-test.img"
    } else if elf_relaunch_smoke::selected(kernel_features) {
        "elf-relaunch-test.img"
    } else if native_durable_failure_smoke::selected(kernel_features) {
        "native-durable-failure-test.img"
    } else if native_interactive_smoke::selected(kernel_features) {
        "native-interactive-editor-test.img"
    } else if native_repeated_smoke::selected(kernel_features) {
        "native-repeated-editor-test.img"
    } else if native_durable_smoke::selected(kernel_features) {
        "native-durable-test.img"
    } else if native_relaunch_smoke::selected(kernel_features) {
        "native-relaunch-test.img"
    } else if elf_document_smoke::selected(kernel_features) {
        "elf-document-test.img"
    } else if elf_application_smoke::selected(kernel_features) {
        "elf-application-test.img"
    } else if native_document_smoke::selected(kernel_features) {
        "native-document-test.img"
    } else if native_manifest_smoke::selected(kernel_features) {
        "native-manifest-test.img"
    } else if native_program_smoke::selected(kernel_features) {
        "native-program-test.img"
    } else if native_runtime_smoke::selected(kernel_features) {
        "native-runtime-test.img"
    } else if native_application_smoke::selected(kernel_features) {
        "native-application-test.img"
    } else if ny_vessel_smoke::selected(kernel_features) {
        "ny-vessel-test.img"
    } else if ny_object_smoke::selected(kernel_features) {
        "ny-object-test.img"
    } else if native_editor_workspace::selected(kernel_features) {
        "native-editor-workspace-test.img"
    } else if mixed_workspace::selected(kernel_features) {
        "mixed-workspace-test.img"
    } else if application_profile_smoke::selected(kernel_features) {
        "application-profile-test.img"
    } else if owned_source_smoke::selected(kernel_features) {
        "owned-source-test.img"
    } else if owned_tls_smoke::selected(kernel_features) {
        "owned-tls-test.img"
    } else if owned_multi_region_smoke::selected(kernel_features) {
        "owned-multi-region-test.img"
    } else if owned_dispatcher_smoke::selected(kernel_features) {
        "owned-dispatcher-test.img"
    } else if dispatcher_smoke::selected(kernel_features) {
        "dispatcher-test.img"
    } else if owned_workspace_cpu::selected(kernel_features) {
        "owned-workspace-cpu-test.img"
    } else if owned_workspace::selected(kernel_features) {
        "owned-workspace-test.img"
    } else if workspace_dev::selected(kernel_features) {
        "workspace-dev.img"
    } else if desktop_dev::selected(kernel_features) {
        "desktop-dev.img"
    } else {
        "anomalous.img"
    });
    assemble_raw_disk(&stage_1, &manifest_path, &stage_2, &kernel, &image)?;
    Ok(built_image {
        path: image,
        supervised_renderer,
        semantic_surface,
    })
}

fn build_and_inspect_normal_image() -> Result<(), String> {
    let image = build_image(&[])?;
    let root = repository_root()?;
    let build = root.join("build");
    let source = build_and_package_semantic_surface_package(
        &root,
        &build,
        &semantic_surface_source_package,
        None,
    )?;
    let snapshot = source
        .snapshot
        .as_deref()
        .ok_or_else(|| "semantic-surface source snapshot missing".to_string())?;
    let presentation_host = semantic_presentation_host_run::prepare(&build)?;
    inspect_normal_feature_exclusion(
        &root,
        &root.join("build/kernel.elf"),
        &root.join("build/kernel.bin"),
        snapshot,
        &source.private_probe_out_directories,
        &presentation_host.snapshot,
        &presentation_host.protected_probes,
    )?;
    println!(
        "normal image excludes feature-test probes: {}",
        image.display()
    );
    Ok(())
}

fn build_kernel(root: &Path, kernel_features: &[&str]) -> Result<(), String> {
    let cargo = env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"));
    let mut arguments = vec![
        OsString::from("build"),
        OsString::from("-Z"),
        OsString::from("build-std=core,compiler_builtins"),
        OsString::from("-Z"),
        OsString::from("build-std-features=compiler-builtins-mem"),
        OsString::from("--release"),
        OsString::from("--package"),
        OsString::from("kernel"),
        OsString::from("--target"),
        OsString::from("x86_64-unknown-none"),
        OsString::from("--locked"),
        OsString::from("--no-default-features"),
    ];
    if !kernel_features.is_empty() {
        arguments.push(OsString::from("--features"));
        arguments.push(OsString::from(kernel_features.join(",")));
    }
    run_checked(root, cargo, arguments)
}

fn link_kernel(
    root: &Path,
    build: &Path,
    supervised_renderer_payload: Option<&Path>,
    semantic_surface_payloads: Option<&semantic_surface_artifacts>,
    workspace_payload: Option<&Path>,
    application_payload: Option<&Path>,
    discard_unreferenced_sections: bool,
) -> Result<PathBuf, String> {
    let kernel_archive = root.join("target/x86_64-unknown-none/release/libanomalous_kernel.a");
    let kernel_linker = root.join("boot/threshold/x86_64/kernel.ld");
    let kernel_elf = build.join("kernel.elf");
    let mut arguments = vec![
        OsString::from("--build-id=none"),
        OsString::from("--fatal-warnings"),
        OsString::from("--orphan-handling=error"),
        OsString::from("--undefined=kernel_main"),
        OsString::from("-z"),
        OsString::from("max-page-size=0x1000"),
        OsString::from("-T"),
        kernel_linker.into_os_string(),
        OsString::from("-o"),
        kernel_elf.clone().into_os_string(),
        kernel_archive.into_os_string(),
    ];
    // The shared-veil, native-runtime and native-document proofs remove archive sections
    // unreachable from kernel_main/entry relocations within Threshold's existing
    // 384 KiB staging bound. Other profiles retain their original link behavior;
    // post-link inspection still requires every selected, qualified payload.
    if discard_unreferenced_sections {
        arguments.push(OsString::from("--gc-sections"));
    }
    if let Some(payload) = application_payload {
        arguments.push(payload.to_path_buf().into_os_string());
    }
    if let Some(payload) = workspace_payload {
        arguments.push(payload.to_path_buf().into_os_string());
    }
    if let Some(payload) = supervised_renderer_payload {
        arguments.push(payload.to_path_buf().into_os_string());
    }
    if let Some(payloads) = semantic_surface_payloads {
        arguments.push(payloads.source.object.clone().into_os_string());
        arguments.push(payloads.consumer.object.clone().into_os_string());
    }
    run_checked(root, "ld", arguments)?;
    let kernel_debug = build.join("kernel.debug");
    run_checked(
        root,
        "objcopy",
        [
            OsString::from("--only-keep-debug"),
            kernel_elf.clone().into_os_string(),
            kernel_debug.into_os_string(),
        ],
    )?;
    Ok(kernel_elf)
}

fn build_and_package_semantic_surface_transport() -> Result<semantic_surface_artifacts, String> {
    let root = repository_root()?;
    let build = root.join("build");
    fs::create_dir_all(&build).map_err(|error| format!("create {}: {error}", build.display()))?;
    build_and_package_semantic_surface_transport_in(&root, &build)
}

fn build_and_package_semantic_surface_transport_in(
    root: &Path,
    build: &Path,
) -> Result<semantic_surface_artifacts, String> {
    verify_semantic_surface_kernel_dependencies(root)?;
    let source = build_and_package_semantic_surface_package(
        root,
        build,
        &semantic_surface_source_package,
        None,
    )?;
    let consumer = build_and_package_semantic_surface_consumer(
        root,
        build,
        &source.private_probe_out_directories,
        source
            .snapshot
            .as_deref()
            .ok_or_else(|| "semantic-surface source snapshot is missing".to_string())?,
    )?;
    Ok(semantic_surface_artifacts { source, consumer })
}

fn build_and_package_semantic_surface_presentation_in(
    root: &Path,
    build: &Path,
    host: &semantic_presentation_host_run,
) -> Result<semantic_surface_artifacts, String> {
    verify_semantic_surface_kernel_dependencies_for(root, semantic_surface_presentation_feature)?;
    if !host.transaction_is_pending() {
        return Err("semantic-presentation host transaction advanced before native build".into());
    }
    let build_inputs = semantic_presentation_build_inputs::prepare(build, &host.snapshot)?;
    let (first_snapshot, second_snapshot) = build_inputs.paths();
    let source = build_and_package_semantic_surface_package(
        root,
        build,
        &semantic_surface_source_package,
        Some((first_snapshot, second_snapshot)),
    )?;
    if source.snapshot.as_deref() != Some(host.snapshot.as_slice()) {
        return Err("packaged semantic-presentation source changed the retained body".into());
    }
    let renderer = build_and_package_semantic_surface_receiver(
        root,
        build,
        &semantic_surface_renderer_package,
        &source.private_probe_out_directories,
        &host.snapshot,
    )?;
    for (label, path) in [
        ("source flat image", &source.binary),
        ("source wrapper", &source.object),
        ("renderer flat image", &renderer.binary),
        ("renderer wrapper", &renderer.object),
    ] {
        let bytes = fs::read(path).map_err(|error| format!("read {}: {error}", path.display()))?;
        reject_semantic_presentation_probes(&bytes, label, &host.protected_probes)?;
    }
    build_inputs.remove_all()?;
    Ok(semantic_surface_artifacts {
        source,
        consumer: renderer,
    })
}

fn verify_semantic_surface_kernel_dependencies(root: &Path) -> Result<(), String> {
    verify_semantic_surface_kernel_dependencies_for(root, semantic_surface_transport_feature)
}

fn verify_semantic_surface_kernel_dependencies_for(
    root: &Path,
    feature: &str,
) -> Result<(), String> {
    let output = run_output_checked(
        root,
        "cargo",
        [
            OsString::from("tree"),
            OsString::from("--locked"),
            OsString::from("--package"),
            OsString::from("kernel"),
            OsString::from("--target"),
            OsString::from("x86_64-unknown-none"),
            OsString::from("--edges"),
            OsString::from("normal"),
            OsString::from("--prefix"),
            OsString::from("none"),
            OsString::from("--format"),
            OsString::from("{p}"),
            OsString::from("--no-default-features"),
            OsString::from("--features"),
            OsString::from(feature),
        ],
    )?;
    let mut actual = String::from_utf8(output)
        .map_err(|_| "semantic-surface kernel dependency listing is not UTF-8".to_string())?
        .lines()
        .filter_map(|line| line.split_whitespace().next())
        .map(str::to_string)
        .collect::<Vec<_>>();
    actual.sort();
    actual.dedup();
    let expected = ["anomalous-abi", "anomalous-desktop-transport", "kernel"];
    if actual != expected {
        return Err(format!(
            "semantic-surface kernel normal dependency set is not exact: {}",
            actual.join(", ")
        ));
    }
    Ok(())
}

fn write_private_bytes(path: &Path, bytes: &[u8]) -> Result<(), String> {
    remove_optional_file(path)?;
    let mut options = OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .map_err(|error| format!("create private artifact {}: {error}", path.display()))?;
    file.write_all(bytes)
        .and_then(|()| file.flush())
        .map_err(|error| format!("write private artifact {}: {error}", path.display()))
}

fn reject_semantic_presentation_probes(
    bytes: &[u8],
    label: &str,
    probes: &[Vec<u8>],
) -> Result<(), String> {
    if probes
        .iter()
        .any(|probe| !probe.is_empty() && bytes.windows(probe.len()).any(|window| window == probe))
    {
        Err(format!(
            "semantic-presentation protected source entered {label}"
        ))
    } else {
        Ok(())
    }
}

fn build_and_package_semantic_surface_package(
    root: &Path,
    build: &Path,
    package: &'static semantic_surface_package,
    snapshot_paths: Option<(&Path, &Path)>,
) -> Result<semantic_surface_artifact, String> {
    if !package.snapshot {
        return build_and_package_semantic_surface_consumer(root, build, &[], &[]);
    }
    let target = root.join(semantic_surface_target_spec);
    verify_semantic_surface_target_cfg(root, &target)?;
    verify_semantic_surface_dependencies(root, &target, package)?;
    let stem = package.role.replace('-', "_");
    let elf = build.join(format!("semantic_surface_{stem}.elf"));
    let binary = build.join(format!("semantic_surface_{stem}.bin"));
    let object = build.join(format!("semantic_surface_{stem}_payload.o"));
    for path in [&elf, &binary, &object] {
        remove_optional_file(path)?;
    }
    let cargo = env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"));
    let arguments = vec![
        OsString::from("-Zjson-target-spec"),
        OsString::from("-Zbuild-std=core,compiler_builtins"),
        OsString::from("-Zbuild-std-features=compiler-builtins-mem"),
        OsString::from("build"),
        OsString::from("--release"),
        OsString::from("--package"),
        OsString::from(package.package),
        OsString::from("--target"),
        target.clone().into_os_string(),
        OsString::from("--locked"),
        OsString::from("--no-default-features"),
    ];
    let first_target = build.join("semantic_surface_source_first_target");
    let second_target = build.join("semantic_surface_source_second_target");
    for target_directory in [&first_target, &second_target] {
        if target_directory.exists() {
            fs::remove_dir_all(target_directory)
                .map_err(|error| format!("remove {}: {error}", target_directory.display()))?;
        }
    }
    let mut first_environment = vec![
        (
            semantic_surface_source_variant_environment,
            OsString::from("first"),
        ),
        ("CARGO_TARGET_DIR", first_target.clone().into_os_string()),
    ];
    if let Some((first, _)) = snapshot_paths {
        first_environment.push((
            "ANOMALOUS_SEMANTIC_SURFACE_SNAPSHOT_PATH",
            first.as_os_str().to_os_string(),
        ));
    }
    run_checked_with_environments(root, &cargo, arguments.clone(), &first_environment)?;
    let cargo_elf = first_target
        .join(semantic_surface_target_name)
        .join("release")
        .join(package.package);
    let first =
        fs::read(&cargo_elf).map_err(|error| format!("read {}: {error}", cargo_elf.display()))?;
    let first_out = semantic_surface_source_out_directory(&first_target)?;
    let mut second_environment = vec![
        (
            semantic_surface_source_variant_environment,
            OsString::from("second"),
        ),
        ("CARGO_TARGET_DIR", second_target.clone().into_os_string()),
    ];
    if let Some((_, second)) = snapshot_paths {
        second_environment.push((
            "ANOMALOUS_SEMANTIC_SURFACE_SNAPSHOT_PATH",
            second.as_os_str().to_os_string(),
        ));
    }
    run_checked_with_environments(root, &cargo, arguments, &second_environment)?;
    let second_elf = second_target
        .join(semantic_surface_target_name)
        .join("release")
        .join(package.package);
    let second =
        fs::read(&second_elf).map_err(|error| format!("read {}: {error}", cargo_elf.display()))?;
    let second_out = semantic_surface_source_out_directory(&second_target)?;
    let probe_out_directories = vec![first_out, second_out];
    scan_semantic_surface_directory(
        &first_target,
        &probe_out_directories,
        "first Cargo target directory",
    )?;
    scan_semantic_surface_directory(
        &second_target,
        &probe_out_directories,
        "second Cargo target directory",
    )?;
    reject_semantic_surface_private_sources(&first, "first target ELF", &probe_out_directories)?;
    reject_semantic_surface_private_sources(&second, "second target ELF", &probe_out_directories)?;
    if first != second {
        return Err("semantic-surface source variants produced different target ELFs".into());
    }
    let snapshot = read_semantic_surface_snapshot(&probe_out_directories[0])?;
    fs::write(&elf, &first).map_err(|error| format!("write {}: {error}", elf.display()))?;
    let (load_bytes, offsets) =
        inspect_semantic_surface_elf(root, &elf, package, &snapshot, &probe_out_directories)?;
    run_checked(
        root,
        "objcopy",
        [
            OsString::from("-O"),
            OsString::from("binary"),
            elf.clone().into_os_string(),
            binary.clone().into_os_string(),
        ],
    )?;
    let flat = fs::read(&binary).map_err(|error| format!("read {}: {error}", binary.display()))?;
    if flat != load_bytes {
        return Err(format!(
            "semantic-surface {} flat binary differs from its sole load",
            package.role
        ));
    }
    if count_subslice(&flat, &snapshot) != 1 {
        return Err(
            "semantic-surface source does not contain exactly one generated 528-byte snapshot"
                .into(),
        );
    }
    reject_semantic_surface_private_sources(&flat, "flat binary", &probe_out_directories)?;
    package_semantic_surface_binary(build, package, &binary, &object, &offsets)?;
    inspect_semantic_surface_wrapper(
        root,
        package,
        &object,
        &flat,
        &offsets,
        &probe_out_directories,
    )?;
    Ok(semantic_surface_artifact {
        package,
        binary,
        object,
        file_size: flat.len() as u64,
        offsets,
        snapshot: Some(snapshot),
        private_probe_out_directories: probe_out_directories,
    })
}
fn semantic_surface_source_out_directory(target: &Path) -> Result<PathBuf, String> {
    // Cargo's target build script emits the canonical body; only its generated
    // artifact is inspected, never executed on the host.
    let build = target
        .join(semantic_surface_target_name)
        .join("release/build");
    let entries =
        fs::read_dir(&build).map_err(|error| format!("read {}: {error}", build.display()))?;
    for entry in entries.flatten() {
        let out = entry.path().join("out");
        if out.join("semantic_surface_snapshot.bin").is_file() {
            return Ok(out);
        }
    }
    Err("semantic-surface generated source OUT_DIR was not found".into())
}
fn semantic_surface_known_probe_directories() -> Result<Vec<PathBuf>, String> {
    let root = repository_root()?;
    let build = root.join("build");
    Ok(vec![
        semantic_surface_source_out_directory(&build.join("semantic_surface_source_first_target"))?,
        semantic_surface_source_out_directory(
            &build.join("semantic_surface_source_second_target"),
        )?,
    ])
}
fn read_semantic_surface_snapshot(out: &Path) -> Result<Vec<u8>, String> {
    let bytes = fs::read(out.join("semantic_surface_snapshot.bin"))
        .map_err(|error| format!("read semantic-surface snapshot: {error}"))?;
    if bytes.len() == semantic_surface_snapshot_size {
        Ok(bytes)
    } else {
        Err("semantic-surface snapshot has wrong size".into())
    }
}

fn build_and_package_semantic_surface_consumer(
    root: &Path,
    build: &Path,
    probes: &[PathBuf],
    snapshot: &[u8],
) -> Result<semantic_surface_artifact, String> {
    let package = &semantic_surface_consumer_package;
    build_and_package_semantic_surface_receiver(root, build, package, probes, snapshot)
}

fn build_and_package_semantic_surface_receiver(
    root: &Path,
    build: &Path,
    package: &'static semantic_surface_package,
    probes: &[PathBuf],
    snapshot: &[u8],
) -> Result<semantic_surface_artifact, String> {
    let target = root.join(semantic_surface_target_spec);
    verify_semantic_surface_target_cfg(root, &target)?;
    verify_semantic_surface_dependencies(root, &target, package)?;
    let target_directory = build.join(format!("semantic_surface_{}_target", package.role));
    if target_directory.exists() {
        fs::remove_dir_all(&target_directory)
            .map_err(|error| format!("remove {}: {error}", target_directory.display()))?;
    }
    let cargo = env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"));
    let arguments = vec![
        OsString::from("-Zjson-target-spec"),
        OsString::from("-Zbuild-std=core,compiler_builtins"),
        OsString::from("-Zbuild-std-features=compiler-builtins-mem"),
        OsString::from("build"),
        OsString::from("--release"),
        OsString::from("--package"),
        OsString::from(package.package),
        OsString::from("--target"),
        target.into_os_string(),
        OsString::from("--locked"),
        OsString::from("--no-default-features"),
    ];
    run_checked_with_environments(
        root,
        &cargo,
        arguments,
        &[(
            "CARGO_TARGET_DIR",
            target_directory.clone().into_os_string(),
        )],
    )?;
    scan_semantic_surface_directory(
        &target_directory,
        probes,
        &format!("{} Cargo target directory", package.role),
    )?;
    let cargo_elf = target_directory
        .join(semantic_surface_target_name)
        .join("release")
        .join(package.package);
    let bytes =
        fs::read(&cargo_elf).map_err(|error| format!("read {}: {error}", cargo_elf.display()))?;
    reject_semantic_surface_private_sources(
        &bytes,
        &format!("{} target ELF", package.role),
        probes,
    )?;
    if package.role == "renderer" {
        reject_adr_0026_capsule(&bytes, "semantic renderer target ELF")?;
    }
    let elf = build.join(format!("semantic_surface_{}.elf", package.role));
    let binary = build.join(format!("semantic_surface_{}.bin", package.role));
    let object = build.join(format!("semantic_surface_{}_payload.o", package.role));
    for path in [&elf, &binary, &object] {
        remove_optional_file(path)?;
    }
    fs::write(&elf, &bytes).map_err(|error| format!("write {}: {error}", elf.display()))?;
    let (load, offsets) = inspect_semantic_surface_elf(root, &elf, package, snapshot, probes)?;
    run_checked(
        root,
        "objcopy",
        [
            OsString::from("-O"),
            OsString::from("binary"),
            elf.clone().into_os_string(),
            binary.clone().into_os_string(),
        ],
    )?;
    let flat = fs::read(&binary).map_err(|error| format!("read {}: {error}", binary.display()))?;
    if flat != load || count_subslice(&flat, snapshot) != 0 {
        return Err(format!(
            "semantic-surface {} flat image is not snapshot-free",
            package.role
        ));
    }
    reject_semantic_surface_private_sources(
        &flat,
        &format!("{} flat binary", package.role),
        probes,
    )?;
    if package.role == "renderer" {
        reject_adr_0026_capsule(&flat, "semantic renderer flat binary")?;
    }
    package_semantic_surface_binary(build, package, &binary, &object, &offsets)?;
    inspect_semantic_surface_wrapper(root, package, &object, &flat, &offsets, probes)?;
    Ok(semantic_surface_artifact {
        package,
        binary,
        object,
        file_size: flat.len() as u64,
        offsets,
        snapshot: None,
        private_probe_out_directories: probes.to_vec(),
    })
}

fn run_checked_with_environments<I, S>(
    directory: &Path,
    program: S,
    arguments: I,
    environments: &[(&str, OsString)],
) -> Result<(), String>
where
    I: IntoIterator<Item = OsString>,
    S: AsRef<OsStr>,
{
    let program = program.as_ref();
    let mut command = Command::new(program);
    command
        .current_dir(directory)
        .args(arguments)
        .env("SOURCE_DATE_EPOCH", "0");
    for (key, value) in environments {
        command.env(key, value);
    }
    let status = command
        .status()
        .map_err(|error| format!("launch {}: {error}", program.to_string_lossy()))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "{} exited with {status}",
            program.to_string_lossy()
        ))
    }
}

fn semantic_surface_private_probes(
    out_directories: &[PathBuf],
) -> Result<semantic_surface_probe_buffers, String> {
    let mut values = Vec::new();
    for out in out_directories {
        let seed = out.as_os_str().as_encoded_bytes();
        if seed.is_empty() {
            return Err("semantic-surface source OUT_DIR is empty".into());
        }
        for (domain, length) in semantic_surface_private_source_variants {
            let mut value = Vec::with_capacity(*length);
            for index in 0..*length {
                value.push(
                    b'a' + (seed[index % seed.len()]
                        .wrapping_add(*domain)
                        .wrapping_add(index as u8)
                        % 26),
                );
            }
            values.push(value);
        }
    }
    Ok(semantic_surface_probe_buffers { values })
}
struct semantic_surface_probe_buffers {
    values: Vec<Vec<u8>>,
}
impl Drop for semantic_surface_probe_buffers {
    fn drop(&mut self) {
        for value in &mut self.values {
            value.fill(0);
        }
    }
}
fn scan_semantic_surface_directory(
    directory: &Path,
    out_directories: &[PathBuf],
    label: &str,
) -> Result<(), String> {
    if out_directories.is_empty() {
        return Ok(());
    }
    let probes = semantic_surface_private_probes(out_directories)?;
    let mut pending = vec![directory.to_path_buf()];
    while let Some(path) = pending.pop() {
        for entry in
            fs::read_dir(&path).map_err(|error| format!("read {}: {error}", path.display()))?
        {
            let entry = entry.map_err(|error| format!("read directory entry: {error}"))?;
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
            } else {
                let bytes =
                    fs::read(&path).map_err(|error| format!("read {}: {error}", path.display()))?;
                if probes
                    .values
                    .iter()
                    .any(|probe| bytes.windows(probe.len()).any(|window| window == probe))
                {
                    return Err(format!(
                        "semantic-surface private source entered {label}: {}",
                        path.display()
                    ));
                }
            }
        }
    }
    Ok(())
}

fn verify_semantic_surface_target_cfg(root: &Path, target: &Path) -> Result<(), String> {
    let output = run_output_checked(
        root,
        env::var_os("RUSTC").unwrap_or_else(|| OsString::from("rustc")),
        [
            OsString::from("-Zunstable-options"),
            OsString::from("--target"),
            target.to_path_buf().into_os_string(),
            OsString::from("--print"),
            OsString::from("cfg"),
        ],
    )?;
    let listing = String::from_utf8(output)
        .map_err(|_| "semantic-surface target cfg is not UTF-8".to_string())?;
    for required in [
        "panic=\"abort\"",
        "relocation_model=\"static\"",
        "target_arch=\"x86_64\"",
        "target_endian=\"little\"",
        "target_object_format=\"elf\"",
        "target_os=\"none\"",
        "target_pointer_width=\"64\"",
    ] {
        if !listing.lines().any(|line| line == required) {
            return Err(format!(
                "semantic-surface target cfg is missing `{required}`"
            ));
        }
    }
    Ok(())
}

fn verify_semantic_surface_dependencies(
    root: &Path,
    target: &Path,
    package: &semantic_surface_package,
) -> Result<(), String> {
    let output = run_output_checked(
        root,
        "cargo",
        [
            OsString::from("-Zjson-target-spec"),
            OsString::from("tree"),
            OsString::from("--locked"),
            OsString::from("--package"),
            OsString::from(package.package),
            OsString::from("--target"),
            target.to_path_buf().into_os_string(),
            OsString::from("--edges"),
            OsString::from("normal"),
            OsString::from("--prefix"),
            OsString::from("none"),
            OsString::from("--format"),
            OsString::from("{p}"),
            OsString::from("--no-default-features"),
        ],
    )?;
    let mut actual = String::from_utf8(output)
        .map_err(|_| "semantic-surface dependency listing is not UTF-8".to_string())?
        .lines()
        .filter_map(|line| line.split_whitespace().next())
        .map(str::to_string)
        .collect::<Vec<_>>();
    actual.sort();
    actual.dedup();
    let mut expected = package
        .dependencies
        .iter()
        .map(|name| (*name).to_string())
        .collect::<Vec<_>>();
    expected.sort();
    if actual != expected {
        return Err(format!(
            "semantic-surface {} normal dependency set is not exact: {}",
            package.role,
            actual.join(", ")
        ));
    }
    Ok(())
}

fn inspect_semantic_surface_elf(
    root: &Path,
    elf: &Path,
    package: &semantic_surface_package,
    snapshot: &[u8],
    probes: &[PathBuf],
) -> Result<(Vec<u8>, Vec<u64>), String> {
    let bytes = fs::read(elf).map_err(|error| format!("read {}: {error}", elf.display()))?;
    reject_semantic_surface_private_sources(&bytes, "ELF", probes)?;
    if package.role == "renderer" {
        reject_adr_0026_capsule(&bytes, "semantic renderer target ELF")?;
    }
    let parsed = parse_elf64(&bytes)?;
    if parsed.kind != 2 || parsed.machine != 62 || parsed.program_headers.len() != 1 {
        return Err(format!(
            "semantic-surface {} is not an x86-64 single-segment ET_EXEC",
            package.role
        ));
    }
    let load = parsed.program_headers[0];
    if load.kind != 1
        || load.flags != 5
        || load.virtual_address != semantic_surface_image_base
        || load.physical_address != semantic_surface_image_base
        || load.file_size == 0
        || load.file_size != load.memory_size
        || load.alignment != circle_page_size
        || load.file_size > semantic_surface_page_limit * circle_page_size
    {
        return Err(format!(
            "semantic-surface {} load segment violates RX/page bounds",
            package.role
        ));
    }
    let symbols = String::from_utf8(run_output_checked(
        root,
        "nm",
        [
            OsString::from("--defined-only"),
            OsString::from("--format=posix"),
            elf.to_path_buf().into_os_string(),
        ],
    )?)
    .map_err(|_| "semantic-surface symbols are not UTF-8".to_string())?;
    for name in package.required_symbols {
        let _ = symbol_u64_from_listing(&symbols, name)?;
    }
    let prefix = format!("anomalous_semantic_surface_{}_", package.role);
    let start = symbol_u64_from_listing(&symbols, &format!("{prefix}image_start"))?;
    let end = symbol_u64_from_listing(&symbols, &format!("{prefix}image_end"))?;
    let payload_end = symbol_u64_from_listing(&symbols, &format!("{prefix}image_payload_end"))?;
    let entry = symbol_u64_from_listing(&symbols, &format!("{prefix}entry"))?;
    if start != semantic_surface_image_base
        || entry != start
        || parsed.entry != entry
        || payload_end != load.virtual_address + load.file_size
        || end != round_up_to_page(payload_end)?
        || end - start > semantic_surface_page_limit * circle_page_size
    {
        return Err(format!(
            "semantic-surface {} image symbols violate fixed bounds",
            package.role
        ));
    }
    if package.role == "renderer" {
        inspect_semantic_renderer_control_flow(root, elf, &bytes, load, &symbols)?;
    }
    let offsets = package
        .wrapper_symbols
        .iter()
        .map(|wrapper| {
            let name = wrapper.replacen("x86_64_", "anomalous_", 1);
            symbol_u64_from_listing(&symbols, &name)?
                .checked_sub(start)
                .ok_or_else(|| format!("semantic-surface {name} precedes image"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    if offsets.iter().any(|offset| *offset >= load.file_size) {
        return Err(format!(
            "semantic-surface {} return symbol is outside image",
            package.role
        ));
    }
    if package.snapshot {
        let snapshot_start =
            symbol_u64_from_listing(&symbols, "anomalous_semantic_surface_source_snapshot_start")?;
        let snapshot_end =
            symbol_u64_from_listing(&symbols, "anomalous_semantic_surface_source_snapshot_end")?;
        if snapshot_end - snapshot_start != semantic_surface_snapshot_size as u64
            || count_subslice(
                elf_virtual_bytes(&bytes, load, start, load.file_size as usize)?,
                snapshot,
            ) != 1
        {
            return Err("semantic-surface source snapshot contract is not exact".into());
        }
    }
    Ok((
        elf_virtual_bytes(&bytes, load, start, load.file_size as usize)?.to_vec(),
        offsets,
    ))
}

fn inspect_semantic_renderer_control_flow(
    root: &Path,
    elf: &Path,
    bytes: &[u8],
    load: elf64_program_header,
    symbols: &str,
) -> Result<(), String> {
    let entry = symbol_u64_from_listing(symbols, "anomalous_semantic_surface_renderer_entry")?;
    let failed = symbol_u64_from_listing(symbols, "anomalous_semantic_surface_renderer_failed")?;
    let returns = [
        symbol_u64_from_listing(
            symbols,
            "anomalous_semantic_surface_renderer_submit_denied_return",
        )?,
        symbol_u64_from_listing(
            symbols,
            "anomalous_semantic_surface_renderer_delivery_return",
        )?,
        symbol_u64_from_listing(
            symbols,
            "anomalous_semantic_surface_renderer_display_return",
        )?,
        symbol_u64_from_listing(symbols, "anomalous_semantic_surface_renderer_exit_return")?,
    ];
    let load_end = load
        .virtual_address
        .checked_add(load.file_size)
        .ok_or_else(|| "semantic renderer load extent overflowed".to_string())?;
    if entry != semantic_surface_image_base
        || !(entry < failed
            && failed < returns[0]
            && returns[0] < returns[1]
            && returns[1] < returns[2]
            && returns[2] < returns[3]
            && returns[3] < load_end)
    {
        return Err("semantic renderer entry, failure, or Rite returns are out of order".into());
    }
    if elf_virtual_bytes(bytes, load, failed, 2)? != [0x0f, 0x0b] {
        return Err("semantic renderer failure target is not UD2".into());
    }
    let mut rite_gate_tail = vec![0x49, 0xbf];
    rite_gate_tail.extend_from_slice(&semantic_surface_renderer_mailbox.to_le_bytes());
    rite_gate_tail.extend_from_slice(&[0x6a, 0x02, 0x9d, 0xcd, 0xf3]);
    for (return_address, role) in returns.into_iter().zip([
        "denied-submit Rite",
        "delivery Rite",
        "display Rite",
        "task-exit Rite",
    ]) {
        verify_semantic_renderer_gate_tail(bytes, load, return_address, &rite_gate_tail, role)?;
    }
    let disassembly = String::from_utf8(run_output_checked(
        root,
        "objdump",
        [
            OsString::from("--disassemble"),
            OsString::from("--no-show-raw-insn"),
            elf.to_path_buf().into_os_string(),
        ],
    )?)
    .map_err(|_| "semantic renderer disassembly is not valid UTF-8".to_string())?;
    verify_semantic_renderer_disassembly(&disassembly)
}

fn verify_semantic_renderer_gate_tail(
    bytes: &[u8],
    load: elf64_program_header,
    return_address: u64,
    expected: &[u8],
    role: &str,
) -> Result<(), String> {
    let start = return_address
        .checked_sub(expected.len() as u64)
        .ok_or_else(|| format!("semantic renderer {role} gate address underflowed"))?;
    if elf_virtual_bytes(bytes, load, start, expected.len())? != expected {
        return Err(format!(
            "semantic renderer {role} does not use the exact mailbox, flags, and Rite vector"
        ));
    }
    Ok(())
}

fn verify_semantic_renderer_disassembly(disassembly: &str) -> Result<(), String> {
    let lowered = disassembly.to_ascii_lowercase();
    for register in [
        "%xmm", "%ymm", "%zmm", "%mm", "%st", "%bnd", "%tmm", "%k", "%cr", "%dr",
    ] {
        if lowered.contains(register) {
            return Err("semantic renderer contains forbidden register use".into());
        }
    }

    let forbidden_exact = [
        "cli", "sti", "hlt", "syscall", "sysenter", "sysexit", "sysret", "lgdt", "lidt", "lldt",
        "ltr", "invlpg", "swapgs", "rdmsr", "wrmsr", "ldmxcsr", "stmxcsr", "emms", "fwait", "wait",
        "cpuid", "rdtsc", "rdtscp", "rdpmc", "rdrand", "rdseed", "rdpid", "xgetbv", "xsetbv",
        "rdpkru", "wrpkru", "rdfsbase", "rdgsbase", "wrfsbase", "wrgsbase", "monitor", "mwait",
        "monitorx", "mwaitx", "umwait", "tpause", "wbinvd", "invd", "clts", "in", "ins", "insb",
        "insw", "insl", "out", "outs", "outsb", "outsw", "outsl", "iret", "iretl", "iretq",
        "lcall", "ljmp", "retf", "vmcall", "vmlaunch", "vmresume", "vmxoff", "encls", "enclu",
        "getsec", "sgdt", "sidt", "sldt", "str", "smsw",
    ];
    let mut rite_gates = 0;
    for line in lowered.lines() {
        let Some((address, instruction)) = line.split_once(':') else {
            continue;
        };
        let address = address.trim();
        if address.is_empty() || !address.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            continue;
        }
        let mut fields = instruction.split_whitespace();
        let Some(mut mnemonic) = fields.next() else {
            continue;
        };
        while matches!(
            mnemonic,
            "rep" | "repe" | "repz" | "repne" | "repnz" | "lock" | "data16" | "addr32"
        ) {
            let Some(next) = fields.next() else {
                return Err("semantic renderer has an incomplete instruction prefix".into());
            };
            mnemonic = next;
        }
        if forbidden_exact.contains(&mnemonic)
            || mnemonic.starts_with('f')
            || mnemonic.starts_with("xsave")
            || mnemonic.starts_with("xrstor")
            || mnemonic.starts_with("fxsave")
            || mnemonic.starts_with("fxrstor")
            || mnemonic.starts_with("vzero")
            || mnemonic.starts_with("tile")
            || matches!(mnemonic, "ldtilecfg" | "sttilecfg" | "(bad)")
        {
            return Err(format!(
                "semantic renderer contains forbidden instruction `{mnemonic}`"
            ));
        }
        if mnemonic == "int" {
            match fields.next() {
                Some("$0xf3") => rite_gates += 1,
                _ => return Err("semantic renderer contains an unexpected software gate".into()),
            }
        } else if matches!(mnemonic, "int3" | "into") {
            return Err("semantic renderer contains an unexpected software interrupt".into());
        }
    }
    if rite_gates != 4 {
        return Err("semantic renderer does not contain exactly four Rite gates".into());
    }
    Ok(())
}

fn reject_adr_0026_capsule(bytes: &[u8], artifact: &str) -> Result<(), String> {
    let has_magic = bytes
        .windows(supervised_renderer_capsule_magic.len())
        .any(|window| window == supervised_renderer_capsule_magic);
    let has_header = bytes
        .windows(supervised_renderer_capsule_header.len())
        .any(|window| window == supervised_renderer_capsule_header);
    let has_structure = bytes
        .windows(supervised_renderer_capsule_size)
        .any(looks_like_adr_0026_capsule);
    if has_magic || has_header || has_structure {
        return Err(format!("ADR 0026 capsule material entered {artifact}"));
    }
    Ok(())
}

fn looks_like_adr_0026_capsule(candidate: &[u8]) -> bool {
    if candidate.len() != supervised_renderer_capsule_size
        || candidate[4..16] != supervised_renderer_capsule_header[4..]
    {
        return false;
    }
    candidate[16..].chunks_exact(16).all(|command| {
        let secondary_color_is_valid = command[3] == 0xff || command[3] <= 14;
        let label_is_valid = command[5] == 0xff || command[5] <= 5;
        let x = u16::from_le_bytes([command[8], command[9]]);
        let y = u16::from_le_bytes([command[10], command[11]]);
        let width = u16::from_le_bytes([command[12], command[13]]);
        let height = u16::from_le_bytes([command[14], command[15]]);
        command[0] <= 6
            && command[1] <= 5
            && command[2] <= 14
            && secondary_color_is_valid
            && command[4] <= 7
            && label_is_valid
            && command[6] <= 1
            && command[7] == 0
            && width != 0
            && height != 0
            && x.checked_add(width).is_some_and(|right| right <= 1024)
            && y.checked_add(height).is_some_and(|bottom| bottom <= 768)
    })
}

fn package_semantic_surface_binary(
    build: &Path,
    package: &semantic_surface_package,
    binary: &Path,
    object: &Path,
    offsets: &[u64],
) -> Result<(), String> {
    let section = format!(".rodata.semantic_surface.{}.payload", package.role);
    let stack_note = build.join(format!("semantic_surface_{}_gnu_stack.note", package.role));
    fs::write(&stack_note, [])
        .map_err(|error| format!("write {}: {error}", stack_note.display()))?;
    let input = binary
        .file_name()
        .ok_or_else(|| "semantic-surface binary name is missing".to_string())?
        .to_string_lossy();
    let output = object
        .file_name()
        .ok_or_else(|| "semantic-surface object name is missing".to_string())?
        .to_string_lossy();
    let prefix = format!("x86_64_semantic_surface_{}_payload", package.role);
    let mut args = vec![
        OsString::from("-I"),
        OsString::from("binary"),
        OsString::from("-O"),
        OsString::from("elf64-x86-64"),
        OsString::from("-B"),
        OsString::from("i386:x86-64"),
        OsString::from("--rename-section"),
        OsString::from(format!(".data={section},alloc,load,readonly,data,contents")),
        OsString::from("--add-section"),
        OsString::from(format!(".note.GNU-stack={}", stack_note.display())),
        OsString::from("--set-section-flags"),
        OsString::from(".note.GNU-stack=readonly"),
    ];
    for (name, value) in [
        (format!("{prefix}_start"), format!("{section}:0x0,global")),
        (
            format!("{prefix}_end"),
            format!(
                "{section}:0x{:x},global",
                fs::metadata(binary)
                    .map_err(|error| format!("read {}: {error}", binary.display()))?
                    .len()
            ),
        ),
        (
            format!("{prefix}_size"),
            format!(
                "0x{:x},global",
                fs::metadata(binary)
                    .map_err(|error| format!("read {}: {error}", binary.display()))?
                    .len()
            ),
        ),
    ] {
        args.push(OsString::from("--add-symbol"));
        args.push(OsString::from(format!("{name}={value}")));
    }
    for (name, offset) in package.wrapper_symbols.iter().zip(offsets) {
        args.push(OsString::from("--add-symbol"));
        args.push(OsString::from(format!(
            "{name}={section}:0x{offset:x},global"
        )));
    }
    args.push(OsString::from(input.as_ref()));
    args.push(OsString::from(output.as_ref()));
    let result = run_checked(build, "objcopy", args);
    let cleanup = remove_optional_file(&stack_note);
    result.and(cleanup)
}

fn inspect_semantic_surface_wrapper(
    root: &Path,
    package: &semantic_surface_package,
    object: &Path,
    binary: &[u8],
    offsets: &[u64],
    probes: &[PathBuf],
) -> Result<(), String> {
    let bytes = fs::read(object).map_err(|error| format!("read {}: {error}", object.display()))?;
    reject_semantic_surface_private_sources(&bytes, "wrapper", probes)?;
    if package.role == "renderer" {
        reject_adr_0026_capsule(&bytes, "semantic renderer wrapper")?;
    }
    let parsed = parse_elf64(&bytes)?;
    if parsed.kind != 1
        || parsed.machine != 62
        || !parsed.program_headers.is_empty()
        || parsed
            .section_headers
            .iter()
            .any(|section| matches!(section.kind, 4 | 9))
    {
        return Err(format!(
            "semantic-surface {} wrapper is not relocation-free x86-64 ET_REL",
            package.role
        ));
    }
    let stack_notes = parsed
        .section_headers
        .iter()
        .filter(|section| section.name == ".note.GNU-stack")
        .collect::<Vec<_>>();
    if stack_notes.len() != 1
        || stack_notes[0].kind != 1
        || stack_notes[0].flags != 0
        || stack_notes[0].size != 0
    {
        return Err(format!(
            "semantic-surface {} wrapper lacks its non-executable-stack declaration",
            package.role
        ));
    }
    let payload = parsed
        .section_headers
        .iter()
        .filter(|section| section.flags & 2 != 0 && section.size != 0)
        .collect::<Vec<_>>();
    if payload.len() != 1
        || payload[0].flags != 2
        || payload[0].size != binary.len() as u64
        || bytes.get(payload[0].offset as usize..payload[0].offset as usize + binary.len())
            != Some(binary)
    {
        return Err(format!(
            "semantic-surface {} wrapper does not preserve one readonly payload",
            package.role
        ));
    }
    let listing = String::from_utf8(run_output_checked(
        root,
        "nm",
        [
            OsString::from("--defined-only"),
            OsString::from("--format=posix"),
            object.to_path_buf().into_os_string(),
        ],
    )?)
    .map_err(|_| "semantic-surface wrapper symbols are not UTF-8".to_string())?;
    let prefix = format!("x86_64_semantic_surface_{}_payload", package.role);
    for name in [
        format!("{prefix}_start"),
        format!("{prefix}_end"),
        format!("{prefix}_size"),
    ] {
        if !listing
            .lines()
            .any(|line| line.split_whitespace().next() == Some(name.as_str()))
        {
            return Err(format!("semantic-surface wrapper is missing `{name}`"));
        }
    }
    for name in package.wrapper_symbols {
        if !listing
            .lines()
            .any(|line| line.split_whitespace().next() == Some(*name))
        {
            return Err(format!("semantic-surface wrapper is missing `{name}`"));
        }
    }
    for (name, offset) in package.wrapper_symbols.iter().zip(offsets) {
        if symbol_u64_from_listing(&listing, name)? != *offset {
            return Err(format!("semantic-surface wrapper moved `{name}`"));
        }
    }
    Ok(())
}

fn inspect_linked_semantic_surface_payload(
    root: &Path,
    kernel_elf: &Path,
    artifact: &semantic_surface_artifact,
) -> Result<(), String> {
    let payload = fs::read(&artifact.binary)
        .map_err(|error| format!("read {}: {error}", artifact.binary.display()))?;
    let kernel =
        fs::read(kernel_elf).map_err(|error| format!("read {}: {error}", kernel_elf.display()))?;
    if artifact.package.role == "renderer" {
        reject_adr_0026_capsule(&payload, "semantic renderer linked payload")?;
        reject_adr_0026_capsule(&kernel, "linked semantic-presentation kernel")?;
    }
    let parsed = parse_elf64(&kernel)?;
    let listing = String::from_utf8(run_output_checked(
        root,
        "nm",
        [
            OsString::from("--defined-only"),
            OsString::from("--format=posix"),
            kernel_elf.to_path_buf().into_os_string(),
        ],
    )?)
    .map_err(|_| "linked semantic-surface symbols are not UTF-8".to_string())?;
    let prefix = format!("x86_64_semantic_surface_{}_payload", artifact.package.role);
    let start = symbol_u64_from_listing(&listing, &format!("{prefix}_start"))?;
    let end = symbol_u64_from_listing(&listing, &format!("{prefix}_end"))?;
    if end - start != payload.len() as u64 || count_subslice(&kernel, &payload) != 1 {
        return Err(format!(
            "linked kernel changed or duplicated semantic-surface {} payload",
            artifact.package.role
        ));
    }
    let load = parsed
        .program_headers
        .iter()
        .copied()
        .find(|load| {
            load.kind == 1
                && start >= load.virtual_address
                && end <= load.virtual_address + load.file_size
        })
        .ok_or_else(|| "linked semantic-surface payload is not load-backed".to_string())?;
    if elf_virtual_bytes(&kernel, load, start, payload.len())? != payload {
        return Err("linked semantic-surface payload bytes changed".into());
    }
    for (name, offset) in artifact
        .package
        .wrapper_symbols
        .iter()
        .zip(&artifact.offsets)
    {
        if symbol_u64_from_listing(&listing, name)? != start + offset {
            return Err(format!("linked semantic-surface symbol `{name}` moved"));
        }
    }
    Ok(())
}

fn reject_semantic_surface_private_sources(
    bytes: &[u8],
    artifact: &str,
    out_directories: &[PathBuf],
) -> Result<(), String> {
    let probes = semantic_surface_private_probes(out_directories)?;
    for probe in &probes.values {
        if bytes.windows(probe.len()).any(|window| window == probe) {
            return Err(format!(
                "semantic-surface private source entered {artifact}"
            ));
        }
    }
    Ok(())
}

fn build_and_package_supervised_renderer() -> Result<supervised_renderer_artifact, String> {
    let root = repository_root()?;
    let build = root.join("build");
    fs::create_dir_all(&build).map_err(|error| format!("create {}: {error}", build.display()))?;
    build_and_package_supervised_renderer_in(&root, &build)
}

fn build_and_package_supervised_renderer_in(
    root: &Path,
    build: &Path,
) -> Result<supervised_renderer_artifact, String> {
    let stable_elf = build.join(supervised_renderer_elf_name);
    let binary = build.join(supervised_renderer_binary_name);
    let object = build.join(supervised_renderer_object_name);
    for path in [&stable_elf, &binary, &object] {
        remove_optional_file(path)?;
    }

    let result = build_and_package_supervised_renderer_inner(
        root,
        build,
        stable_elf.clone(),
        binary.clone(),
        object.clone(),
    );
    if result.is_err() {
        for path in [&stable_elf, &binary, &object] {
            let _ = fs::remove_file(path);
        }
    }
    result
}

fn build_and_package_supervised_renderer_inner(
    root: &Path,
    build: &Path,
    stable_elf: PathBuf,
    binary: PathBuf,
    object: PathBuf,
) -> Result<supervised_renderer_artifact, String> {
    let target_spec = root.join(supervised_renderer_target_spec);
    if !target_spec.is_file() {
        return Err(format!(
            "supervised renderer target specification is missing: {}",
            target_spec.display()
        ));
    }
    verify_supervised_renderer_target_cfg(root, &target_spec)?;
    verify_supervised_renderer_dependencies(root, &target_spec)?;

    let cargo = env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"));
    let cargo_arguments = vec![
        OsString::from("-Zjson-target-spec"),
        OsString::from("-Zbuild-std=core,compiler_builtins"),
        OsString::from("-Zbuild-std-features=compiler-builtins-mem"),
        OsString::from("build"),
        OsString::from("--release"),
        OsString::from("--package"),
        OsString::from(supervised_renderer_package),
        OsString::from("--target"),
        target_spec.clone().into_os_string(),
        OsString::from("--locked"),
        OsString::from("--no-default-features"),
    ];
    run_checked_with_environment(
        root,
        &cargo,
        cargo_arguments.clone(),
        supervised_renderer_source_variant_environment,
        supervised_renderer_source_variant_first,
    )?;

    let cargo_elf = root
        .join("target")
        .join(supervised_renderer_target_name)
        .join("release")
        .join(supervised_renderer_package);
    if !cargo_elf.is_file() {
        return Err(format!(
            "Cargo did not produce the supervised renderer ELF at {}",
            cargo_elf.display()
        ));
    }
    let first_elf =
        fs::read(&cargo_elf).map_err(|error| format!("read {}: {error}", cargo_elf.display()))?;
    reject_supervised_renderer_protected_sources(&first_elf, "first protected-source ELF")?;

    run_checked_with_environment(
        root,
        &cargo,
        cargo_arguments,
        supervised_renderer_source_variant_environment,
        supervised_renderer_source_variant_second,
    )?;
    let second_elf =
        fs::read(&cargo_elf).map_err(|error| format!("read {}: {error}", cargo_elf.display()))?;
    reject_supervised_renderer_protected_sources(&second_elf, "second protected-source ELF")?;
    if first_elf != second_elf {
        return Err(
            "protected-source variants produced different supervised renderer target ELFs".into(),
        );
    }
    fs::write(&stable_elf, &first_elf)
        .map_err(|error| format!("write {}: {error}", stable_elf.display()))?;

    let (inspection, load_bytes) = inspect_supervised_renderer_elf(root, &stable_elf)?;
    run_checked(
        root,
        "objcopy",
        [
            OsString::from("-O"),
            OsString::from("binary"),
            stable_elf.clone().into_os_string(),
            binary.clone().into_os_string(),
        ],
    )?;
    let binary_bytes =
        fs::read(&binary).map_err(|error| format!("read {}: {error}", binary.display()))?;
    if binary_bytes != load_bytes {
        return Err("supervised renderer flat binary does not equal its sole load segment".into());
    }
    if count_subslice(&binary_bytes, &inspection.capsule) != 1 {
        return Err("supervised renderer flat binary does not contain exactly one capsule".into());
    }
    reject_supervised_renderer_protected_sources(&binary_bytes, "flat binary")?;

    package_supervised_renderer_binary(build, &inspection)?;
    inspect_supervised_renderer_wrapper(&object, &binary_bytes, &inspection)?;

    Ok(supervised_renderer_artifact {
        elf: stable_elf,
        binary,
        object,
        file_size: inspection.file_size,
        image_pages: inspection.image_pages,
        generation_1_entry: inspection.generation_1_entry,
        generation_2_entry: inspection.generation_2_entry,
        generation_1_fault: inspection.generation_1_fault,
        linked_symbol_offsets: inspection.linked_symbol_offsets.clone(),
    })
}

fn verify_supervised_renderer_target_cfg(root: &Path, target_spec: &Path) -> Result<(), String> {
    let rustc = env::var_os("RUSTC").unwrap_or_else(|| OsString::from("rustc"));
    let output = run_output_checked(
        root,
        rustc,
        [
            OsString::from("-Zunstable-options"),
            OsString::from("--target"),
            target_spec.to_path_buf().into_os_string(),
            OsString::from("--print"),
            OsString::from("cfg"),
        ],
    )?;
    let listing = String::from_utf8(output)
        .map_err(|_| "renderer target cfg listing is not valid UTF-8".to_string())?;
    for required in [
        "panic=\"abort\"",
        "relocation_model=\"static\"",
        "target_arch=\"x86_64\"",
        "target_endian=\"little\"",
        "target_object_format=\"elf\"",
        "target_os=\"none\"",
        "target_pointer_width=\"64\"",
    ] {
        if !listing.lines().any(|line| line == required) {
            return Err(format!(
                "supervised renderer target cfg is missing `{required}`"
            ));
        }
    }
    if let Some(feature) = listing
        .lines()
        .find(|line| line.starts_with("target_feature="))
    {
        return Err(format!(
            "supervised renderer target unexpectedly enables `{feature}`"
        ));
    }
    Ok(())
}

fn verify_supervised_renderer_dependencies(root: &Path, target_spec: &Path) -> Result<(), String> {
    let cargo = env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"));
    let output = run_output_checked(
        root,
        cargo,
        [
            OsString::from("-Zjson-target-spec"),
            OsString::from("tree"),
            OsString::from("--locked"),
            OsString::from("--package"),
            OsString::from(supervised_renderer_package),
            OsString::from("--target"),
            target_spec.to_path_buf().into_os_string(),
            OsString::from("--edges"),
            OsString::from("normal"),
            OsString::from("--prefix"),
            OsString::from("none"),
            OsString::from("--format"),
            OsString::from("{p}"),
            OsString::from("--no-default-features"),
        ],
    )?;
    let listing = String::from_utf8(output)
        .map_err(|_| "Cargo dependency listing is not valid UTF-8".to_string())?;
    let mut packages = listing
        .lines()
        .filter_map(|line| line.split_whitespace().next())
        .map(str::to_string)
        .collect::<Vec<_>>();
    packages.sort();
    packages.dedup();
    let mut expected = supervised_renderer_allowed_target_dependencies
        .iter()
        .map(|name| (*name).to_string())
        .collect::<Vec<_>>();
    expected.sort();
    if packages != expected {
        return Err(format!(
            "supervised renderer target dependency set is not the exact bounded set: {}",
            packages.join(", ")
        ));
    }
    Ok(())
}

fn inspect_supervised_renderer_elf(
    root: &Path,
    elf: &Path,
) -> Result<(supervised_renderer_inspection, Vec<u8>), String> {
    let bytes = fs::read(elf).map_err(|error| format!("read {}: {error}", elf.display()))?;
    reject_supervised_renderer_protected_sources(&bytes, "ELF")?;
    let parsed = parse_elf64(&bytes)?;
    if parsed.kind != 2 || parsed.machine != 62 {
        return Err("supervised renderer must be an x86-64 ET_EXEC image".into());
    }
    if parsed.program_headers.len() != 1 {
        return Err("supervised renderer must have exactly one program header".into());
    }
    let load = parsed.program_headers[0];
    if load.kind != 1
        || load.flags != 5
        || load.virtual_address != supervised_renderer_image_base
        || load.physical_address != supervised_renderer_image_base
        || load.file_size == 0
        || load.file_size != load.memory_size
        || load.alignment != circle_page_size
    {
        return Err("supervised renderer does not have one exact fixed R+X load segment".into());
    }
    let load_end = load
        .virtual_address
        .checked_add(load.file_size)
        .ok_or_else(|| "supervised renderer load extent overflowed".to_string())?;
    if load.file_size > supervised_renderer_image_page_limit * circle_page_size {
        return Err("supervised renderer exceeds its 64-page image cap".into());
    }

    let capsule_section_address = inspect_supervised_renderer_sections(&parsed, load)?;

    let symbol_output = run_output_checked(
        root,
        "nm",
        [
            OsString::from("--defined-only"),
            OsString::from("--format=posix"),
            elf.to_path_buf().into_os_string(),
        ],
    )?;
    let symbols = String::from_utf8(symbol_output)
        .map_err(|_| "supervised renderer symbol table is not valid UTF-8".to_string())?;
    for required in supervised_renderer_required_symbols {
        let _ = symbol_u64_from_listing(&symbols, required)?;
    }
    if symbols.contains("anomalous_desktop_scene") || symbols.contains("scene_conformance") {
        return Err("host-only scene symbols entered the supervised renderer ELF".into());
    }
    let undefined = run_output_checked(
        root,
        "nm",
        [
            OsString::from("--undefined-only"),
            OsString::from("--format=posix"),
            elf.to_path_buf().into_os_string(),
        ],
    )?;
    if !undefined.iter().all(u8::is_ascii_whitespace) {
        return Err("supervised renderer ELF has unresolved symbols".into());
    }

    let image_start =
        symbol_u64_from_listing(&symbols, "anomalous_supervised_renderer_image_start")?;
    let payload_end =
        symbol_u64_from_listing(&symbols, "anomalous_supervised_renderer_image_payload_end")?;
    let image_end = symbol_u64_from_listing(&symbols, "anomalous_supervised_renderer_image_end")?;
    let capsule_start =
        symbol_u64_from_listing(&symbols, "anomalous_supervised_renderer_capsule_start")?;
    let capsule_end =
        symbol_u64_from_listing(&symbols, "anomalous_supervised_renderer_capsule_end")?;
    let generation_1_entry =
        symbol_u64_from_listing(&symbols, "anomalous_supervised_renderer_generation_1_entry")?;
    let generation_2_entry =
        symbol_u64_from_listing(&symbols, "anomalous_supervised_renderer_generation_2_entry")?;
    let generation_1_fault =
        symbol_u64_from_listing(&symbols, "anomalous_supervised_renderer_generation_1_fault")?;
    let generation_1_ready_return = symbol_u64_from_listing(
        &symbols,
        "anomalous_supervised_renderer_generation_1_ready_return",
    )?;
    let generation_1_preemption_wait = symbol_u64_from_listing(
        &symbols,
        "anomalous_supervised_renderer_generation_1_preemption_wait",
    )?;
    let generation_1_preemption_resume = symbol_u64_from_listing(
        &symbols,
        "anomalous_supervised_renderer_generation_1_preemption_resume",
    )?;
    let generation_1_failed = symbol_u64_from_listing(
        &symbols,
        "anomalous_supervised_renderer_generation_1_failed",
    )?;
    let generation_2_exit_return = symbol_u64_from_listing(
        &symbols,
        "anomalous_supervised_renderer_generation_2_exit_return",
    )?;
    let generation_2_failed = symbol_u64_from_listing(
        &symbols,
        "anomalous_supervised_renderer_generation_2_failed",
    )?;
    let submit_display_rite = symbol_u64_from_listing(
        &symbols,
        "anomalous_supervised_renderer_submit_display_rite",
    )?;
    let display_rite_return = symbol_u64_from_listing(
        &symbols,
        "anomalous_supervised_renderer_display_rite_return",
    )?;
    let submit_exit_rite =
        symbol_u64_from_listing(&symbols, "anomalous_supervised_renderer_submit_exit_rite")?;
    let task_exit_rite_return = symbol_u64_from_listing(
        &symbols,
        "anomalous_supervised_renderer_task_exit_rite_return",
    )?;
    if image_start != supervised_renderer_image_base
        || parsed.entry != generation_1_entry
        || generation_1_entry != supervised_renderer_image_base
        || generation_2_entry != supervised_renderer_generation_2_entry
        || payload_end != load_end
        || image_end != round_up_to_page(payload_end)?
        || image_end
            > supervised_renderer_image_base
                + supervised_renderer_image_page_limit * circle_page_size
        || !(image_start..payload_end).contains(&generation_1_fault)
        || !(image_start..payload_end).contains(&generation_2_entry)
    {
        return Err("supervised renderer linked symbols violate the fixed image contract".into());
    }
    let linked_symbol_addresses = supervised_renderer_linked_symbols
        .iter()
        .map(|symbol| symbol_u64_from_listing(&symbols, symbol.elf_name))
        .collect::<Result<Vec<_>, _>>()?;
    if linked_symbol_addresses
        .iter()
        .any(|address| !(image_start..payload_end).contains(address))
        || !(generation_1_entry < generation_1_ready_return
            && generation_1_ready_return.checked_add(6) == Some(generation_1_preemption_wait)
            && generation_1_preemption_wait < generation_1_preemption_resume
            && generation_1_preemption_resume < generation_1_fault
            && generation_1_fault < generation_1_failed
            && generation_1_failed < generation_2_entry
            && generation_2_entry < generation_2_exit_return
            && generation_2_exit_return == generation_2_failed
            && generation_2_failed < submit_display_rite
            && submit_display_rite < display_rite_return
            && display_rite_return < submit_exit_rite
            && submit_exit_rite < task_exit_rite_return)
    {
        return Err("supervised renderer code symbols violate their fixed ordering".into());
    }
    if image_end >= supervised_renderer_mailbox
        || supervised_renderer_mailbox + circle_page_size >= supervised_renderer_stack_base
        || supervised_renderer_stack_top - supervised_renderer_stack_base != 4 * circle_page_size
        || (image_start >> 21) != ((supervised_renderer_stack_top - 1) >> 21)
    {
        return Err(
            "supervised renderer image, guards, mailbox, and stack leave one fixed branch".into(),
        );
    }
    if capsule_start != capsule_section_address
        || capsule_end.checked_sub(capsule_start) != Some(supervised_renderer_capsule_size as u64)
    {
        return Err("supervised renderer capsule symbols do not span exactly 416 bytes".into());
    }
    let capsule = elf_virtual_bytes(
        &bytes,
        load,
        capsule_start,
        supervised_renderer_capsule_size,
    )?
    .to_vec();
    if !capsule.starts_with(supervised_renderer_capsule_magic) {
        return Err("supervised renderer capsule has the wrong magic".into());
    }
    if count_subslice(
        elf_virtual_bytes(&bytes, load, image_start, load.file_size as usize)?,
        &capsule,
    ) != 1
    {
        return Err("supervised renderer load segment does not contain exactly one capsule".into());
    }
    reject_supervised_renderer_protected_sources(&capsule, "capsule")?;
    for (address, role) in [
        (generation_1_fault, "generation-1 expected fault"),
        (generation_1_failed, "generation-1 failure"),
        (generation_2_failed, "generation-2 failure"),
    ] {
        if elf_virtual_bytes(&bytes, load, address, 2)? != [0x0f, 0x0b] {
            return Err(format!("supervised renderer {role} is not UD2"));
        }
    }
    let mut scheduler_gate_tail = vec![0x49, 0xbf];
    scheduler_gate_tail.extend_from_slice(&supervised_renderer_mailbox.to_le_bytes());
    scheduler_gate_tail
        .extend_from_slice(&[0xb8, 0x01, 0x00, 0x00, 0x00, 0x6a, 0x02, 0x9d, 0xcd, 0xf2]);
    verify_supervised_renderer_gate_tail(
        &bytes,
        load,
        generation_1_ready_return,
        &scheduler_gate_tail,
        "scheduler-ready",
    )?;
    if elf_virtual_bytes(&bytes, load, generation_1_ready_return, 6)?
        != [0x68, 0x02, 0x02, 0x00, 0x00, 0x9d]
    {
        return Err(
            "supervised renderer ready return does not request the exact running flags".into(),
        );
    }
    let mut rite_gate_tail = vec![0x49, 0xbf];
    rite_gate_tail.extend_from_slice(&supervised_renderer_mailbox.to_le_bytes());
    rite_gate_tail.extend_from_slice(&[0x6a, 0x02, 0x9d, 0xcd, 0xf3]);
    verify_supervised_renderer_gate_tail(
        &bytes,
        load,
        display_rite_return,
        &rite_gate_tail,
        "display Rite",
    )?;
    verify_supervised_renderer_gate_tail(
        &bytes,
        load,
        task_exit_rite_return,
        &rite_gate_tail,
        "task-exit Rite",
    )?;

    let disassembly = run_output_checked(
        root,
        "objdump",
        [
            OsString::from("--disassemble"),
            OsString::from("--no-show-raw-insn"),
            elf.to_path_buf().into_os_string(),
        ],
    )?;
    let disassembly = String::from_utf8(disassembly)
        .map_err(|_| "supervised renderer disassembly is not valid UTF-8".to_string())?;
    verify_supervised_renderer_disassembly(&disassembly)?;

    let load_bytes =
        elf_virtual_bytes(&bytes, load, image_start, load.file_size as usize)?.to_vec();
    if count_subslice(&load_bytes, &supervised_renderer_glyph_fingerprint) != 1 {
        return Err(
            "supervised renderer image does not contain exactly one frozen glyph fingerprint"
                .into(),
        );
    }
    let image_pages = image_end
        .checked_sub(image_start)
        .map(|length| length / circle_page_size)
        .filter(|pages| (1..=supervised_renderer_image_page_limit).contains(pages))
        .ok_or_else(|| "supervised renderer page count is outside its bounded range".to_string())?;
    let linked_symbol_offsets = linked_symbol_addresses
        .into_iter()
        .map(|address| {
            address
                .checked_sub(image_start)
                .ok_or_else(|| "supervised renderer linked-symbol offset underflowed".to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok((
        supervised_renderer_inspection {
            file_size: load.file_size,
            image_pages,
            generation_1_entry,
            generation_2_entry,
            generation_1_fault,
            linked_symbol_offsets,
            capsule,
        },
        load_bytes,
    ))
}

fn verify_supervised_renderer_gate_tail(
    bytes: &[u8],
    load: elf64_program_header,
    return_address: u64,
    expected: &[u8],
    role: &str,
) -> Result<(), String> {
    let start = return_address
        .checked_sub(expected.len() as u64)
        .ok_or_else(|| format!("supervised renderer {role} gate address underflowed"))?;
    if elf_virtual_bytes(bytes, load, start, expected.len())? != expected {
        return Err(format!(
            "supervised renderer {role} gate does not use the exact vector, mailbox, and flags"
        ));
    }
    Ok(())
}

fn inspect_supervised_renderer_sections(
    parsed: &parsed_elf64,
    load: elf64_program_header,
) -> Result<u64, String> {
    const section_write: u64 = 1;
    const section_alloc: u64 = 2;
    const section_execute: u64 = 4;
    const section_tls: u64 = 0x400;
    const section_rela: u32 = 4;
    const section_dynamic: u32 = 6;
    const section_nobits: u32 = 8;
    const section_rel: u32 = 9;
    const section_dynsym: u32 = 11;

    let mut capsule_section_address = None;
    for section in &parsed.section_headers {
        if matches!(
            section.kind,
            section_rela | section_dynamic | section_rel | section_dynsym
        ) || section.flags & section_tls != 0
        {
            return Err(format!(
                "supervised renderer contains forbidden ELF section `{}`",
                section.name
            ));
        }
        if section.flags & section_alloc == 0 || section.size == 0 {
            continue;
        }
        if section.flags & section_write != 0 || section.kind == section_nobits {
            return Err(format!(
                "supervised renderer contains writable or unbacked allocated section `{}`",
                section.name
            ));
        }
        let section_end = section
            .address
            .checked_add(section.size)
            .ok_or_else(|| "supervised renderer section extent overflowed".to_string())?;
        let load_end = load
            .virtual_address
            .checked_add(load.file_size)
            .ok_or_else(|| "supervised renderer load extent overflowed".to_string())?;
        let expected_offset = load
            .offset
            .checked_add(
                section
                    .address
                    .checked_sub(load.virtual_address)
                    .ok_or_else(|| "allocated section precedes renderer load".to_string())?,
            )
            .ok_or_else(|| "supervised renderer section offset overflowed".to_string())?;
        if section.address < load.virtual_address
            || section_end > load_end
            || section.offset != expected_offset
        {
            return Err(format!(
                "allocated section `{}` is not backed exactly by the sole renderer load",
                section.name
            ));
        }
        if section.name == ".renderer_capsule" {
            if capsule_section_address.is_some()
                || section.kind != 1
                || section.flags & section_alloc == 0
                || section.flags & (section_write | section_execute | section_tls) != 0
                || section.size != supervised_renderer_capsule_size as u64
            {
                return Err(
                    "supervised renderer capsule section violates its exact contract".into(),
                );
            }
            capsule_section_address = Some(section.address);
        }
    }
    capsule_section_address
        .ok_or_else(|| "supervised renderer must have exactly one allocated capsule section".into())
}

fn verify_supervised_renderer_disassembly(disassembly: &str) -> Result<(), String> {
    let lowered = disassembly.to_ascii_lowercase();
    for register in [
        "%xmm", "%ymm", "%zmm", "%mm", "%st", "%bnd", "%tmm", "%k", "%cr", "%dr",
    ] {
        if lowered.contains(register) {
            return Err("supervised renderer contains forbidden register use".into());
        }
    }

    let forbidden_exact = [
        "cli", "sti", "hlt", "syscall", "sysenter", "sysexit", "sysret", "lgdt", "lidt", "lldt",
        "ltr", "invlpg", "swapgs", "rdmsr", "wrmsr", "ldmxcsr", "stmxcsr", "emms", "fwait", "wait",
        "cpuid", "rdtsc", "rdtscp", "rdpmc", "rdrand", "rdseed", "xgetbv", "xsetbv", "rdpkru",
        "wrpkru", "rdfsbase", "rdgsbase", "wrfsbase", "wrgsbase", "monitor", "mwait", "monitorx",
        "mwaitx", "umwait", "tpause", "wbinvd", "invd", "clts", "in", "ins", "insb", "insw",
        "insl", "out", "outs", "outsb", "outsw", "outsl", "iret", "iretl", "iretq", "vmcall",
        "vmlaunch", "vmresume", "vmxoff", "encls", "enclu", "getsec",
    ];
    let mut scheduler_gates = 0;
    let mut rite_gates = 0;
    for line in lowered.lines() {
        let Some((address, instruction)) = line.split_once(':') else {
            continue;
        };
        let address = address.trim();
        if address.is_empty() || !address.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            continue;
        }
        let mut fields = instruction.split_whitespace();
        let Some(mut mnemonic) = fields.next() else {
            continue;
        };
        while matches!(
            mnemonic,
            "rep" | "repe" | "repz" | "repne" | "repnz" | "lock" | "data16" | "addr32"
        ) {
            let Some(next) = fields.next() else {
                return Err("supervised renderer has an incomplete instruction prefix".into());
            };
            mnemonic = next;
        }
        if forbidden_exact.contains(&mnemonic)
            || mnemonic.starts_with('f')
            || mnemonic.starts_with("xsave")
            || mnemonic.starts_with("xrstor")
            || mnemonic.starts_with("fxsave")
            || mnemonic.starts_with("fxrstor")
            || mnemonic.starts_with("vzero")
            || mnemonic.starts_with("tile")
            || matches!(mnemonic, "ldtilecfg" | "sttilecfg" | "(bad)")
        {
            return Err(format!(
                "supervised renderer contains forbidden instruction `{mnemonic}`"
            ));
        }
        if mnemonic == "int" {
            match fields.next() {
                Some("$0xf2") => scheduler_gates += 1,
                Some("$0xf3") => rite_gates += 1,
                _ => return Err("supervised renderer contains an unexpected software gate".into()),
            }
        } else if matches!(mnemonic, "int3" | "into") {
            return Err("supervised renderer contains an unexpected software interrupt".into());
        }
    }
    if scheduler_gates != 1 || rite_gates != 2 {
        return Err(
            "supervised renderer does not contain the exact three gate instructions".into(),
        );
    }
    Ok(())
}

fn package_supervised_renderer_binary(
    build: &Path,
    inspection: &supervised_renderer_inspection,
) -> Result<(), String> {
    if inspection.linked_symbol_offsets.len() != supervised_renderer_linked_symbols.len() {
        return Err("supervised renderer linked-symbol metadata is incomplete".into());
    }
    let stack_note = build.join("supervised_renderer_gnu_stack.note");
    fs::write(&stack_note, [])
        .map_err(|error| format!("write {}: {error}", stack_note.display()))?;
    let mut arguments = vec![
        OsString::from("-I"),
        OsString::from("binary"),
        OsString::from("-O"),
        OsString::from("elf64-x86-64"),
        OsString::from("-B"),
        OsString::from("i386:x86-64"),
        OsString::from("--rename-section"),
        OsString::from(format!(
            ".data={supervised_renderer_payload_section},alloc,load,readonly,data,contents"
        )),
        OsString::from("--add-section"),
        OsString::from(format!(".note.GNU-stack={}", stack_note.display())),
        OsString::from("--set-section-flags"),
        OsString::from(".note.GNU-stack=readonly"),
        OsString::from("--redefine-sym"),
        OsString::from(format!(
            "_binary_supervised_renderer_bin_start={supervised_renderer_payload_start}"
        )),
        OsString::from("--redefine-sym"),
        OsString::from(format!(
            "_binary_supervised_renderer_bin_end={supervised_renderer_payload_end}"
        )),
        OsString::from("--redefine-sym"),
        OsString::from(format!(
            "_binary_supervised_renderer_bin_size={supervised_renderer_payload_size}"
        )),
    ];
    for (symbol, offset) in supervised_renderer_linked_symbols
        .iter()
        .zip(&inspection.linked_symbol_offsets)
    {
        if *offset >= inspection.file_size {
            return Err(format!(
                "supervised renderer wrapper symbol `{}` is outside its payload",
                symbol.wrapper_name
            ));
        }
        arguments.push(OsString::from("--add-symbol"));
        arguments.push(OsString::from(format!(
            "{}={}:0x{offset:x},global",
            symbol.wrapper_name, supervised_renderer_payload_section
        )));
    }
    arguments.push(OsString::from(supervised_renderer_binary_name));
    arguments.push(OsString::from(supervised_renderer_object_name));
    let result = run_checked(build, "objcopy", arguments);
    let cleanup = remove_optional_file(&stack_note);
    result.and(cleanup)
}

fn inspect_supervised_renderer_wrapper(
    object: &Path,
    binary: &[u8],
    inspection: &supervised_renderer_inspection,
) -> Result<(), String> {
    let bytes = fs::read(object).map_err(|error| format!("read {}: {error}", object.display()))?;
    reject_supervised_renderer_protected_sources(&bytes, "wrapper object")?;
    let parsed = parse_elf64(&bytes)?;
    if parsed.kind != 1
        || parsed.machine != 62
        || parsed.entry != 0
        || !parsed.program_headers.is_empty()
    {
        return Err(
            "supervised renderer wrapper is not a bounded x86-64 relocatable object".into(),
        );
    }
    if parsed
        .section_headers
        .iter()
        .any(|section| matches!(section.kind, 4 | 9))
    {
        return Err("supervised renderer wrapper contains relocation records".into());
    }
    let stack_notes = parsed
        .section_headers
        .iter()
        .filter(|section| section.name == ".note.GNU-stack")
        .collect::<Vec<_>>();
    if stack_notes.len() != 1
        || stack_notes[0].kind != 1
        || stack_notes[0].flags != 0
        || stack_notes[0].size != 0
    {
        return Err(
            "supervised renderer wrapper lacks its exact non-executable-stack declaration".into(),
        );
    }
    let mut allocated = parsed
        .section_headers
        .iter()
        .filter(|section| section.flags & 2 != 0 && section.size != 0);
    let payload = allocated.next().ok_or_else(|| {
        "supervised renderer wrapper has no allocated payload section".to_string()
    })?;
    if allocated.next().is_some()
        || payload.name != supervised_renderer_payload_section
        || payload.kind != 1
        || payload.flags != 2
        || payload.address != 0
        || payload.size != binary.len() as u64
    {
        return Err(
            "supervised renderer wrapper section violates the read-only payload contract".into(),
        );
    }
    let offset = usize::try_from(payload.offset)
        .map_err(|_| "supervised renderer wrapper offset does not fit usize".to_string())?;
    let end = offset
        .checked_add(binary.len())
        .ok_or_else(|| "supervised renderer wrapper extent overflowed".to_string())?;
    if bytes.get(offset..end) != Some(binary) {
        return Err("supervised renderer wrapper changed the opaque payload bytes".into());
    }

    let root = repository_root()?;
    let undefined = run_output_checked(
        &root,
        "nm",
        [
            OsString::from("--undefined-only"),
            OsString::from("--format=posix"),
            object.to_path_buf().into_os_string(),
        ],
    )?;
    if !undefined.iter().all(u8::is_ascii_whitespace) {
        return Err("supervised renderer wrapper has undefined symbols".into());
    }
    let output = run_output_checked(
        &root,
        "nm",
        [
            OsString::from("--defined-only"),
            OsString::from("--format=posix"),
            object.to_path_buf().into_os_string(),
        ],
    )?;
    let listing = String::from_utf8(output)
        .map_err(|_| "supervised renderer wrapper symbols are not valid UTF-8".to_string())?;
    let names = listing
        .lines()
        .filter_map(|line| line.split_whitespace().next())
        .collect::<Vec<_>>();
    if names.len() != 3 + supervised_renderer_linked_symbols.len()
        || ![
            supervised_renderer_payload_start,
            supervised_renderer_payload_end,
            supervised_renderer_payload_size,
        ]
        .iter()
        .all(|required| names.contains(required))
        || symbol_u64_from_listing(&listing, supervised_renderer_payload_start)? != 0
        || symbol_u64_from_listing(&listing, supervised_renderer_payload_end)?
            != binary.len() as u64
        || symbol_u64_from_listing(&listing, supervised_renderer_payload_size)?
            != binary.len() as u64
        || symbol_kind_from_listing(&listing, supervised_renderer_payload_start)? != 'R'
        || symbol_kind_from_listing(&listing, supervised_renderer_payload_end)? != 'R'
        || symbol_kind_from_listing(&listing, supervised_renderer_payload_size)? != 'A'
    {
        return Err(
            "supervised renderer wrapper symbols violate the stable boundary contract".into(),
        );
    }
    if inspection.linked_symbol_offsets.len() != supervised_renderer_linked_symbols.len() {
        return Err("supervised renderer wrapper inspection metadata is incomplete".into());
    }
    for (symbol, expected_offset) in supervised_renderer_linked_symbols
        .iter()
        .zip(&inspection.linked_symbol_offsets)
    {
        if !names.contains(&symbol.wrapper_name)
            || symbol_u64_from_listing(&listing, symbol.wrapper_name)? != *expected_offset
            || symbol_kind_from_listing(&listing, symbol.wrapper_name)? != 'R'
        {
            return Err(format!(
                "supervised renderer wrapper symbol `{}` moved",
                symbol.wrapper_name
            ));
        }
    }
    Ok(())
}

fn inspect_linked_supervised_renderer_payload(
    root: &Path,
    kernel_elf: &Path,
    artifact: &supervised_renderer_artifact,
) -> Result<(), String> {
    let payload = fs::read(&artifact.binary)
        .map_err(|error| format!("read {}: {error}", artifact.binary.display()))?;
    if payload.len() as u64 != artifact.file_size
        || artifact.linked_symbol_offsets.len() != supervised_renderer_linked_symbols.len()
    {
        return Err("linked supervised-renderer metadata is incomplete".into());
    }
    let kernel_bytes =
        fs::read(kernel_elf).map_err(|error| format!("read {}: {error}", kernel_elf.display()))?;
    let parsed = parse_elf64(&kernel_bytes)?;
    let symbol_output = run_output_checked(
        root,
        "nm",
        [
            OsString::from("--defined-only"),
            OsString::from("--format=posix"),
            kernel_elf.to_path_buf().into_os_string(),
        ],
    )?;
    let listing = String::from_utf8(symbol_output)
        .map_err(|_| "linked-kernel symbol table is not valid UTF-8".to_string())?;
    let payload_start = symbol_u64_from_listing(&listing, supervised_renderer_payload_start)?;
    let payload_end = symbol_u64_from_listing(&listing, supervised_renderer_payload_end)?;
    let payload_size = symbol_u64_from_listing(&listing, supervised_renderer_payload_size)?;
    if payload_end.checked_sub(payload_start) != Some(payload.len() as u64)
        || payload_size != payload.len() as u64
        || symbol_kind_from_listing(&listing, supervised_renderer_payload_start)? != 'R'
        || symbol_kind_from_listing(&listing, supervised_renderer_payload_end)? != 'R'
        || symbol_kind_from_listing(&listing, supervised_renderer_payload_size)? != 'A'
    {
        return Err("linked kernel changed the supervised-renderer payload boundary".into());
    }
    let covering_sections = parsed
        .section_headers
        .iter()
        .filter(|section| {
            section.flags & 2 != 0
                && payload_start >= section.address
                && payload_end <= section.address.checked_add(section.size).unwrap_or(0)
        })
        .collect::<Vec<_>>();
    if covering_sections.len() != 1
        || covering_sections[0].kind != 1
        || covering_sections[0].flags & 1 != 0
    {
        return Err("linked kernel placed the supervised renderer outside read-only data".into());
    }
    let covering_loads = parsed
        .program_headers
        .iter()
        .copied()
        .filter(|load| {
            load.kind == 1
                && payload_start >= load.virtual_address
                && payload_end
                    <= load
                        .virtual_address
                        .checked_add(load.file_size)
                        .unwrap_or(0)
        })
        .collect::<Vec<_>>();
    if covering_loads.len() != 1
        || elf_virtual_bytes(
            &kernel_bytes,
            covering_loads[0],
            payload_start,
            payload.len(),
        )? != payload
        || count_subslice(&kernel_bytes, &payload) != 1
    {
        return Err("linked kernel changed or duplicated the supervised-renderer payload".into());
    }
    for (symbol, offset) in supervised_renderer_linked_symbols
        .iter()
        .zip(&artifact.linked_symbol_offsets)
    {
        let expected = payload_start
            .checked_add(*offset)
            .ok_or_else(|| "linked supervised-renderer symbol overflowed".to_string())?;
        if symbol_u64_from_listing(&listing, symbol.wrapper_name)? != expected
            || symbol_kind_from_listing(&listing, symbol.wrapper_name)? != 'R'
        {
            return Err(format!(
                "linked kernel changed supervised-renderer symbol `{}`",
                symbol.wrapper_name
            ));
        }
    }
    Ok(())
}

fn reject_supervised_renderer_protected_sources(
    bytes: &[u8],
    artifact: &str,
) -> Result<(), String> {
    for source in [
        renderer_protected_source(7, 19),
        renderer_protected_source(11, 23),
    ] {
        if bytes.windows(source.len()).any(|window| window == source) {
            return Err(format!(
                "protected scene source entered the supervised renderer {artifact}"
            ));
        }
    }
    Ok(())
}

fn renderer_protected_source(multiplier: u8, offset: u8) -> Vec<u8> {
    (0_u32..37)
        .map(|index| b'a' + ((index * u32::from(multiplier) + u32::from(offset)) % 26) as u8)
        .collect()
}

fn count_subslice(bytes: &[u8], needle: &[u8]) -> usize {
    if needle.is_empty() {
        return 0;
    }
    bytes
        .windows(needle.len())
        .filter(|window| *window == needle)
        .count()
}

fn round_up_to_page(value: u64) -> Result<u64, String> {
    value
        .checked_add(circle_page_size - 1)
        .map(|value| value & !(circle_page_size - 1))
        .ok_or_else(|| "page rounding overflowed".to_string())
}

fn elf_virtual_bytes(
    bytes: &[u8],
    load: elf64_program_header,
    address: u64,
    length: usize,
) -> Result<&[u8], String> {
    let relative = address
        .checked_sub(load.virtual_address)
        .ok_or_else(|| "ELF virtual range precedes its load segment".to_string())?;
    let length =
        u64::try_from(length).map_err(|_| "ELF virtual length is too large".to_string())?;
    if relative
        .checked_add(length)
        .is_none_or(|end| end > load.file_size)
    {
        return Err("ELF virtual range exceeds its file-backed load segment".into());
    }
    let start = load
        .offset
        .checked_add(relative)
        .and_then(|offset| usize::try_from(offset).ok())
        .ok_or_else(|| "ELF virtual file offset overflowed".to_string())?;
    let end = start
        .checked_add(length as usize)
        .ok_or_else(|| "ELF virtual byte range overflowed".to_string())?;
    bytes
        .get(start..end)
        .ok_or_else(|| "ELF virtual byte range is truncated".to_string())
}

fn parse_elf64(bytes: &[u8]) -> Result<parsed_elf64, String> {
    if bytes.len() < 64
        || bytes.get(0..4) != Some(b"\x7fELF")
        || bytes[4] != 2
        || bytes[5] != 1
        || bytes[6] != 1
    {
        return Err("artifact is not a little-endian ELF64 image".into());
    }
    let kind = elf_u16(bytes, 16)?;
    let machine = elf_u16(bytes, 18)?;
    if elf_u32(bytes, 20)? != 1 || elf_u32(bytes, 48)? != 0 || elf_u16(bytes, 52)? != 64 {
        return Err("ELF64 header has unsupported version, flags, or size".into());
    }
    let entry = elf_u64(bytes, 24)?;
    let program_offset = elf_u64(bytes, 32)?;
    let section_offset = elf_u64(bytes, 40)?;
    let program_entry_size = elf_u16(bytes, 54)?;
    let program_count = elf_u16(bytes, 56)? as usize;
    let section_entry_size = elf_u16(bytes, 58)?;
    let section_count = elf_u16(bytes, 60)? as usize;
    let string_section_index = elf_u16(bytes, 62)? as usize;
    if (program_count != 0 && program_entry_size != 56)
        || (program_count == 0 && program_entry_size != 0 && program_entry_size != 56)
        || section_entry_size != 64
        || section_count == 0
        || string_section_index == 0
        || string_section_index >= section_count
    {
        return Err("ELF64 uses unsupported header table dimensions".into());
    }

    let mut program_headers = Vec::with_capacity(program_count);
    for index in 0..program_count {
        let offset = table_entry_offset(program_offset, index, 56, bytes.len())?;
        let header = elf64_program_header {
            kind: elf_u32(bytes, offset)?,
            flags: elf_u32(bytes, offset + 4)?,
            offset: elf_u64(bytes, offset + 8)?,
            virtual_address: elf_u64(bytes, offset + 16)?,
            physical_address: elf_u64(bytes, offset + 24)?,
            file_size: elf_u64(bytes, offset + 32)?,
            memory_size: elf_u64(bytes, offset + 40)?,
            alignment: elf_u64(bytes, offset + 48)?,
        };
        let file_end = header
            .offset
            .checked_add(header.file_size)
            .ok_or_else(|| "ELF64 program extent overflowed".to_string())?;
        if file_end > bytes.len() as u64 || header.file_size > header.memory_size {
            return Err("ELF64 program header has an invalid extent".into());
        }
        program_headers.push(header);
    }

    let mut section_headers = Vec::with_capacity(section_count);
    let mut name_offsets = Vec::with_capacity(section_count);
    for index in 0..section_count {
        let offset = table_entry_offset(section_offset, index, 64, bytes.len())?;
        name_offsets.push(elf_u32(bytes, offset)? as usize);
        let header = elf64_section_header {
            name: String::new(),
            kind: elf_u32(bytes, offset + 4)?,
            flags: elf_u64(bytes, offset + 8)?,
            address: elf_u64(bytes, offset + 16)?,
            offset: elf_u64(bytes, offset + 24)?,
            size: elf_u64(bytes, offset + 32)?,
        };
        if header.kind != 8 {
            let file_end = header
                .offset
                .checked_add(header.size)
                .ok_or_else(|| "ELF64 section extent overflowed".to_string())?;
            if file_end > bytes.len() as u64 {
                return Err("ELF64 section header has a truncated extent".into());
            }
        }
        section_headers.push(header);
    }
    let strings = &section_headers[string_section_index];
    if strings.kind != 3 {
        return Err("ELF64 section-name table is not a string table".into());
    }
    let strings_start = usize::try_from(strings.offset)
        .map_err(|_| "ELF64 string-table offset is too large".to_string())?;
    let strings_end = strings_start
        .checked_add(strings.size as usize)
        .ok_or_else(|| "ELF64 string-table extent overflowed".to_string())?;
    let strings = bytes
        .get(strings_start..strings_end)
        .ok_or_else(|| "ELF64 string table is truncated".to_string())?;
    for (header, name_offset) in section_headers.iter_mut().zip(name_offsets) {
        header.name = elf_string(strings, name_offset)?;
    }

    Ok(parsed_elf64 {
        kind,
        machine,
        entry,
        program_headers,
        section_headers,
    })
}

fn table_entry_offset(
    table_offset: u64,
    index: usize,
    entry_size: usize,
    file_size: usize,
) -> Result<usize, String> {
    let offset = table_offset
        .checked_add(
            u64::try_from(index)
                .ok()
                .and_then(|index| index.checked_mul(entry_size as u64))
                .ok_or_else(|| "ELF64 table offset overflowed".to_string())?,
        )
        .and_then(|offset| usize::try_from(offset).ok())
        .ok_or_else(|| "ELF64 table offset does not fit usize".to_string())?;
    if offset
        .checked_add(entry_size)
        .is_none_or(|end| end > file_size)
    {
        return Err("ELF64 header table is truncated".into());
    }
    Ok(offset)
}

fn elf_string(strings: &[u8], offset: usize) -> Result<String, String> {
    let suffix = strings
        .get(offset..)
        .ok_or_else(|| "ELF64 section name offset is outside its string table".to_string())?;
    let end = suffix
        .iter()
        .position(|byte| *byte == 0)
        .ok_or_else(|| "ELF64 section name is not terminated".to_string())?;
    std::str::from_utf8(&suffix[..end])
        .map(str::to_string)
        .map_err(|_| "ELF64 section name is not valid UTF-8".to_string())
}

fn elf_u16(bytes: &[u8], offset: usize) -> Result<u16, String> {
    let value = bytes
        .get(offset..offset.saturating_add(2))
        .ok_or_else(|| "ELF64 u16 field is truncated".to_string())?;
    Ok(u16::from_le_bytes([value[0], value[1]]))
}

fn elf_u32(bytes: &[u8], offset: usize) -> Result<u32, String> {
    let value = bytes
        .get(offset..offset.saturating_add(4))
        .ok_or_else(|| "ELF64 u32 field is truncated".to_string())?;
    Ok(u32::from_le_bytes([value[0], value[1], value[2], value[3]]))
}

fn elf_u64(bytes: &[u8], offset: usize) -> Result<u64, String> {
    let value = bytes
        .get(offset..offset.saturating_add(8))
        .ok_or_else(|| "ELF64 u64 field is truncated".to_string())?;
    Ok(u64::from_le_bytes([
        value[0], value[1], value[2], value[3], value[4], value[5], value[6], value[7],
    ]))
}

fn link_stage_2(root: &Path, build: &Path) -> Result<PathBuf, String> {
    let threshold_root = root.join("boot/threshold/x86_64");
    let stage_2_object = build.join("threshold_stage_2.o");
    run_checked(
        root,
        "as",
        [
            OsString::from("--64"),
            OsString::from("-g"),
            OsString::from("-o"),
            stage_2_object.clone().into_os_string(),
            threshold_root.join("stage_2.S").into_os_string(),
        ],
    )?;

    let stage_2_elf = build.join("threshold_stage_2.elf");
    run_checked(
        root,
        "ld",
        [
            OsString::from("--build-id=none"),
            OsString::from("--fatal-warnings"),
            OsString::from("--orphan-handling=error"),
            OsString::from("-z"),
            OsString::from("max-page-size=0x1000"),
            OsString::from("-T"),
            threshold_root.join("stage_2.ld").into_os_string(),
            OsString::from("-o"),
            stage_2_elf.clone().into_os_string(),
            stage_2_object.into_os_string(),
        ],
    )?;
    Ok(stage_2_elf)
}

fn build_stage_1(root: &Path, build: &Path) -> Result<PathBuf, String> {
    let threshold_root = root.join("boot/threshold/x86_64");
    let stage_1_object = build.join("threshold_stage_1.o");
    run_checked(
        root,
        "as",
        [
            OsString::from("--32"),
            OsString::from("-g"),
            OsString::from("-o"),
            stage_1_object.clone().into_os_string(),
            threshold_root.join("stage_1.S").into_os_string(),
        ],
    )?;

    let stage_1_elf = build.join("threshold_stage_1.elf");
    run_checked(
        root,
        "ld",
        [
            OsString::from("-m"),
            OsString::from("elf_i386"),
            OsString::from("--build-id=none"),
            OsString::from("--fatal-warnings"),
            OsString::from("--orphan-handling=error"),
            OsString::from("-T"),
            threshold_root.join("stage_1.ld").into_os_string(),
            OsString::from("-o"),
            stage_1_elf.clone().into_os_string(),
            stage_1_object.into_os_string(),
        ],
    )?;

    let stage_1 = build.join("threshold_stage_1.bin");
    binary_from_elf(root, &stage_1_elf, &stage_1)
}

fn binary_from_elf(root: &Path, elf: &Path, binary: &Path) -> Result<PathBuf, String> {
    run_checked(
        root,
        "objcopy",
        [
            OsString::from("-O"),
            OsString::from("binary"),
            elf.to_path_buf().into_os_string(),
            binary.to_path_buf().into_os_string(),
        ],
    )?;
    Ok(binary.to_path_buf())
}

fn inspect_extent(path: &Path, maximum_size: u32, label: &str) -> Result<image_extent, String> {
    let bytes = fs::read(path).map_err(|error| format!("read {}: {error}", path.display()))?;
    let file_size =
        u32::try_from(bytes.len()).map_err(|_| format!("{label} is larger than 4 GiB"))?;
    if file_size == 0 || file_size > maximum_size {
        return Err(format!(
            "{label} is {file_size} bytes; the supported range is 1..={maximum_size}"
        ));
    }
    let sector_count = file_size.div_ceil(sector_size as u32);
    Ok(image_extent {
        file_size,
        sector_count,
        hash: fnv_1a(&bytes),
    })
}

fn symbol_u32(root: &Path, elf: &Path, name: &str) -> Result<u32, String> {
    let address = symbol_u64(root, elf, name)?;
    u32::try_from(address).map_err(|_| format!("symbol {name} lies above the 32-bit bootstrap map"))
}

fn symbol_u64(root: &Path, elf: &Path, name: &str) -> Result<u64, String> {
    let output = run_output_checked(
        root,
        "nm",
        [
            OsString::from("--defined-only"),
            OsString::from("--format=posix"),
            elf.to_path_buf().into_os_string(),
        ],
    )?;
    let listing = String::from_utf8(output)
        .map_err(|error| format!("nm output for {} is not UTF-8: {error}", elf.display()))?;
    symbol_u64_from_listing(&listing, name).map_err(|error| format!("{error} in {}", elf.display()))
}

fn symbol_u64_from_listing(listing: &str, name: &str) -> Result<u64, String> {
    for line in listing.lines() {
        let mut fields = line.split_whitespace();
        if fields.next() == Some(name) {
            let _kind = fields.next();
            let value = fields
                .next()
                .ok_or_else(|| format!("nm omitted the value for symbol {name}"))?;
            let address = u64::from_str_radix(value, 16)
                .map_err(|error| format!("parse address of symbol {name}: {error}"))?;
            return Ok(address);
        }
    }
    Err(format!("symbol {name} is missing"))
}

fn symbol_kind_from_listing(listing: &str, name: &str) -> Result<char, String> {
    for line in listing.lines() {
        let mut fields = line.split_whitespace();
        if fields.next() == Some(name) {
            let kind = fields
                .next()
                .ok_or_else(|| format!("nm omitted the kind for symbol {name}"))?;
            let mut characters = kind.chars();
            let value = characters
                .next()
                .ok_or_else(|| format!("nm emitted an empty kind for symbol {name}"))?;
            if characters.next().is_some() {
                return Err(format!("nm emitted a noncanonical kind for symbol {name}"));
            }
            return Ok(value);
        }
    }
    Err(format!("symbol {name} is missing"))
}

fn resolve_circle_fault_addresses(
    root: &Path,
    elf: &Path,
) -> Result<circle_fault_addresses, String> {
    let template_start = symbol_u64(root, elf, "x86_64_circle_generation_1_start")?;
    let denied_read = symbol_u64(root, elf, "x86_64_circle_generation_1_denied_read")?;
    let canary = symbol_u64(root, elf, "circle_kernel_canary")?;
    circle_fault_addresses_from_symbols(template_start, denied_read, canary)
}

fn circle_fault_addresses_from_symbols(
    template_start: u64,
    denied_read: u64,
    canary: u64,
) -> Result<circle_fault_addresses, String> {
    let denied_offset = denied_read
        .checked_sub(template_start)
        .filter(|offset| *offset < circle_page_size)
        .ok_or_else(|| {
            "circle denied-read symbol is outside its one-page payload template".to_string()
        })?;
    let generation_1_instruction_pointer = circle_user_code_address
        .checked_add(denied_offset)
        .ok_or_else(|| "circle denied-read runtime address overflowed".to_string())?;
    let kernel_start = u64::from(kernel_load_address);
    let kernel_file_limit = kernel_start + u64::from(kernel_staging_capacity);
    if !(kernel_start..kernel_file_limit).contains(&canary) {
        return Err(format!(
            "circle canary symbol {canary:#x} is outside the low kernel file extent"
        ));
    }
    Ok(circle_fault_addresses {
        generation_1_instruction_pointer,
        generation_1_fault_address: canary,
    })
}

fn resolve_scheduler_fault_address(
    root: &Path,
    elf: &Path,
) -> Result<scheduler_fault_address, String> {
    let template_start = symbol_u64(root, elf, "x86_64_scheduler_faulting_start")?;
    let fault_instruction = symbol_u64(root, elf, "x86_64_scheduler_fault_instruction")?;
    scheduler_fault_address_from_symbols(template_start, fault_instruction)
}

fn scheduler_fault_address_from_symbols(
    template_start: u64,
    fault_instruction: u64,
) -> Result<scheduler_fault_address, String> {
    let offset = fault_instruction
        .checked_sub(template_start)
        .filter(|offset| *offset < circle_page_size)
        .ok_or_else(|| {
            "scheduler fault instruction is outside its one-page payload template".to_string()
        })?;
    let instruction_pointer = scheduler_user_code_address
        .checked_add(offset)
        .ok_or_else(|| "scheduler fault instruction runtime address overflowed".to_string())?;
    Ok(scheduler_fault_address {
        instruction_pointer,
    })
}

fn resolve_supervision_fault_address(
    root: &Path,
    elf: &Path,
) -> Result<supervision_fault_address, String> {
    let template_start = symbol_u64(root, elf, "x86_64_supervision_vessel_generation_1_start")?;
    let fault_instruction = symbol_u64(
        root,
        elf,
        "x86_64_supervision_vessel_generation_1_fault_instruction",
    )?;
    supervision_fault_address_from_symbols(template_start, fault_instruction)
}

fn supervision_fault_address_from_symbols(
    template_start: u64,
    fault_instruction: u64,
) -> Result<supervision_fault_address, String> {
    let offset = fault_instruction
        .checked_sub(template_start)
        .filter(|offset| *offset < circle_page_size)
        .ok_or_else(|| {
            "supervision fault instruction is outside its one-page payload template".to_string()
        })?;
    let instruction_pointer = supervision_user_code_address
        .checked_add(offset)
        .ok_or_else(|| "supervision fault instruction runtime address overflowed".to_string())?;
    Ok(supervision_fault_address {
        instruction_pointer,
    })
}

fn resolve_shell_vessel_fault_address(
    root: &Path,
    elf: &Path,
) -> Result<shell_vessel_fault_address, String> {
    let template_start = symbol_u64(root, elf, "x86_64_shell_vessel_generation_1_start")?;
    let fault_instruction = symbol_u64(
        root,
        elf,
        "x86_64_shell_vessel_generation_1_fault_instruction",
    )?;
    shell_vessel_fault_address_from_symbols(template_start, fault_instruction)
}

fn shell_vessel_fault_address_from_symbols(
    template_start: u64,
    fault_instruction: u64,
) -> Result<shell_vessel_fault_address, String> {
    let offset = fault_instruction
        .checked_sub(template_start)
        .filter(|offset| *offset < circle_page_size)
        .ok_or_else(|| {
            "shell-vessel fault instruction is outside its one-page payload template".to_string()
        })?;
    let instruction_pointer = shell_vessel_user_code_address
        .checked_add(offset)
        .ok_or_else(|| "shell-vessel fault instruction runtime address overflowed".to_string())?;
    Ok(shell_vessel_fault_address {
        instruction_pointer,
    })
}

fn inspect_normal_feature_exclusion(
    root: &Path,
    elf: &Path,
    binary: &Path,
    snapshot: &[u8],
    probe_out_directories: &[PathBuf],
    semantic_presentation_body: &[u8; surface_snapshot_v1_byte_count],
    semantic_presentation_probes: &[Vec<u8>],
) -> Result<(), String> {
    let symbol_output = run_output_checked(
        root,
        "nm",
        [
            OsString::from("--defined-only"),
            OsString::from("--format=posix"),
            elf.to_path_buf().into_os_string(),
        ],
    )?;
    let symbol_listing = String::from_utf8(symbol_output)
        .map_err(|error| format!("nm output for {} is not UTF-8: {error}", elf.display()))?;
    desktop_dev::verify_symbols_absent(&symbol_listing)?;
    verify_circle_symbols_absent(&symbol_listing)?;
    verify_control_symbols_absent(&symbol_listing)?;
    verify_rite_symbols_absent(&symbol_listing)?;
    verify_scheduler_symbols_absent(&symbol_listing)?;
    verify_supervision_symbols_absent(&symbol_listing)?;
    verify_shell_vessel_symbols_absent(&symbol_listing)?;
    verify_input_wait_symbols_absent(&symbol_listing)?;
    verify_memory_symbols_absent(&symbol_listing)?;
    verify_interrupt_symbols_absent(&symbol_listing)?;
    verify_display_vessel_symbols_absent(&symbol_listing)?;
    verify_supervised_renderer_symbols_absent(&symbol_listing)?;
    verify_semantic_presentation_symbols_absent(&symbol_listing)?;
    verify_semantic_surface_symbols_absent(&symbol_listing)?;

    let bytes = fs::read(binary).map_err(|error| format!("read {}: {error}", binary.display()))?;
    desktop_dev::verify_bytes_absent(&bytes)?;
    workspace_dev::verify_absent(&symbol_listing, &bytes)?;
    owned_workspace::verify_absent(&symbol_listing, &bytes)?;
    owned_workspace_cpu::verify_absent(&symbol_listing, &bytes)?;
    dispatcher_smoke::verify_absent(&symbol_listing, &bytes)?;
    owned_dispatcher_smoke::verify_absent(&symbol_listing, &bytes)?;
    owned_multi_region_smoke::verify_absent(&symbol_listing, &bytes)?;
    owned_source_smoke::verify_absent(&symbol_listing, &bytes)?;
    owned_tls_smoke::verify_absent(&symbol_listing, &bytes)?;
    application_profile_smoke::verify_absent(&symbol_listing, &bytes)?;
    mixed_workspace::verify_absent(&symbol_listing, &bytes)?;
    native_editor_workspace::verify_absent(&symbol_listing, &bytes)?;
    ny_object_artifact::verify_absent(&symbol_listing, &bytes)?;
    native_application_artifact::verify_absent(&symbol_listing, &bytes)?;
    native_runtime_artifact::verify_absent(&symbol_listing, &bytes)?;
    native_manifest_artifact::verify_absent(&symbol_listing, &bytes)?;
    native_manifest_smoke::verify_absent(&symbol_listing, &bytes)?;
    native_program_artifact::verify_absent(&symbol_listing, &bytes)?;
    native_program_smoke::verify_absent(&symbol_listing, &bytes)?;
    native_durable_artifact::verify_absent(&symbol_listing, &bytes)?;
    native_durable_failure_artifact::verify_absent(&symbol_listing, &bytes)?;
    native_document_artifact::verify_absent(&symbol_listing, &bytes)?;
    elf_document_artifact::verify_absent(&symbol_listing, &bytes)?;
    native_relaunch_artifact::verify_absent(&symbol_listing, &bytes)?;
    elf_relaunch_artifact::verify_absent(&symbol_listing, &bytes)?;
    elf_application_artifact::verify_absent(&symbol_listing, &bytes)?;
    linux_application_artifact::verify_absent(&symbol_listing, &bytes)?;
    veil_application_artifact::verify_absent(&symbol_listing, &bytes)?;
    linux_tls_artifact::verify_absent(&symbol_listing, &bytes)?;
    linux_loader_entry_artifact::verify_absent(&symbol_listing, &bytes)?;
    verify_circle_bytes_absent(&bytes)?;
    verify_control_bytes_absent(&bytes)?;
    verify_rite_bytes_absent(&bytes)?;
    verify_scheduler_bytes_absent(&bytes)?;
    verify_supervision_bytes_absent(&bytes)?;
    verify_shell_vessel_bytes_absent(&bytes)?;
    verify_input_wait_bytes_absent(&bytes)?;
    verify_memory_bytes_absent(&bytes)?;
    verify_interrupt_bytes_absent(&bytes)?;
    verify_display_vessel_bytes_absent(&bytes)?;
    verify_supervised_renderer_bytes_absent(&bytes)?;
    verify_semantic_presentation_bytes_absent(&bytes, semantic_presentation_body)?;
    reject_semantic_presentation_probes(
        &bytes,
        "normal kernel image",
        semantic_presentation_probes,
    )?;
    verify_normal_semantic_surface_bytes_absent(&bytes, snapshot, probe_out_directories)?;
    Ok(())
}

fn verify_normal_semantic_surface_bytes_absent(
    bytes: &[u8],
    snapshot: &[u8],
    probe_out_directories: &[PathBuf],
) -> Result<(), String> {
    verify_semantic_surface_bytes_absent(bytes)?;
    reject_semantic_surface_private_sources(bytes, "normal kernel image", probe_out_directories)?;
    if count_subslice(bytes, snapshot) != 0 {
        return Err("normal kernel unexpectedly contains the semantic-surface snapshot".into());
    }
    Ok(())
}

fn inspect_display_vessel_feature_image() -> Result<(), String> {
    let root = repository_root()?;
    let elf = root.join("build/kernel.elf");
    let binary = root.join("build/kernel.bin");
    let symbol_output = run_output_checked(
        &root,
        "nm",
        [
            OsString::from("--defined-only"),
            OsString::from("--format=posix"),
            elf.clone().into_os_string(),
        ],
    )?;
    let symbol_listing = String::from_utf8(symbol_output)
        .map_err(|error| format!("nm output for {} is not UTF-8: {error}", elf.display()))?;
    verify_display_vessel_symbols_present(&symbol_listing)?;
    let bytes = fs::read(&binary).map_err(|error| format!("read {}: {error}", binary.display()))?;
    verify_display_vessel_bytes_present(&bytes)
}

fn inspect_supervised_renderer_feature_image(
    root: &Path,
    elf: &Path,
    binary: &Path,
) -> Result<(), String> {
    let symbol_output = run_output_checked(
        root,
        "nm",
        [
            OsString::from("--defined-only"),
            OsString::from("--format=posix"),
            elf.to_path_buf().into_os_string(),
        ],
    )?;
    let symbol_listing = String::from_utf8(symbol_output)
        .map_err(|error| format!("nm output for {} is not UTF-8: {error}", elf.display()))?;
    verify_supervised_renderer_symbols_present(&symbol_listing)?;
    let bytes = fs::read(binary).map_err(|error| format!("read {}: {error}", binary.display()))?;
    verify_supervised_renderer_bytes_present(&bytes)?;
    reject_supervised_renderer_protected_sources(&bytes, "feature kernel image")
}

fn inspect_semantic_presentation_feature_image(
    root: &Path,
    elf: &Path,
    binary: &Path,
) -> Result<(), String> {
    let listing = String::from_utf8(run_output_checked(
        root,
        "nm",
        [
            OsString::from("--defined-only"),
            OsString::from("--format=posix"),
            elf.to_path_buf().into_os_string(),
        ],
    )?)
    .map_err(|_| "semantic-presentation kernel symbols are not UTF-8".to_string())?;
    verify_semantic_presentation_symbols_present(&listing)?;
    verify_semantic_presentation_precedence_symbols(&listing)?;
    let bytes = fs::read(binary).map_err(|error| format!("read {}: {error}", binary.display()))?;
    verify_semantic_presentation_bytes_present(&bytes)?;
    verify_semantic_presentation_precedence_bytes(&bytes)
}

fn verify_semantic_presentation_symbols_present(listing: &str) -> Result<(), String> {
    for required in semantic_presentation_required_symbols {
        if !listing
            .lines()
            .any(|line| line.split_whitespace().next() == Some(*required))
        {
            return Err(format!(
                "semantic-presentation kernel does not define required symbol `{required}`"
            ));
        }
    }
    if !listing.lines().any(|line| {
        line.split_whitespace()
            .next()
            .is_some_and(|symbol| symbol.contains("semantic_surface_presentation_test"))
    }) {
        return Err("semantic-presentation kernel has no feature runtime symbol".into());
    }
    Ok(())
}

fn verify_semantic_presentation_bytes_present(bytes: &[u8]) -> Result<(), String> {
    for required in semantic_presentation_required_binary_strings {
        if !bytes
            .windows(required.len())
            .any(|window| window == *required)
        {
            return Err(format!(
                "semantic-presentation kernel does not contain required event `{}`",
                display_probe(required)
            ));
        }
    }
    Ok(())
}

fn verify_semantic_presentation_precedence_symbols(listing: &str) -> Result<(), String> {
    for old in semantic_surface_preceded_fixture_symbols
        .iter()
        .copied()
        .chain(std::iter::once("run_semantic_surface_transport_test"))
    {
        if listing.lines().any(|line| {
            line.split_whitespace()
                .next()
                .is_some_and(|symbol| symbol.contains(old))
        }) {
            return Err(format!(
                "semantic-presentation precedence failed: linked kernel retains old fixture symbol `{old}`"
            ));
        }
    }
    Ok(())
}

fn verify_semantic_presentation_precedence_bytes(bytes: &[u8]) -> Result<(), String> {
    for marker in semantic_surface_preceded_fixture_event_markers
        .iter()
        .copied()
        .chain(std::iter::once(
            b"kernel.semantic_surface_transport.passed".as_slice(),
        ))
    {
        if bytes.windows(marker.len()).any(|window| window == marker) {
            return Err(format!(
                "semantic-presentation precedence failed: linked kernel retains old fixture event `{}`",
                display_probe(marker)
            ));
        }
    }
    Ok(())
}

fn inspect_semantic_surface_feature_image(
    root: &Path,
    elf: &Path,
    binary: &Path,
) -> Result<(), String> {
    let listing = String::from_utf8(run_output_checked(
        root,
        "nm",
        [
            OsString::from("--defined-only"),
            OsString::from("--format=posix"),
            elf.to_path_buf().into_os_string(),
        ],
    )?)
    .map_err(|_| "semantic-surface kernel symbols are not UTF-8".to_string())?;
    for name in semantic_surface_source_package
        .wrapper_symbols
        .iter()
        .chain(semantic_surface_consumer_package.wrapper_symbols)
    {
        if !listing
            .lines()
            .any(|line| line.split_whitespace().next() == Some(*name))
        {
            return Err(format!("semantic-surface kernel lacks `{name}`"));
        }
    }
    if !listing
        .lines()
        .any(|line| line.contains("semantic_surface_transport"))
    {
        return Err("semantic-surface kernel has no feature runtime symbols".into());
    }
    verify_semantic_surface_precedence_symbols(&listing)?;
    let bytes = fs::read(binary).map_err(|error| format!("read {}: {error}", binary.display()))?;
    verify_semantic_surface_precedence_bytes(&bytes)?;
    reject_semantic_surface_private_sources(
        &bytes,
        "feature kernel image",
        &semantic_surface_known_probe_directories()?,
    )
}

fn verify_semantic_surface_precedence_symbols(listing: &str) -> Result<(), String> {
    for old in semantic_surface_preceded_fixture_symbols {
        if listing.lines().any(|line| {
            line.split_whitespace()
                .next()
                .is_some_and(|symbol| symbol.contains(old))
        }) {
            return Err(format!(
                "semantic-surface precedence failed: linked kernel retains old fixture symbol `{old}`"
            ));
        }
    }
    Ok(())
}

fn verify_semantic_surface_precedence_bytes(bytes: &[u8]) -> Result<(), String> {
    for marker in semantic_surface_preceded_fixture_event_markers {
        if bytes.windows(marker.len()).any(|window| window == *marker) {
            return Err(format!(
                "semantic-surface precedence failed: linked kernel retains old fixture event `{}`",
                display_probe(marker)
            ));
        }
    }
    Ok(())
}

fn verify_supervised_renderer_symbols_present(listing: &str) -> Result<(), String> {
    for required in supervised_renderer_kernel_required_symbols {
        let present = listing.lines().any(|line| {
            line.split_whitespace()
                .next()
                .is_some_and(|symbol| symbol == *required)
        });
        if !present {
            return Err(format!(
                "supervised-renderer kernel does not define required symbol `{required}`"
            ));
        }
    }
    if !listing.lines().any(|line| {
        line.split_whitespace()
            .next()
            .is_some_and(|symbol| symbol.contains("supervised_renderer_test"))
    }) {
        return Err("supervised-renderer kernel has no feature runtime symbol".into());
    }
    Ok(())
}

fn verify_supervised_renderer_bytes_present(bytes: &[u8]) -> Result<(), String> {
    for required in supervised_renderer_forbidden_binary_strings {
        if !bytes
            .windows(required.len())
            .any(|window| window == *required)
        {
            return Err(format!(
                "supervised-renderer kernel does not contain required payload `{}`",
                display_probe(required)
            ));
        }
    }
    Ok(())
}

fn verify_display_vessel_symbols_present(listing: &str) -> Result<(), String> {
    for required in display_vessel_required_symbols {
        let present = listing.lines().any(|line| {
            line.split_whitespace()
                .next()
                .is_some_and(|symbol| symbol == *required)
        });
        if !present {
            return Err(format!(
                "display-vessel test kernel does not define required symbol `{required}`"
            ));
        }
    }
    Ok(())
}

fn verify_display_vessel_bytes_present(bytes: &[u8]) -> Result<(), String> {
    for required in display_vessel_forbidden_binary_strings {
        if !bytes
            .windows(required.len())
            .any(|window| window == *required)
        {
            return Err(format!(
                "display-vessel test kernel does not contain required payload `{}`",
                display_probe(required)
            ));
        }
    }
    Ok(())
}

fn verify_circle_symbols_absent(listing: &str) -> Result<(), String> {
    for line in listing.lines() {
        let Some(symbol) = line.split_whitespace().next() else {
            continue;
        };
        if symbol.contains("circle_test") || circle_forbidden_symbols.contains(&symbol) {
            return Err(format!(
                "normal kernel unexpectedly defines circle-test symbol `{symbol}`"
            ));
        }
    }
    Ok(())
}

fn verify_circle_bytes_absent(bytes: &[u8]) -> Result<(), String> {
    for forbidden in circle_forbidden_binary_strings {
        if bytes
            .windows(forbidden.len())
            .any(|window| window == *forbidden)
        {
            return Err(format!(
                "normal kernel unexpectedly contains circle-test payload `{}`",
                display_probe(forbidden)
            ));
        }
    }
    Ok(())
}

fn verify_control_symbols_absent(listing: &str) -> Result<(), String> {
    for line in listing.lines() {
        let Some(symbol) = line.split_whitespace().next() else {
            continue;
        };
        if symbol.contains("control_test") || control_forbidden_symbols.contains(&symbol) {
            return Err(format!(
                "normal kernel unexpectedly defines control-test symbol `{symbol}`"
            ));
        }
    }
    Ok(())
}

fn verify_control_bytes_absent(bytes: &[u8]) -> Result<(), String> {
    for forbidden in control_forbidden_binary_strings {
        if bytes
            .windows(forbidden.len())
            .any(|window| window == *forbidden)
        {
            return Err(format!(
                "normal kernel unexpectedly contains control-test payload `{}`",
                display_probe(forbidden)
            ));
        }
    }
    Ok(())
}

fn verify_rite_symbols_absent(listing: &str) -> Result<(), String> {
    for line in listing.lines() {
        let Some(symbol) = line.split_whitespace().next() else {
            continue;
        };
        if symbol.contains("rite_test")
            || rite_forbidden_symbols
                .iter()
                .any(|probe| symbol.contains(probe))
        {
            return Err(format!(
                "normal kernel unexpectedly defines rite-test symbol `{symbol}`"
            ));
        }
    }
    Ok(())
}

fn verify_rite_bytes_absent(bytes: &[u8]) -> Result<(), String> {
    for forbidden in rite_forbidden_binary_strings {
        if bytes
            .windows(forbidden.len())
            .any(|window| window == *forbidden)
        {
            return Err(format!(
                "normal kernel unexpectedly contains rite-test payload `{}`",
                display_probe(forbidden)
            ));
        }
    }
    Ok(())
}

fn verify_scheduler_symbols_absent(listing: &str) -> Result<(), String> {
    for line in listing.lines() {
        let Some(symbol) = line.split_whitespace().next() else {
            continue;
        };
        if symbol.contains("scheduler_test")
            || scheduler_forbidden_symbols
                .iter()
                .any(|probe| symbol.contains(probe))
        {
            return Err(format!(
                "normal kernel unexpectedly defines scheduler-test symbol `{symbol}`"
            ));
        }
    }
    Ok(())
}

fn verify_scheduler_bytes_absent(bytes: &[u8]) -> Result<(), String> {
    for forbidden in scheduler_forbidden_binary_strings {
        if bytes
            .windows(forbidden.len())
            .any(|window| window == *forbidden)
        {
            return Err(format!(
                "normal kernel unexpectedly contains scheduler-test payload `{}`",
                display_probe(forbidden)
            ));
        }
    }
    Ok(())
}

fn verify_supervision_symbols_absent(listing: &str) -> Result<(), String> {
    for line in listing.lines() {
        let Some(symbol) = line.split_whitespace().next() else {
            continue;
        };
        if symbol.contains("supervision_test")
            || supervision_forbidden_symbols
                .iter()
                .any(|probe| symbol.contains(probe))
        {
            return Err(format!(
                "normal kernel unexpectedly defines supervision-test symbol `{symbol}`"
            ));
        }
    }
    Ok(())
}

fn verify_supervision_bytes_absent(bytes: &[u8]) -> Result<(), String> {
    for forbidden in supervision_forbidden_binary_strings {
        if bytes
            .windows(forbidden.len())
            .any(|window| window == *forbidden)
        {
            return Err(format!(
                "normal kernel unexpectedly contains supervision-test payload `{}`",
                display_probe(forbidden)
            ));
        }
    }
    Ok(())
}

fn verify_shell_vessel_symbols_absent(listing: &str) -> Result<(), String> {
    for line in listing.lines() {
        let Some(symbol) = line.split_whitespace().next() else {
            continue;
        };
        if symbol.contains("shell_vessel_test")
            || shell_vessel_forbidden_symbols
                .iter()
                .any(|probe| symbol.contains(probe))
        {
            return Err(format!(
                "normal kernel unexpectedly defines shell-vessel-test symbol `{symbol}`"
            ));
        }
    }
    Ok(())
}

fn verify_shell_vessel_bytes_absent(bytes: &[u8]) -> Result<(), String> {
    for forbidden in shell_vessel_forbidden_binary_strings {
        if bytes
            .windows(forbidden.len())
            .any(|window| window == *forbidden)
        {
            return Err(format!(
                "normal kernel unexpectedly contains shell-vessel-test payload `{}`",
                display_probe(forbidden)
            ));
        }
    }
    Ok(())
}

fn verify_input_wait_symbols_absent(listing: &str) -> Result<(), String> {
    for line in listing.lines() {
        let Some(symbol) = line.split_whitespace().next() else {
            continue;
        };
        if symbol.contains("input_wait_test")
            || input_wait_forbidden_symbols
                .iter()
                .any(|probe| symbol.contains(probe))
        {
            return Err(format!(
                "normal kernel unexpectedly defines input-wait-test symbol `{symbol}`"
            ));
        }
    }
    Ok(())
}

fn verify_input_wait_bytes_absent(bytes: &[u8]) -> Result<(), String> {
    for forbidden in input_wait_forbidden_binary_strings {
        if bytes
            .windows(forbidden.len())
            .any(|window| window == *forbidden)
        {
            return Err(format!(
                "normal kernel unexpectedly contains input-wait-test payload `{}`",
                display_probe(forbidden)
            ));
        }
    }
    Ok(())
}

fn verify_memory_symbols_absent(listing: &str) -> Result<(), String> {
    for line in listing.lines() {
        let Some(symbol) = line.split_whitespace().next() else {
            continue;
        };
        if symbol.contains("memory_test")
            || symbol.contains("user_mapping_test")
            || symbol.starts_with("x86_64_owned_user_")
            || symbol.starts_with("owned_user_")
        {
            return Err(format!(
                "normal kernel unexpectedly defines memory-test symbol `{symbol}`"
            ));
        }
    }
    Ok(())
}

fn verify_memory_bytes_absent(bytes: &[u8]) -> Result<(), String> {
    for forbidden in memory_test_forbidden_binary_strings {
        if bytes
            .windows(forbidden.len())
            .any(|window| window == *forbidden)
        {
            return Err(format!(
                "normal kernel unexpectedly contains memory-test payload `{}`",
                display_probe(forbidden)
            ));
        }
    }
    Ok(())
}

fn verify_interrupt_symbols_absent(listing: &str) -> Result<(), String> {
    for line in listing.lines() {
        let Some(symbol) = line.split_whitespace().next() else {
            continue;
        };
        if symbol.contains("interrupt_test")
            || interrupt_forbidden_symbols
                .iter()
                .any(|probe| symbol.contains(probe))
        {
            return Err(format!(
                "normal kernel unexpectedly defines interrupt-test symbol `{symbol}`"
            ));
        }
    }
    Ok(())
}

fn verify_interrupt_bytes_absent(bytes: &[u8]) -> Result<(), String> {
    for forbidden in interrupt_forbidden_binary_strings {
        if bytes
            .windows(forbidden.len())
            .any(|window| window == *forbidden)
        {
            return Err(format!(
                "normal kernel unexpectedly contains interrupt-test payload `{}`",
                display_probe(forbidden)
            ));
        }
    }
    Ok(())
}

fn verify_display_vessel_symbols_absent(listing: &str) -> Result<(), String> {
    for line in listing.lines() {
        let Some(symbol) = line.split_whitespace().next() else {
            continue;
        };
        if symbol.contains("display_vessel_test")
            || display_vessel_forbidden_symbols
                .iter()
                .any(|probe| symbol.contains(probe))
        {
            return Err(format!(
                "normal kernel unexpectedly defines display-vessel-test symbol `{symbol}`"
            ));
        }
    }
    Ok(())
}

fn verify_display_vessel_bytes_absent(bytes: &[u8]) -> Result<(), String> {
    for forbidden in display_vessel_forbidden_binary_strings {
        if bytes
            .windows(forbidden.len())
            .any(|window| window == *forbidden)
        {
            return Err(format!(
                "normal kernel unexpectedly contains display-vessel-test payload `{}`",
                display_probe(forbidden)
            ));
        }
    }
    Ok(())
}

fn verify_supervised_renderer_symbols_absent(listing: &str) -> Result<(), String> {
    for line in listing.lines() {
        let Some(symbol) = line.split_whitespace().next() else {
            continue;
        };
        if symbol.contains("supervised_renderer") {
            return Err(format!(
                "normal kernel unexpectedly defines supervised-renderer symbol `{symbol}`"
            ));
        }
    }
    Ok(())
}

fn verify_supervised_renderer_bytes_absent(bytes: &[u8]) -> Result<(), String> {
    for forbidden in supervised_renderer_forbidden_binary_strings
        .iter()
        .chain(supervised_renderer_private_source_sentinels)
    {
        if bytes
            .windows(forbidden.len())
            .any(|window| window == *forbidden)
        {
            return Err(format!(
                "normal kernel unexpectedly contains supervised-renderer payload `{}`",
                display_probe(forbidden)
            ));
        }
    }
    Ok(())
}

fn verify_semantic_presentation_symbols_absent(listing: &str) -> Result<(), String> {
    for line in listing.lines() {
        let Some(symbol) = line.split_whitespace().next() else {
            continue;
        };
        if symbol.contains("semantic_surface_presentation") {
            return Err(format!(
                "normal kernel unexpectedly defines semantic-presentation symbol `{symbol}`"
            ));
        }
    }
    Ok(())
}

fn verify_semantic_presentation_bytes_absent(
    bytes: &[u8],
    exact_body: &[u8; surface_snapshot_v1_byte_count],
) -> Result<(), String> {
    for forbidden in semantic_presentation_forbidden_binary_strings {
        if bytes
            .windows(forbidden.len())
            .any(|window| window == *forbidden)
        {
            return Err(format!(
                "normal kernel unexpectedly contains semantic-presentation payload `{}`",
                display_probe(forbidden)
            ));
        }
    }
    if count_subslice(bytes, exact_body) != 0 {
        return Err(
            "normal kernel unexpectedly contains the semantic-presentation ordinal-61 body".into(),
        );
    }
    Ok(())
}

fn verify_semantic_surface_symbols_absent(listing: &str) -> Result<(), String> {
    for line in listing.lines() {
        if line
            .split_whitespace()
            .next()
            .is_some_and(|symbol| symbol.contains("semantic_surface"))
        {
            return Err(format!(
                "normal kernel unexpectedly defines semantic-surface symbol `{}`",
                line
            ));
        }
    }
    Ok(())
}

fn verify_semantic_surface_bytes_absent(bytes: &[u8]) -> Result<(), String> {
    for forbidden in semantic_surface_forbidden_binary_strings {
        if bytes
            .windows(forbidden.len())
            .any(|window| window == *forbidden)
        {
            return Err(format!(
                "normal kernel unexpectedly contains semantic-surface bytes `{}`",
                display_probe(forbidden)
            ));
        }
    }
    Ok(())
}

fn display_probe(bytes: &[u8]) -> String {
    match std::str::from_utf8(bytes) {
        Ok(text) if text.chars().all(|character| !character.is_control()) => text.into(),
        _ => bytes
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<Vec<_>>()
            .join(""),
    }
}

fn assemble_raw_disk(
    stage_1: &Path,
    manifest: &Path,
    stage_2: &Path,
    kernel: &Path,
    image_path: &Path,
) -> Result<(), String> {
    let boot_sector =
        fs::read(stage_1).map_err(|error| format!("read {}: {error}", stage_1.display()))?;
    if boot_sector.len() != sector_size as usize {
        return Err(format!(
            "{} is {} bytes; expected one 512-byte sector",
            stage_1.display(),
            boot_sector.len()
        ));
    }
    if boot_sector[510..] != [0x55, 0xaa] {
        return Err("Threshold stage 1 is missing the BIOS boot signature".into());
    }

    let manifest_bytes =
        fs::read(manifest).map_err(|error| format!("read {}: {error}", manifest.display()))?;
    if manifest_bytes.len() != manifest_sector_size {
        return Err("Threshold manifest must occupy exactly one sector".into());
    }
    let stage_2_bytes =
        fs::read(stage_2).map_err(|error| format!("read {}: {error}", stage_2.display()))?;
    let kernel_bytes =
        fs::read(kernel).map_err(|error| format!("read {}: {error}", kernel.display()))?;
    let stage_2_sectors = (stage_2_bytes.len() as u64).div_ceil(sector_size);
    let kernel_lba = stage_2_lba + stage_2_sectors;
    let kernel_end = kernel_lba
        .checked_mul(sector_size)
        .and_then(|offset| offset.checked_add(kernel_bytes.len() as u64))
        .ok_or_else(|| "Threshold disk layout overflowed".to_string())?;
    if kernel_end > disk_size {
        return Err(format!(
            "Threshold image content ends at byte {kernel_end}; disk size is {disk_size}"
        ));
    }

    let mut image = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(image_path)
        .map_err(|error| format!("create {}: {error}", image_path.display()))?;
    image
        .set_len(disk_size)
        .and_then(|()| image.seek(SeekFrom::Start(0)).map(|_| ()))
        .and_then(|()| image.write_all(&boot_sector))
        .and_then(|()| {
            image
                .seek(SeekFrom::Start(manifest_lba * sector_size))
                .map(|_| ())
        })
        .and_then(|()| image.write_all(&manifest_bytes))
        .and_then(|()| {
            image
                .seek(SeekFrom::Start(stage_2_lba * sector_size))
                .map(|_| ())
        })
        .and_then(|()| image.write_all(&stage_2_bytes))
        .and_then(|()| {
            image
                .seek(SeekFrom::Start(kernel_lba * sector_size))
                .map(|_| ())
        })
        .and_then(|()| image.write_all(&kernel_bytes))
        .and_then(|()| image.sync_all())
        .map_err(|error| format!("write {}: {error}", image_path.display()))?;
    Ok(())
}

fn run_qemu(image: &Path, smoke_test: bool) -> Result<(), String> {
    let mut command = qemu_command(image)?;

    if smoke_test {
        command
            .arg("-display")
            .arg("none")
            .arg("-monitor")
            .arg("none")
            .arg("-device")
            .arg("isa-debug-exit,iobase=0xf4,iosize=0x04");
        run_qemu_smoke(command, smoke_timeout, &[ready_event])
    } else {
        command
            .arg("-no-shutdown")
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());
        let status = command
            .status()
            .map_err(|error| format!("launch QEMU: {error}"))?;
        if status.success() {
            Ok(())
        } else {
            Err(format!("QEMU exited with {status}"))
        }
    }
}

fn run_qemu_fault_smoke(image: &Path) -> Result<(), String> {
    let mut command = qemu_command(image)?;
    command
        .arg("-display")
        .arg("none")
        .arg("-monitor")
        .arg("none")
        .arg("-device")
        .arg("isa-debug-exit,iobase=0xf4,iosize=0x04");
    run_qemu_smoke(
        command,
        smoke_timeout,
        &[ready_event, fault_path_ready_event, fault_test_passed_event],
    )
}

fn run_qemu_circle_smoke(
    image: &Path,
    fault_addresses: circle_fault_addresses,
) -> Result<(), String> {
    let mut command = qemu_command(image)?;
    command
        .arg("-display")
        .arg("none")
        .arg("-monitor")
        .arg("none")
        .arg("-device")
        .arg("isa-debug-exit,iobase=0xf4,iosize=0x04");
    run_qemu_with_transcript(command, smoke_timeout, move |status, transcript| {
        verify_circle_smoke_output(status, transcript, fault_addresses)
    })
}

fn run_qemu_control_smoke(image: &Path) -> Result<(), String> {
    let mut command = qemu_command(image)?;
    command
        .arg("-display")
        .arg("none")
        .arg("-monitor")
        .arg("none")
        .arg("-device")
        .arg("isa-debug-exit,iobase=0xf4,iosize=0x04");
    run_qemu_with_transcript(command, smoke_timeout, verify_control_smoke_output)
}

fn run_qemu_semantic_surface_transport_smoke(image: &Path) -> Result<(), String> {
    let mut command = qemu_command(image)?;
    command
        .arg("-no-reboot")
        .arg("-display")
        .arg("none")
        .arg("-device")
        .arg("isa-debug-exit,iobase=0xf4,iosize=0x04");
    run_qemu_with_transcript(command, smoke_timeout, |status, transcript| {
        if status.code() != Some(qemu_success_status) {
            return Err(format!(
                "QEMU did not report semantic-surface transport success (process status: {:?})",
                status.code()
            ));
        }
        verify_semantic_surface_transport_transcript(transcript)
    })
}

fn verify_semantic_surface_transport_transcript(transcript: &str) -> Result<(), String> {
    verify_semantic_surface_transport_transcript_with_probe_directories(
        transcript,
        &semantic_surface_known_probe_directories()?,
    )
}

fn verify_semantic_surface_transport_transcript_with_probe_directories(
    transcript: &str,
    probe_directories: &[PathBuf],
) -> Result<(), String> {
    reject_semantic_surface_private_sources(
        transcript.as_bytes(),
        "transcript",
        probe_directories,
    )?;
    let mut records = Vec::new();
    for (index, line) in transcript.lines().enumerate() {
        if line.is_empty() || !line.starts_with('{') {
            return Err(format!(
                "semantic-surface transcript contains unstructured serial content on line {}",
                index + 1
            ));
        }
        let record = json_parser::new(line).parse_record().map_err(|error| {
            format!(
                "semantic-surface transcript has malformed record on line {}: {error}",
                index + 1
            )
        })?;
        let event = required_string(&record, "semantic-surface record", "event")?;
        let level = required_string(&record, event, "level")?;
        if record_u64(&record, "schema") != Some(1)
            || matches!(level, "error" | "fatal")
            || event.ends_with(".failed")
        {
            return Err(format!(
                "semantic-surface transcript contains forbidden event `{event}`"
            ));
        }
        if event.starts_with(semantic_surface_event_prefix) {
            for key in record.keys() {
                if [
                    "address",
                    "attempt",
                    "body",
                    "buffer",
                    "content",
                    "endpoint",
                    "generation",
                    "label",
                    "mailbox",
                    "payload",
                    "pointer",
                    "private",
                    "revision",
                    "scene",
                    "snapshot",
                    "source",
                    "surface",
                    "task",
                    "trace",
                ]
                .iter()
                .any(|forbidden| key.contains(forbidden))
                    && key != "source_image_pages"
                    && key != "source_retired_before_delivery"
                    && key != "content_redacted"
                    && key != "mailbox_pages"
                    && key != "private_storage_cleared"
                    && key != "queued_body_retained"
                    && key != "resources_restored"
                    && key != "body_bytes"
                    && key != "body_copies"
                {
                    return Err(format!(
                        "semantic-surface event `{event}` exposes forbidden field `{key}`"
                    ));
                }
            }
        }
        records.push(record);
    }
    let events = records
        .iter()
        .map(|record| record_string(record, "event").unwrap_or("<missing>"))
        .collect::<Vec<_>>();
    if events != semantic_surface_expected_events {
        return Err(format!(
            "semantic-surface lifecycle is missing, duplicated, unknown, or out of order: {events:?}"
        ));
    }
    for (record, event) in records.iter().zip(semantic_surface_expected_events) {
        require_string(record, event, "event", event)?;
        require_string(record, event, "level", "info")?;
    }
    validate_control_boot_records(&records[..7])?;
    validate_scheduler_interrupt_ready(&records[7])?;
    let transport = &records[8..];
    require_semantic_surface_fields(
        &transport[0],
        "kernel.semantic_surface_transport.ready",
        &[
            "schema",
            "event",
            "level",
            "arch",
            "source_image_pages",
            "consumer_image_pages",
            "mailbox_pages",
            "stack_pages",
            "grants_split",
            "content_redacted",
        ],
    )?;
    require_string(
        &transport[0],
        "kernel.semantic_surface_transport.ready",
        "arch",
        "x86_64",
    )?;
    for field in ["source_image_pages", "consumer_image_pages"] {
        let pages = required_u64(
            &transport[0],
            "kernel.semantic_surface_transport.ready",
            field,
        )?;
        if !(1..=semantic_surface_page_limit).contains(&pages) {
            return Err(format!("semantic-surface {field} is outside 64-page cap"));
        }
    }
    require_number(
        &transport[0],
        "kernel.semantic_surface_transport.ready",
        "mailbox_pages",
        1,
    )?;
    require_number(
        &transport[0],
        "kernel.semantic_surface_transport.ready",
        "stack_pages",
        4,
    )?;
    require_bool(
        &transport[0],
        "kernel.semantic_surface_transport.ready",
        "grants_split",
        true,
    )?;
    require_bool(
        &transport[0],
        "kernel.semantic_surface_transport.ready",
        "content_redacted",
        true,
    )?;
    require_semantic_surface_fields(
        &transport[1],
        "kernel.semantic_surface_transport.source.released",
        &[
            "schema",
            "event",
            "level",
            "queued_body_retained",
            "source_retired_before_delivery",
            "content_redacted",
        ],
    )?;
    for field in [
        "queued_body_retained",
        "source_retired_before_delivery",
        "content_redacted",
    ] {
        require_bool(
            &transport[1],
            "kernel.semantic_surface_transport.source.released",
            field,
            true,
        )?;
    }
    require_semantic_surface_fields(
        &transport[2],
        "kernel.semantic_surface_transport.consumer.released",
        &[
            "schema",
            "event",
            "level",
            "delivery_consumed_once",
            "content_redacted",
        ],
    )?;
    require_bool(
        &transport[2],
        "kernel.semantic_surface_transport.consumer.released",
        "delivery_consumed_once",
        true,
    )?;
    require_bool(
        &transport[2],
        "kernel.semantic_surface_transport.consumer.released",
        "content_redacted",
        true,
    )?;
    require_semantic_surface_fields(
        &transport[3],
        "kernel.semantic_surface_transport.released",
        &[
            "schema",
            "event",
            "level",
            "free_frames",
            "resources_restored",
            "private_storage_cleared",
            "content_redacted",
        ],
    )?;
    if required_u64(
        &transport[3],
        "kernel.semantic_surface_transport.released",
        "free_frames",
    )? == 0
    {
        return Err("semantic-surface proof reported zero free frames".into());
    }
    for field in [
        "resources_restored",
        "private_storage_cleared",
        "content_redacted",
    ] {
        require_bool(
            &transport[3],
            "kernel.semantic_surface_transport.released",
            field,
            true,
        )?;
    }
    let passed = &transport[4];
    require_semantic_surface_fields(
        passed,
        "kernel.semantic_surface_transport.passed",
        &[
            "schema",
            "event",
            "level",
            "lifecycle_complete",
            "rite_calls",
            "body_copies",
            "body_bytes",
            "submissions",
            "deliveries",
            "backpressure",
            "denials",
            "malformed",
            "exits",
            "source_retired_before_delivery",
            "header_published_last",
            "stale_reply_cleared",
            "kernel_continuations",
            "source_image_pages",
            "consumer_image_pages",
            "free_frames",
            "content_redacted",
        ],
    )?;
    for field in ["source_image_pages", "consumer_image_pages"] {
        if required_u64(passed, "kernel.semantic_surface_transport.passed", field)?
            != required_u64(
                &transport[0],
                "kernel.semantic_surface_transport.ready",
                field,
            )?
        {
            return Err(format!(
                "semantic-surface passed `{field}` does not match ready evidence"
            ));
        }
    }
    for (field, expected) in [
        ("rite_calls", 11),
        ("body_copies", 9),
        ("body_bytes", 2176),
        ("submissions", 1),
        ("deliveries", 1),
        ("backpressure", 1),
        ("denials", 2),
        ("malformed", 2),
        ("exits", 2),
        ("kernel_continuations", 2),
    ] {
        require_number(
            passed,
            "kernel.semantic_surface_transport.passed",
            field,
            expected,
        )?;
    }
    for field in [
        "lifecycle_complete",
        "source_retired_before_delivery",
        "header_published_last",
        "stale_reply_cleared",
        "content_redacted",
    ] {
        require_bool(
            passed,
            "kernel.semantic_surface_transport.passed",
            field,
            true,
        )?;
    }
    if required_u64(
        passed,
        "kernel.semantic_surface_transport.passed",
        "free_frames",
    )? != required_u64(
        &transport[3],
        "kernel.semantic_surface_transport.released",
        "free_frames",
    )? {
        return Err("semantic-surface final free-frame accounting changed after release".into());
    }
    Ok(())
}

fn require_semantic_surface_fields(
    record: &structured_record,
    event: &str,
    expected: &[&str],
) -> Result<(), String> {
    let actual = record.keys().map(String::as_str).collect::<Vec<_>>();
    let mut expected = expected.to_vec();
    expected.sort_unstable();
    if actual != expected {
        return Err(format!(
            "semantic-surface event `{event}` fields are not exact: {actual:?}"
        ));
    }
    Ok(())
}

fn run_qemu_rite_smoke(image: &Path) -> Result<(), String> {
    let mut command = qemu_command(image)?;
    command
        .arg("-display")
        .arg("none")
        .arg("-monitor")
        .arg("none")
        .arg("-device")
        .arg("isa-debug-exit,iobase=0xf4,iosize=0x04");
    run_qemu_with_transcript(command, smoke_timeout, verify_rite_smoke_output)
}

fn run_qemu_scheduler_smoke(
    image: &Path,
    fault_address: scheduler_fault_address,
) -> Result<(), String> {
    let mut command = qemu_command(image)?;
    command
        .arg("-display")
        .arg("none")
        .arg("-monitor")
        .arg("none")
        .arg("-device")
        .arg("isa-debug-exit,iobase=0xf4,iosize=0x04");
    run_qemu_with_transcript(command, smoke_timeout, move |status, transcript| {
        verify_scheduler_smoke_output(status, transcript, fault_address)
    })
}

fn run_qemu_supervision_smoke(
    image: &Path,
    fault_address: supervision_fault_address,
) -> Result<(), String> {
    let mut command = qemu_command(image)?;
    command
        .arg("-display")
        .arg("none")
        .arg("-monitor")
        .arg("none")
        .arg("-device")
        .arg("isa-debug-exit,iobase=0xf4,iosize=0x04");
    run_qemu_with_transcript(command, smoke_timeout, move |status, transcript| {
        verify_supervision_smoke_output(status, transcript, fault_address)
    })
}

fn run_qemu_shell_vessel_smoke(
    image: &Path,
    fault_address: shell_vessel_fault_address,
) -> Result<(), String> {
    let mut command = qemu_command(image)?;
    command
        .arg("-display")
        .arg("none")
        .arg("-monitor")
        .arg("none")
        .arg("-device")
        .arg("isa-debug-exit,iobase=0xf4,iosize=0x04");
    run_qemu_with_transcript(command, smoke_timeout, move |status, transcript| {
        verify_shell_vessel_smoke_output(status, transcript, fault_address)
    })
}

fn run_qemu_input_wait_smoke(image: &Path) -> Result<(), String> {
    let mut command = qemu_command(image)?;
    command
        .arg("-display")
        .arg("none")
        .arg("-monitor")
        .arg("none")
        .arg("-device")
        .arg("isa-debug-exit,iobase=0xf4,iosize=0x04");
    run_qemu_with_transcript(command, smoke_timeout, verify_input_wait_smoke_output)
}

fn run_qemu_display_vessel_smoke(image: &Path) -> Result<(), String> {
    let root = repository_root()?;
    let serial_log = root.join("build/qemu_display_vessel.log");
    let frame_path = root.join("build/display_vessel.ppm");
    remove_optional_file(&serial_log)?;
    remove_optional_file(&frame_path)?;

    let mut child = Command::new("qemu-system-x86_64")
        .current_dir(&root)
        .arg("-machine")
        .arg("q35")
        .arg("-m")
        .arg("256M")
        .arg("-drive")
        .arg(format!("format=raw,file={}", image.display()))
        .arg("-no-reboot")
        .arg("-no-shutdown")
        .arg("-display")
        .arg("none")
        .arg("-serial")
        .arg(format!("file:{}", serial_log.display()))
        .arg("-monitor")
        .arg("stdio")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("launch QEMU display-vessel smoke test: {error}"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "capture QEMU display-vessel stderr".to_string())?;
    let stderr_reader = thread::spawn(move || {
        let mut bytes = Vec::new();
        let mut bounded = stderr.take((display_vessel_stderr_limit + 1) as u64);
        bounded.read_to_end(&mut bytes).map(|_| bytes)
    });
    let discard_unverified_artifacts = || {
        let _ = fs::remove_file(&serial_log);
        let _ = fs::remove_file(&frame_path);
    };

    let boot_deadline = Instant::now() + smoke_timeout;
    let capture_result =
        capture_display_vessel_frame(&mut child, &serial_log, &frame_path, boot_deadline);
    if capture_result.is_err() {
        let _ = child.kill();
    }
    let exit_deadline = Instant::now() + smoke_timeout;
    let status_result = wait_for_display_vessel_exit(&mut child, exit_deadline);
    if status_result.is_err() {
        let _ = child.kill();
        let _ = child.wait();
    }
    let stderr = match stderr_reader.join() {
        Ok(Ok(bytes)) => bytes,
        Ok(Err(error)) => {
            discard_unverified_artifacts();
            return Err(format!("read QEMU display-vessel stderr: {error}"));
        }
        Err(_) => {
            discard_unverified_artifacts();
            return Err("QEMU display-vessel stderr reader panicked".into());
        }
    };
    let status = match status_result {
        Ok(status) => status,
        Err(error) => {
            discard_unverified_artifacts();
            return Err(error);
        }
    };
    if let Err(error) = capture_result {
        discard_unverified_artifacts();
        return Err(error);
    }
    if stderr.len() > display_vessel_stderr_limit {
        discard_unverified_artifacts();
        return Err("QEMU display-vessel stderr exceeded its bounded diagnostic limit".into());
    }
    if !status.success() {
        discard_unverified_artifacts();
        return Err(format!(
            "QEMU display-vessel capture exited with {status}; bounded stderr was redacted"
        ));
    }
    let transcript =
        match read_bounded_optional_utf8_file(&serial_log, display_vessel_transcript_limit) {
            Ok(transcript) => transcript,
            Err(error) => {
                discard_unverified_artifacts();
                return Err(error);
            }
        };
    if verify_display_vessel_transcript(&transcript).is_err() {
        discard_unverified_artifacts();
        return Err(
            "display-vessel transcript was rejected; unverified content was not echoed and artifacts were removed"
                .into(),
        );
    }
    if verify_display_vessel_frame_file(&frame_path).is_err() {
        discard_unverified_artifacts();
        return Err(
            "display-vessel frame was rejected; unverified pixels were not echoed and artifacts were removed"
                .into(),
        );
    }
    print!("{transcript}");
    println!(
        "display-vessel smoke matched all {} pixels; retained {} and {}",
        display_vessel_width * display_vessel_height,
        frame_path.display(),
        serial_log.display()
    );
    Ok(())
}

fn capture_display_vessel_frame(
    child: &mut Child,
    serial_log: &Path,
    frame_path: &Path,
    deadline: Instant,
) -> Result<(), String> {
    wait_for_display_vessel_serial_event(child, serial_log, display_vessel_passed_event, deadline)?;
    let mut monitor = child
        .stdin
        .take()
        .ok_or_else(|| "capture QEMU display-vessel monitor input".to_string())?;
    writeln!(monitor, "screendump {}", frame_path.display())
        .and_then(|()| monitor.flush())
        .map_err(|error| format!("request QEMU display-vessel screendump: {error}"))?;
    writeln!(monitor, "quit")
        .and_then(|()| monitor.flush())
        .map_err(|error| format!("request QEMU display-vessel quit: {error}"))
}

fn wait_for_display_vessel_serial_event(
    child: &mut Child,
    serial_log: &Path,
    event: &str,
    deadline: Instant,
) -> Result<(), String> {
    loop {
        let transcript =
            read_bounded_optional_utf8_file(serial_log, display_vessel_transcript_limit)?;
        if display_vessel_serial_event_is_complete(&transcript, event) {
            return Ok(());
        }
        if let Some(status) = child
            .try_wait()
            .map_err(|error| format!("poll QEMU display-vessel smoke test: {error}"))?
        {
            return Err(format!(
                "QEMU display-vessel smoke test exited with {status} before `{event}`"
            ));
        }
        if Instant::now() >= deadline {
            return Err(format!(
                "QEMU display-vessel smoke test exceeded its {}-second deadline waiting for `{event}`",
                smoke_timeout.as_secs_f64()
            ));
        }
        thread::sleep(Duration::from_millis(10));
    }
}

fn display_vessel_serial_event_is_complete(transcript: &str, event: &str) -> bool {
    // Serial output arrives a byte at a time. Matching the event name before
    // its record finishes can let monitor quit truncate the final evidence.
    if !transcript.ends_with('\n') {
        return false;
    }
    let Some(line) = transcript
        .lines()
        .rev()
        .find(|line| !line.trim().is_empty())
    else {
        return false;
    };
    let Ok(record) = json_parser::new(line.trim()).parse_record() else {
        return false;
    };
    record_string(&record, "event") == Some(event)
        && verify_display_vessel_transcript(transcript).is_ok()
}

fn wait_for_display_vessel_exit(
    child: &mut Child,
    deadline: Instant,
) -> Result<ExitStatus, String> {
    loop {
        match child
            .try_wait()
            .map_err(|error| format!("wait for QEMU display-vessel smoke test: {error}"))?
        {
            Some(status) => return Ok(status),
            None if Instant::now() < deadline => thread::sleep(Duration::from_millis(10)),
            None => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!(
                    "QEMU display-vessel smoke test exceeded its {}-second exit deadline",
                    smoke_timeout.as_secs_f64()
                ));
            }
        }
    }
}

fn run_qemu_display_demo(image: &Path) -> Result<(), String> {
    println!("display demo will retain the completed public frame until the QEMU window is closed");
    let mut command = qemu_command(image)?;
    command
        .arg("-no-shutdown")
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    let status = command
        .status()
        .map_err(|error| format!("launch QEMU display demo: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("QEMU display demo exited with {status}"))
    }
}

fn run_qemu_supervised_renderer_smoke(
    image: &Path,
    inspected_image_pages: u64,
) -> Result<(), String> {
    let root = repository_root()?;
    let serial_log = root.join("build/qemu_supervised_renderer.log");
    let frame_path = root.join("build/supervised_renderer.ppm");
    let serial_part = root.join("build/qemu_supervised_renderer.log.part");
    let frame_part = root.join("build/supervised_renderer.ppm.part");
    remove_supervised_renderer_capture_artifacts()?;
    create_private_capture_file(&serial_part)?;
    create_private_capture_file(&frame_part)?;

    let mut child = Command::new("qemu-system-x86_64")
        .current_dir(&root)
        .arg("-machine")
        .arg("q35")
        .arg("-m")
        .arg("256M")
        .arg("-drive")
        .arg(format!("format=raw,file={}", image.display()))
        .arg("-no-reboot")
        .arg("-no-shutdown")
        .arg("-display")
        .arg("none")
        .arg("-serial")
        .arg(format!("file:{}", serial_part.display()))
        .arg("-monitor")
        .arg("stdio")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("launch QEMU supervised-renderer smoke test: {error}"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "capture QEMU supervised-renderer stderr".to_string())?;
    let stderr_reader = thread::spawn(move || {
        let mut bytes = Vec::new();
        let mut bounded = stderr.take((supervised_renderer_stderr_limit + 1) as u64);
        bounded.read_to_end(&mut bytes).map(|_| bytes)
    });
    let discard_unverified_artifacts = || {
        let _ = fs::remove_file(&serial_part);
        let _ = fs::remove_file(&frame_part);
        let _ = fs::remove_file(&serial_log);
        let _ = fs::remove_file(&frame_path);
    };

    let deadline = Instant::now() + smoke_timeout;
    let capture_result =
        capture_supervised_renderer_frame(&mut child, &serial_part, &frame_part, deadline);
    if capture_result.is_err() {
        let _ = child.kill();
    }
    let status_result =
        wait_for_supervised_renderer_exit(&mut child, Instant::now() + smoke_timeout);
    if status_result.is_err() {
        let _ = child.kill();
        let _ = child.wait();
    }
    let stderr = match stderr_reader.join() {
        Ok(Ok(bytes)) => bytes,
        Ok(Err(error)) => {
            discard_unverified_artifacts();
            return Err(format!("read QEMU supervised-renderer stderr: {error}"));
        }
        Err(_) => {
            discard_unverified_artifacts();
            return Err("QEMU supervised-renderer stderr reader panicked".into());
        }
    };
    let status = match status_result {
        Ok(status) => status,
        Err(error) => {
            discard_unverified_artifacts();
            return Err(error);
        }
    };
    if let Err(error) = capture_result {
        discard_unverified_artifacts();
        return Err(error);
    }
    if stderr.len() > supervised_renderer_stderr_limit {
        discard_unverified_artifacts();
        return Err("QEMU supervised-renderer stderr exceeded its bounded limit".into());
    }
    if !status.success() {
        discard_unverified_artifacts();
        return Err(format!(
            "QEMU supervised-renderer capture exited with {status}; bounded stderr was redacted"
        ));
    }
    let transcript =
        match read_bounded_optional_utf8_file(&serial_part, supervised_renderer_transcript_limit) {
            Ok(transcript) => transcript,
            Err(error) => {
                discard_unverified_artifacts();
                return Err(error);
            }
        };
    if verify_supervised_renderer_transcript(&transcript, inspected_image_pages).is_err() {
        discard_unverified_artifacts();
        return Err(
            "supervised-renderer transcript was rejected; unverified content was not echoed and artifacts were removed"
                .into(),
        );
    }
    if verify_supervised_renderer_frame_file(&frame_part).is_err() {
        discard_unverified_artifacts();
        return Err(
            "supervised-renderer frame was rejected; unverified pixels were not echoed and artifacts were removed"
                .into(),
        );
    }
    if let Err(error) =
        fs::rename(&serial_part, &serial_log).and_then(|()| fs::rename(&frame_part, &frame_path))
    {
        discard_unverified_artifacts();
        return Err(format!(
            "publish verified supervised-renderer capture artifacts: {error}"
        ));
    }
    print!("{transcript}");
    println!(
        "supervised renderer matched the independent {}x{} semantic oracle; retained {} and {}",
        supervised_renderer_oracle::frame_width,
        supervised_renderer_oracle::frame_height,
        frame_path.display(),
        serial_log.display()
    );
    Ok(())
}

fn run_qemu_semantic_presentation_smoke(
    image: &Path,
    inspected_source_pages: u64,
    inspected_renderer_pages: u64,
    host: semantic_presentation_host_run,
) -> Result<(), String> {
    if !host.transaction_is_pending() {
        return Err("semantic-presentation transaction is not pending before QEMU".into());
    }
    let root = repository_root()?;
    let mut artifacts = semantic_presentation_capture_artifacts::prepare(&root.join("build"))?;
    let child = Command::new("qemu-system-x86_64")
        .current_dir(&root)
        .arg("-machine")
        .arg("q35")
        .arg("-m")
        .arg("256M")
        .arg("-drive")
        .arg(format!("format=raw,file={}", image.display()))
        .arg("-no-reboot")
        .arg("-no-shutdown")
        .arg("-display")
        .arg("none")
        .arg("-serial")
        .arg(format!("file:{}", artifacts.serial_part.display()))
        .arg("-monitor")
        .arg("stdio")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("launch QEMU semantic-presentation smoke test: {error}"))?;
    let mut child = semantic_presentation_child::new(child);
    let stderr = child
        .child_mut()
        .stderr
        .take()
        .ok_or_else(|| "capture QEMU semantic-presentation stderr".to_string())?;
    let stderr_reader = thread::spawn(move || {
        let mut bytes = Vec::new();
        let mut bounded = stderr.take((semantic_presentation_stderr_limit + 1) as u64);
        bounded.read_to_end(&mut bytes).map(|_| bytes)
    });

    let capture_result = capture_semantic_presentation_frame(
        child.child_mut(),
        &artifacts.serial_part,
        &artifacts.frame_part,
        Instant::now() + smoke_timeout,
    );
    if capture_result.is_err() {
        let _ = child.child_mut().kill();
    }
    let status_result =
        wait_for_supervised_renderer_exit(child.child_mut(), Instant::now() + smoke_timeout);
    if status_result.is_err() {
        child.terminate_and_reap();
    }
    let stderr = match stderr_reader.join() {
        Ok(Ok(bytes)) => bytes,
        Ok(Err(error)) => return Err(format!("read QEMU semantic-presentation stderr: {error}")),
        Err(_) => return Err("QEMU semantic-presentation stderr reader panicked".into()),
    };
    let status = status_result?;
    capture_result?;
    if stderr.len() > semantic_presentation_stderr_limit || !status.success() {
        return Err(
            "semantic-presentation QEMU run failed; bounded diagnostics were redacted".into(),
        );
    }
    drop(child);
    let transcript = read_bounded_optional_utf8_file(
        &artifacts.serial_part,
        semantic_presentation_transcript_limit,
    )?;
    let frame = fs::read(&artifacts.frame_part)
        .map_err(|error| format!("read {}: {error}", artifacts.frame_part.display()))?;
    let evidence = match verify_semantic_presentation_evidence(
        &transcript,
        &frame,
        inspected_source_pages,
        inspected_renderer_pages,
        &host,
    ) {
        Ok(evidence) => evidence,
        Err(semantic_presentation_evidence_error::transcript) => {
            return Err(
                "semantic-presentation transcript was rejected; unverified content was not echoed and artifacts were removed"
                    .into(),
            );
        }
        Err(semantic_presentation_evidence_error::frame) => {
            return Err(
                "semantic-presentation frame was rejected; unverified pixels were not echoed and artifacts were removed"
                    .into(),
            );
        }
        Err(semantic_presentation_evidence_error::host_binding) => {
            return Err("semantic-presentation evidence did not preserve its host binding".into());
        }
    };
    complete_semantic_presentation_and_publish(
        host,
        evidence,
        &mut artifacts,
        &transcript,
        &frame,
    )?;
    print!("{transcript}");
    println!(
        "semantic presentation matched the independent {}x{} oracle; retained {} and {}",
        supervised_renderer_oracle::frame_width,
        supervised_renderer_oracle::frame_height,
        artifacts.frame.display(),
        artifacts.serial_log.display()
    );
    Ok(())
}

fn complete_semantic_presentation_and_publish(
    host: semantic_presentation_host_run,
    evidence: verified_semantic_presentation,
    artifacts: &mut semantic_presentation_capture_artifacts,
    transcript: &str,
    frame: &[u8],
) -> Result<(), String> {
    host.complete(evidence)?;
    artifacts.publish_exact(transcript.as_bytes(), frame)
}

fn verify_semantic_presentation_evidence(
    transcript: &str,
    frame: &[u8],
    inspected_source_pages: u64,
    inspected_renderer_pages: u64,
    host: &semantic_presentation_host_run,
) -> Result<verified_semantic_presentation, semantic_presentation_evidence_error> {
    if !host.transaction_is_pending() {
        return Err(semantic_presentation_evidence_error::host_binding);
    }
    verify_semantic_presentation_transcript(
        transcript,
        inspected_source_pages,
        inspected_renderer_pages,
        &host.protected_probes,
    )
    .map_err(|_| semantic_presentation_evidence_error::transcript)?;
    supervised_renderer_oracle::verify_ppm(frame)
        .map_err(|_| semantic_presentation_evidence_error::frame)?;
    if reject_semantic_presentation_probes(frame, "verified P6 capture", &host.protected_probes)
        .is_err()
        || count_subslice(frame, &host.snapshot) != 0
        || !host.transaction_is_pending()
    {
        return Err(semantic_presentation_evidence_error::host_binding);
    }
    Ok(verified_semantic_presentation { _private: () })
}

fn remove_semantic_presentation_capture_artifacts() -> Result<(), String> {
    let root = repository_root()?;
    for path in [
        root.join("build/qemu_semantic_presentation.log"),
        root.join("build/semantic_presentation.ppm"),
        root.join("build/qemu_semantic_presentation.log.part"),
        root.join("build/semantic_presentation.ppm.part"),
    ] {
        remove_optional_file(&path)?;
    }
    Ok(())
}

fn capture_semantic_presentation_frame(
    child: &mut Child,
    serial_log: &Path,
    frame_path: &Path,
    deadline: Instant,
) -> Result<(), String> {
    wait_for_semantic_presentation_event(child, serial_log, deadline)?;
    let mut monitor = child
        .stdin
        .take()
        .ok_or_else(|| "capture QEMU semantic-presentation monitor input".to_string())?;
    writeln!(monitor, "screendump {}", frame_path.display())
        .and_then(|()| monitor.flush())
        .map_err(|error| format!("request QEMU semantic-presentation screendump: {error}"))?;
    writeln!(monitor, "quit")
        .and_then(|()| monitor.flush())
        .map_err(|error| format!("request QEMU semantic-presentation quit: {error}"))
}

fn wait_for_semantic_presentation_event(
    child: &mut Child,
    serial_log: &Path,
    deadline: Instant,
) -> Result<(), String> {
    loop {
        let transcript =
            read_bounded_optional_utf8_file(serial_log, semantic_presentation_transcript_limit)?;
        if semantic_presentation_passed_record_is_complete(&transcript) {
            return Ok(());
        }
        if let Some(_status) = child
            .try_wait()
            .map_err(|error| format!("poll QEMU semantic-presentation smoke test: {error}"))?
        {
            return Err(
                "QEMU semantic-presentation run exited before verified-presentation evidence"
                    .into(),
            );
        }
        if Instant::now() >= deadline {
            return Err(format!(
                "QEMU semantic-presentation run exceeded its {}-second deadline",
                smoke_timeout.as_secs_f64()
            ));
        }
        thread::sleep(Duration::from_millis(10));
    }
}

fn semantic_presentation_passed_record_is_complete(transcript: &str) -> bool {
    // Serial writes can expose the event name before the rest of its record.
    // Do not send QEMU quit until the final canonical CRLF is present. This is
    // only a capture trigger; the complete private transcript and pixel oracle
    // still control publication after the process has exited.
    let Some(body) = transcript.strip_suffix("\r\n") else {
        return false;
    };
    let final_record = body.rsplit("\r\n").next().unwrap_or("");
    let Ok(fields) = supervised_renderer_canonical_fields(final_record) else {
        return false;
    };
    let Some(expected) = semantic_presentation_feature_fields.last() else {
        return false;
    };
    if fields.as_slice() != *expected {
        return false;
    }
    let Ok(record) = json_parser::new(final_record).parse_record() else {
        return false;
    };
    record_string(&record, "event") == Some(semantic_presentation_passed_event)
        && record_string(&record, "level") == Some("info")
        && matches!(record.get("schema"), Some(json_scalar::number(value)) if value == "1")
        && expected[3..].iter().all(|field| {
            matches!(record.get(*field), Some(json_scalar::number(value)) if value.parse::<u64>().is_ok())
        })
}

fn remove_supervised_renderer_capture_artifacts() -> Result<(), String> {
    let root = repository_root()?;
    for path in [
        root.join("build/qemu_supervised_renderer.log"),
        root.join("build/supervised_renderer.ppm"),
        root.join("build/qemu_supervised_renderer.log.part"),
        root.join("build/supervised_renderer.ppm.part"),
    ] {
        remove_optional_file(&path)?;
    }
    Ok(())
}

fn create_private_capture_file(path: &Path) -> Result<(), String> {
    let mut options = OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options
        .open(path)
        .map(|_| ())
        .map_err(|error| format!("create private capture {}: {error}", path.display()))
}

fn capture_supervised_renderer_frame(
    child: &mut Child,
    serial_log: &Path,
    frame_path: &Path,
    deadline: Instant,
) -> Result<(), String> {
    wait_for_supervised_renderer_event(
        child,
        serial_log,
        supervised_renderer_passed_event,
        deadline,
    )?;
    let mut monitor = child
        .stdin
        .take()
        .ok_or_else(|| "capture QEMU supervised-renderer monitor input".to_string())?;
    writeln!(monitor, "screendump {}", frame_path.display())
        .and_then(|()| monitor.flush())
        .map_err(|error| format!("request QEMU supervised-renderer screendump: {error}"))?;
    writeln!(monitor, "quit")
        .and_then(|()| monitor.flush())
        .map_err(|error| format!("request QEMU supervised-renderer quit: {error}"))
}

fn wait_for_supervised_renderer_event(
    child: &mut Child,
    serial_log: &Path,
    event: &str,
    deadline: Instant,
) -> Result<(), String> {
    loop {
        let transcript =
            read_bounded_optional_utf8_file(serial_log, supervised_renderer_transcript_limit)?;
        if transcript.contains(event) {
            return Ok(());
        }
        if let Some(status) = child
            .try_wait()
            .map_err(|error| format!("poll QEMU supervised-renderer smoke test: {error}"))?
        {
            return Err(format!(
                "QEMU supervised-renderer smoke test exited with {status} before `{event}`"
            ));
        }
        if Instant::now() >= deadline {
            return Err(format!(
                "QEMU supervised-renderer smoke test exceeded its {}-second deadline waiting for `{event}`",
                smoke_timeout.as_secs_f64()
            ));
        }
        thread::sleep(Duration::from_millis(10));
    }
}

fn wait_for_supervised_renderer_exit(
    child: &mut Child,
    deadline: Instant,
) -> Result<ExitStatus, String> {
    loop {
        match child
            .try_wait()
            .map_err(|error| format!("wait for QEMU supervised-renderer smoke test: {error}"))?
        {
            Some(status) => return Ok(status),
            None if Instant::now() < deadline => thread::sleep(Duration::from_millis(10)),
            None => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!(
                    "QEMU supervised-renderer smoke test exceeded its {}-second exit deadline",
                    smoke_timeout.as_secs_f64()
                ));
            }
        }
    }
}

fn verify_supervised_renderer_frame_file(path: &Path) -> Result<(), String> {
    let expected_pixels = supervised_renderer_oracle::frame_width
        .checked_mul(supervised_renderer_oracle::frame_height)
        .and_then(|pixels| pixels.checked_mul(supervised_renderer_oracle::channel_count))
        .ok_or_else(|| "supervised-renderer PPM extent overflowed".to_string())?;
    let expected_length = supervised_renderer_oracle::ppm_header
        .len()
        .checked_add(expected_pixels)
        .ok_or_else(|| "supervised-renderer PPM length overflowed".to_string())?;
    let actual_length = fs::metadata(path)
        .map_err(|error| format!("inspect {}: {error}", path.display()))?
        .len();
    if actual_length != expected_length as u64 {
        return Err(format!(
            "supervised-renderer PPM has {actual_length} bytes; expected exactly {expected_length}"
        ));
    }
    let bytes = fs::read(path).map_err(|error| format!("read {}: {error}", path.display()))?;
    supervised_renderer_oracle::verify_ppm(&bytes)
}

fn run_qemu_supervised_renderer_demo(image: &Path) -> Result<(), String> {
    println!(
        "supervised renderer demo will retain the verified public frame until the QEMU window is closed"
    );
    let mut command = qemu_command(image)?;
    command
        .arg("-no-shutdown")
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    let status = command
        .status()
        .map_err(|error| format!("launch QEMU supervised-renderer demo: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "QEMU supervised-renderer demo exited with {status}"
        ))
    }
}

fn remove_optional_file(path: &Path) -> Result<(), String> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("remove {}: {error}", path.display())),
    }
}

fn read_bounded_optional_utf8_file(path: &Path, limit: usize) -> Result<String, String> {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(String::new()),
        Err(error) => return Err(format!("open {}: {error}", path.display())),
    };
    let read_limit = u64::try_from(limit)
        .ok()
        .and_then(|limit| limit.checked_add(1))
        .ok_or_else(|| "display-vessel diagnostic limit is invalid".to_string())?;
    let mut bytes = Vec::with_capacity(limit.min(8192) + 1);
    file.take(read_limit)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("read bounded {}: {error}", path.display()))?;
    if bytes.len() > limit {
        return Err(format!(
            "{} exceeds the display-vessel transcript limit",
            path.display()
        ));
    }
    String::from_utf8(bytes).map_err(|_| format!("{} is not valid UTF-8", path.display()))
}

fn verify_display_vessel_frame_file(path: &Path) -> Result<(), String> {
    let expected_pixel_bytes = display_vessel_width
        .checked_mul(display_vessel_height)
        .and_then(|pixels| pixels.checked_mul(display_vessel_channels))
        .ok_or_else(|| "display-vessel PPM extent overflowed".to_string())?;
    let expected_length = display_vessel_ppm_header
        .len()
        .checked_add(expected_pixel_bytes)
        .ok_or_else(|| "display-vessel PPM file length overflowed".to_string())?;
    let actual_length = fs::metadata(path)
        .map_err(|error| format!("inspect {}: {error}", path.display()))?
        .len();
    if actual_length != expected_length as u64 {
        return Err(format!(
            "display-vessel PPM has {actual_length} bytes; expected exactly {expected_length}"
        ));
    }
    let bytes = fs::read(path).map_err(|error| format!("read {}: {error}", path.display()))?;
    verify_display_vessel_frame(&bytes)
}

fn verify_display_vessel_frame_file_redacted(path: &Path) -> Result<(), String> {
    verify_display_vessel_frame_file(path).map_err(|_| {
        "display-vessel frame was rejected without echoing unverified pixel content".to_string()
    })
}

fn verify_display_vessel_frame(ppm: &[u8]) -> Result<(), String> {
    let expected_pixel_bytes = display_vessel_width
        .checked_mul(display_vessel_height)
        .and_then(|pixels| pixels.checked_mul(display_vessel_channels))
        .ok_or_else(|| "display-vessel PPM extent overflowed".to_string())?;
    let expected_length = display_vessel_ppm_header
        .len()
        .checked_add(expected_pixel_bytes)
        .ok_or_else(|| "display-vessel PPM file length overflowed".to_string())?;
    if ppm.len() != expected_length {
        return Err(format!(
            "display-vessel PPM has {} bytes; expected exactly {expected_length}",
            ppm.len()
        ));
    }
    if !ppm.starts_with(display_vessel_ppm_header) {
        return Err("display-vessel frame is not canonical P6 1024x768 RGB8".into());
    }

    let observed = &ppm[display_vessel_ppm_header.len()..];
    let expected = render_display_vessel_fixture()?;
    if let Some(index) = observed
        .iter()
        .zip(expected.iter())
        .position(|(observed, expected)| observed != expected)
    {
        let pixel_index = index / display_vessel_channels;
        let x = pixel_index % display_vessel_width;
        let y = pixel_index / display_vessel_width;
        let channel = ["red", "green", "blue"][index % display_vessel_channels];
        return Err(format!(
            "display-vessel frame differs at ({x}, {y}) {channel}: observed {:#04x}, expected {:#04x}",
            observed[index], expected[index]
        ));
    }
    Ok(())
}

fn render_display_vessel_fixture() -> Result<Vec<u8>, String> {
    let byte_length = display_vessel_width
        .checked_mul(display_vessel_height)
        .and_then(|pixels| pixels.checked_mul(display_vessel_channels))
        .ok_or_else(|| "display fixture extent overflowed".to_string())?;
    let mut pixels = vec![0; byte_length];
    for rectangle in display_fixture_rectangles {
        let end_x = rectangle
            .x
            .checked_add(rectangle.width)
            .ok_or_else(|| "display fixture rectangle x extent overflowed".to_string())?;
        let end_y = rectangle
            .y
            .checked_add(rectangle.height)
            .ok_or_else(|| "display fixture rectangle y extent overflowed".to_string())?;
        if rectangle.width == 0
            || rectangle.height == 0
            || end_x > display_vessel_width
            || end_y > display_vessel_height
        {
            return Err("display fixture rectangle is outside the public frame".into());
        }
        for y in rectangle.y..end_y {
            for x in rectangle.x..end_x {
                let offset = (y * display_vessel_width + x) * display_vessel_channels;
                pixels[offset] = rectangle.color.red;
                pixels[offset + 1] = rectangle.color.green;
                pixels[offset + 2] = rectangle.color.blue;
            }
        }
    }
    Ok(pixels)
}

fn run_qemu_memory_smoke(image: &Path, bounds: memory_image_bounds) -> Result<(), String> {
    let small = run_qemu_memory_size(image, "96M", bounds)?;
    let large = run_qemu_memory_size(image, "320M", bounds)?;
    verify_memory_size_evidence(small, large)?;
    println!(
        "memory smoke proved deterministic ownership, inactive construction, resident CPL0 root switching, prepared launch/cancellation, owned low-address ring-3 execution, exact protection faults, physical-alias preservation, and cleanup/retry at 96M and 320M (free frames: {} -> {})",
        small.initial_free_frames, large.initial_free_frames
    );
    Ok(())
}

fn run_qemu_memory_size(
    image: &Path,
    memory_size: &str,
    bounds: memory_image_bounds,
) -> Result<memory_run_evidence, String> {
    let mut command = qemu_command_with_memory(image, memory_size)?;
    command
        .arg("-display")
        .arg("none")
        .arg("-monitor")
        .arg("none")
        .arg("-device")
        .arg("isa-debug-exit,iobase=0xf4,iosize=0x04");
    println!("memory smoke QEMU RAM: {memory_size}");
    let mut evidence = None;
    run_qemu_with_transcript(command, smoke_timeout, |status, transcript| {
        if status.code() != Some(qemu_success_status) {
            return Err(format!(
                "QEMU did not report memory-test success for {memory_size} (process status: {:?})",
                status.code()
            ));
        }
        evidence = Some(verify_memory_transcript(transcript, bounds)?);
        Ok(())
    })?;
    evidence.ok_or_else(|| format!("memory smoke verifier produced no evidence for {memory_size}"))
}

fn run_qemu_interrupt_smoke(image: &Path) -> Result<(), String> {
    let root = repository_root()?;
    let serial_log = root.join("build/qemu_interrupt.log");
    match fs::remove_file(&serial_log) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(format!("remove {}: {error}", serial_log.display())),
    }

    let mut child = Command::new("qemu-system-x86_64")
        .current_dir(&root)
        .arg("-machine")
        .arg("q35")
        .arg("-m")
        .arg("256M")
        .arg("-drive")
        .arg(format!("format=raw,file={}", image.display()))
        .arg("-no-reboot")
        .arg("-display")
        .arg("none")
        .arg("-serial")
        .arg(format!("file:{}", serial_log.display()))
        .arg("-monitor")
        .arg("stdio")
        .arg("-device")
        .arg("isa-debug-exit,iobase=0xf4,iosize=0x04")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("launch QEMU interrupt smoke test: {error}"))?;

    let deadline = Instant::now() + smoke_timeout;
    let exercise_result = exercise_interrupt_test(&mut child, &serial_log, deadline);
    let status_result = if exercise_result.is_ok() {
        wait_for_interrupt_exit(&mut child, deadline)
    } else {
        let _ = child.kill();
        child
            .wait()
            .map_err(|error| format!("reap failed QEMU interrupt smoke test: {error}"))
    };
    if status_result.is_err() {
        let _ = child.kill();
        let _ = child.wait();
    }

    let mut stderr = Vec::new();
    if let Some(mut stream) = child.stderr.take() {
        stream
            .read_to_end(&mut stderr)
            .map_err(|error| format!("read QEMU interrupt smoke stderr: {error}"))?;
    }
    let transcript = fs::read_to_string(&serial_log)
        .map_err(|error| format!("read {}: {error}", serial_log.display()))?;
    print!("{transcript}");

    exercise_result?;
    let status = status_result?;
    verify_interrupt_smoke_output(status, &transcript).map(|evidence| {
        println!(
            "interrupt smoke proved {} PIT IRQs, {} private transitions, and {} explicit losses",
            evidence.timer_interrupts, evidence.transitions, evidence.lost_count
        );
    }).map_err(|message| {
        format!(
            "{message} (QEMU status: {status}; stderr: {})",
            String::from_utf8_lossy(&stderr).trim()
        )
    })
}

fn exercise_interrupt_test(
    child: &mut Child,
    serial_log: &Path,
    deadline: Instant,
) -> Result<(), String> {
    wait_for_interrupt_serial_event(child, serial_log, interrupt_ready_event, deadline)?;
    wait_for_interrupt_serial_event(child, serial_log, interrupt_timer_event, deadline)?;
    let mut monitor = child
        .stdin
        .take()
        .ok_or_else(|| "capture QEMU interrupt-test monitor input".to_string())?;
    type_interrupt_probe(&mut monitor, privacy_probe)?;
    wait_for_interrupt_serial_event(child, serial_log, interrupt_rearmed_event, deadline)?;
    type_interrupt_probe(&mut monitor, interrupt_post_rearm_probe)?;
    wait_for_interrupt_serial_event(child, serial_log, interrupt_passed_event, deadline)?;
    Ok(())
}

fn wait_for_interrupt_serial_event(
    child: &mut Child,
    serial_log: &Path,
    event: &str,
    deadline: Instant,
) -> Result<String, String> {
    loop {
        let transcript = match fs::read_to_string(serial_log) {
            Ok(transcript) => transcript,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(error) => return Err(format!("read {}: {error}", serial_log.display())),
        };
        if transcript.contains(event) {
            return Ok(transcript);
        }
        if let Some(status) = child
            .try_wait()
            .map_err(|error| format!("poll QEMU interrupt smoke test: {error}"))?
        {
            return Err(format!(
                "QEMU interrupt smoke test exited with {status} before `{event}`"
            ));
        }
        if Instant::now() >= deadline {
            return Err(format!(
                "QEMU interrupt smoke test exceeded its {}-second deadline waiting for `{event}`",
                smoke_timeout.as_secs_f64()
            ));
        }
        thread::sleep(Duration::from_millis(10));
    }
}

fn type_interrupt_probe(monitor: &mut ChildStdin, probe: &str) -> Result<(), String> {
    for byte in probe.bytes() {
        let key = match byte {
            b'a'..=b'z' => char::from(byte),
            _ => {
                return Err(format!(
                    "unsupported interrupt-smoke input byte {byte:#04x}"
                ));
            }
        };
        writeln!(monitor, "sendkey {key} 5")
            .and_then(|()| monitor.flush())
            .map_err(|error| format!("write QEMU interrupt-test key: {error}"))?;
        // The feature proof expects one complete make/break pair before the
        // next key, so leave margin beyond QEMU's five-millisecond hold.
        thread::sleep(Duration::from_millis(20));
    }
    Ok(())
}

fn wait_for_interrupt_exit(child: &mut Child, deadline: Instant) -> Result<ExitStatus, String> {
    loop {
        match child
            .try_wait()
            .map_err(|error| format!("wait for QEMU interrupt smoke test: {error}"))?
        {
            Some(status) => return Ok(status),
            None if Instant::now() < deadline => thread::sleep(Duration::from_millis(10)),
            None => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!(
                    "QEMU interrupt smoke test exceeded its {}-second exit deadline",
                    smoke_timeout.as_secs_f64()
                ));
            }
        }
    }
}

fn qemu_command(image: &Path) -> Result<Command, String> {
    qemu_command_with_memory(image, "256M")
}

fn qemu_command_with_memory(image: &Path, memory_size: &str) -> Result<Command, String> {
    let root = repository_root()?;
    let mut command = Command::new("qemu-system-x86_64");
    // Proofs using isa-debug-exit must exit on guest shutdown. Interactive
    // callers opt into -no-shutdown separately to retain their final frame.
    command
        .current_dir(root)
        .arg("-machine")
        .arg("q35")
        .arg("-m")
        .arg(memory_size)
        .arg("-drive")
        .arg(format!("format=raw,file={}", image.display()))
        .arg("-no-reboot")
        .arg("-serial")
        .arg("stdio");
    Ok(command)
}

fn run_qemu_shell_smoke(image: &Path) -> Result<(), String> {
    let root = repository_root()?;
    let serial_log = root.join("build/qemu_shell.log");
    match fs::remove_file(&serial_log) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(format!("remove {}: {error}", serial_log.display())),
    }

    let mut child = Command::new("qemu-system-x86_64")
        .current_dir(&root)
        .arg("-machine")
        .arg("q35")
        .arg("-m")
        .arg("256M")
        .arg("-drive")
        .arg(format!("format=raw,file={}", image.display()))
        .arg("-no-reboot")
        .arg("-no-shutdown")
        .arg("-display")
        .arg("none")
        .arg("-serial")
        .arg(format!("file:{}", serial_log.display()))
        .arg("-monitor")
        .arg("stdio")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("launch QEMU shell smoke test: {error}"))?;
    let result = exercise_bootstrap_shell(&mut child, &serial_log);
    let _ = child.kill();
    let status = child
        .wait()
        .map_err(|error| format!("reap QEMU shell smoke test: {error}"))?;
    let mut stderr = Vec::new();
    if let Some(mut stream) = child.stderr.take() {
        stream
            .read_to_end(&mut stderr)
            .map_err(|error| format!("read QEMU shell smoke stderr: {error}"))?;
    }
    let transcript = fs::read_to_string(&serial_log)
        .map_err(|error| format!("read {}: {error}", serial_log.display()))?;
    print!("{transcript}");
    let result = result.and_then(|()| {
        if transcript.contains(privacy_probe) {
            Err("bootstrap shell leaked command text into structured serial events".into())
        } else {
            Ok(())
        }
    });

    result.map_err(|message| {
        format!(
            "{message} (QEMU status: {status}; stderr: {})",
            String::from_utf8_lossy(&stderr).trim()
        )
    })
}

fn exercise_bootstrap_shell(child: &mut Child, serial_log: &Path) -> Result<(), String> {
    let deadline = Instant::now() + smoke_timeout;
    wait_for_serial_event(child, serial_log, bootstrap_shell_ready_event, deadline)?;
    let mut monitor = child
        .stdin
        .take()
        .ok_or_else(|| "capture QEMU monitor input".to_string())?;

    type_monitor_text(&mut monitor, "versiom")?;
    type_monitor_named_key(&mut monitor, "backspace")?;
    type_monitor_line(&mut monitor, "n")?;
    wait_for_serial_event(child, serial_log, bootstrap_shell_version_event, deadline)?;
    type_monitor_line(&mut monitor, &format!("echo {privacy_probe}"))?;
    wait_for_serial_event(child, serial_log, bootstrap_shell_echo_event, deadline)?;
    type_monitor_line(&mut monitor, privacy_probe)?;
    wait_for_serial_event(child, serial_log, bootstrap_shell_unknown_event, deadline)?;
    let transcript = wait_for_serial_event(
        child,
        serial_log,
        bootstrap_shell_final_prompt_event,
        deadline,
    )?;
    ensure_child_running(child)?;
    if transcript.contains(privacy_probe) {
        return Err("bootstrap shell leaked command text before the final prompt".into());
    }
    Ok(())
}

fn ensure_child_running(child: &mut Child) -> Result<(), String> {
    if let Some(status) = child
        .try_wait()
        .map_err(|error| format!("poll QEMU shell smoke test: {error}"))?
    {
        Err(format!("QEMU exited unexpectedly with {status}"))
    } else {
        Ok(())
    }
}

fn wait_for_serial_event(
    child: &mut Child,
    serial_log: &Path,
    event: &str,
    deadline: Instant,
) -> Result<String, String> {
    loop {
        ensure_child_running(child)?;
        let transcript = match fs::read_to_string(serial_log) {
            Ok(transcript) => transcript,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(error) => return Err(format!("read {}: {error}", serial_log.display())),
        };
        if transcript.contains(event) {
            return Ok(transcript);
        }
        if Instant::now() >= deadline {
            return Err(format!(
                "QEMU shell smoke test exceeded its {}-second deadline waiting for `{event}`",
                smoke_timeout.as_secs_f64()
            ));
        }
        thread::sleep(Duration::from_millis(10));
    }
}

fn type_monitor_line(monitor: &mut ChildStdin, line: &str) -> Result<(), String> {
    type_monitor_text(monitor, line)?;
    type_monitor_named_key(monitor, "ret")
}

fn type_monitor_text(monitor: &mut ChildStdin, text: &str) -> Result<(), String> {
    for byte in text.bytes() {
        let key = match byte {
            b'a'..=b'z' | b'0'..=b'9' => char::from(byte).to_string(),
            b' ' => "spc".to_string(),
            _ => return Err(format!("unsupported shell-smoke input byte {byte:#04x}")),
        };
        writeln!(monitor, "sendkey {key} 1")
            .map_err(|error| format!("write QEMU monitor key: {error}"))?;
        monitor
            .flush()
            .map_err(|error| format!("flush QEMU monitor key: {error}"))?;
        thread::sleep(Duration::from_millis(3));
    }
    Ok(())
}

fn type_monitor_named_key(monitor: &mut ChildStdin, key: &str) -> Result<(), String> {
    writeln!(monitor, "sendkey {key} 1")
        .and_then(|()| monitor.flush())
        .map_err(|error| format!("send QEMU monitor key `{key}`: {error}"))?;
    thread::sleep(Duration::from_millis(3));
    Ok(())
}

fn run_qemu_smoke(
    command: Command,
    timeout: Duration,
    required_events: &[&str],
) -> Result<(), String> {
    run_qemu_with_transcript(command, timeout, |status, transcript| {
        verify_smoke_output(status, transcript, required_events)
    })
}

fn run_qemu_with_transcript(
    mut command: Command,
    timeout: Duration,
    verifier: impl FnOnce(ExitStatus, &str) -> Result<(), String>,
) -> Result<(), String> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command
        .spawn()
        .map_err(|error| format!("launch QEMU: {error}"))?;
    let mut stdout = child
        .stdout
        .take()
        .ok_or_else(|| "capture QEMU stdout".to_string())?;
    let mut stderr = child
        .stderr
        .take()
        .ok_or_else(|| "capture QEMU stderr".to_string())?;
    let stdout_reader = thread::spawn(move || {
        let mut bytes = Vec::new();
        stdout.read_to_end(&mut bytes).map(|_| bytes)
    });
    let stderr_reader = thread::spawn(move || {
        let mut bytes = Vec::new();
        stderr.read_to_end(&mut bytes).map(|_| bytes)
    });

    let started = Instant::now();
    let (status, timed_out) = loop {
        match child.try_wait() {
            Ok(Some(status)) => break (status, false),
            Ok(None) if started.elapsed() < timeout => {
                thread::sleep(Duration::from_millis(10));
            }
            Ok(None) => {
                let _ = child.kill();
                let status = child
                    .wait()
                    .map_err(|error| format!("reap timed-out QEMU: {error}"))?;
                break (status, true);
            }
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("wait for QEMU: {error}"));
            }
        }
    };

    let mut bytes = stdout_reader
        .join()
        .map_err(|_| "QEMU stdout reader panicked".to_string())?
        .map_err(|error| format!("read QEMU stdout: {error}"))?;
    bytes.extend(
        stderr_reader
            .join()
            .map_err(|_| "QEMU stderr reader panicked".to_string())?
            .map_err(|error| format!("read QEMU stderr: {error}"))?,
    );
    let transcript = String::from_utf8_lossy(&bytes);
    print!("{transcript}");
    if timed_out {
        return Err(format!(
            "QEMU smoke test exceeded its {}-second deadline",
            timeout.as_secs_f64()
        ));
    }
    verifier(status, &transcript)
}

fn verify_smoke_output(
    status: ExitStatus,
    transcript: &str,
    required_events: &[&str],
) -> Result<(), String> {
    if status.code() != Some(qemu_success_status) {
        return Err(format!(
            "QEMU did not report kernel success (process status: {:?})",
            status.code()
        ));
    }
    for event in required_events {
        if !transcript.contains(event) {
            return Err(format!(
                "serial transcript did not contain required event `{event}`"
            ));
        }
    }
    Ok(())
}

fn validate_memory_image_bounds(bounds: memory_image_bounds) -> Result<(), String> {
    if bounds.kernel_start != u64::from(kernel_load_address)
        || bounds.kernel_end <= bounds.kernel_start
        || bounds.kernel_end > memory_address_limit
        || !bounds.kernel_end.is_multiple_of(memory_frame_size)
    {
        return Err(format!(
            "memory-test kernel bounds are invalid: {:#x}..{:#x}",
            bounds.kernel_start, bounds.kernel_end
        ));
    }
    Ok(())
}

fn verify_memory_size_evidence(
    small: memory_run_evidence,
    large: memory_run_evidence,
) -> Result<(), String> {
    if small.total_frames != memory_total_frames || large.total_frames != memory_total_frames {
        return Err("memory-test runs did not report the fixed four-GiB frame ledger".into());
    }
    let expected_added_frames = (320u64 - 96) * 1024 * 1024 / memory_frame_size;
    let added_frames = large
        .initial_free_frames
        .checked_sub(small.initial_free_frames)
        .ok_or_else(|| "the 320M run exposed no more free frames than the 96M run".to_string())?;
    if added_frames != expected_added_frames {
        return Err(format!(
            "the RAM-size evidence added {added_frames} free frames; expected {expected_added_frames}"
        ));
    }
    let removed_unavailable = small
        .unavailable_frames
        .checked_sub(large.unavailable_frames)
        .ok_or_else(|| "the 320M run did not reduce unavailable-frame coverage".to_string())?;
    if removed_unavailable != expected_added_frames {
        return Err(format!(
            "the RAM-size evidence removed {removed_unavailable} unavailable frames; expected {expected_added_frames}"
        ));
    }
    if (small.data_a, small.data_b, small.root_a, small.root_b)
        != (large.data_a, large.data_b, large.root_a, large.root_b)
    {
        return Err("lowest-address allocation was not deterministic across RAM sizes".into());
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum json_scalar {
    string(String),
    number(String),
    boolean(bool),
    null,
}

type structured_record = BTreeMap<String, json_scalar>;

struct json_parser<'a> {
    bytes: &'a [u8],
    cursor: usize,
}

impl<'a> json_parser<'a> {
    fn new(text: &'a str) -> Self {
        Self {
            bytes: text.as_bytes(),
            cursor: 0,
        }
    }

    fn parse_record(mut self) -> Result<structured_record, String> {
        self.skip_whitespace();
        self.expect(b'{')?;
        let mut record = structured_record::new();
        self.skip_whitespace();
        if self.consume(b'}') {
            self.finish()?;
            return Ok(record);
        }

        loop {
            let key = self.parse_string()?;
            self.skip_whitespace();
            self.expect(b':')?;
            self.skip_whitespace();
            let value = self.parse_scalar()?;
            if record.insert(key.clone(), value).is_some() {
                return Err(format!("duplicate JSON field `{key}`"));
            }
            self.skip_whitespace();
            if self.consume(b'}') {
                self.finish()?;
                return Ok(record);
            }
            self.expect(b',')?;
            self.skip_whitespace();
        }
    }

    fn parse_scalar(&mut self) -> Result<json_scalar, String> {
        match self.peek() {
            Some(b'"') => self.parse_string().map(json_scalar::string),
            Some(b'-' | b'0'..=b'9') => self.parse_number().map(json_scalar::number),
            Some(b't') => {
                self.expect_keyword(b"true")?;
                Ok(json_scalar::boolean(true))
            }
            Some(b'f') => {
                self.expect_keyword(b"false")?;
                Ok(json_scalar::boolean(false))
            }
            Some(b'n') => {
                self.expect_keyword(b"null")?;
                Ok(json_scalar::null)
            }
            Some(other) => Err(format!(
                "unsupported JSON value beginning with byte {other:#04x}"
            )),
            None => Err("unexpected end of JSON value".into()),
        }
    }

    fn parse_string(&mut self) -> Result<String, String> {
        self.expect(b'"')?;
        let mut value = String::new();
        loop {
            let byte = self
                .next()
                .ok_or_else(|| "unterminated JSON string".to_string())?;
            match byte {
                b'"' => return Ok(value),
                b'\\' => {
                    let escaped = self
                        .next()
                        .ok_or_else(|| "unterminated JSON escape".to_string())?;
                    match escaped {
                        b'"' => value.push('"'),
                        b'\\' => value.push('\\'),
                        b'/' => value.push('/'),
                        b'b' => value.push('\u{0008}'),
                        b'f' => value.push('\u{000c}'),
                        b'n' => value.push('\n'),
                        b'r' => value.push('\r'),
                        b't' => value.push('\t'),
                        b'u' => value.push(self.parse_unicode_escape()?),
                        _ => {
                            return Err(format!("invalid JSON escape `\\{}`", char::from(escaped)));
                        }
                    }
                }
                0x00..=0x1f => return Err("control byte in JSON string".into()),
                0x20..=0x7f => value.push(char::from(byte)),
                _ => return Err("non-ASCII byte in serial JSON string".into()),
            }
        }
    }

    fn parse_unicode_escape(&mut self) -> Result<char, String> {
        let mut value = 0u32;
        for _ in 0..4 {
            let byte = self
                .next()
                .ok_or_else(|| "truncated JSON Unicode escape".to_string())?;
            value = (value << 4)
                | char::from(byte)
                    .to_digit(16)
                    .ok_or_else(|| "invalid JSON Unicode escape".to_string())?;
        }
        char::from_u32(value).ok_or_else(|| "invalid JSON Unicode scalar".into())
    }

    fn parse_number(&mut self) -> Result<String, String> {
        let start = self.cursor;
        self.consume(b'-');
        match self.peek() {
            Some(b'0') => {
                self.cursor += 1;
            }
            Some(b'1'..=b'9') => {
                self.cursor += 1;
                while matches!(self.peek(), Some(b'0'..=b'9')) {
                    self.cursor += 1;
                }
            }
            _ => return Err("invalid JSON number".into()),
        }
        if self.consume(b'.') {
            self.consume_digits("JSON fraction requires a digit")?;
        }
        if matches!(self.peek(), Some(b'e' | b'E')) {
            self.cursor += 1;
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.cursor += 1;
            }
            self.consume_digits("JSON exponent requires a digit")?;
        }
        String::from_utf8(self.bytes[start..self.cursor].to_vec())
            .map_err(|error| format!("invalid JSON number encoding: {error}"))
    }

    fn consume_digits(&mut self, error: &str) -> Result<(), String> {
        let start = self.cursor;
        while matches!(self.peek(), Some(b'0'..=b'9')) {
            self.cursor += 1;
        }
        if self.cursor == start {
            Err(error.into())
        } else {
            Ok(())
        }
    }

    fn expect_keyword(&mut self, keyword: &[u8]) -> Result<(), String> {
        if self.bytes.get(self.cursor..self.cursor + keyword.len()) == Some(keyword) {
            self.cursor += keyword.len();
            Ok(())
        } else {
            Err(format!("invalid JSON token at byte offset {}", self.cursor))
        }
    }

    fn skip_whitespace(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\r' | b'\n')) {
            self.cursor += 1;
        }
    }

    fn expect(&mut self, expected: u8) -> Result<(), String> {
        if self.consume(expected) {
            Ok(())
        } else {
            Err(format!(
                "expected byte {expected:#04x} at JSON offset {}",
                self.cursor
            ))
        }
    }

    fn consume(&mut self, expected: u8) -> bool {
        if self.peek() == Some(expected) {
            self.cursor += 1;
            true
        } else {
            false
        }
    }

    fn next(&mut self) -> Option<u8> {
        let byte = self.peek()?;
        self.cursor += 1;
        Some(byte)
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.cursor).copied()
    }

    fn finish(&mut self) -> Result<(), String> {
        self.skip_whitespace();
        if self.cursor == self.bytes.len() {
            Ok(())
        } else {
            Err(format!("trailing content at JSON offset {}", self.cursor))
        }
    }
}

const expected_memory_events: &[&str] = &[
    "kernel.memory.ready",
    "kernel.memory_test.leases.ready",
    "kernel.memory_test.roots.ready",
    "kernel.memory_test.switch.verified",
    "kernel.memory_test.roots.withdrawn",
    "kernel.memory_test.reuse.verified",
    "kernel.memory_test.released",
    "kernel.memory_test.physical_window.ready",
    "kernel.memory_test.staged.verified",
    "kernel.memory_test.staged.released",
    "kernel.memory_test.resident.ready",
    "kernel.memory_test.resident.verified",
    "kernel.memory_test.resident.released",
    "kernel.memory_test.user.ready",
    "kernel.memory_test.user.verified",
    "kernel.memory_test.user.released",
    "kernel.memory_test.launch.verified",
    "kernel.memory_test.passed",
];

const expected_memory_lifecycle_events: &[&str] = &[
    "boot.omen.accepted",
    "boot.memory.map",
    "kernel.memory.ready",
    "boot.ready",
    "kernel.memory_test.leases.ready",
    "kernel.memory_test.roots.ready",
    "kernel.memory_test.switch.verified",
    "kernel.memory_test.roots.withdrawn",
    "kernel.memory_test.reuse.verified",
    "kernel.memory_test.released",
    "kernel.memory_test.physical_window.ready",
    "kernel.memory_test.staged.verified",
    "kernel.memory_test.staged.released",
    "kernel.memory_test.resident.ready",
    "kernel.memory_test.resident.verified",
    "kernel.memory_test.resident.released",
    "kernel.memory_test.user.ready",
    "kernel.memory_test.user.verified",
    "kernel.memory_test.user.released",
    "kernel.memory_test.launch.verified",
    "kernel.memory_test.passed",
];

fn verify_interrupt_smoke_output(
    status: ExitStatus,
    transcript: &str,
) -> Result<interrupt_run_evidence, String> {
    if status.code() != Some(qemu_success_status) {
        return Err(format!(
            "QEMU did not report interrupt-test success (process status: {:?})",
            status.code()
        ));
    }
    verify_interrupt_transcript(transcript)
}

fn verify_interrupt_transcript(transcript: &str) -> Result<interrupt_run_evidence, String> {
    if transcript.contains(privacy_probe) {
        return Err("interrupt smoke leaked the private input probe into COM1".into());
    }

    let mut fault_path_line = None;
    let mut lifecycle_records = Vec::new();
    let mut observed_serial_events = Vec::new();
    for (index, line) in transcript.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if !line.starts_with('{') {
            return Err(format!(
                "interrupt smoke transcript contains unstructured serial content on line {}",
                index + 1
            ));
        }
        let record = json_parser::new(line).parse_record().map_err(|error| {
            format!(
                "malformed structured record on serial line {}: {error}",
                index + 1
            )
        })?;
        reject_interrupt_private_fields(&record, index + 1)?;
        if let Some(level @ ("error" | "fatal")) = record_string(&record, "level") {
            let event = record_string(&record, "event").unwrap_or("<missing>");
            return Err(format!(
                "interrupt smoke transcript contains {level} record `{event}`"
            ));
        }
        let event = required_string(&record, "interrupt serial record", "event")?;
        if event.ends_with(".failed") {
            return Err(format!(
                "interrupt smoke transcript contains forbidden event `{event}`"
            ));
        }
        validate_interrupt_serial_record(&record, event)?;
        observed_serial_events.push(event.to_string());
        if event == "kernel.fault_path.ready" && fault_path_line.replace(index).is_some() {
            return Err("interrupt smoke contains duplicate fault-path readiness".into());
        }
        if event == "kernel.interrupts.ready" || event.starts_with(interrupt_event_prefix) {
            lifecycle_records.push((index, record));
        }
    }

    let observed_serial_events = observed_serial_events
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    if observed_serial_events != expected_interrupt_serial_events {
        return Err(format!(
            "interrupt serial lifecycle is missing, duplicated, unexpected, or out of order\nexpected: {expected_interrupt_serial_events:?}\nobserved: {observed_serial_events:?}"
        ));
    }

    let observed = lifecycle_records
        .iter()
        .map(|(_, record)| classify_interrupt_observation(record))
        .collect::<Result<Vec<_>, _>>()?;
    if observed != expected_interrupt_observations {
        return Err(format!(
            "interrupt lifecycle is missing, duplicated, unexpected, or out of order\nexpected: {expected_interrupt_observations:?}\nobserved: {observed:?}"
        ));
    }
    let fault_path_line = fault_path_line
        .ok_or_else(|| "interrupt smoke is missing fault-path readiness".to_string())?;
    if fault_path_line >= lifecycle_records[0].0 {
        return Err("fault-path readiness did not precede external-interrupt readiness".into());
    }

    let ready = &lifecycle_records[0].1;
    require_interrupt_header(
        ready,
        "kernel.interrupts.ready",
        &[
            "backend",
            "vector_base",
            "vector_count",
            "pit_input_hz",
            "pit_divisor",
            "timebase_generation",
            "initial_master_mask",
            "initial_slave_mask",
            "master_mask",
            "slave_mask",
            "input_available",
            "input_source_generation",
            "input_queue_capacity",
            "nmi_masked",
            "if_enabled",
        ],
    )?;
    require_string(ready, "kernel.interrupts.ready", "backend", "legacy_pc")?;
    require_number(ready, "kernel.interrupts.ready", "vector_base", 32)?;
    require_number(ready, "kernel.interrupts.ready", "vector_count", 16)?;
    require_number(ready, "kernel.interrupts.ready", "pit_input_hz", 1_193_182)?;
    require_number(ready, "kernel.interrupts.ready", "pit_divisor", 11_932)?;
    require_number(ready, "kernel.interrupts.ready", "timebase_generation", 1)?;
    require_number(ready, "kernel.interrupts.ready", "initial_master_mask", 255)?;
    require_number(ready, "kernel.interrupts.ready", "initial_slave_mask", 255)?;
    require_number(ready, "kernel.interrupts.ready", "master_mask", 252)?;
    require_number(ready, "kernel.interrupts.ready", "slave_mask", 255)?;
    require_bool(ready, "kernel.interrupts.ready", "input_available", true)?;
    require_number(
        ready,
        "kernel.interrupts.ready",
        "input_source_generation",
        1,
    )?;
    require_number(ready, "kernel.interrupts.ready", "input_queue_capacity", 64)?;
    require_bool(ready, "kernel.interrupts.ready", "nmi_masked", true)?;
    require_bool(ready, "kernel.interrupts.ready", "if_enabled", true)?;

    let timer = &lifecycle_records[1].1;
    let timer_event = "kernel.interrupt_test.timer.verified";
    require_interrupt_header(
        timer,
        timer_event,
        &[
            "timebase_generation",
            "start_tick",
            "end_tick",
            "interrupts",
        ],
    )?;
    let timebase_generation = required_number(timer, timer_event, "timebase_generation")?;
    let start_tick = required_number(timer, timer_event, "start_tick")?;
    let end_tick = required_number(timer, timer_event, "end_tick")?;
    let timer_interrupts = required_number(timer, timer_event, "interrupts")?;
    if timebase_generation != 1
        || timer_interrupts < interrupt_minimum_ticks
        || end_tick.checked_sub(start_tick) != Some(timer_interrupts)
    {
        return Err(format!(
            "event `{timer_event}` does not prove one-generation PIT progress: generation={timebase_generation}, ticks={start_tick}..{end_tick}, interrupts={timer_interrupts}"
        ));
    }

    let input = &lifecycle_records[2].1;
    let input_event = "kernel.interrupt_test.input.verified";
    require_interrupt_header(
        input,
        input_event,
        &[
            "source_generation",
            "first_sequence",
            "last_sequence",
            "transitions",
            "ordered",
            "monotonic",
        ],
    )?;
    let source_generation = required_number(input, input_event, "source_generation")?;
    let first_sequence = required_number(input, input_event, "first_sequence")?;
    let last_sequence = required_number(input, input_event, "last_sequence")?;
    let transitions = required_number(input, input_event, "transitions")?;
    require_bool(input, input_event, "ordered", true)?;
    require_bool(input, input_event, "monotonic", true)?;
    if source_generation != 1
        || first_sequence != 0
        || last_sequence != interrupt_expected_transitions - 1
        || transitions != interrupt_expected_transitions
        || inclusive_sequence_count(first_sequence, last_sequence) != Some(transitions)
    {
        return Err(format!(
            "event `{input_event}` has an invalid source-local span: generation={source_generation}, sequences={first_sequence}..{last_sequence}, transitions={transitions}"
        ));
    }

    let returned = &lifecycle_records[3].1;
    let return_event = "kernel.interrupt_test.return.verified";
    require_interrupt_header(
        returned,
        return_event,
        &["timer", "keyboard", "state_preserved"],
    )?;
    require_bool(returned, return_event, "timer", true)?;
    require_bool(returned, return_event, "keyboard", true)?;
    require_bool(returned, return_event, "state_preserved", true)?;

    let rearmed = &lifecycle_records[4].1;
    let rearmed_event = "kernel.interrupt_test.loss.rearmed";
    require_interrupt_header(
        rearmed,
        rearmed_event,
        &[
            "old_source_generation",
            "new_source_generation",
            "contained_master_mask",
            "rearmed_master_mask",
            "input_available",
        ],
    )?;
    require_number(
        rearmed,
        rearmed_event,
        "old_source_generation",
        source_generation,
    )?;
    require_number(
        rearmed,
        rearmed_event,
        "new_source_generation",
        interrupt_expected_rearmed_generation,
    )?;
    require_number(rearmed, rearmed_event, "contained_master_mask", 254)?;
    require_number(rearmed, rearmed_event, "rearmed_master_mask", 252)?;
    require_bool(rearmed, rearmed_event, "input_available", true)?;

    let loss = &lifecycle_records[5].1;
    let loss_event = "kernel.interrupt_test.loss.verified";
    require_interrupt_header(
        loss,
        loss_event,
        &[
            "source_generation",
            "first_sequence",
            "last_sequence",
            "lost_count",
            "delivered_before",
            "delivered_after",
            "resynchronization_required",
            "new_source_generation",
            "post_rearm_first_sequence",
            "post_rearm_last_sequence",
            "post_rearm_transitions",
        ],
    )?;
    require_number(loss, loss_event, "source_generation", source_generation)?;
    let loss_first_sequence = required_number(loss, loss_event, "first_sequence")?;
    let loss_last_sequence = required_number(loss, loss_event, "last_sequence")?;
    let lost_count = required_number(loss, loss_event, "lost_count")?;
    require_bool(loss, loss_event, "delivered_before", true)?;
    require_bool(loss, loss_event, "delivered_after", true)?;
    require_bool(loss, loss_event, "resynchronization_required", true)?;
    require_number(
        loss,
        loss_event,
        "new_source_generation",
        interrupt_expected_rearmed_generation,
    )?;
    require_number(loss, loss_event, "post_rearm_first_sequence", 0)?;
    require_number(loss, loss_event, "post_rearm_last_sequence", 1)?;
    require_number(
        loss,
        loss_event,
        "post_rearm_transitions",
        interrupt_expected_post_rearm_transitions,
    )?;
    let expected_loss_sequence = last_sequence
        .checked_add(interrupt_expected_queue_capacity)
        .and_then(|sequence| sequence.checked_add(1));
    if lost_count != interrupt_expected_lost_count
        || Some(loss_first_sequence) != expected_loss_sequence
        || loss_last_sequence != loss_first_sequence
        || inclusive_sequence_count(loss_first_sequence, loss_last_sequence) != Some(lost_count)
    {
        return Err(format!(
            "event `{loss_event}` does not prove an exact later loss range: input_last={last_sequence}, loss={loss_first_sequence}..{loss_last_sequence}, count={lost_count}"
        ));
    }

    let masked = &lifecycle_records[6].1;
    let masked_event = "kernel.interrupt_test.masked";
    require_interrupt_header(
        masked,
        masked_event,
        &["master_mask", "slave_mask", "if_enabled"],
    )?;
    require_number(masked, masked_event, "master_mask", 255)?;
    require_number(masked, masked_event, "slave_mask", 255)?;
    require_bool(masked, masked_event, "if_enabled", false)?;

    let passed = &lifecycle_records[7].1;
    let passed_event = "kernel.interrupt_test.passed";
    require_interrupt_header(passed, passed_event, &["ticks", "transitions", "lost"])?;
    require_number(passed, passed_event, "ticks", end_tick)?;
    require_number(passed, passed_event, "transitions", transitions)?;
    require_number(passed, passed_event, "lost", lost_count)?;

    Ok(interrupt_run_evidence {
        timebase_generation,
        start_tick,
        end_tick,
        timer_interrupts,
        source_generation,
        first_sequence,
        last_sequence,
        transitions,
        lost_count,
    })
}

fn validate_interrupt_serial_record(record: &structured_record, event: &str) -> Result<(), String> {
    match event {
        "boot.kernel.enter" => {
            require_interrupt_header(record, event, &["arch"])?;
            require_string(record, event, "arch", "x86_64")
        }
        "kernel.fault_path.ready" => validate_interrupt_fault_path(record),
        "boot.omen.accepted" => {
            require_interrupt_header(record, event, &["version", "boot_drive"])?;
            require_number(record, event, "version", memory_boot_omen_version)?;
            require_number(record, event, "boot_drive", memory_boot_drive)
        }
        "boot.memory.map" => {
            require_interrupt_header(record, event, &["regions", "usable_bytes"])?;
            let regions = required_number(record, event, "regions")?;
            let usable_bytes = required_number(record, event, "usable_bytes")?;
            if !(1..=memory_maximum_regions).contains(&regions)
                || usable_bytes == 0
                || !usable_bytes.is_multiple_of(memory_frame_size)
            {
                return Err(format!(
                    "event `{event}` does not describe bounded page-aligned usable memory"
                ));
            }
            Ok(())
        }
        "kernel.memory.ready" => {
            require_interrupt_header(
                record,
                event,
                &[
                    "frame_size",
                    "address_limit",
                    "total_frames",
                    "free_frames",
                    "unavailable_frames",
                    "data_owned_frames",
                    "table_owned_frames",
                ],
            )?;
            require_number(record, event, "frame_size", memory_frame_size)?;
            require_number(record, event, "address_limit", memory_address_limit)?;
            require_number(record, event, "total_frames", memory_total_frames)?;
            require_number(record, event, "data_owned_frames", 0)?;
            require_number(record, event, "table_owned_frames", 0)?;
            let free_frames = required_number(record, event, "free_frames")?;
            let unavailable_frames = required_number(record, event, "unavailable_frames")?;
            if free_frames == 0
                || free_frames.checked_add(unavailable_frames) != Some(memory_total_frames)
            {
                return Err(format!(
                    "event `{event}` accounting does not cover the exact frame ledger"
                ));
            }
            Ok(())
        }
        "boot.display.ready" => {
            require_interrupt_header(record, event, &["width", "height", "pitch", "bpp"])?;
            require_number(record, event, "width", 1024)?;
            require_number(record, event, "height", 768)?;
            require_number(record, event, "pitch", 3072)?;
            require_number(record, event, "bpp", 24)
        }
        "boot.ready" => {
            require_interrupt_header(record, event, &["control_abi", "framebuffer"])?;
            require_string(record, event, "control_abi", memory_control_abi)?;
            require_bool(record, event, "framebuffer", true)
        }
        "kernel.interrupts.ready"
        | "kernel.interrupt_test.timer.verified"
        | "kernel.interrupt_test.input.verified"
        | "kernel.interrupt_test.return.verified"
        | "kernel.interrupt_test.loss.rearmed"
        | "kernel.interrupt_test.loss.verified"
        | "kernel.interrupt_test.masked"
        | "kernel.interrupt_test.passed" => Ok(()),
        _ => Err(format!(
            "interrupt smoke transcript contains unexpected serial event `{event}`"
        )),
    }
}

fn classify_interrupt_observation(
    record: &structured_record,
) -> Result<interrupt_observation, String> {
    let event = required_string(record, "interrupt lifecycle", "event")?;
    match event {
        "kernel.interrupts.ready" => Ok(interrupt_observation::ready),
        "kernel.interrupt_test.timer.verified" => Ok(interrupt_observation::timer_verified),
        "kernel.interrupt_test.input.verified" => Ok(interrupt_observation::input_verified),
        "kernel.interrupt_test.return.verified" => Ok(interrupt_observation::return_verified),
        "kernel.interrupt_test.loss.rearmed" => Ok(interrupt_observation::loss_rearmed),
        "kernel.interrupt_test.loss.verified" => Ok(interrupt_observation::loss_verified),
        "kernel.interrupt_test.masked" => Ok(interrupt_observation::masked),
        "kernel.interrupt_test.passed" => Ok(interrupt_observation::passed),
        _ => Err(format!(
            "interrupt smoke transcript contains unexpected event `{event}`"
        )),
    }
}

fn validate_interrupt_fault_path(record: &structured_record) -> Result<(), String> {
    let event = "kernel.fault_path.ready";
    require_interrupt_header(
        record,
        event,
        &[
            "vectors",
            "tss",
            "double_fault_ist",
            "nmi_ist",
            "nmi_masked",
        ],
    )?;
    require_number(record, event, "vectors", 32)?;
    require_bool(record, event, "tss", true)?;
    require_number(record, event, "double_fault_ist", 1)?;
    require_number(record, event, "nmi_ist", 2)?;
    require_bool(record, event, "nmi_masked", true)
}

fn reject_interrupt_private_fields(
    record: &structured_record,
    line_number: usize,
) -> Result<(), String> {
    if let Some(field) = interrupt_private_fields
        .iter()
        .find(|field| record.contains_key(**field))
    {
        return Err(format!(
            "structured serial line {line_number} exposes private input field `{field}`"
        ));
    }
    Ok(())
}

fn require_interrupt_header(
    record: &structured_record,
    event: &str,
    event_fields: &[&str],
) -> Result<(), String> {
    let expected_count = event_fields.len() + 3;
    if record.len() != expected_count
        || !record.contains_key("schema")
        || !record.contains_key("event")
        || !record.contains_key("level")
        || event_fields
            .iter()
            .any(|field| !record.contains_key(*field))
    {
        let actual = record.keys().map(String::as_str).collect::<Vec<_>>();
        let mut expected = vec!["schema", "event", "level"];
        expected.extend_from_slice(event_fields);
        expected.sort_unstable();
        return Err(format!(
            "event `{event}` has fields {actual:?}; expected exactly {expected:?}"
        ));
    }
    require_number(record, event, "schema", 1)?;
    require_string(record, event, "event", event)?;
    require_string(record, event, "level", "info")
}

fn inclusive_sequence_count(first: u64, last: u64) -> Option<u64> {
    last.checked_sub(first)?.checked_add(1)
}

fn verify_memory_transcript(
    transcript: &str,
    bounds: memory_image_bounds,
) -> Result<memory_run_evidence, String> {
    validate_memory_image_bounds(bounds)?;
    let mut lifecycle_records = Vec::new();
    for (index, line) in transcript.lines().enumerate() {
        let line = line.trim();
        if !line.starts_with('{') {
            continue;
        }
        let record = json_parser::new(line).parse_record().map_err(|error| {
            format!(
                "malformed structured record on serial line {}: {error}",
                index + 1
            )
        })?;
        if let Some(level @ ("error" | "fatal")) = record_string(&record, "level") {
            let event = record_string(&record, "event").unwrap_or("<missing>");
            return Err(format!(
                "memory smoke transcript contains {level} record `{event}`"
            ));
        }
        let Some(event) = record_string(&record, "event") else {
            continue;
        };
        if event == memory_failed_event || event.ends_with(".failed") {
            return Err(format!(
                "memory smoke transcript contains forbidden event `{event}`"
            ));
        }
        if matches!(
            event,
            "boot.omen.accepted" | "boot.memory.map" | "kernel.memory.ready" | "boot.ready"
        ) || event.starts_with(memory_event_prefix)
        {
            lifecycle_records.push(record);
        }
    }

    let observed_lifecycle_events = lifecycle_records
        .iter()
        .map(|record| record_string(record, "event").unwrap_or("<missing>"))
        .collect::<Vec<_>>();
    if observed_lifecycle_events != expected_memory_lifecycle_events {
        return Err(format!(
            "memory lifecycle is missing, duplicated, unexpected, or out of order\nexpected: {expected_memory_lifecycle_events:?}\nobserved: {observed_lifecycle_events:?}"
        ));
    }

    let memory_records = [
        &lifecycle_records[2],
        &lifecycle_records[4],
        &lifecycle_records[5],
        &lifecycle_records[6],
        &lifecycle_records[7],
        &lifecycle_records[8],
        &lifecycle_records[9],
        &lifecycle_records[10],
        &lifecycle_records[11],
        &lifecycle_records[12],
        &lifecycle_records[13],
        &lifecycle_records[14],
        &lifecycle_records[15],
        &lifecycle_records[16],
        &lifecycle_records[17],
        &lifecycle_records[18],
        &lifecycle_records[19],
        &lifecycle_records[20],
    ];
    let observed_memory_events = memory_records
        .iter()
        .map(|record| record_string(record, "event").unwrap_or("<missing>"))
        .collect::<Vec<_>>();
    if observed_memory_events != expected_memory_events {
        return Err(format!(
            "memory proof lifecycle changed\nexpected: {expected_memory_events:?}\nobserved: {observed_memory_events:?}"
        ));
    }

    let omen = &lifecycle_records[0];
    require_memory_header(
        omen,
        expected_memory_lifecycle_events[0],
        &["version", "boot_drive"],
    )?;
    require_number(
        omen,
        expected_memory_lifecycle_events[0],
        "version",
        memory_boot_omen_version,
    )?;
    require_number(
        omen,
        expected_memory_lifecycle_events[0],
        "boot_drive",
        memory_boot_drive,
    )?;

    let memory_map = &lifecycle_records[1];
    require_memory_header(
        memory_map,
        expected_memory_lifecycle_events[1],
        &["regions", "usable_bytes"],
    )?;
    let region_count = required_number(memory_map, expected_memory_lifecycle_events[1], "regions")?;
    if !(1..=memory_maximum_regions).contains(&region_count) {
        return Err(format!(
            "event `{}` field `regions` is {region_count}; expected 1..={memory_maximum_regions}",
            expected_memory_lifecycle_events[1]
        ));
    }

    let boot_ready = &lifecycle_records[3];
    require_memory_header(
        boot_ready,
        expected_memory_lifecycle_events[3],
        &["control_abi", "framebuffer"],
    )?;
    require_string(
        boot_ready,
        expected_memory_lifecycle_events[3],
        "control_abi",
        memory_control_abi,
    )?;
    if record_bool(boot_ready, "framebuffer").is_none() {
        return Err(format!(
            "event `{}` has missing or invalid boolean field `framebuffer`",
            expected_memory_lifecycle_events[3]
        ));
    }

    let ready = memory_records[0];
    require_memory_header(
        ready,
        expected_memory_events[0],
        &[
            "frame_size",
            "address_limit",
            "total_frames",
            "free_frames",
            "unavailable_frames",
            "data_owned_frames",
            "table_owned_frames",
        ],
    )?;
    require_number(
        ready,
        expected_memory_events[0],
        "frame_size",
        memory_frame_size,
    )?;
    require_number(
        ready,
        expected_memory_events[0],
        "address_limit",
        memory_address_limit,
    )?;
    require_number(
        ready,
        expected_memory_events[0],
        "total_frames",
        memory_total_frames,
    )?;
    require_number(ready, expected_memory_events[0], "data_owned_frames", 0)?;
    require_number(ready, expected_memory_events[0], "table_owned_frames", 0)?;
    let initial_free_frames = required_number(ready, expected_memory_events[0], "free_frames")?;
    let unavailable_frames =
        required_number(ready, expected_memory_events[0], "unavailable_frames")?;
    if initial_free_frames == 0
        || initial_free_frames.checked_add(unavailable_frames) != Some(memory_total_frames)
    {
        return Err("kernel.memory.ready accounting does not cover the exact frame ledger".into());
    }
    let expected_usable_bytes = initial_free_frames
        .checked_mul(memory_frame_size)
        .ok_or_else(|| "kernel.memory.ready usable-byte accounting overflowed".to_string())?;
    require_number(
        memory_map,
        expected_memory_lifecycle_events[1],
        "usable_bytes",
        expected_usable_bytes,
    )?;

    let leases = memory_records[1];
    require_memory_header(
        leases,
        expected_memory_events[1],
        &[
            "data_a",
            "data_b",
            "root_a",
            "root_b",
            "data_frames",
            "table_frames",
            "zeroed",
            "distinct",
        ],
    )?;
    require_number(
        leases,
        expected_memory_events[1],
        "data_frames",
        memory_test_data_frames,
    )?;
    require_number(
        leases,
        expected_memory_events[1],
        "table_frames",
        memory_test_table_frames,
    )?;
    require_bool(leases, expected_memory_events[1], "zeroed", true)?;
    require_bool(leases, expected_memory_events[1], "distinct", true)?;
    let data_a = required_canonical_hex(leases, expected_memory_events[1], "data_a")?;
    let data_b = required_canonical_hex(leases, expected_memory_events[1], "data_b")?;
    let root_a = required_canonical_hex(leases, expected_memory_events[1], "root_a")?;
    let root_b = required_canonical_hex(leases, expected_memory_events[1], "root_b")?;
    let owned_extents = [
        (data_a, 1, "data_a"),
        (data_b, 1, "data_b"),
        (root_a, memory_test_root_frame_count, "root_a"),
        (root_b, memory_test_root_frame_count, "root_b"),
    ];
    validate_memory_owned_extents(&owned_extents, bounds)?;

    let roots = memory_records[2];
    require_memory_header(
        roots,
        expected_memory_events[2],
        &[
            "virtual_address",
            "writable",
            "executable",
            "supervisor",
            "exact_walk",
            "duplicate_binding_rejected",
            "overlap_rejected",
        ],
    )?;
    require_canonical_hex(
        roots,
        expected_memory_events[2],
        "virtual_address",
        memory_test_virtual_address,
    )?;
    require_bool(roots, expected_memory_events[2], "writable", true)?;
    require_bool(roots, expected_memory_events[2], "executable", false)?;
    require_bool(roots, expected_memory_events[2], "supervisor", true)?;
    require_bool(roots, expected_memory_events[2], "exact_walk", true)?;
    require_bool(
        roots,
        expected_memory_events[2],
        "duplicate_binding_rejected",
        true,
    )?;
    require_bool(roots, expected_memory_events[2], "overlap_rejected", true)?;

    let switched = memory_records[3];
    require_memory_header(
        switched,
        expected_memory_events[3],
        &[
            "sequence",
            "distinct",
            "marker_a",
            "marker_b",
            "active_root_rejected",
        ],
    )?;
    require_string(switched, expected_memory_events[3], "sequence", "a_b_a")?;
    require_bool(switched, expected_memory_events[3], "distinct", true)?;
    require_canonical_hex(
        switched,
        expected_memory_events[3],
        "marker_a",
        memory_test_marker_a,
    )?;
    require_canonical_hex(
        switched,
        expected_memory_events[3],
        "marker_b",
        memory_test_marker_b,
    )?;
    require_bool(
        switched,
        expected_memory_events[3],
        "active_root_rejected",
        true,
    )?;

    let withdrawn = memory_records[4];
    require_memory_header(
        withdrawn,
        expected_memory_events[4],
        &["kernel_cr3_restored", "mappings_removed", "roots_inactive"],
    )?;
    require_bool(
        withdrawn,
        expected_memory_events[4],
        "kernel_cr3_restored",
        true,
    )?;
    require_bool(
        withdrawn,
        expected_memory_events[4],
        "mappings_removed",
        true,
    )?;
    require_bool(withdrawn, expected_memory_events[4], "roots_inactive", true)?;

    let reused = memory_records[5];
    require_memory_header(
        reused,
        expected_memory_events[5],
        &["data_a", "data_b", "zeroed", "same_frames"],
    )?;
    require_canonical_hex(reused, expected_memory_events[5], "data_a", data_a)?;
    require_canonical_hex(reused, expected_memory_events[5], "data_b", data_b)?;
    require_bool(reused, expected_memory_events[5], "zeroed", true)?;
    require_bool(reused, expected_memory_events[5], "same_frames", true)?;

    let released = memory_records[6];
    require_memory_header(
        released,
        expected_memory_events[6],
        &[
            "free_frames",
            "initial_free_frames",
            "data_owned_frames",
            "table_owned_frames",
        ],
    )?;
    require_number(
        released,
        expected_memory_events[6],
        "free_frames",
        initial_free_frames,
    )?;
    require_number(
        released,
        expected_memory_events[6],
        "initial_free_frames",
        initial_free_frames,
    )?;
    require_number(released, expected_memory_events[6], "data_owned_frames", 0)?;
    require_number(released, expected_memory_events[6], "table_owned_frames", 0)?;

    let window = memory_records[7];
    require_memory_header(
        window,
        expected_memory_events[7],
        &[
            "base",
            "length",
            "supervisor",
            "writable",
            "executable",
            "bootstrap_root_unchanged",
        ],
    )?;
    require_canonical_hex(
        window,
        expected_memory_events[7],
        "base",
        0xffff_9000_0000_0000,
    )?;
    require_number(
        window,
        expected_memory_events[7],
        "length",
        memory_address_limit,
    )?;
    require_bool(window, expected_memory_events[7], "supervisor", true)?;
    require_bool(window, expected_memory_events[7], "writable", true)?;
    require_bool(window, expected_memory_events[7], "executable", false)?;
    require_bool(
        window,
        expected_memory_events[7],
        "bootstrap_root_unchanged",
        true,
    )?;

    let staged = memory_records[8];
    require_memory_header(
        staged,
        expected_memory_events[8],
        &[
            "regions",
            "data_frames",
            "table_frames",
            "low_code",
            "exact_walk",
            "guards_absent",
            "distinct_ownership",
            "inactive",
            "allocation_failures",
            "population_failures",
            "cleanup_failures",
            "retry_restored",
        ],
    )?;
    for (field, expected) in [
        ("regions", 4),
        ("data_frames", 7),
        ("table_frames", 9),
        ("allocation_failures", 5),
        ("population_failures", 1),
        ("cleanup_failures", 6),
    ] {
        require_number(staged, expected_memory_events[8], field, expected)?;
    }
    require_canonical_hex(staged, expected_memory_events[8], "low_code", 0x0040_0000)?;
    for field in [
        "exact_walk",
        "guards_absent",
        "distinct_ownership",
        "inactive",
        "retry_restored",
    ] {
        require_bool(staged, expected_memory_events[8], field, true)?;
    }

    let staged_released = memory_records[9];
    require_memory_header(
        staged_released,
        expected_memory_events[9],
        &[
            "free_frames",
            "initial_free_frames",
            "data_owned_frames",
            "table_owned_frames",
            "active_leases",
            "bound_leases",
            "window_removed",
            "kernel_cr3_restored",
        ],
    )?;
    for field in ["free_frames", "initial_free_frames"] {
        require_number(
            staged_released,
            expected_memory_events[9],
            field,
            initial_free_frames,
        )?;
    }
    for field in [
        "data_owned_frames",
        "table_owned_frames",
        "active_leases",
        "bound_leases",
    ] {
        require_number(staged_released, expected_memory_events[9], field, 0)?;
    }
    require_bool(
        staged_released,
        expected_memory_events[9],
        "window_removed",
        true,
    )?;
    require_bool(
        staged_released,
        expected_memory_events[9],
        "kernel_cr3_restored",
        true,
    )?;

    let resident_ready = memory_records[10];
    require_memory_header(
        resident_ready,
        expected_memory_events[10],
        &[
            "roots",
            "user_regions",
            "data_frames",
            "table_frames",
            "probe_frames",
            "active_leases",
            "bound_leases",
            "kernel_permissions_checked",
            "descriptor_residency_checked",
            "supervisor_stack_checked",
        ],
    )?;
    for (field, expected) in [
        ("roots", 2),
        ("user_regions", 4),
        ("data_frames", 14),
        ("table_frames", 20),
        ("probe_frames", 1),
        ("active_leases", 11),
        ("bound_leases", 10),
    ] {
        require_number(resident_ready, expected_memory_events[10], field, expected)?;
    }
    for field in [
        "kernel_permissions_checked",
        "descriptor_residency_checked",
        "supervisor_stack_checked",
    ] {
        require_bool(resident_ready, expected_memory_events[10], field, true)?;
    }

    let resident_verified = memory_records[11];
    require_memory_header(
        resident_verified,
        expected_memory_events[11],
        &[
            "switches",
            "low_code",
            "physical_collision",
            "isolated_values",
            "physical_alias_preserved",
            "read_only_faults",
            "guard_faults",
            "no_execute_faults",
            "exact_fault_resume",
            "accessed_dirty_validated",
            "bootstrap_restored",
        ],
    )?;
    for (field, expected) in [
        ("switches", 4),
        ("read_only_faults", 2),
        ("guard_faults", 2),
        ("no_execute_faults", 2),
    ] {
        require_number(
            resident_verified,
            expected_memory_events[11],
            field,
            expected,
        )?;
    }
    for field in ["low_code", "physical_collision"] {
        require_canonical_hex(
            resident_verified,
            expected_memory_events[11],
            field,
            0x0040_0000,
        )?;
    }
    for field in [
        "isolated_values",
        "physical_alias_preserved",
        "exact_fault_resume",
        "accessed_dirty_validated",
        "bootstrap_restored",
    ] {
        require_bool(resident_verified, expected_memory_events[11], field, true)?;
    }

    let resident_released = memory_records[12];
    require_memory_header(
        resident_released,
        expected_memory_events[12],
        &[
            "free_frames",
            "initial_free_frames",
            "data_owned_frames",
            "table_owned_frames",
            "active_leases",
            "bound_leases",
            "window_removed",
            "kernel_cr3_restored",
        ],
    )?;
    for field in ["free_frames", "initial_free_frames"] {
        require_number(
            resident_released,
            expected_memory_events[12],
            field,
            initial_free_frames,
        )?;
    }
    for field in [
        "data_owned_frames",
        "table_owned_frames",
        "active_leases",
        "bound_leases",
    ] {
        require_number(resident_released, expected_memory_events[12], field, 0)?;
    }
    for field in ["window_removed", "kernel_cr3_restored"] {
        require_bool(resident_released, expected_memory_events[12], field, true)?;
    }

    let user_ready = memory_records[13];
    require_memory_header(
        user_ready,
        expected_memory_events[13],
        &[
            "roots",
            "tasks",
            "circles",
            "regions_per_root",
            "data_frames",
            "table_frames",
            "active_leases",
            "bound_leases",
            "code_base",
            "guarded_stack",
            "entry_admitted",
            "privilege_stack_resident",
            "gpr_only",
        ],
    )?;
    for (field, expected) in [
        ("roots", 2),
        ("tasks", 2),
        ("circles", 2),
        ("regions_per_root", 3),
        ("data_frames", 10),
        ("table_frames", 18),
        ("active_leases", 8),
        ("bound_leases", 8),
    ] {
        require_number(user_ready, expected_memory_events[13], field, expected)?;
    }
    require_canonical_hex(
        user_ready,
        expected_memory_events[13],
        "code_base",
        0x0040_0000,
    )?;
    for field in [
        "guarded_stack",
        "entry_admitted",
        "privilege_stack_resident",
        "gpr_only",
    ] {
        require_bool(user_ready, expected_memory_events[13], field, true)?;
    }

    let user_verified = memory_records[14];
    require_memory_header(
        user_verified,
        expected_memory_events[14],
        &[
            "entries",
            "resumed_reports",
            "root_switches",
            "faulted_tasks",
            "exited_tasks",
            "supervisor_read_denied",
            "peer_continued",
            "private_data_isolated",
            "noninitial_stack_return",
            "bootstrap_restored",
            "ts_restored",
            "tss_restored",
            "continuations_cleared",
        ],
    )?;
    for (field, expected) in [
        ("entries", 2),
        ("resumed_reports", 4),
        ("root_switches", 3),
        ("faulted_tasks", 1),
        ("exited_tasks", 1),
    ] {
        require_number(user_verified, expected_memory_events[14], field, expected)?;
    }
    for field in [
        "supervisor_read_denied",
        "peer_continued",
        "private_data_isolated",
        "noninitial_stack_return",
        "bootstrap_restored",
        "ts_restored",
        "tss_restored",
        "continuations_cleared",
    ] {
        require_bool(user_verified, expected_memory_events[14], field, true)?;
    }

    let user_released = memory_records[15];
    require_memory_header(
        user_released,
        expected_memory_events[15],
        &[
            "free_frames",
            "initial_free_frames",
            "data_owned_frames",
            "table_owned_frames",
            "active_leases",
            "bound_leases",
            "active_tasks",
            "active_circles",
            "attached_contexts",
            "generations_retired",
            "window_removed",
            "kernel_cr3_restored",
        ],
    )?;
    for field in ["free_frames", "initial_free_frames"] {
        require_number(
            user_released,
            expected_memory_events[15],
            field,
            initial_free_frames,
        )?;
    }
    for field in [
        "data_owned_frames",
        "table_owned_frames",
        "active_leases",
        "bound_leases",
        "active_tasks",
        "active_circles",
        "attached_contexts",
    ] {
        require_number(user_released, expected_memory_events[15], field, 0)?;
    }
    for field in [
        "generations_retired",
        "window_removed",
        "kernel_cr3_restored",
    ] {
        require_bool(user_released, expected_memory_events[15], field, true)?;
    }

    let launch = memory_records[16];
    let launch_event = expected_memory_events[16];
    require_memory_header(
        launch,
        launch_event,
        &[
            "allocation_failures",
            "table_write_failures",
            "payload_failures",
            "circle_failures",
            "prepared_cancellations",
            "unstarted_cancellations",
            "cleanup_failures",
            "cleanup_retries",
            "successful_commits",
            "prepared_without_tasks",
            "cancelled_before_cleanup",
            "commit_before_entry",
            "retry_restored",
        ],
    )?;
    for (field, expected) in [
        ("allocation_failures", 4),
        ("table_write_failures", 1),
        ("payload_failures", 1),
        ("circle_failures", 1),
        ("prepared_cancellations", 1),
        ("unstarted_cancellations", 5),
        ("cleanup_failures", 5),
        ("cleanup_retries", 5),
        ("successful_commits", 2),
    ] {
        require_number(launch, launch_event, field, expected)?;
    }
    for field in [
        "prepared_without_tasks",
        "cancelled_before_cleanup",
        "commit_before_entry",
        "retry_restored",
    ] {
        require_bool(launch, launch_event, field, true)?;
    }

    let passed = memory_records[17];
    require_memory_header(
        passed,
        expected_memory_events[17],
        &["roots", "switches", "reused_frames"],
    )?;
    require_number(passed, expected_memory_events[17], "roots", 2)?;
    require_number(passed, expected_memory_events[17], "switches", 3)?;
    require_number(
        passed,
        expected_memory_events[17],
        "reused_frames",
        memory_test_data_frames,
    )?;

    Ok(memory_run_evidence {
        total_frames: memory_total_frames,
        initial_free_frames,
        unavailable_frames,
        data_a,
        data_b,
        root_a,
        root_b,
    })
}

fn require_memory_header(
    record: &structured_record,
    event: &str,
    event_fields: &[&str],
) -> Result<(), String> {
    let expected_count = event_fields.len() + 3;
    if record.len() != expected_count
        || !record.contains_key("schema")
        || !record.contains_key("event")
        || !record.contains_key("level")
        || event_fields
            .iter()
            .any(|field| !record.contains_key(*field))
    {
        let actual = record.keys().map(String::as_str).collect::<Vec<_>>();
        let mut expected = vec!["schema", "event", "level"];
        expected.extend_from_slice(event_fields);
        expected.sort_unstable();
        return Err(format!(
            "event `{event}` has fields {actual:?}; expected exactly {expected:?}"
        ));
    }
    require_number(record, event, "schema", 1)?;
    require_string(record, event, "event", event)?;
    require_string(record, event, "level", "info")
}

fn required_number(record: &structured_record, event: &str, field: &str) -> Result<u64, String> {
    match record.get(field) {
        Some(json_scalar::number(value)) => value
            .parse()
            .map_err(|_| format!("event `{event}` has invalid unsigned numeric field `{field}`")),
        _ => Err(format!(
            "event `{event}` has missing or non-numeric field `{field}`"
        )),
    }
}

fn require_number(
    record: &structured_record,
    event: &str,
    field: &str,
    expected: u64,
) -> Result<(), String> {
    let actual = required_number(record, event, field)?;
    if actual == expected {
        Ok(())
    } else {
        Err(format!(
            "event `{event}` field `{field}` is {actual}; expected {expected}"
        ))
    }
}

fn required_canonical_hex(
    record: &structured_record,
    event: &str,
    field: &str,
) -> Result<u64, String> {
    let value = required_string(record, event, field)?;
    let digits = value
        .strip_prefix("0x")
        .ok_or_else(|| format!("event `{event}` field `{field}` is not 0x-prefixed hexadecimal"))?;
    if digits.len() != 16
        || !digits
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(format!(
            "event `{event}` field `{field}` is not 16-digit lowercase hexadecimal"
        ));
    }
    u64::from_str_radix(digits, 16)
        .map_err(|error| format!("parse event `{event}` field `{field}`: {error}"))
}

fn require_canonical_hex(
    record: &structured_record,
    event: &str,
    field: &str,
    expected: u64,
) -> Result<(), String> {
    let actual = required_canonical_hex(record, event, field)?;
    if actual == expected {
        Ok(())
    } else {
        Err(format!(
            "event `{event}` field `{field}` is {actual:#018x}; expected {expected:#018x}"
        ))
    }
}

fn require_canonical_selector(
    record: &structured_record,
    event: &str,
    field: &str,
    expected: u64,
) -> Result<(), String> {
    let value = required_string(record, event, field)?;
    let digits = value
        .strip_prefix("0x")
        .ok_or_else(|| format!("event `{event}` field `{field}` is not 0x-prefixed hexadecimal"))?;
    if digits.len() != 4
        || !digits
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(format!(
            "event `{event}` field `{field}` is not 4-digit lowercase hexadecimal"
        ));
    }
    let actual = u64::from_str_radix(digits, 16)
        .map_err(|error| format!("parse event `{event}` field `{field}`: {error}"))?;
    if actual == expected {
        Ok(())
    } else {
        Err(format!(
            "event `{event}` field `{field}` is {actual:#06x}; expected {expected:#06x}"
        ))
    }
}

fn validate_memory_owned_extents(
    extents: &[(u64, u64, &str)],
    bounds: memory_image_bounds,
) -> Result<(), String> {
    let mut checked = Vec::new();
    for (start, frames, label) in extents {
        if *start < 1024 * 1024 || !start.is_multiple_of(memory_frame_size) || *frames == 0 {
            return Err(format!(
                "memory-test extent `{label}` begins at invalid address {start:#x}"
            ));
        }
        let end = frames
            .checked_mul(memory_frame_size)
            .and_then(|length| start.checked_add(length))
            .ok_or_else(|| format!("memory-test extent `{label}` overflows"))?;
        if end > memory_address_limit {
            return Err(format!(
                "memory-test extent `{label}` crosses the four-GiB limit"
            ));
        }
        if ranges_overlap(*start, end, bounds.kernel_start, bounds.kernel_end) {
            return Err(format!(
                "memory-test extent `{label}` overlaps the linked kernel"
            ));
        }
        checked.push((*start, end, *label));
    }
    for first in 0..checked.len() {
        for second in first + 1..checked.len() {
            if ranges_overlap(
                checked[first].0,
                checked[first].1,
                checked[second].0,
                checked[second].1,
            ) {
                return Err(format!(
                    "memory-test extents `{}` and `{}` overlap",
                    checked[first].2, checked[second].2
                ));
            }
        }
    }
    Ok(())
}

fn ranges_overlap(first_start: u64, first_end: u64, second_start: u64, second_end: u64) -> bool {
    first_start < second_end && second_start < first_end
}

fn verify_input_wait_smoke_output(status: ExitStatus, transcript: &str) -> Result<(), String> {
    if status.code() != Some(qemu_success_status) {
        return Err(format!(
            "QEMU did not report input-wait-test success (process status: {:?})",
            status.code()
        ));
    }
    verify_input_wait_transcript(transcript)
}

fn verify_input_wait_transcript(transcript: &str) -> Result<(), String> {
    let lowercase = transcript.to_ascii_lowercase();
    let normalized = lowercase.replace(['_', ' ', '-'], "");
    for private in [
        "☃",
        r"\u2603",
        "e29883",
        "101112131415161718191a1b1c1d1e1f",
        "202122232425262728292a2b2c2d2e2f",
        "303132333435363738393a3b3c3d3e3f",
    ] {
        if lowercase.contains(private) || normalized.contains(private) {
            return Err("input-wait transcript exposed private fixture content or identity".into());
        }
    }

    let uses_crlf = transcript
        .as_bytes()
        .windows(2)
        .any(|bytes| bytes == b"\r\n");
    for (index, byte) in transcript.as_bytes().iter().copied().enumerate() {
        if byte == b'\r' && transcript.as_bytes().get(index + 1).copied() != Some(b'\n') {
            return Err("input-wait transcript contains a bare carriage return".into());
        }
        if byte == b'\n'
            && uses_crlf
            && index
                .checked_sub(1)
                .and_then(|prior| transcript.as_bytes().get(prior))
                .copied()
                != Some(b'\r')
        {
            return Err("input-wait transcript mixes LF and CRLF serial framing".into());
        }
    }

    let mut records = Vec::new();
    for (index, framed_line) in transcript.split_terminator('\n').enumerate() {
        let line = if uses_crlf {
            framed_line.strip_suffix('\r').unwrap_or(framed_line)
        } else {
            framed_line
        };
        if line.is_empty() {
            return Err(format!(
                "input-wait transcript contains an empty serial record on line {}",
                index + 1
            ));
        }
        if line.bytes().any(|byte| byte.is_ascii_whitespace()) {
            return Err(format!(
                "input-wait transcript contains noncanonical serial whitespace on line {}",
                index + 1
            ));
        }
        if !line.starts_with('{') {
            return Err(format!(
                "input-wait transcript contains unstructured serial content on line {}",
                index + 1
            ));
        }
        let record = json_parser::new(line).parse_record().map_err(|error| {
            format!(
                "malformed structured record on serial line {}: {error}",
                index + 1
            )
        })?;
        let event = record_string(&record, "event")
            .ok_or_else(|| format!("structured serial line {} has no event", index + 1))?;
        let level = record_string(&record, "level")
            .ok_or_else(|| format!("structured serial line {} has no level", index + 1))?;
        if matches!(level, "error" | "fatal")
            || event == input_wait_failed_event
            || event.ends_with(".failed")
        {
            return Err(format!(
                "input-wait transcript contains forbidden {level} event `{event}`"
            ));
        }
        if level != "info" {
            return Err(format!(
                "input-wait transcript contains non-info event `{event}`"
            ));
        }
        if record_u64(&record, "schema") != Some(1) {
            return Err(format!("event `{event}` does not declare schema 1"));
        }
        records.push(record);
    }

    let events = records
        .iter()
        .map(|record| record_string(record, "event").unwrap_or("<missing>"))
        .collect::<Vec<_>>();
    if events != expected_input_wait_serial_events {
        return Err(format!(
            "input-wait lifecycle is missing, duplicated, unknown, or out of order\nexpected: {expected_input_wait_serial_events:?}\nobserved: {events:?}"
        ));
    }

    validate_scheduler_boot_records(&records[..8])?;
    let input = &records[8..];
    validate_input_wait_ready(&input[0])?;
    validate_input_wait_immediate(&input[1])?;
    validate_input_wait_blocked(&input[2], "kernel.input_wait_test.wait.blocked")?;
    validate_input_wait_publication(&input[3])?;
    validate_input_wait_delivery(&input[4])?;
    validate_input_wait_blocked(&input[5], "kernel.input_wait_test.wait.blocked_again")?;
    validate_input_wait_close(&input[6])?;
    validate_input_wait_fault(&input[7])?;
    validate_input_wait_interruption(&input[8])?;
    validate_input_wait_released(&input[9])?;
    validate_input_wait_passed(&input[10])
}

fn require_input_wait_header(
    record: &structured_record,
    event: &str,
    event_fields: &[&str],
) -> Result<(), String> {
    if !event.starts_with(input_wait_event_prefix) {
        return Err(format!(
            "event `{event}` is outside the input-wait namespace"
        ));
    }
    require_control_header(record, event, event_fields)
}

fn validate_input_wait_ready(record: &structured_record) -> Result<(), String> {
    let event = "kernel.input_wait_test.ready";
    require_input_wait_header(
        record,
        event,
        &[
            "arch",
            "policy",
            "content_redacted",
            "nmi_masked",
            "bsp_only",
        ],
    )?;
    require_string(record, event, "arch", "x86_64")?;
    require_string(record, event, "policy", "fixed_session_input")?;
    require_bool(record, event, "content_redacted", true)?;
    require_bool(record, event, "nmi_masked", true)?;
    require_bool(record, event, "bsp_only", true)
}

fn validate_input_wait_immediate(record: &structured_record) -> Result<(), String> {
    let event = "kernel.input_wait_test.immediate.completed";
    require_input_wait_header(
        record,
        event,
        &[
            "body_copied",
            "source_consumed",
            "header_published_last",
            "content_redacted",
        ],
    )?;
    for field in [
        "body_copied",
        "source_consumed",
        "header_published_last",
        "content_redacted",
    ] {
        require_bool(record, event, field, true)?;
    }
    Ok(())
}

fn validate_input_wait_blocked(record: &structured_record, event: &str) -> Result<(), String> {
    require_input_wait_header(
        record,
        event,
        &[
            "pending_owned",
            "user_pointer_retained",
            "exact_reservation",
            "content_redacted",
        ],
    )?;
    require_bool(record, event, "pending_owned", true)?;
    require_bool(record, event, "user_pointer_retained", false)?;
    require_bool(record, event, "exact_reservation", true)?;
    require_bool(record, event, "content_redacted", true)
}

fn validate_input_wait_publication(record: &structured_record) -> Result<(), String> {
    let event = "kernel.input_wait_test.publication.woke";
    require_input_wait_header(
        record,
        event,
        &[
            "same_semantic_shape",
            "publisher_remained_current",
            "fifo_tail",
            "source_consumed",
            "exact_wake",
            "content_redacted",
        ],
    )?;
    for field in [
        "same_semantic_shape",
        "publisher_remained_current",
        "fifo_tail",
        "exact_wake",
        "content_redacted",
    ] {
        require_bool(record, event, field, true)?;
    }
    require_bool(record, event, "source_consumed", false)
}

fn validate_input_wait_delivery(record: &structured_record) -> Result<(), String> {
    let event = "kernel.input_wait_test.delivery.selected";
    require_input_wait_header(
        record,
        event,
        &[
            "reserved_selection",
            "body_copied",
            "source_consumed",
            "channel_settled",
            "header_published_last",
            "content_redacted",
        ],
    )?;
    for field in [
        "reserved_selection",
        "body_copied",
        "source_consumed",
        "channel_settled",
        "header_published_last",
        "content_redacted",
    ] {
        require_bool(record, event, field, true)?;
    }
    Ok(())
}

fn validate_input_wait_close(record: &structured_record) -> Result<(), String> {
    let event = "kernel.input_wait_test.route.closed";
    require_input_wait_header(
        record,
        event,
        &["route_queue_cleared", "exact_wake", "content_redacted"],
    )?;
    for field in ["route_queue_cleared", "exact_wake", "content_redacted"] {
        require_bool(record, event, field, true)?;
    }
    Ok(())
}

fn validate_input_wait_fault(record: &structured_record) -> Result<(), String> {
    let event = "kernel.input_wait_test.fault.selected";
    require_input_wait_header(
        record,
        event,
        &[
            "kind",
            "peer_terminal",
            "reserved_selection",
            "content_redacted",
        ],
    )?;
    require_string(record, event, "kind", "invalid_opcode")?;
    for field in ["peer_terminal", "reserved_selection", "content_redacted"] {
        require_bool(record, event, field, true)?;
    }
    Ok(())
}

fn validate_input_wait_interruption(record: &structured_record) -> Result<(), String> {
    let event = "kernel.input_wait_test.interruption.completed";
    require_input_wait_header(
        record,
        event,
        &[
            "payload_free",
            "channel_settled",
            "header_published_last",
            "content_redacted",
        ],
    )?;
    for field in [
        "payload_free",
        "channel_settled",
        "header_published_last",
        "content_redacted",
    ] {
        require_bool(record, event, field, true)?;
    }
    Ok(())
}

fn validate_input_wait_released(record: &structured_record) -> Result<(), String> {
    let event = "kernel.input_wait_test.released";
    let fields = [
        "frame_accounting_restored",
        "tasks_retired",
        "contexts_withdrawn",
        "roots_withdrawn",
        "private_storage_cleared",
        "channel_retired",
        "endpoint_retired",
        "scratch_wiped",
        "kernel_cr3_restored",
        "tss_rsp0_restored",
        "ts_restored",
        "content_redacted",
    ];
    require_input_wait_header(record, event, &fields)?;
    for field in fields {
        require_bool(record, event, field, true)?;
    }
    Ok(())
}

fn validate_input_wait_passed(record: &structured_record) -> Result<(), String> {
    let event = "kernel.input_wait_test.passed";
    let fields = [
        "lifecycle_complete",
        "exact_wake",
        "copy_before_consume",
        "payload_free_interruption",
        "kernel_continuation_restored",
        "content_redacted",
    ];
    require_input_wait_header(record, event, &fields)?;
    for field in fields {
        require_bool(record, event, field, true)?;
    }
    Ok(())
}

fn verify_scheduler_smoke_output(
    status: ExitStatus,
    transcript: &str,
    fault_address: scheduler_fault_address,
) -> Result<(), String> {
    if status.code() != Some(qemu_success_status) {
        return Err(format!(
            "QEMU did not report scheduler-test success (process status: {:?})",
            status.code()
        ));
    }
    verify_scheduler_transcript(transcript, fault_address)
}

fn verify_scheduler_transcript(
    transcript: &str,
    fault_address: scheduler_fault_address,
) -> Result<(), String> {
    let normalized_transcript = transcript.to_ascii_lowercase().replace('_', "");
    for marker in scheduler_private_markers {
        if normalized_transcript.contains(&format!("{marker:016x}")) {
            return Err("scheduler smoke transcript exposed a private fixture marker".into());
        }
    }

    let mut records = Vec::new();
    for (index, line) in transcript.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if !line.starts_with('{') {
            return Err(format!(
                "scheduler smoke transcript contains unstructured serial content on line {}",
                index + 1
            ));
        }
        let record = json_parser::new(line).parse_record().map_err(|error| {
            format!(
                "malformed structured record on serial line {}: {error}",
                index + 1
            )
        })?;
        let event = record_string(&record, "event")
            .ok_or_else(|| format!("structured serial line {} has no event", index + 1))?;
        let level = record_string(&record, "level")
            .ok_or_else(|| format!("structured serial line {} has no level", index + 1))?;
        if matches!(level, "error" | "fatal")
            || event == scheduler_failed_event
            || event.ends_with(".failed")
        {
            return Err(format!(
                "scheduler smoke transcript contains forbidden {level} event `{event}`"
            ));
        }
        if level != "info" {
            return Err(format!(
                "scheduler smoke transcript contains non-info event `{event}`"
            ));
        }
        if record_u64(&record, "schema") != Some(1) {
            return Err(format!("event `{event}` does not declare schema 1"));
        }
        if event.starts_with(scheduler_event_prefix) {
            for field in record.keys() {
                if scheduler_private_fields
                    .iter()
                    .any(|probe| field.contains(probe))
                {
                    return Err(format!(
                        "scheduler event `{event}` exposes forbidden field `{field}`"
                    ));
                }
            }
        }
        records.push(record);
    }

    let events = records
        .iter()
        .map(|record| record_string(record, "event").unwrap_or("<missing>"))
        .collect::<Vec<_>>();
    if events != expected_scheduler_serial_events {
        return Err(format!(
            "scheduler lifecycle is missing, duplicated, unknown, or out of order\nexpected: {expected_scheduler_serial_events:?}\nobserved: {events:?}"
        ));
    }

    validate_scheduler_boot_records(&records[..8])?;
    let scheduler = &records[8..];
    validate_scheduler_ready(&scheduler[0])?;
    validate_scheduler_task_ready(&scheduler[1], "faulting_fixture")?;
    validate_scheduler_task_ready(&scheduler[2], "peer_fixture")?;
    validate_scheduler_dispatch(&scheduler[3], "faulting_fixture", 0)?;
    let first_tick = validate_scheduler_preemption(&scheduler[4], "faulting_fixture", 0)?;
    validate_scheduler_dispatch(&scheduler[5], "peer_fixture", 1)?;
    let second_tick = validate_scheduler_preemption(&scheduler[6], "peer_fixture", 1)?;
    if first_tick == 0 || second_tick <= first_tick {
        return Err(format!(
            "scheduler PIT ticks are not strictly increasing: {first_tick}, {second_tick}"
        ));
    }
    validate_scheduler_resume(&scheduler[7], "faulting_fixture", 2, 0)?;
    validate_scheduler_invalid_opcode(&scheduler[8], fault_address)?;
    validate_scheduler_faulted(&scheduler[9])?;
    validate_scheduler_resume(&scheduler[10], "peer_fixture", 3, 1)?;
    validate_scheduler_completed(&scheduler[11])?;
    validate_scheduler_masked(&scheduler[12])?;
    validate_scheduler_withdrawn(&scheduler[13], "faulting_fixture")?;
    validate_scheduler_withdrawn(&scheduler[14], "peer_fixture")?;
    validate_scheduler_released(&records[4], &scheduler[15])?;
    validate_scheduler_passed(&scheduler[16])
}

fn validate_scheduler_boot_records(records: &[structured_record]) -> Result<(), String> {
    if records.len() != 8 {
        return Err("scheduler proof does not contain exactly eight boot prerequisites".into());
    }
    for record in &records[..7] {
        let event = required_string(record, "scheduler boot prerequisite", "event")?;
        validate_interrupt_serial_record(record, event)?;
    }
    let usable_bytes = required_number(&records[3], "boot.memory.map", "usable_bytes")?;
    let free_frames = required_number(&records[4], "kernel.memory.ready", "free_frames")?;
    if free_frames.checked_mul(memory_frame_size) != Some(usable_bytes) {
        return Err(
            "scheduler boot memory map does not match the allocator's exact free-frame count"
                .into(),
        );
    }
    validate_scheduler_interrupt_ready(&records[7])
}

fn validate_scheduler_interrupt_ready(record: &structured_record) -> Result<(), String> {
    let event = "kernel.interrupts.ready";
    require_scheduler_header(
        record,
        event,
        &[
            "backend",
            "vector_base",
            "vector_count",
            "pit_input_hz",
            "pit_divisor",
            "timebase_generation",
            "initial_master_mask",
            "initial_slave_mask",
            "master_mask",
            "slave_mask",
            "input_available",
            "input_source_generation",
            "input_queue_capacity",
            "nmi_masked",
            "if_enabled",
        ],
    )?;
    require_string(record, event, "backend", "legacy_pc")?;
    for (field, expected) in [
        ("vector_base", 32),
        ("vector_count", 16),
        ("pit_input_hz", 1_193_182),
        ("pit_divisor", 11_932),
        ("timebase_generation", 1),
        ("initial_master_mask", 255),
        ("initial_slave_mask", 255),
        ("master_mask", 252),
        ("slave_mask", 255),
        ("input_source_generation", 1),
        ("input_queue_capacity", 64),
    ] {
        require_number(record, event, field, expected)?;
    }
    require_bool(record, event, "input_available", true)?;
    require_bool(record, event, "nmi_masked", true)?;
    require_bool(record, event, "if_enabled", true)
}

fn validate_scheduler_ready(record: &structured_record) -> Result<(), String> {
    let event = "kernel.scheduler_test.ready";
    require_scheduler_header(
        record,
        event,
        &[
            "policy",
            "circles",
            "tasks",
            "queue_capacity",
            "preemption_budget",
            "timer_vector",
            "master_mask",
            "slave_mask",
            "privilege_stack_pages",
            "data_frames",
            "table_frames",
            "free_frame_delta",
            "ts_set",
            "fpu_state_saved",
            "nmi_masked",
            "bsp_only",
        ],
    )?;
    require_string(record, event, "policy", "fixed_round_robin")?;
    for (field, expected) in [
        ("circles", 2),
        ("tasks", 2),
        ("queue_capacity", 2),
        ("preemption_budget", 2),
        ("timer_vector", 32),
        ("master_mask", 254),
        ("slave_mask", 255),
        ("privilege_stack_pages", 4),
        ("data_frames", 14),
        ("table_frames", 8),
        ("free_frame_delta", 22),
    ] {
        require_number(record, event, field, expected)?;
    }
    require_bool(record, event, "ts_set", true)?;
    require_bool(record, event, "fpu_state_saved", false)?;
    require_bool(record, event, "nmi_masked", true)?;
    require_bool(record, event, "bsp_only", true)
}

fn validate_scheduler_task_ready(record: &structured_record, role: &str) -> Result<(), String> {
    let event = "kernel.scheduler_test.task.ready";
    require_scheduler_header(
        record,
        event,
        &[
            "role",
            "circle_generation",
            "task_generation",
            "state",
            "mappings",
            "privilege_stack_pages",
            "context_attached",
            "initial_if",
        ],
    )?;
    require_string(record, event, "role", role)?;
    require_number(record, event, "circle_generation", 1)?;
    require_number(record, event, "task_generation", 1)?;
    require_string(record, event, "state", "ready")?;
    require_number(record, event, "mappings", 3)?;
    require_number(record, event, "privilege_stack_pages", 4)?;
    require_bool(record, event, "context_attached", true)?;
    require_bool(record, event, "initial_if", false)
}

fn validate_scheduler_dispatch(
    record: &structured_record,
    role: &str,
    dispatch: u64,
) -> Result<(), String> {
    let event = "kernel.scheduler_test.task.dispatched";
    require_scheduler_header(
        record,
        event,
        &[
            "role",
            "dispatch",
            "kind",
            "state",
            "root_selected",
            "stack_selected",
            "initial_if",
        ],
    )?;
    require_string(record, event, "role", role)?;
    require_number(record, event, "dispatch", dispatch)?;
    require_string(record, event, "kind", "initial")?;
    require_string(record, event, "state", "running")?;
    require_bool(record, event, "root_selected", true)?;
    require_bool(record, event, "stack_selected", true)?;
    require_bool(record, event, "initial_if", false)
}

fn validate_scheduler_preemption(
    record: &structured_record,
    role: &str,
    preemption: u64,
) -> Result<u64, String> {
    let event = "kernel.scheduler_test.task.preempted";
    require_scheduler_header(
        record,
        event,
        &[
            "role",
            "preemption",
            "tick",
            "state",
            "frame_retained",
            "eoi_sent",
        ],
    )?;
    require_string(record, event, "role", role)?;
    require_number(record, event, "preemption", preemption)?;
    require_string(record, event, "state", "suspended")?;
    require_bool(record, event, "frame_retained", true)?;
    require_bool(record, event, "eoi_sent", true)?;
    required_number(record, event, "tick")
}

fn validate_scheduler_resume(
    record: &structured_record,
    role: &str,
    dispatch: u64,
    resume: u64,
) -> Result<(), String> {
    let event = "kernel.scheduler_test.task.resumed";
    require_scheduler_header(
        record,
        event,
        &[
            "role",
            "dispatch",
            "resume",
            "state",
            "context_preserved",
            "frame_owned",
        ],
    )?;
    require_string(record, event, "role", role)?;
    require_number(record, event, "dispatch", dispatch)?;
    require_number(record, event, "resume", resume)?;
    require_string(record, event, "state", "running")?;
    require_bool(record, event, "context_preserved", true)?;
    require_bool(record, event, "frame_owned", true)
}

fn validate_scheduler_invalid_opcode(
    record: &structured_record,
    fault_address: scheduler_fault_address,
) -> Result<(), String> {
    let event = "kernel.exception";
    require_scheduler_header(
        record,
        event,
        &[
            "vector",
            "kind",
            "error_code",
            "instruction_pointer",
            "stack_pointer",
            "stack_segment",
            "stack_switched",
            "code_segment",
            "flags",
            "origin",
            "disposition",
            "fault_address",
        ],
    )?;
    require_number(record, event, "vector", 6)?;
    require_string(record, event, "kind", "invalid_opcode")?;
    require_canonical_hex(record, event, "error_code", 0)?;
    require_canonical_hex(
        record,
        event,
        "instruction_pointer",
        fault_address.instruction_pointer,
    )?;
    require_canonical_hex(record, event, "stack_pointer", scheduler_user_stack_top)?;
    require_canonical_selector(record, event, "stack_segment", scheduler_user_data_selector)?;
    require_bool(record, event, "stack_switched", true)?;
    require_canonical_selector(record, event, "code_segment", scheduler_user_code_selector)?;
    let flags = required_canonical_hex(record, event, "flags")?;
    if flags != scheduler_user_flags && flags != scheduler_user_flags | scheduler_fault_resume_flag
    {
        return Err(format!(
            "event `{event}` field `flags` is {flags:#x}; expected IF plus optional RF"
        ));
    }
    require_string(record, event, "origin", "user")?;
    require_string(record, event, "disposition", "contained_test")?;
    match record.get("fault_address") {
        Some(json_scalar::null) => Ok(()),
        _ => Err(format!(
            "event `{event}` field `fault_address` must be null for #UD"
        )),
    }
}

fn validate_scheduler_faulted(record: &structured_record) -> Result<(), String> {
    let event = "kernel.scheduler_test.task.faulted";
    require_scheduler_header(
        record,
        event,
        &[
            "role",
            "state",
            "vector",
            "error_code",
            "contained",
            "frame_retained",
        ],
    )?;
    require_string(record, event, "role", "faulting_fixture")?;
    require_string(record, event, "state", "faulted")?;
    require_number(record, event, "vector", 6)?;
    require_number(record, event, "error_code", 0)?;
    require_bool(record, event, "contained", true)?;
    require_bool(record, event, "frame_retained", true)
}

fn validate_scheduler_completed(record: &structured_record) -> Result<(), String> {
    let event = "kernel.scheduler_test.task.completed";
    require_scheduler_header(
        record,
        event,
        &[
            "role",
            "state",
            "completion_gate",
            "peer_continued",
            "kernel_continuation_restored",
        ],
    )?;
    require_string(record, event, "role", "peer_fixture")?;
    require_string(record, event, "state", "exited")?;
    require_bool(record, event, "completion_gate", true)?;
    require_bool(record, event, "peer_continued", true)?;
    require_bool(record, event, "kernel_continuation_restored", true)
}

fn validate_scheduler_masked(record: &structured_record) -> Result<(), String> {
    let event = "kernel.scheduler_test.interrupts.masked";
    require_scheduler_header(record, event, &["master_mask", "slave_mask", "if_enabled"])?;
    require_number(record, event, "master_mask", 255)?;
    require_number(record, event, "slave_mask", 255)?;
    require_bool(record, event, "if_enabled", false)
}

fn validate_scheduler_withdrawn(record: &structured_record, role: &str) -> Result<(), String> {
    let event = "kernel.scheduler_test.task.withdrawn";
    require_scheduler_header(
        record,
        event,
        &[
            "role",
            "context_withdrawn",
            "root_withdrawn",
            "storage_wiped",
            "privilege_stack_wiped",
        ],
    )?;
    require_string(record, event, "role", role)?;
    require_bool(record, event, "context_withdrawn", true)?;
    require_bool(record, event, "root_withdrawn", true)?;
    require_bool(record, event, "storage_wiped", true)?;
    require_bool(record, event, "privilege_stack_wiped", true)
}

fn validate_scheduler_released(
    memory_ready: &structured_record,
    record: &structured_record,
) -> Result<(), String> {
    let event = "kernel.scheduler_test.released";
    require_scheduler_header(
        record,
        event,
        &[
            "free_frames",
            "initial_free_frames",
            "data_owned_frames",
            "table_owned_frames",
            "active_circles",
            "active_tasks",
            "active_mappings",
            "active_leases",
            "bound_leases",
            "scheduler_depth",
            "attached_contexts",
            "kernel_cr3_restored",
            "tss_rsp0_restored",
            "ts_restored",
        ],
    )?;
    let initial_free = required_number(record, event, "initial_free_frames")?;
    if initial_free < 22 {
        return Err("scheduler proof reported insufficient initial free frames".into());
    }
    require_number(
        memory_ready,
        "kernel.memory.ready",
        "free_frames",
        initial_free,
    )?;
    require_number(record, event, "free_frames", initial_free)?;
    for field in [
        "data_owned_frames",
        "table_owned_frames",
        "active_circles",
        "active_tasks",
        "active_mappings",
        "active_leases",
        "bound_leases",
        "scheduler_depth",
        "attached_contexts",
    ] {
        require_number(record, event, field, 0)?;
    }
    require_bool(record, event, "kernel_cr3_restored", true)?;
    require_bool(record, event, "tss_rsp0_restored", true)?;
    require_bool(record, event, "ts_restored", true)
}

fn validate_scheduler_passed(record: &structured_record) -> Result<(), String> {
    let event = "kernel.scheduler_test.passed";
    require_scheduler_header(
        record,
        event,
        &[
            "dispatches",
            "preemptions",
            "resumes",
            "faults",
            "completions",
            "peer_continued",
        ],
    )?;
    require_number(record, event, "dispatches", 4)?;
    require_number(record, event, "preemptions", 2)?;
    require_number(record, event, "resumes", 2)?;
    require_number(record, event, "faults", 1)?;
    require_number(record, event, "completions", 1)?;
    require_bool(record, event, "peer_continued", true)
}

fn require_scheduler_header(
    record: &structured_record,
    event: &str,
    event_fields: &[&str],
) -> Result<(), String> {
    require_control_header(record, event, event_fields)
}

fn verify_supervision_smoke_output(
    status: ExitStatus,
    transcript: &str,
    fault_address: supervision_fault_address,
) -> Result<(), String> {
    if status.code() != Some(qemu_success_status) {
        return Err(format!(
            "QEMU did not report supervision-test success (process status: {:?})",
            status.code()
        ));
    }
    verify_supervision_transcript(transcript, fault_address)
}

fn verify_supervision_transcript(
    transcript: &str,
    fault_address: supervision_fault_address,
) -> Result<(), String> {
    let normalized_transcript = transcript.to_ascii_lowercase().replace('_', "");
    for marker in supervision_private_markers {
        if normalized_transcript.contains(&format!("{marker:016x}")) {
            return Err("supervision smoke transcript exposed a private fixture marker".into());
        }
    }

    let mut records = Vec::new();
    for (index, line) in transcript.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if !line.starts_with('{') {
            return Err(format!(
                "supervision smoke transcript contains unstructured serial content on line {}",
                index + 1
            ));
        }
        let record = json_parser::new(line).parse_record().map_err(|error| {
            format!(
                "malformed structured record on serial line {}: {error}",
                index + 1
            )
        })?;
        let event = record_string(&record, "event")
            .ok_or_else(|| format!("structured serial line {} has no event", index + 1))?;
        let level = record_string(&record, "level")
            .ok_or_else(|| format!("structured serial line {} has no level", index + 1))?;
        if matches!(level, "error" | "fatal")
            || event == supervision_failed_event
            || event.ends_with(".failed")
        {
            return Err(format!(
                "supervision smoke transcript contains forbidden {level} event `{event}`"
            ));
        }
        if level != "info" {
            return Err(format!(
                "supervision smoke transcript contains non-info event `{event}`"
            ));
        }
        if record_u64(&record, "schema") != Some(1) {
            return Err(format!("event `{event}` does not declare schema 1"));
        }
        if event.starts_with(supervision_event_prefix) {
            for field in record.keys() {
                if supervision_private_fields
                    .iter()
                    .any(|probe| field.contains(probe))
                {
                    return Err(format!(
                        "supervision event `{event}` exposes forbidden field `{field}`"
                    ));
                }
            }
        }
        records.push(record);
    }

    let events = records
        .iter()
        .map(|record| record_string(record, "event").unwrap_or("<missing>"))
        .collect::<Vec<_>>();
    if events != expected_supervision_serial_events {
        return Err(format!(
            "supervision lifecycle is missing, duplicated, unknown, or out of order\nexpected: {expected_supervision_serial_events:?}\nobserved: {events:?}"
        ));
    }

    validate_scheduler_boot_records(&records[..8])?;
    let supervision = &records[8..];
    validate_supervision_ready(&supervision[0])?;
    validate_supervision_vessel_created(&supervision[1], 1, 1, false, 1)?;
    validate_supervision_supervisor_ready(&supervision[2])?;
    validate_supervision_dispatch(&supervision[3], "vessel_fixture", Some(1), 0)?;
    let tick = validate_supervision_preemption(&supervision[4])?;
    if tick == 0 {
        return Err("supervision PIT tick must be positive".into());
    }
    validate_supervision_dispatch(&supervision[5], "supervisor_fixture", None, 1)?;
    validate_supervision_blocked(&supervision[6], 0, "await_fault", false)?;
    validate_supervision_masked(&supervision[7])?;
    validate_supervision_resume(&supervision[8], "vessel_fixture", Some(1), 2, 0)?;
    validate_supervision_invalid_opcode(&supervision[9], fault_address)?;
    validate_supervision_faulted(&supervision[10])?;
    validate_supervision_resume(&supervision[11], "supervisor_fixture", None, 3, 1)?;
    validate_supervision_observation(&supervision[12], 1, 1, "invalid_instruction_fault")?;
    validate_supervision_restart_requested(&supervision[13])?;
    validate_supervision_vessel_withdrawn(&supervision[14], 1, "invalid_instruction_fault")?;
    validate_supervision_vessel_created(&supervision[15], 2, 2, true, 0)?;
    validate_supervision_dispatch(&supervision[16], "vessel_fixture", Some(2), 4)?;
    validate_supervision_completed(&supervision[17])?;
    validate_supervision_resume(&supervision[18], "supervisor_fixture", None, 5, 2)?;
    validate_supervision_observation(&supervision[19], 2, 2, "successful_exit")?;
    validate_supervision_supervisor_completed(&supervision[20])?;
    validate_supervision_vessel_withdrawn(&supervision[21], 2, "successful_exit")?;
    validate_supervision_supervisor_withdrawn(&supervision[22])?;
    validate_supervision_released(&records[4], &supervision[23])?;
    validate_supervision_passed(&supervision[24])
}

fn validate_supervision_ready(record: &structured_record) -> Result<(), String> {
    let event = "kernel.supervision_test.ready";
    require_supervision_header(
        record,
        event,
        &[
            "policy",
            "supervision_capacity",
            "relationships",
            "circles",
            "tasks",
            "queue_capacity",
            "restart_budget",
            "observation_capacity",
            "timer_vector",
            "master_mask",
            "slave_mask",
            "privilege_stack_pages",
            "data_frames",
            "table_frames",
            "free_frame_delta",
            "ts_set",
            "fpu_state_saved",
            "nmi_masked",
            "bsp_only",
        ],
    )?;
    require_string(record, event, "policy", "fixed_supervision")?;
    for (field, expected) in [
        ("supervision_capacity", 1),
        ("relationships", 1),
        ("circles", 2),
        ("tasks", 2),
        ("queue_capacity", 2),
        ("restart_budget", 1),
        ("observation_capacity", 1),
        ("timer_vector", 32),
        ("master_mask", 254),
        ("slave_mask", 255),
        ("privilege_stack_pages", 4),
        ("data_frames", 14),
        ("table_frames", 8),
        ("free_frame_delta", 22),
    ] {
        require_number(record, event, field, expected)?;
    }
    require_bool(record, event, "ts_set", true)?;
    require_bool(record, event, "fpu_state_saved", false)?;
    require_bool(record, event, "nmi_masked", true)?;
    require_bool(record, event, "bsp_only", true)
}

fn validate_supervision_vessel_created(
    record: &structured_record,
    incarnation: u64,
    identity_generation: u64,
    replacement: bool,
    restart_budget_remaining: u64,
) -> Result<(), String> {
    let event = "kernel.supervision_test.vessel.generation.created";
    require_supervision_header(
        record,
        event,
        &[
            "role",
            "supervision_generation",
            "incarnation",
            "circle_generation",
            "task_generation",
            "state",
            "mappings",
            "privilege_stack_pages",
            "context_attached",
            "storage_zero",
            "replacement",
            "restart_budget_remaining",
            "initial_if",
        ],
    )?;
    require_string(record, event, "role", "vessel_fixture")?;
    require_number(record, event, "supervision_generation", 1)?;
    require_number(record, event, "incarnation", incarnation)?;
    require_number(record, event, "circle_generation", identity_generation)?;
    require_number(record, event, "task_generation", identity_generation)?;
    require_string(record, event, "state", "ready")?;
    require_number(record, event, "mappings", 3)?;
    require_number(record, event, "privilege_stack_pages", 4)?;
    require_bool(record, event, "context_attached", true)?;
    require_bool(record, event, "storage_zero", true)?;
    require_bool(record, event, "replacement", replacement)?;
    require_number(
        record,
        event,
        "restart_budget_remaining",
        restart_budget_remaining,
    )?;
    require_bool(record, event, "initial_if", false)
}

fn validate_supervision_supervisor_ready(record: &structured_record) -> Result<(), String> {
    let event = "kernel.supervision_test.supervisor.ready";
    require_supervision_header(
        record,
        event,
        &[
            "role",
            "supervision_generation",
            "circle_generation",
            "task_generation",
            "state",
            "mappings",
            "privilege_stack_pages",
            "context_attached",
            "initial_if",
        ],
    )?;
    require_string(record, event, "role", "supervisor_fixture")?;
    require_number(record, event, "supervision_generation", 1)?;
    require_number(record, event, "circle_generation", 1)?;
    require_number(record, event, "task_generation", 1)?;
    require_string(record, event, "state", "ready")?;
    require_number(record, event, "mappings", 3)?;
    require_number(record, event, "privilege_stack_pages", 4)?;
    require_bool(record, event, "context_attached", true)?;
    require_bool(record, event, "initial_if", false)
}

fn validate_supervision_dispatch(
    record: &structured_record,
    role: &str,
    incarnation: Option<u64>,
    dispatch: u64,
) -> Result<(), String> {
    let event = "kernel.supervision_test.task.dispatched";
    let fields_with_incarnation = [
        "role",
        "incarnation",
        "dispatch",
        "kind",
        "state",
        "root_selected",
        "stack_selected",
        "initial_if",
    ];
    let fields_without_incarnation = [
        "role",
        "dispatch",
        "kind",
        "state",
        "root_selected",
        "stack_selected",
        "initial_if",
    ];
    require_supervision_header(
        record,
        event,
        if incarnation.is_some() {
            &fields_with_incarnation
        } else {
            &fields_without_incarnation
        },
    )?;
    require_string(record, event, "role", role)?;
    if let Some(incarnation) = incarnation {
        require_number(record, event, "incarnation", incarnation)?;
    }
    require_number(record, event, "dispatch", dispatch)?;
    require_string(record, event, "kind", "initial")?;
    require_string(record, event, "state", "running")?;
    require_bool(record, event, "root_selected", true)?;
    require_bool(record, event, "stack_selected", true)?;
    require_bool(record, event, "initial_if", false)
}

fn validate_supervision_preemption(record: &structured_record) -> Result<u64, String> {
    let event = "kernel.supervision_test.task.preempted";
    require_supervision_header(
        record,
        event,
        &[
            "role",
            "incarnation",
            "preemption",
            "tick",
            "state",
            "frame_retained",
            "eoi_sent",
        ],
    )?;
    require_string(record, event, "role", "vessel_fixture")?;
    require_number(record, event, "incarnation", 1)?;
    require_number(record, event, "preemption", 0)?;
    require_string(record, event, "state", "suspended")?;
    require_bool(record, event, "frame_retained", true)?;
    require_bool(record, event, "eoi_sent", true)?;
    required_number(record, event, "tick")
}

fn validate_supervision_blocked(
    record: &structured_record,
    block: u64,
    wait: &str,
    scheduler_empty: bool,
) -> Result<(), String> {
    let event = "kernel.supervision_test.supervisor.blocked";
    require_supervision_header(
        record,
        event,
        &[
            "role",
            "supervision_generation",
            "block",
            "wait",
            "state",
            "frame_retained",
            "vessel_resumed",
        ],
    )?;
    require_string(record, event, "role", "supervisor_fixture")?;
    require_number(record, event, "supervision_generation", 1)?;
    require_number(record, event, "block", block)?;
    require_string(record, event, "wait", wait)?;
    require_string(record, event, "state", "blocked")?;
    require_bool(record, event, "frame_retained", true)?;
    require_bool(record, event, "vessel_resumed", !scheduler_empty)
}

fn validate_supervision_masked(record: &structured_record) -> Result<(), String> {
    let event = "kernel.supervision_test.interrupts.masked";
    require_supervision_header(record, event, &["master_mask", "slave_mask", "if_enabled"])?;
    require_number(record, event, "master_mask", 255)?;
    require_number(record, event, "slave_mask", 255)?;
    require_bool(record, event, "if_enabled", false)
}

fn validate_supervision_resume(
    record: &structured_record,
    role: &str,
    incarnation: Option<u64>,
    dispatch: u64,
    resume: u64,
) -> Result<(), String> {
    let event = "kernel.supervision_test.task.resumed";
    let fields_with_incarnation = [
        "role",
        "incarnation",
        "dispatch",
        "resume",
        "state",
        "context_preserved",
        "frame_owned",
    ];
    let fields_without_incarnation = [
        "role",
        "dispatch",
        "resume",
        "state",
        "context_preserved",
        "frame_owned",
    ];
    require_supervision_header(
        record,
        event,
        if incarnation.is_some() {
            &fields_with_incarnation
        } else {
            &fields_without_incarnation
        },
    )?;
    require_string(record, event, "role", role)?;
    if let Some(incarnation) = incarnation {
        require_number(record, event, "incarnation", incarnation)?;
    }
    require_number(record, event, "dispatch", dispatch)?;
    require_number(record, event, "resume", resume)?;
    require_string(record, event, "state", "running")?;
    require_bool(record, event, "context_preserved", true)?;
    require_bool(record, event, "frame_owned", true)
}

fn validate_supervision_invalid_opcode(
    record: &structured_record,
    fault_address: supervision_fault_address,
) -> Result<(), String> {
    let event = "kernel.exception";
    require_supervision_header(
        record,
        event,
        &[
            "vector",
            "kind",
            "error_code",
            "instruction_pointer",
            "stack_pointer",
            "stack_segment",
            "stack_switched",
            "code_segment",
            "flags",
            "origin",
            "disposition",
            "fault_address",
        ],
    )?;
    require_number(record, event, "vector", 6)?;
    require_string(record, event, "kind", "invalid_opcode")?;
    require_canonical_hex(record, event, "error_code", 0)?;
    require_canonical_hex(
        record,
        event,
        "instruction_pointer",
        fault_address.instruction_pointer,
    )?;
    require_canonical_hex(record, event, "stack_pointer", supervision_user_stack_top)?;
    require_canonical_selector(
        record,
        event,
        "stack_segment",
        supervision_user_data_selector,
    )?;
    require_bool(record, event, "stack_switched", true)?;
    require_canonical_selector(
        record,
        event,
        "code_segment",
        supervision_user_code_selector,
    )?;
    let flags = required_canonical_hex(record, event, "flags")?;
    if flags != supervision_user_flags
        && flags != supervision_user_flags | supervision_fault_resume_flag
    {
        return Err(format!(
            "event `{event}` field `flags` is {flags:#x}; expected IF plus optional RF"
        ));
    }
    require_string(record, event, "origin", "user")?;
    require_string(record, event, "disposition", "contained_test")?;
    match record.get("fault_address") {
        Some(json_scalar::null) => Ok(()),
        _ => Err(format!(
            "event `{event}` field `fault_address` must be null for #UD"
        )),
    }
}

fn validate_supervision_faulted(record: &structured_record) -> Result<(), String> {
    let event = "kernel.supervision_test.vessel.generation.faulted";
    require_supervision_header(
        record,
        event,
        &[
            "role",
            "supervision_generation",
            "incarnation",
            "state",
            "outcome",
            "event_sequence",
            "vector",
            "error_code",
            "observation_published",
            "supervisor_woken",
            "frame_retained",
        ],
    )?;
    require_string(record, event, "role", "vessel_fixture")?;
    require_number(record, event, "supervision_generation", 1)?;
    require_number(record, event, "incarnation", 1)?;
    require_string(record, event, "state", "faulted")?;
    require_string(record, event, "outcome", "invalid_instruction_fault")?;
    require_number(record, event, "event_sequence", 1)?;
    require_number(record, event, "vector", 6)?;
    require_number(record, event, "error_code", 0)?;
    require_bool(record, event, "observation_published", true)?;
    require_bool(record, event, "supervisor_woken", true)?;
    require_bool(record, event, "frame_retained", true)
}

fn validate_supervision_observation(
    record: &structured_record,
    event_sequence: u64,
    incarnation: u64,
    outcome: &str,
) -> Result<(), String> {
    let event = "kernel.supervision_test.observation.consumed";
    require_supervision_header(
        record,
        event,
        &[
            "role",
            "supervision_generation",
            "event_sequence",
            "incarnation",
            "outcome",
            "single_use",
            "pending",
        ],
    )?;
    require_string(record, event, "role", "supervisor_fixture")?;
    require_number(record, event, "supervision_generation", 1)?;
    require_number(record, event, "event_sequence", event_sequence)?;
    require_number(record, event, "incarnation", incarnation)?;
    require_string(record, event, "outcome", outcome)?;
    require_bool(record, event, "single_use", true)?;
    require_bool(record, event, "pending", false)
}

fn validate_supervision_restart_requested(record: &structured_record) -> Result<(), String> {
    let event = "kernel.supervision_test.restart.requested";
    require_supervision_header(
        record,
        event,
        &[
            "role",
            "supervision_generation",
            "restart_sequence",
            "incarnation",
            "block",
            "wait",
            "state",
            "restart_budget_remaining",
            "scheduler_empty",
            "kernel_continuation_restored",
        ],
    )?;
    require_string(record, event, "role", "supervisor_fixture")?;
    require_number(record, event, "supervision_generation", 1)?;
    require_number(record, event, "restart_sequence", 1)?;
    require_number(record, event, "incarnation", 1)?;
    require_number(record, event, "block", 1)?;
    require_string(record, event, "wait", "restart_checkpoint")?;
    require_string(record, event, "state", "blocked")?;
    require_number(record, event, "restart_budget_remaining", 0)?;
    require_bool(record, event, "scheduler_empty", true)?;
    require_bool(record, event, "kernel_continuation_restored", true)
}

fn validate_supervision_vessel_withdrawn(
    record: &structured_record,
    incarnation: u64,
    outcome: &str,
) -> Result<(), String> {
    let event = "kernel.supervision_test.vessel.generation.withdrawn";
    require_supervision_header(
        record,
        event,
        &[
            "role",
            "incarnation",
            "circle_generation",
            "task_generation",
            "outcome",
            "context_withdrawn",
            "root_withdrawn",
            "storage_wiped",
            "privilege_stack_wiped",
            "task_identity_stale",
            "circle_identity_stale",
        ],
    )?;
    require_string(record, event, "role", "vessel_fixture")?;
    require_number(record, event, "incarnation", incarnation)?;
    require_number(record, event, "circle_generation", incarnation)?;
    require_number(record, event, "task_generation", incarnation)?;
    require_string(record, event, "outcome", outcome)?;
    require_bool(record, event, "context_withdrawn", true)?;
    require_bool(record, event, "root_withdrawn", true)?;
    require_bool(record, event, "storage_wiped", true)?;
    require_bool(record, event, "privilege_stack_wiped", true)?;
    require_bool(record, event, "task_identity_stale", true)?;
    require_bool(record, event, "circle_identity_stale", true)
}

fn validate_supervision_completed(record: &structured_record) -> Result<(), String> {
    let event = "kernel.supervision_test.vessel.generation.completed";
    require_supervision_header(
        record,
        event,
        &[
            "role",
            "supervision_generation",
            "incarnation",
            "state",
            "outcome",
            "event_sequence",
            "completion_gate",
            "observation_published",
            "supervisor_woken",
        ],
    )?;
    require_string(record, event, "role", "vessel_fixture")?;
    require_number(record, event, "supervision_generation", 1)?;
    require_number(record, event, "incarnation", 2)?;
    require_string(record, event, "state", "exited")?;
    require_string(record, event, "outcome", "successful_exit")?;
    require_number(record, event, "event_sequence", 2)?;
    require_bool(record, event, "completion_gate", true)?;
    require_bool(record, event, "observation_published", true)?;
    require_bool(record, event, "supervisor_woken", true)
}

fn validate_supervision_supervisor_completed(record: &structured_record) -> Result<(), String> {
    let event = "kernel.supervision_test.supervisor.completed";
    require_supervision_header(
        record,
        event,
        &[
            "role",
            "state",
            "completion_gate",
            "vessel_completed",
            "kernel_continuation_restored",
        ],
    )?;
    require_string(record, event, "role", "supervisor_fixture")?;
    require_string(record, event, "state", "exited")?;
    require_bool(record, event, "completion_gate", true)?;
    require_bool(record, event, "vessel_completed", true)?;
    require_bool(record, event, "kernel_continuation_restored", true)
}

fn validate_supervision_supervisor_withdrawn(record: &structured_record) -> Result<(), String> {
    let event = "kernel.supervision_test.supervisor.withdrawn";
    require_supervision_header(
        record,
        event,
        &[
            "role",
            "context_withdrawn",
            "root_withdrawn",
            "storage_wiped",
            "privilege_stack_wiped",
            "relationship_detached",
        ],
    )?;
    require_string(record, event, "role", "supervisor_fixture")?;
    require_bool(record, event, "context_withdrawn", true)?;
    require_bool(record, event, "root_withdrawn", true)?;
    require_bool(record, event, "storage_wiped", true)?;
    require_bool(record, event, "privilege_stack_wiped", true)?;
    require_bool(record, event, "relationship_detached", true)
}

fn validate_supervision_released(
    memory_ready: &structured_record,
    record: &structured_record,
) -> Result<(), String> {
    let event = "kernel.supervision_test.released";
    require_supervision_header(
        record,
        event,
        &[
            "free_frames",
            "initial_free_frames",
            "data_owned_frames",
            "table_owned_frames",
            "active_circles",
            "active_tasks",
            "active_mappings",
            "active_leases",
            "bound_leases",
            "scheduler_depth",
            "attached_contexts",
            "active_endpoints",
            "active_sigils",
            "active_bindings",
            "active_lineages",
            "active_supervisions",
            "supervision_children",
            "observation_cleared",
            "wait_cleared",
            "relationship_cleared",
            "kernel_cr3_restored",
            "tss_rsp0_restored",
            "ts_restored",
        ],
    )?;
    let initial_free = required_number(record, event, "initial_free_frames")?;
    if initial_free < 22 {
        return Err("supervision proof reported insufficient initial free frames".into());
    }
    require_number(
        memory_ready,
        "kernel.memory.ready",
        "free_frames",
        initial_free,
    )?;
    require_number(record, event, "free_frames", initial_free)?;
    for field in [
        "data_owned_frames",
        "table_owned_frames",
        "active_circles",
        "active_tasks",
        "active_mappings",
        "active_leases",
        "bound_leases",
        "scheduler_depth",
        "attached_contexts",
        "active_endpoints",
        "active_sigils",
        "active_bindings",
        "active_lineages",
        "active_supervisions",
        "supervision_children",
    ] {
        require_number(record, event, field, 0)?;
    }
    require_bool(record, event, "observation_cleared", true)?;
    require_bool(record, event, "wait_cleared", true)?;
    require_bool(record, event, "relationship_cleared", true)?;
    require_bool(record, event, "kernel_cr3_restored", true)?;
    require_bool(record, event, "tss_rsp0_restored", true)?;
    require_bool(record, event, "ts_restored", true)
}

fn validate_supervision_passed(record: &structured_record) -> Result<(), String> {
    let event = "kernel.supervision_test.passed";
    require_supervision_header(
        record,
        event,
        &[
            "dispatches",
            "preemptions",
            "resumes",
            "blocks",
            "wakes",
            "observations",
            "faults",
            "restarts",
            "completions",
            "clean_replacement",
            "supervisor_continued",
        ],
    )?;
    for (field, expected) in [
        ("dispatches", 6),
        ("preemptions", 1),
        ("resumes", 3),
        ("blocks", 2),
        ("wakes", 2),
        ("observations", 2),
        ("faults", 1),
        ("restarts", 1),
        ("completions", 1),
    ] {
        require_number(record, event, field, expected)?;
    }
    require_bool(record, event, "clean_replacement", true)?;
    require_bool(record, event, "supervisor_continued", true)
}

fn require_supervision_header(
    record: &structured_record,
    event: &str,
    event_fields: &[&str],
) -> Result<(), String> {
    require_control_header(record, event, event_fields)
}

fn verify_shell_vessel_smoke_output(
    status: ExitStatus,
    transcript: &str,
    fault_address: shell_vessel_fault_address,
) -> Result<(), String> {
    if status.code() != Some(qemu_success_status) {
        return Err(format!(
            "QEMU did not report shell-vessel-test success (process status: {:?})",
            status.code()
        ));
    }
    verify_shell_vessel_transcript(transcript, fault_address)
}

fn verify_shell_vessel_transcript(
    transcript: &str,
    fault_address: shell_vessel_fault_address,
) -> Result<(), String> {
    let lowercase_transcript = transcript.to_ascii_lowercase();
    for private in [shell_vessel_private_output, shell_vessel_private_source] {
        let private = core::str::from_utf8(private)
            .map_err(|_| "shell-vessel private probe is not UTF-8".to_string())?;
        if lowercase_transcript.contains(&private.to_ascii_lowercase()) {
            return Err("shell-vessel transcript exposed private source or output".into());
        }
    }
    let normalized_transcript = lowercase_transcript.replace('_', "");
    for marker in shell_vessel_private_markers {
        if normalized_transcript.contains(&format!("{marker:016x}")) {
            return Err("shell-vessel transcript exposed a private fixture marker".into());
        }
    }

    let mut records = Vec::new();
    for (index, line) in transcript.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if !line.starts_with('{') {
            return Err(format!(
                "shell-vessel transcript contains unstructured serial content on line {}",
                index + 1
            ));
        }
        let record = json_parser::new(line).parse_record().map_err(|error| {
            format!(
                "malformed structured record on serial line {}: {error}",
                index + 1
            )
        })?;
        let event = record_string(&record, "event")
            .ok_or_else(|| format!("structured serial line {} has no event", index + 1))?;
        let level = record_string(&record, "level")
            .ok_or_else(|| format!("structured serial line {} has no level", index + 1))?;
        if matches!(level, "error" | "fatal")
            || event == shell_vessel_failed_event
            || event.ends_with(".failed")
        {
            return Err(format!(
                "shell-vessel transcript contains forbidden {level} event `{event}`"
            ));
        }
        if level != "info" {
            return Err(format!(
                "shell-vessel transcript contains non-info event `{event}`"
            ));
        }
        if record_u64(&record, "schema") != Some(1) {
            return Err(format!("event `{event}` does not declare schema 1"));
        }
        if event.starts_with(shell_vessel_event_prefix) {
            for field in record.keys() {
                if shell_vessel_field_is_private(field) {
                    return Err(format!(
                        "shell-vessel event `{event}` exposes forbidden field `{field}`"
                    ));
                }
            }
        }
        records.push(record);
    }

    let events = records
        .iter()
        .map(|record| record_string(record, "event").unwrap_or("<missing>"))
        .collect::<Vec<_>>();
    if events != expected_shell_vessel_serial_events {
        return Err(format!(
            "shell-vessel lifecycle is missing, duplicated, unknown, or out of order\nexpected: {expected_shell_vessel_serial_events:?}\nobserved: {events:?}"
        ));
    }

    validate_scheduler_boot_records(&records[..8])?;
    let vessel = &records[8..];
    validate_shell_vessel_ready(&vessel[0])?;
    validate_shell_vessel_created(&vessel[1], 1, 1, false, 1)?;
    validate_shell_vessel_supervisor_ready(&vessel[2])?;
    validate_shell_vessel_dispatch(&vessel[3], "shell_vessel_fixture", Some(1), 0)?;
    validate_shell_vessel_console(&vessel[4], 1, "fixed_human_source", false)?;
    validate_shell_vessel_input_empty(&vessel[5], 1)?;
    let tick = validate_shell_vessel_preemption(&vessel[6])?;
    if tick == 0 {
        return Err("shell-vessel PIT tick must be positive".into());
    }
    validate_shell_vessel_dispatch(&vessel[7], "supervisor_fixture", None, 1)?;
    validate_shell_vessel_blocked(&vessel[8])?;
    validate_shell_vessel_masked(&vessel[9])?;
    validate_shell_vessel_resume(&vessel[10], "shell_vessel_fixture", Some(1), 2, 0)?;
    validate_shell_vessel_invalid_opcode(&vessel[11], fault_address)?;
    validate_shell_vessel_faulted(&vessel[12])?;
    validate_shell_vessel_resume(&vessel[13], "supervisor_fixture", None, 3, 1)?;
    validate_shell_vessel_observation(&vessel[14], 1, 1, "invalid_instruction_fault")?;
    validate_shell_vessel_restart(&vessel[15])?;
    validate_shell_vessel_withdrawn(&vessel[16], 1, "invalid_instruction_fault")?;
    validate_shell_vessel_created(&vessel[17], 2, 2, true, 0)?;
    validate_shell_vessel_dispatch(&vessel[18], "shell_vessel_fixture", Some(2), 4)?;
    validate_shell_vessel_console(&vessel[19], 2, "machine_direct", true)?;
    validate_shell_vessel_input_empty(&vessel[20], 2)?;
    validate_shell_vessel_task_exit(&vessel[21])?;
    validate_shell_vessel_completed(&vessel[22])?;
    validate_shell_vessel_resume(&vessel[23], "supervisor_fixture", None, 5, 2)?;
    validate_shell_vessel_observation(&vessel[24], 2, 2, "successful_exit")?;
    validate_shell_vessel_supervisor_completed(&vessel[25])?;
    validate_shell_vessel_withdrawn(&vessel[26], 2, "successful_exit")?;
    validate_shell_vessel_supervisor_withdrawn(&vessel[27])?;
    validate_shell_vessel_released(&records[4], &vessel[28])?;
    validate_shell_vessel_passed(&vessel[29])
}

fn shell_vessel_field_is_private(field: &str) -> bool {
    shell_vessel_private_fields.iter().any(|probe| {
        if *probe == "text" {
            field.split('_').any(|component| component == *probe)
        } else {
            field.contains(probe)
        }
    })
}

fn validate_shell_vessel_ready(record: &structured_record) -> Result<(), String> {
    let event = "kernel.shell_vessel_test.ready";
    require_shell_vessel_header(
        record,
        event,
        &[
            "arch",
            "policy",
            "supervision_capacity",
            "relationships",
            "circles",
            "tasks",
            "queue_capacity",
            "restart_budget",
            "observation_capacity",
            "scheduler_gate_vector",
            "rite_gate_vector",
            "rite_grant",
            "transport_limit",
            "timer_vector",
            "master_mask",
            "slave_mask",
            "privilege_stack_pages",
            "data_frames",
            "table_frames",
            "free_frame_delta",
            "ts_set",
            "fpu_state_saved",
            "nmi_masked",
            "bsp_only",
        ],
    )?;
    require_string(record, event, "arch", "x86_64")?;
    require_string(record, event, "policy", "fixed_supervised_shell")?;
    for (field, expected) in [
        ("supervision_capacity", 1),
        ("relationships", 1),
        ("circles", 2),
        ("tasks", 2),
        ("queue_capacity", 2),
        ("restart_budget", 1),
        ("observation_capacity", 1),
        ("scheduler_gate_vector", 242),
        ("rite_gate_vector", 243),
        ("rite_grant", 7),
        ("transport_limit", 4056),
        ("timer_vector", 32),
        ("master_mask", 254),
        ("slave_mask", 255),
        ("privilege_stack_pages", 4),
        ("data_frames", 14),
        ("table_frames", 8),
        ("free_frame_delta", 22),
    ] {
        require_number(record, event, field, expected)?;
    }
    require_bool(record, event, "ts_set", true)?;
    require_bool(record, event, "fpu_state_saved", false)?;
    require_bool(record, event, "nmi_masked", true)?;
    require_bool(record, event, "bsp_only", true)
}

fn validate_shell_vessel_created(
    record: &structured_record,
    incarnation: u64,
    identity_generation: u64,
    replacement: bool,
    restart_budget_remaining: u64,
) -> Result<(), String> {
    let event = "kernel.shell_vessel_test.vessel.generation.created";
    require_shell_vessel_header(
        record,
        event,
        &[
            "role",
            "supervision_generation",
            "incarnation",
            "circle_generation",
            "task_generation",
            "state",
            "mappings",
            "privilege_stack_pages",
            "context_attached",
            "storage_zero",
            "replacement",
            "restart_budget_remaining",
            "rite_grant",
            "rite_sequence_start",
            "transport_zero",
            "initial_if",
        ],
    )?;
    require_string(record, event, "role", "shell_vessel_fixture")?;
    require_number(record, event, "supervision_generation", 1)?;
    require_number(record, event, "incarnation", incarnation)?;
    require_number(record, event, "circle_generation", identity_generation)?;
    require_number(record, event, "task_generation", identity_generation)?;
    require_string(record, event, "state", "ready")?;
    require_number(record, event, "mappings", 3)?;
    require_number(record, event, "privilege_stack_pages", 4)?;
    require_bool(record, event, "context_attached", true)?;
    require_bool(record, event, "storage_zero", true)?;
    require_bool(record, event, "replacement", replacement)?;
    require_number(
        record,
        event,
        "restart_budget_remaining",
        restart_budget_remaining,
    )?;
    require_number(record, event, "rite_grant", 7)?;
    require_number(record, event, "rite_sequence_start", 1)?;
    require_bool(record, event, "transport_zero", true)?;
    require_bool(record, event, "initial_if", false)
}

fn validate_shell_vessel_supervisor_ready(record: &structured_record) -> Result<(), String> {
    let event = "kernel.shell_vessel_test.supervisor.ready";
    require_shell_vessel_header(
        record,
        event,
        &[
            "role",
            "supervision_generation",
            "circle_generation",
            "task_generation",
            "state",
            "mappings",
            "privilege_stack_pages",
            "context_attached",
            "initial_if",
        ],
    )?;
    require_string(record, event, "role", "supervisor_fixture")?;
    require_number(record, event, "supervision_generation", 1)?;
    require_number(record, event, "circle_generation", 1)?;
    require_number(record, event, "task_generation", 1)?;
    require_string(record, event, "state", "ready")?;
    require_number(record, event, "mappings", 3)?;
    require_number(record, event, "privilege_stack_pages", 4)?;
    require_bool(record, event, "context_attached", true)?;
    require_bool(record, event, "initial_if", false)
}

fn validate_shell_vessel_dispatch(
    record: &structured_record,
    role: &str,
    incarnation: Option<u64>,
    dispatch: u64,
) -> Result<(), String> {
    let event = "kernel.shell_vessel_test.task.dispatched";
    let fields_with_incarnation = [
        "role",
        "incarnation",
        "dispatch",
        "kind",
        "state",
        "root_selected",
        "stack_selected",
        "initial_if",
    ];
    let fields_without_incarnation = [
        "role",
        "dispatch",
        "kind",
        "state",
        "root_selected",
        "stack_selected",
        "initial_if",
    ];
    require_shell_vessel_header(
        record,
        event,
        if incarnation.is_some() {
            &fields_with_incarnation
        } else {
            &fields_without_incarnation
        },
    )?;
    require_string(record, event, "role", role)?;
    if let Some(incarnation) = incarnation {
        require_number(record, event, "incarnation", incarnation)?;
    }
    require_number(record, event, "dispatch", dispatch)?;
    require_string(record, event, "kind", "initial")?;
    require_string(record, event, "state", "running")?;
    require_bool(record, event, "root_selected", true)?;
    require_bool(record, event, "stack_selected", true)?;
    require_bool(record, event, "initial_if", false)
}

fn validate_shell_vessel_console(
    record: &structured_record,
    incarnation: u64,
    projection: &str,
    semantic_equal: bool,
) -> Result<(), String> {
    let event = "kernel.shell_vessel_test.rite.console.committed";
    let generation_1_fields = ["incarnation", "sequence", "status", "bytes", "projection"];
    let generation_2_fields = [
        "incarnation",
        "sequence",
        "status",
        "bytes",
        "projection",
        "semantic_equal",
    ];
    require_shell_vessel_header(
        record,
        event,
        if incarnation == 1 {
            &generation_1_fields
        } else {
            &generation_2_fields
        },
    )?;
    require_number(record, event, "incarnation", incarnation)?;
    require_number(record, event, "sequence", 1)?;
    require_string(record, event, "status", "ok")?;
    require_number(
        record,
        event,
        "bytes",
        shell_vessel_private_output.len() as u64,
    )?;
    require_string(record, event, "projection", projection)?;
    if incarnation == 2 {
        require_bool(record, event, "semantic_equal", semantic_equal)?;
    }
    Ok(())
}

fn validate_shell_vessel_input_empty(
    record: &structured_record,
    incarnation: u64,
) -> Result<(), String> {
    let event = "kernel.shell_vessel_test.rite.input.empty";
    require_shell_vessel_header(
        record,
        event,
        &["incarnation", "sequence", "status", "state_unchanged"],
    )?;
    require_number(record, event, "incarnation", incarnation)?;
    require_number(record, event, "sequence", 2)?;
    require_string(record, event, "status", "would_block")?;
    require_bool(record, event, "state_unchanged", true)
}

fn validate_shell_vessel_preemption(record: &structured_record) -> Result<u64, String> {
    let event = "kernel.shell_vessel_test.task.preempted";
    require_shell_vessel_header(
        record,
        event,
        &[
            "role",
            "incarnation",
            "preemption",
            "tick",
            "state",
            "frame_retained",
            "eoi_sent",
        ],
    )?;
    require_string(record, event, "role", "shell_vessel_fixture")?;
    require_number(record, event, "incarnation", 1)?;
    require_number(record, event, "preemption", 0)?;
    require_string(record, event, "state", "suspended")?;
    require_bool(record, event, "frame_retained", true)?;
    require_bool(record, event, "eoi_sent", true)?;
    required_number(record, event, "tick")
}

fn validate_shell_vessel_blocked(record: &structured_record) -> Result<(), String> {
    let event = "kernel.shell_vessel_test.supervisor.blocked";
    require_shell_vessel_header(
        record,
        event,
        &[
            "role",
            "supervision_generation",
            "block",
            "wait",
            "state",
            "frame_retained",
            "vessel_resumed",
        ],
    )?;
    require_string(record, event, "role", "supervisor_fixture")?;
    require_number(record, event, "supervision_generation", 1)?;
    require_number(record, event, "block", 0)?;
    require_string(record, event, "wait", "await_fault")?;
    require_string(record, event, "state", "blocked")?;
    require_bool(record, event, "frame_retained", true)?;
    require_bool(record, event, "vessel_resumed", true)
}

fn validate_shell_vessel_masked(record: &structured_record) -> Result<(), String> {
    let event = "kernel.shell_vessel_test.interrupts.masked";
    require_shell_vessel_header(record, event, &["master_mask", "slave_mask", "if_enabled"])?;
    require_number(record, event, "master_mask", 255)?;
    require_number(record, event, "slave_mask", 255)?;
    require_bool(record, event, "if_enabled", false)
}

fn validate_shell_vessel_resume(
    record: &structured_record,
    role: &str,
    incarnation: Option<u64>,
    dispatch: u64,
    resume: u64,
) -> Result<(), String> {
    let event = "kernel.shell_vessel_test.task.resumed";
    let fields_with_incarnation = [
        "role",
        "incarnation",
        "dispatch",
        "resume",
        "state",
        "context_preserved",
        "frame_owned",
    ];
    let fields_without_incarnation = [
        "role",
        "dispatch",
        "resume",
        "state",
        "context_preserved",
        "frame_owned",
    ];
    require_shell_vessel_header(
        record,
        event,
        if incarnation.is_some() {
            &fields_with_incarnation
        } else {
            &fields_without_incarnation
        },
    )?;
    require_string(record, event, "role", role)?;
    if let Some(incarnation) = incarnation {
        require_number(record, event, "incarnation", incarnation)?;
    }
    require_number(record, event, "dispatch", dispatch)?;
    require_number(record, event, "resume", resume)?;
    require_string(record, event, "state", "running")?;
    require_bool(record, event, "context_preserved", true)?;
    require_bool(record, event, "frame_owned", true)
}

fn validate_shell_vessel_invalid_opcode(
    record: &structured_record,
    fault_address: shell_vessel_fault_address,
) -> Result<(), String> {
    let event = "kernel.exception";
    require_shell_vessel_header(
        record,
        event,
        &[
            "vector",
            "kind",
            "error_code",
            "instruction_pointer",
            "stack_pointer",
            "stack_segment",
            "stack_switched",
            "code_segment",
            "flags",
            "origin",
            "disposition",
            "fault_address",
        ],
    )?;
    require_number(record, event, "vector", 6)?;
    require_string(record, event, "kind", "invalid_opcode")?;
    require_canonical_hex(record, event, "error_code", 0)?;
    require_canonical_hex(
        record,
        event,
        "instruction_pointer",
        fault_address.instruction_pointer,
    )?;
    require_canonical_hex(record, event, "stack_pointer", shell_vessel_user_stack_top)?;
    require_canonical_selector(
        record,
        event,
        "stack_segment",
        shell_vessel_user_data_selector,
    )?;
    require_bool(record, event, "stack_switched", true)?;
    require_canonical_selector(
        record,
        event,
        "code_segment",
        shell_vessel_user_code_selector,
    )?;
    let flags = required_canonical_hex(record, event, "flags")?;
    if flags != shell_vessel_user_flags
        && flags != shell_vessel_user_flags | shell_vessel_fault_resume_flag
    {
        return Err(format!(
            "event `{event}` field `flags` is {flags:#x}; expected IF plus optional RF"
        ));
    }
    require_string(record, event, "origin", "user")?;
    require_string(record, event, "disposition", "contained_test")?;
    match record.get("fault_address") {
        Some(json_scalar::null) => Ok(()),
        _ => Err(format!(
            "event `{event}` field `fault_address` must be null for #UD"
        )),
    }
}

fn validate_shell_vessel_faulted(record: &structured_record) -> Result<(), String> {
    let event = "kernel.shell_vessel_test.vessel.generation.faulted";
    require_shell_vessel_header(
        record,
        event,
        &[
            "role",
            "supervision_generation",
            "incarnation",
            "state",
            "outcome",
            "event_sequence",
            "vector",
            "error_code",
            "observation_published",
            "supervisor_woken",
            "frame_retained",
        ],
    )?;
    require_string(record, event, "role", "shell_vessel_fixture")?;
    require_number(record, event, "supervision_generation", 1)?;
    require_number(record, event, "incarnation", 1)?;
    require_string(record, event, "state", "faulted")?;
    require_string(record, event, "outcome", "invalid_instruction_fault")?;
    require_number(record, event, "event_sequence", 1)?;
    require_number(record, event, "vector", 6)?;
    require_number(record, event, "error_code", 0)?;
    require_bool(record, event, "observation_published", true)?;
    require_bool(record, event, "supervisor_woken", true)?;
    require_bool(record, event, "frame_retained", true)
}

fn validate_shell_vessel_observation(
    record: &structured_record,
    event_sequence: u64,
    incarnation: u64,
    outcome: &str,
) -> Result<(), String> {
    let event = "kernel.shell_vessel_test.observation.consumed";
    require_shell_vessel_header(
        record,
        event,
        &[
            "role",
            "supervision_generation",
            "event_sequence",
            "incarnation",
            "outcome",
            "single_use",
            "pending",
        ],
    )?;
    require_string(record, event, "role", "supervisor_fixture")?;
    require_number(record, event, "supervision_generation", 1)?;
    require_number(record, event, "event_sequence", event_sequence)?;
    require_number(record, event, "incarnation", incarnation)?;
    require_string(record, event, "outcome", outcome)?;
    require_bool(record, event, "single_use", true)?;
    require_bool(record, event, "pending", false)
}

fn validate_shell_vessel_restart(record: &structured_record) -> Result<(), String> {
    let event = "kernel.shell_vessel_test.restart.requested";
    require_shell_vessel_header(
        record,
        event,
        &[
            "role",
            "supervision_generation",
            "restart_sequence",
            "incarnation",
            "block",
            "wait",
            "state",
            "restart_budget_remaining",
            "scheduler_empty",
            "kernel_continuation_restored",
        ],
    )?;
    require_string(record, event, "role", "supervisor_fixture")?;
    require_number(record, event, "supervision_generation", 1)?;
    require_number(record, event, "restart_sequence", 1)?;
    require_number(record, event, "incarnation", 1)?;
    require_number(record, event, "block", 1)?;
    require_string(record, event, "wait", "restart_checkpoint")?;
    require_string(record, event, "state", "blocked")?;
    require_number(record, event, "restart_budget_remaining", 0)?;
    require_bool(record, event, "scheduler_empty", true)?;
    require_bool(record, event, "kernel_continuation_restored", true)
}

fn validate_shell_vessel_withdrawn(
    record: &structured_record,
    incarnation: u64,
    outcome: &str,
) -> Result<(), String> {
    let event = "kernel.shell_vessel_test.vessel.generation.withdrawn";
    require_shell_vessel_header(
        record,
        event,
        &[
            "role",
            "incarnation",
            "circle_generation",
            "task_generation",
            "outcome",
            "context_withdrawn",
            "root_withdrawn",
            "storage_wiped",
            "privilege_stack_wiped",
            "task_identity_stale",
            "circle_identity_stale",
            "channel_retired",
            "transport_wiped",
        ],
    )?;
    require_string(record, event, "role", "shell_vessel_fixture")?;
    require_number(record, event, "incarnation", incarnation)?;
    require_number(record, event, "circle_generation", incarnation)?;
    require_number(record, event, "task_generation", incarnation)?;
    require_string(record, event, "outcome", outcome)?;
    for field in [
        "context_withdrawn",
        "root_withdrawn",
        "storage_wiped",
        "privilege_stack_wiped",
        "task_identity_stale",
        "circle_identity_stale",
        "channel_retired",
        "transport_wiped",
    ] {
        require_bool(record, event, field, true)?;
    }
    Ok(())
}

fn validate_shell_vessel_task_exit(record: &structured_record) -> Result<(), String> {
    let event = "kernel.shell_vessel_test.rite.task.exited";
    require_shell_vessel_header(
        record,
        event,
        &[
            "incarnation",
            "sequence",
            "status",
            "state",
            "returned_from_user",
            "reply_copied",
            "iret_to_exited_vessel",
            "supervisor_selected",
        ],
    )?;
    require_number(record, event, "incarnation", 2)?;
    require_number(record, event, "sequence", 3)?;
    require_string(record, event, "status", "ok")?;
    require_string(record, event, "state", "exited")?;
    require_bool(record, event, "returned_from_user", true)?;
    require_bool(record, event, "reply_copied", false)?;
    require_bool(record, event, "iret_to_exited_vessel", false)?;
    require_bool(record, event, "supervisor_selected", true)
}

fn validate_shell_vessel_completed(record: &structured_record) -> Result<(), String> {
    let event = "kernel.shell_vessel_test.vessel.generation.completed";
    require_shell_vessel_header(
        record,
        event,
        &[
            "role",
            "supervision_generation",
            "incarnation",
            "state",
            "outcome",
            "event_sequence",
            "completion_gate",
            "observation_published",
            "supervisor_woken",
        ],
    )?;
    require_string(record, event, "role", "shell_vessel_fixture")?;
    require_number(record, event, "supervision_generation", 1)?;
    require_number(record, event, "incarnation", 2)?;
    require_string(record, event, "state", "exited")?;
    require_string(record, event, "outcome", "successful_exit")?;
    require_number(record, event, "event_sequence", 2)?;
    require_bool(record, event, "completion_gate", true)?;
    require_bool(record, event, "observation_published", true)?;
    require_bool(record, event, "supervisor_woken", true)
}

fn validate_shell_vessel_supervisor_completed(record: &structured_record) -> Result<(), String> {
    let event = "kernel.shell_vessel_test.supervisor.completed";
    require_shell_vessel_header(
        record,
        event,
        &[
            "role",
            "state",
            "completion_gate",
            "vessel_completed",
            "kernel_continuation_restored",
        ],
    )?;
    require_string(record, event, "role", "supervisor_fixture")?;
    require_string(record, event, "state", "exited")?;
    require_bool(record, event, "completion_gate", true)?;
    require_bool(record, event, "vessel_completed", true)?;
    require_bool(record, event, "kernel_continuation_restored", true)
}

fn validate_shell_vessel_supervisor_withdrawn(record: &structured_record) -> Result<(), String> {
    let event = "kernel.shell_vessel_test.supervisor.withdrawn";
    require_shell_vessel_header(
        record,
        event,
        &[
            "role",
            "context_withdrawn",
            "root_withdrawn",
            "storage_wiped",
            "privilege_stack_wiped",
            "relationship_detached",
        ],
    )?;
    require_string(record, event, "role", "supervisor_fixture")?;
    for field in [
        "context_withdrawn",
        "root_withdrawn",
        "storage_wiped",
        "privilege_stack_wiped",
        "relationship_detached",
    ] {
        require_bool(record, event, field, true)?;
    }
    Ok(())
}

fn validate_shell_vessel_released(
    memory_ready: &structured_record,
    record: &structured_record,
) -> Result<(), String> {
    let event = "kernel.shell_vessel_test.released";
    require_shell_vessel_header(
        record,
        event,
        &[
            "free_frames",
            "initial_free_frames",
            "data_owned_frames",
            "table_owned_frames",
            "active_circles",
            "active_tasks",
            "active_mappings",
            "active_endpoints",
            "active_sigils",
            "active_bindings",
            "active_lineages",
            "active_supervisions",
            "supervision_children",
            "active_leases",
            "bound_leases",
            "scheduler_depth",
            "attached_contexts",
            "active_rite_channels",
            "observation_cleared",
            "wait_cleared",
            "relationship_cleared",
            "channels_retired",
            "transport_wiped",
            "scratch_wiped",
            "kernel_cr3_restored",
            "tss_rsp0_restored",
            "ts_restored",
        ],
    )?;
    let initial_free = required_number(record, event, "initial_free_frames")?;
    if initial_free < 22 {
        return Err("shell-vessel proof reported insufficient initial free frames".into());
    }
    require_number(
        memory_ready,
        "kernel.memory.ready",
        "free_frames",
        initial_free,
    )?;
    require_number(record, event, "free_frames", initial_free)?;
    for field in [
        "data_owned_frames",
        "table_owned_frames",
        "active_circles",
        "active_tasks",
        "active_mappings",
        "active_endpoints",
        "active_sigils",
        "active_bindings",
        "active_lineages",
        "active_supervisions",
        "supervision_children",
        "active_leases",
        "bound_leases",
        "scheduler_depth",
        "attached_contexts",
        "active_rite_channels",
    ] {
        require_number(record, event, field, 0)?;
    }
    for field in [
        "observation_cleared",
        "wait_cleared",
        "relationship_cleared",
        "channels_retired",
        "transport_wiped",
        "scratch_wiped",
        "kernel_cr3_restored",
        "tss_rsp0_restored",
        "ts_restored",
    ] {
        require_bool(record, event, field, true)?;
    }
    Ok(())
}

fn validate_shell_vessel_passed(record: &structured_record) -> Result<(), String> {
    let event = "kernel.shell_vessel_test.passed";
    require_shell_vessel_header(
        record,
        event,
        &[
            "dispatches",
            "preemptions",
            "resumes",
            "blocks",
            "wakes",
            "observations",
            "faults",
            "restarts",
            "completions",
            "rite_calls",
            "console_commits",
            "would_block",
            "terminal_exits",
            "sequence_reset",
            "clean_replacement",
            "supervisor_continued",
        ],
    )?;
    for (field, expected) in [
        ("dispatches", 6),
        ("preemptions", 1),
        ("resumes", 3),
        ("blocks", 2),
        ("wakes", 2),
        ("observations", 2),
        ("faults", 1),
        ("restarts", 1),
        ("completions", 1),
        ("rite_calls", 5),
        ("console_commits", 2),
        ("would_block", 2),
        ("terminal_exits", 1),
    ] {
        require_number(record, event, field, expected)?;
    }
    require_bool(record, event, "sequence_reset", true)?;
    require_bool(record, event, "clean_replacement", true)?;
    require_bool(record, event, "supervisor_continued", true)
}

fn require_shell_vessel_header(
    record: &structured_record,
    event: &str,
    event_fields: &[&str],
) -> Result<(), String> {
    require_control_header(record, event, event_fields)
}

fn verify_rite_smoke_output(status: ExitStatus, transcript: &str) -> Result<(), String> {
    if status.code() != Some(qemu_success_status) {
        return Err(format!(
            "QEMU did not report rite-test success (process status: {:?})",
            status.code()
        ));
    }
    verify_rite_transcript(transcript)
}

fn verify_display_vessel_transcript(transcript: &str) -> Result<(), String> {
    let fixture_marker = std::str::from_utf8(display_vessel_fixture_marker)
        .map_err(|error| format!("display fixture marker is not UTF-8: {error}"))?;
    if transcript
        .to_ascii_uppercase()
        .contains(&fixture_marker.to_ascii_uppercase())
    {
        return Err("display-vessel transcript exposed the fixture marker".into());
    }

    let mut records = Vec::new();
    for (index, line) in transcript.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if !line.starts_with('{') {
            return Err(format!(
                "display-vessel transcript contains unstructured serial content on line {}",
                index + 1
            ));
        }
        let record = json_parser::new(line).parse_record().map_err(|error| {
            format!(
                "malformed structured record on serial line {}: {error}",
                index + 1
            )
        })?;
        let event = record_string(&record, "event")
            .ok_or_else(|| format!("structured serial line {} has no event", index + 1))?;
        let level = record_string(&record, "level")
            .ok_or_else(|| format!("structured serial line {} has no level", index + 1))?;
        if matches!(level, "error" | "fatal")
            || event == display_vessel_failed_event
            || event.ends_with(".failed")
        {
            return Err(format!(
                "display-vessel transcript contains forbidden {level} event `{event}`"
            ));
        }
        if level != "info" {
            return Err(format!(
                "display-vessel transcript contains non-info event `{event}`"
            ));
        }
        if record_u64(&record, "schema") != Some(1) {
            return Err(format!("event `{event}` does not declare schema 1"));
        }
        if event.starts_with(display_vessel_event_prefix) {
            for field in record.keys() {
                if display_vessel_private_fields
                    .iter()
                    .any(|probe| field.contains(probe))
                {
                    return Err(format!(
                        "display-vessel event `{event}` exposes forbidden field `{field}`"
                    ));
                }
            }
        }
        records.push(record);
    }

    let events = records
        .iter()
        .map(|record| record_string(record, "event").unwrap_or("<missing>"))
        .collect::<Vec<_>>();
    if events != expected_display_vessel_serial_events {
        return Err(format!(
            "display-vessel lifecycle is missing, duplicated, unknown, or out of order\nexpected: {expected_display_vessel_serial_events:?}\nobserved: {events:?}"
        ));
    }

    validate_control_boot_records(&records[..7])?;
    let display = &records[7..];
    validate_display_vessel_ready(&display[0])?;
    validate_display_vessel_task_entered(&display[1])?;
    validate_display_vessel_rejection(&display[2], 1, "denied", "authority")?;
    validate_display_vessel_rejection(&display[3], 2, "bad_payload", "malformed")?;
    validate_display_vessel_rejection(&display[4], 3, "bad_payload", "bounds")?;
    validate_display_vessel_frame_committed(&display[5])?;
    validate_display_vessel_task_exited(&display[6])?;
    validate_display_vessel_released(&records[4], &display[7])?;
    validate_display_vessel_passed(&display[8])
}

fn verify_display_vessel_transcript_redacted(transcript: &str) -> Result<(), String> {
    verify_display_vessel_transcript(transcript).map_err(|_| {
        "display-vessel transcript was rejected without echoing unverified content".to_string()
    })
}

fn verify_supervised_renderer_transcript(
    transcript: &str,
    inspected_image_pages: u64,
) -> Result<(), String> {
    for protected in [
        renderer_protected_source(7, 19),
        renderer_protected_source(11, 23),
    ] {
        if transcript
            .as_bytes()
            .windows(protected.len())
            .any(|window| window == protected)
        {
            return Err("supervised-renderer transcript exposed protected fixture source".into());
        }
    }

    let transcript_body = transcript.strip_suffix("\r\n").ok_or_else(|| {
        "supervised-renderer transcript lacks canonical CRLF termination".to_string()
    })?;
    let mut records = Vec::new();
    for (index, line) in transcript_body.split("\r\n").enumerate() {
        if line.is_empty() {
            return Err(format!(
                "supervised-renderer transcript contains an empty record at position {}",
                index + 1
            ));
        }
        let fields = supervised_renderer_canonical_fields(line).map_err(|error| {
            format!(
                "noncanonical supervised-renderer record at position {}: {error}",
                index + 1
            )
        })?;
        let expected_fields = expected_supervised_renderer_serial_fields
            .get(index)
            .ok_or_else(|| "supervised-renderer transcript contains extra records".to_string())?;
        if fields.as_slice() != *expected_fields {
            return Err(format!(
                "supervised-renderer record at position {} has noncanonical fields or ordering",
                index + 1
            ));
        }
        let record = json_parser::new(line).parse_record().map_err(|error| {
            format!(
                "malformed supervised-renderer record on line {}: {error}",
                index + 1
            )
        })?;
        let event = record_string(&record, "event")
            .ok_or_else(|| format!("structured serial line {} has no event", index + 1))?;
        let level = record_string(&record, "level")
            .ok_or_else(|| format!("structured serial line {} has no level", index + 1))?;
        if matches!(level, "error" | "fatal")
            || event == supervised_renderer_failed_event
            || event.ends_with(".failed")
        {
            return Err(format!(
                "supervised-renderer transcript contains forbidden {level} event `{event}`"
            ));
        }
        if level != "info" || record_u64(&record, "schema") != Some(1) {
            return Err(format!(
                "supervised-renderer event `{event}` lacks its exact info/schema header"
            ));
        }
        if event.starts_with(supervised_renderer_event_prefix) {
            for field in record.keys() {
                if supervised_renderer_private_fields
                    .iter()
                    .any(|probe| field.contains(probe))
                {
                    return Err(format!(
                        "supervised-renderer event `{event}` exposes forbidden field `{field}`"
                    ));
                }
            }
        }
        records.push(record);
    }

    let events = records
        .iter()
        .map(|record| record_string(record, "event").unwrap_or("<missing>"))
        .collect::<Vec<_>>();
    if events != expected_supervised_renderer_serial_events {
        return Err(format!(
            "supervised-renderer lifecycle is missing, duplicated, unknown, or out of order\nexpected: {expected_supervised_renderer_serial_events:?}\nobserved: {events:?}"
        ));
    }

    validate_control_boot_records(&records[..7])?;
    validate_scheduler_interrupt_ready(&records[7])?;
    let renderer = &records[8..];
    validate_supervised_renderer_ready(&renderer[0], inspected_image_pages)?;
    validate_supervised_renderer_entered(&renderer[1], inspected_image_pages)?;
    validate_supervised_renderer_preempted(&renderer[2])?;
    validate_supervised_renderer_stage(&renderer[3], 1, 384, false, true)?;
    validate_supervised_renderer_faulted(&renderer[4])?;
    validate_supervised_renderer_generation_released(&renderer[5])?;
    validate_supervised_renderer_restarted(&renderer[6])?;
    validate_supervised_renderer_stage(&renderer[7], 2, 768, true, true)?;
    validate_supervised_renderer_exited(&renderer[8])?;
    validate_supervised_renderer_supervisor_completed(&renderer[9])?;
    validate_supervised_renderer_committed(&renderer[10])?;
    validate_supervised_renderer_released(&records[4], &renderer[11])?;
    validate_supervised_renderer_passed(&renderer[12])
}

const semantic_presentation_feature_fields: &[&[&str]] = &[
    &[
        "schema",
        "event",
        "level",
        "source_image_pages",
        "renderer_image_pages",
        "supervisor_image_pages",
        "stage_frames",
        "source_mappings",
        "renderer_mappings",
        "supervisor_mappings",
    ],
    &["schema", "event", "level", "queued", "task_released"],
    &[
        "schema",
        "event",
        "level",
        "deliveries",
        "rows",
        "frame_bytes",
        "observations",
    ],
    &[
        "schema",
        "event",
        "level",
        "tasks",
        "circles",
        "channels",
        "endpoints",
        "supervisions",
        "retained_stage_frames",
        "framebuffer_unchanged",
    ],
    &[
        "schema",
        "event",
        "level",
        "frame_commits",
        "framebuffer_verified",
        "stage_clears",
    ],
    &[
        "schema",
        "event",
        "level",
        "free_frames",
        "data_owned_frames",
        "table_owned_frames",
        "active_leases",
    ],
    &[
        "schema",
        "event",
        "level",
        "rite_calls",
        "body_copies",
        "body_bytes",
        "submissions",
        "deliveries",
        "backpressure",
        "denials",
        "malformed",
        "freshness_denials",
        "rows",
        "exits",
        "observations",
        "frame_commits",
    ],
];

fn verify_semantic_presentation_transcript(
    transcript: &str,
    inspected_source_pages: u64,
    inspected_renderer_pages: u64,
    protected_probes: &[Vec<u8>],
) -> Result<(), String> {
    reject_semantic_presentation_probes(transcript.as_bytes(), "transcript", protected_probes)?;
    let body = transcript.strip_suffix("\r\n").ok_or_else(|| {
        "semantic-presentation transcript lacks canonical CRLF termination".to_string()
    })?;
    let mut records = Vec::new();
    for (index, line) in body.split("\r\n").enumerate() {
        if line.is_empty() {
            return Err(format!(
                "semantic-presentation transcript contains empty record {}",
                index + 1
            ));
        }
        let fields = supervised_renderer_canonical_fields(line).map_err(|error| {
            format!(
                "noncanonical semantic-presentation record {}: {error}",
                index + 1
            )
        })?;
        let expected = if index < 8 {
            expected_supervised_renderer_serial_fields
                .get(index)
                .copied()
        } else {
            semantic_presentation_feature_fields.get(index - 8).copied()
        }
        .ok_or_else(|| "semantic-presentation transcript contains extra records".to_string())?;
        if fields.as_slice() != expected {
            return Err(format!(
                "semantic-presentation record {} has noncanonical fields or order",
                index + 1
            ));
        }
        let record = json_parser::new(line).parse_record().map_err(|error| {
            format!(
                "malformed semantic-presentation record {}: {error}",
                index + 1
            )
        })?;
        let event = required_string(&record, "semantic-presentation record", "event")?;
        let level = required_string(&record, event, "level")?;
        if record_u64(&record, "schema") != Some(1)
            || level != "info"
            || event == semantic_presentation_failed_event
            || event.ends_with(".failed")
        {
            return Err(format!(
                "semantic-presentation transcript contains forbidden event `{event}`"
            ));
        }
        if event.starts_with(semantic_presentation_event_prefix) {
            for field in record.keys() {
                let permitted_evidence = matches!(
                    field.as_str(),
                    "framebuffer_unchanged"
                        | "framebuffer_verified"
                        | "body_copies"
                        | "body_bytes"
                        | "task_released"
                        | "tasks"
                        | "endpoints"
                );
                if !permitted_evidence
                    && semantic_presentation_private_fields
                        .iter()
                        .any(|probe| field.contains(probe))
                {
                    return Err(format!(
                        "semantic-presentation event `{event}` exposes forbidden field `{field}`"
                    ));
                }
            }
        }
        records.push(record);
    }
    let events = records
        .iter()
        .map(|record| record_string(record, "event").unwrap_or("<missing>"))
        .collect::<Vec<_>>();
    if events != semantic_presentation_expected_events {
        return Err(format!(
            "semantic-presentation lifecycle is incomplete or reordered: {events:?}"
        ));
    }

    validate_control_boot_records(&records[..7])?;
    validate_scheduler_interrupt_ready(&records[7])?;
    validate_semantic_presentation_records(
        &records[4],
        &records[8..],
        inspected_source_pages,
        inspected_renderer_pages,
    )
}

// Validate presentation evidence in its caller's actual lifecycle order.
// Desktop development resumes after presentation; the terminal proof does not.
fn validate_semantic_presentation_records(
    memory_record: &structured_record,
    feature: &[structured_record],
    inspected_source_pages: u64,
    inspected_renderer_pages: u64,
) -> Result<(), String> {
    let ready = &feature[0];
    for (field, expected) in [
        ("source_image_pages", inspected_source_pages),
        ("renderer_image_pages", inspected_renderer_pages),
        ("supervisor_image_pages", 1),
        ("stage_frames", 576),
        ("source_mappings", 3),
        ("renderer_mappings", 3),
        ("supervisor_mappings", 3),
    ] {
        require_number(
            ready,
            semantic_presentation_expected_events[8],
            field,
            expected,
        )?;
    }
    for field in ["queued", "task_released"] {
        require_bool(
            &feature[1],
            semantic_presentation_expected_events[9],
            field,
            true,
        )?;
    }
    for (field, expected) in [
        ("deliveries", 1),
        ("rows", 768),
        ("frame_bytes", 2_359_296),
        ("observations", 1),
    ] {
        require_number(
            &feature[2],
            semantic_presentation_expected_events[10],
            field,
            expected,
        )?;
    }
    for field in ["tasks", "circles", "channels", "endpoints", "supervisions"] {
        require_number(
            &feature[3],
            semantic_presentation_expected_events[11],
            field,
            0,
        )?;
    }
    require_number(
        &feature[3],
        semantic_presentation_expected_events[11],
        "retained_stage_frames",
        576,
    )?;
    require_bool(
        &feature[3],
        semantic_presentation_expected_events[11],
        "framebuffer_unchanged",
        true,
    )?;
    for (field, expected) in [
        ("frame_commits", 1),
        ("framebuffer_verified", 1),
        ("stage_clears", 1),
    ] {
        if field == "framebuffer_verified" {
            require_bool(
                &feature[4],
                semantic_presentation_expected_events[12],
                field,
                true,
            )?;
        } else {
            require_number(
                &feature[4],
                semantic_presentation_expected_events[12],
                field,
                expected,
            )?;
        }
    }
    let initial_free = required_u64(memory_record, "kernel.memory.ready", "free_frames")?;
    require_number(
        &feature[5],
        semantic_presentation_expected_events[13],
        "free_frames",
        initial_free,
    )?;
    for field in ["data_owned_frames", "table_owned_frames", "active_leases"] {
        require_number(
            &feature[5],
            semantic_presentation_expected_events[13],
            field,
            0,
        )?;
    }
    for (field, expected) in [
        ("rite_calls", 777),
        ("body_copies", 775),
        ("body_bytes", 2_392_160),
        ("submissions", 1),
        ("deliveries", 1),
        ("backpressure", 1),
        ("denials", 2),
        ("malformed", 1),
        ("freshness_denials", 1),
        ("rows", 768),
        ("exits", 2),
        ("observations", 1),
        ("frame_commits", 1),
    ] {
        require_number(
            &feature[6],
            semantic_presentation_expected_events[14],
            field,
            expected,
        )?;
    }
    Ok(())
}

fn supervised_renderer_canonical_fields(line: &str) -> Result<Vec<&str>, String> {
    let bytes = line.as_bytes();
    if bytes.iter().any(u8::is_ascii_whitespace) {
        return Err("ASCII whitespace is forbidden".into());
    }
    if bytes.contains(&b'\\') {
        return Err("JSON escape spellings are forbidden".into());
    }
    if bytes.first() != Some(&b'{') || bytes.last() != Some(&b'}') {
        return Err("record is not a JSON object".into());
    }

    let mut fields = Vec::new();
    let mut cursor = 1usize;
    loop {
        if cursor == bytes.len() - 1 {
            return Ok(fields);
        }
        if bytes.get(cursor) != Some(&b'"') {
            return Err("field name does not begin with a quote".into());
        }
        cursor += 1;
        let field_start = cursor;
        while bytes.get(cursor).is_some_and(|byte| *byte != b'"') {
            cursor += 1;
        }
        if bytes.get(cursor) != Some(&b'"') {
            return Err("field name is unterminated".into());
        }
        let field = str::from_utf8(&bytes[field_start..cursor])
            .map_err(|_| "field name is not UTF-8".to_string())?;
        cursor += 1;
        if bytes.get(cursor) != Some(&b':') {
            return Err("field name is not followed by a colon".into());
        }
        cursor += 1;

        if bytes.get(cursor) == Some(&b'"') {
            cursor += 1;
            while bytes.get(cursor).is_some_and(|byte| *byte != b'"') {
                cursor += 1;
            }
            if bytes.get(cursor) != Some(&b'"') {
                return Err("string value is unterminated".into());
            }
            cursor += 1;
        } else {
            let value_start = cursor;
            while bytes
                .get(cursor)
                .is_some_and(|byte| !matches!(*byte, b',' | b'}'))
            {
                cursor += 1;
            }
            if cursor == value_start {
                return Err("scalar value is empty".into());
            }
        }
        fields.push(field);

        match bytes.get(cursor) {
            Some(b',') => cursor += 1,
            Some(b'}') if cursor == bytes.len() - 1 => return Ok(fields),
            _ => return Err("field is not followed by a comma or final brace".into()),
        }
    }
}

fn validate_supervised_renderer_ready(
    record: &structured_record,
    inspected_image_pages: u64,
) -> Result<(), String> {
    let event = "kernel.supervised_renderer_test.ready";
    require_control_header(
        record,
        event,
        &[
            "arch",
            "grant",
            "image_pages",
            "mappings",
            "mailbox_pages",
            "stack_pages",
            "stage_frames",
            "supervisors",
            "renderer_limit",
            "restart_budget",
            "timer_budget",
            "nmi_masked",
        ],
    )?;
    require_string(record, event, "arch", "x86_64")?;
    for (field, expected) in [
        ("grant", 20),
        ("image_pages", inspected_image_pages),
        ("mappings", 3),
        ("mailbox_pages", 1),
        ("stack_pages", 4),
        ("stage_frames", 576),
        ("supervisors", 1),
        ("renderer_limit", 1),
        ("restart_budget", 1),
        ("timer_budget", 1),
    ] {
        require_number(record, event, field, expected)?;
    }
    require_bool(record, event, "nmi_masked", true)
}

fn validate_supervised_renderer_entered(
    record: &structured_record,
    inspected_image_pages: u64,
) -> Result<(), String> {
    let event = "kernel.supervised_renderer_test.renderer.entered";
    require_control_header(
        record,
        event,
        &[
            "generation",
            "origin",
            "image_pages",
            "mappings",
            "code_permissions",
            "mailbox_permissions",
            "stack_permissions",
            "guards_absent",
            "stage_generation",
            "sequence_start",
        ],
    )?;
    require_number(record, event, "generation", 1)?;
    require_string(record, event, "origin", "user")?;
    require_number(record, event, "image_pages", inspected_image_pages)?;
    require_number(record, event, "mappings", 3)?;
    require_string(record, event, "code_permissions", "user_rx")?;
    require_string(record, event, "mailbox_permissions", "user_rw_nx")?;
    require_string(record, event, "stack_permissions", "user_rw_nx")?;
    require_bool(record, event, "guards_absent", true)?;
    require_number(record, event, "stage_generation", 1)?;
    require_number(record, event, "sequence_start", 1)
}

fn validate_supervised_renderer_preempted(record: &structured_record) -> Result<(), String> {
    let event = "kernel.supervised_renderer_test.renderer.preempted";
    require_control_header(
        record,
        event,
        &[
            "generation",
            "timer_interrupts",
            "timer_masked",
            "flags_normalized",
        ],
    )?;
    require_number(record, event, "generation", 1)?;
    require_number(record, event, "timer_interrupts", 1)?;
    require_bool(record, event, "timer_masked", true)?;
    require_bool(record, event, "flags_normalized", true)
}

fn validate_supervised_renderer_stage(
    record: &structured_record,
    generation: u64,
    rows: u64,
    complete: bool,
    surface_unchanged: bool,
) -> Result<(), String> {
    let event = "kernel.supervised_renderer_test.generation.staged";
    let fields: &[&str] = if surface_unchanged {
        &["generation", "rows", "complete", "surface_unchanged"]
    } else {
        &["generation", "rows", "complete"]
    };
    require_control_header(record, event, fields)?;
    require_number(record, event, "generation", generation)?;
    require_number(record, event, "rows", rows)?;
    require_bool(record, event, "complete", complete)?;
    if surface_unchanged {
        require_bool(record, event, "surface_unchanged", true)?;
    }
    Ok(())
}

fn validate_supervised_renderer_faulted(record: &structured_record) -> Result<(), String> {
    let event = "kernel.supervised_renderer_test.generation.faulted";
    require_control_header(
        record,
        event,
        &[
            "generation",
            "vector",
            "kind",
            "fault_site_verified",
            "saved_frame_verified",
            "contained",
            "supervisor_woken",
        ],
    )?;
    require_number(record, event, "generation", 1)?;
    require_number(record, event, "vector", 6)?;
    require_string(record, event, "kind", "invalid_opcode")?;
    require_bool(record, event, "fault_site_verified", true)?;
    require_bool(record, event, "saved_frame_verified", true)?;
    require_bool(record, event, "contained", true)?;
    require_bool(record, event, "supervisor_woken", true)
}

fn validate_supervised_renderer_generation_released(
    record: &structured_record,
) -> Result<(), String> {
    let event = "kernel.supervised_renderer_test.generation.released";
    require_control_header(
        record,
        event,
        &[
            "generation",
            "stage_cleared",
            "channel_retired",
            "mappings_removed",
            "root_withdrawn",
            "privilege_stack_wiped",
            "mailbox_wiped",
            "image_wiped",
            "scratch_wiped",
            "task_destroyed",
            "circle_destroyed",
            "surface_unchanged",
        ],
    )?;
    require_number(record, event, "generation", 1)?;
    for field in [
        "stage_cleared",
        "channel_retired",
        "mappings_removed",
        "root_withdrawn",
        "privilege_stack_wiped",
        "mailbox_wiped",
        "image_wiped",
        "scratch_wiped",
        "task_destroyed",
        "circle_destroyed",
        "surface_unchanged",
    ] {
        require_bool(record, event, field, true)?;
    }
    Ok(())
}

fn validate_supervised_renderer_restarted(record: &structured_record) -> Result<(), String> {
    let event = "kernel.supervised_renderer_test.renderer.restarted";
    require_control_header(
        record,
        event,
        &[
            "generation",
            "fresh_task",
            "fresh_circle",
            "fresh_channel",
            "fresh_stage",
            "sequence_start",
        ],
    )?;
    require_number(record, event, "generation", 2)?;
    for field in ["fresh_task", "fresh_circle", "fresh_channel", "fresh_stage"] {
        require_bool(record, event, field, true)?;
    }
    require_number(record, event, "sequence_start", 1)
}

fn validate_supervised_renderer_exited(record: &structured_record) -> Result<(), String> {
    let event = "kernel.supervised_renderer_test.renderer.exited";
    require_control_header(
        record,
        event,
        &["generation", "terminal", "replied", "supervisor_woken"],
    )?;
    require_number(record, event, "generation", 2)?;
    require_bool(record, event, "terminal", true)?;
    require_bool(record, event, "replied", false)?;
    require_bool(record, event, "supervisor_woken", true)
}

fn validate_supervised_renderer_supervisor_completed(
    record: &structured_record,
) -> Result<(), String> {
    let event = "kernel.supervised_renderer_test.supervisor.completed";
    require_control_header(
        record,
        event,
        &[
            "observations",
            "faults",
            "restarts",
            "completions",
            "success",
        ],
    )?;
    for (field, expected) in [
        ("observations", 2),
        ("faults", 1),
        ("restarts", 1),
        ("completions", 1),
    ] {
        require_number(record, event, field, expected)?;
    }
    require_bool(record, event, "success", true)
}

fn validate_supervised_renderer_committed(record: &structured_record) -> Result<(), String> {
    let event = "kernel.supervised_renderer_test.frame.committed";
    require_control_header(
        record,
        event,
        &["frames", "rows", "surface_withdrawn", "preflight_complete"],
    )?;
    require_number(record, event, "frames", 1)?;
    require_number(record, event, "rows", 768)?;
    require_bool(record, event, "surface_withdrawn", true)?;
    require_bool(record, event, "preflight_complete", true)
}

fn validate_supervised_renderer_released(
    memory_ready: &structured_record,
    record: &structured_record,
) -> Result<(), String> {
    let event = "kernel.supervised_renderer_test.released";
    require_control_header(
        record,
        event,
        &[
            "stages",
            "tasks",
            "circles",
            "channels",
            "stage_cleared",
            "resources_restored",
            "interrupts_masked",
            "free_frames",
            "data_owned_frames",
            "table_owned_frames",
            "active_leases",
            "bound_leases",
            "active_circles",
            "active_tasks",
            "active_mappings",
            "active_endpoints",
            "active_sigils",
            "active_bindings",
            "active_lineages",
            "active_supervisions",
            "supervision_children",
            "scheduler_depth",
            "attached_contexts",
            "active_rite_channels",
            "supervisor_retired",
            "channels_retired",
            "mailboxes_wiped",
            "stacks_wiped",
            "images_wiped",
            "scratch_wiped",
            "kernel_cr3_restored",
            "tss_rsp0_restored",
            "ts_restored",
        ],
    )?;
    for (field, expected) in [("stages", 2), ("tasks", 2), ("circles", 2), ("channels", 2)] {
        require_number(record, event, field, expected)?;
    }
    for field in ["stage_cleared", "resources_restored", "interrupts_masked"] {
        require_bool(record, event, field, true)?;
    }
    let initial_free = required_number(memory_ready, "kernel.memory.ready", "free_frames")?;
    require_number(record, event, "free_frames", initial_free)?;
    for field in [
        "data_owned_frames",
        "table_owned_frames",
        "active_leases",
        "bound_leases",
        "active_circles",
        "active_tasks",
        "active_mappings",
        "active_endpoints",
        "active_sigils",
        "active_bindings",
        "active_lineages",
        "active_supervisions",
        "supervision_children",
        "scheduler_depth",
        "attached_contexts",
        "active_rite_channels",
    ] {
        require_number(record, event, field, 0)?;
    }
    for field in [
        "supervisor_retired",
        "channels_retired",
        "mailboxes_wiped",
        "stacks_wiped",
        "images_wiped",
        "scratch_wiped",
        "kernel_cr3_restored",
        "tss_rsp0_restored",
        "ts_restored",
    ] {
        require_bool(record, event, field, true)?;
    }
    Ok(())
}

fn validate_supervised_renderer_passed(record: &structured_record) -> Result<(), String> {
    let event = "kernel.supervised_renderer_test.passed";
    require_control_header(
        record,
        event,
        &[
            "generations",
            "timer_interrupts",
            "rows_generation_1",
            "rows_generation_2",
            "faults",
            "restarts",
            "terminal_exits",
            "frame_commits",
            "resources_restored",
        ],
    )?;
    for (field, expected) in [
        ("generations", 2),
        ("timer_interrupts", 1),
        ("rows_generation_1", 384),
        ("rows_generation_2", 768),
        ("faults", 1),
        ("restarts", 1),
        ("terminal_exits", 1),
        ("frame_commits", 1),
    ] {
        require_number(record, event, field, expected)?;
    }
    require_bool(record, event, "resources_restored", true)
}

fn validate_display_vessel_ready(record: &structured_record) -> Result<(), String> {
    let event = "kernel.display_vessel_test.ready";
    require_control_header(
        record,
        event,
        &[
            "arch", "grant", "calls", "patches", "width", "height", "format",
        ],
    )?;
    require_string(record, event, "arch", "x86_64")?;
    require_number(record, event, "grant", 20)?;
    require_number(record, event, "calls", 772)?;
    require_number(record, event, "patches", 768)?;
    require_number(record, event, "width", display_vessel_width as u64)?;
    require_number(record, event, "height", display_vessel_height as u64)?;
    require_string(record, event, "format", "rgb8")
}

fn validate_display_vessel_task_entered(record: &structured_record) -> Result<(), String> {
    let event = "kernel.display_vessel_test.task.entered";
    require_control_header(
        record,
        event,
        &[
            "origin",
            "mappings",
            "code_permissions",
            "mailbox_permissions",
            "stack_permissions",
            "guards_absent",
            "aperture_user_accessible",
        ],
    )?;
    require_string(record, event, "origin", "user")?;
    require_number(record, event, "mappings", 3)?;
    require_string(record, event, "code_permissions", "user_rx")?;
    require_string(record, event, "mailbox_permissions", "user_rw_nx")?;
    require_string(record, event, "stack_permissions", "user_rw_nx")?;
    require_bool(record, event, "guards_absent", true)?;
    require_bool(record, event, "aperture_user_accessible", false)
}

fn validate_display_vessel_rejection(
    record: &structured_record,
    sequence: u64,
    status: &str,
    kind: &str,
) -> Result<(), String> {
    let event = "kernel.display_vessel_test.call.rejected";
    require_control_header(
        record,
        event,
        &[
            "sequence",
            "status",
            "kind",
            "state_unchanged",
            "surface_unchanged",
        ],
    )?;
    require_number(record, event, "sequence", sequence)?;
    require_string(record, event, "status", status)?;
    require_string(record, event, "kind", kind)?;
    require_bool(record, event, "state_unchanged", true)?;
    require_bool(record, event, "surface_unchanged", true)
}

fn validate_display_vessel_frame_committed(record: &structured_record) -> Result<(), String> {
    let event = "kernel.display_vessel_test.frame.committed";
    require_control_header(record, event, &["patches", "width", "height", "format"])?;
    require_number(record, event, "patches", 768)?;
    require_number(record, event, "width", display_vessel_width as u64)?;
    require_number(record, event, "height", display_vessel_height as u64)?;
    require_string(record, event, "format", "rgb8")
}

fn validate_display_vessel_task_exited(record: &structured_record) -> Result<(), String> {
    let event = "kernel.display_vessel_test.task.exited";
    require_control_header(
        record,
        event,
        &[
            "sequence",
            "status",
            "state",
            "returned_from_user",
            "reply_copied",
        ],
    )?;
    require_number(record, event, "sequence", 772)?;
    require_string(record, event, "status", "ok")?;
    require_string(record, event, "state", "exited")?;
    require_bool(record, event, "returned_from_user", true)?;
    require_bool(record, event, "reply_copied", false)
}

fn validate_display_vessel_released(
    memory_ready: &structured_record,
    record: &structured_record,
) -> Result<(), String> {
    let event = "kernel.display_vessel_test.released";
    require_control_header(
        record,
        event,
        &[
            "free_frames",
            "initial_free_frames",
            "data_owned_frames",
            "table_owned_frames",
            "active_circles",
            "active_tasks",
            "active_mappings",
            "channel_retired",
            "mailbox_wiped",
            "transition_stack_wiped",
            "scratch_wiped",
        ],
    )?;
    let initial_free_frames = required_number(record, event, "initial_free_frames")?;
    if initial_free_frames == 0 {
        return Err("display-vessel proof reported zero initial free frames".into());
    }
    require_number(
        memory_ready,
        "kernel.memory.ready",
        "free_frames",
        initial_free_frames,
    )?;
    require_number(record, event, "free_frames", initial_free_frames)?;
    for field in [
        "data_owned_frames",
        "table_owned_frames",
        "active_circles",
        "active_tasks",
        "active_mappings",
    ] {
        require_number(record, event, field, 0)?;
    }
    for field in [
        "channel_retired",
        "mailbox_wiped",
        "transition_stack_wiped",
        "scratch_wiped",
    ] {
        require_bool(record, event, field, true)?;
    }
    Ok(())
}

fn validate_display_vessel_passed(record: &structured_record) -> Result<(), String> {
    let event = "kernel.display_vessel_test.passed";
    require_control_header(
        record,
        event,
        &["calls", "rejections", "commits", "terminal"],
    )?;
    require_number(record, event, "calls", 772)?;
    require_number(record, event, "rejections", 3)?;
    require_number(record, event, "commits", 768)?;
    require_bool(record, event, "terminal", true)
}

fn verify_rite_transcript(transcript: &str) -> Result<(), String> {
    if transcript
        .to_ascii_uppercase()
        .contains(rite_private_sentinel_text)
    {
        return Err("rite smoke transcript exposed the private console sentinel".into());
    }

    let mut records = Vec::new();
    for (index, line) in transcript.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if !line.starts_with('{') {
            return Err(format!(
                "rite smoke transcript contains unstructured serial content on line {}",
                index + 1
            ));
        }
        let record = json_parser::new(line).parse_record().map_err(|error| {
            format!(
                "malformed structured record on serial line {}: {error}",
                index + 1
            )
        })?;
        let event = record_string(&record, "event")
            .ok_or_else(|| format!("structured serial line {} has no event", index + 1))?;
        let level = record_string(&record, "level")
            .ok_or_else(|| format!("structured serial line {} has no level", index + 1))?;
        if matches!(level, "error" | "fatal")
            || event == rite_failed_event
            || event.ends_with(".failed")
        {
            return Err(format!(
                "rite smoke transcript contains forbidden {level} event `{event}`"
            ));
        }
        if level != "info" {
            return Err(format!(
                "rite smoke transcript contains non-info event `{event}`"
            ));
        }
        if record_u64(&record, "schema") != Some(1) {
            return Err(format!("event `{event}` does not declare schema 1"));
        }
        if event.starts_with(rite_event_prefix) {
            for field in record.keys() {
                if rite_private_fields
                    .iter()
                    .any(|probe| field.contains(probe))
                {
                    return Err(format!(
                        "rite event `{event}` exposes forbidden field `{field}`"
                    ));
                }
            }
        }
        records.push(record);
    }

    let events = records
        .iter()
        .map(|record| record_string(record, "event").unwrap_or("<missing>"))
        .collect::<Vec<_>>();
    if events != expected_rite_serial_events {
        return Err(format!(
            "rite lifecycle is missing, duplicated, unknown, or out of order\nexpected: {expected_rite_serial_events:?}\nobserved: {events:?}"
        ));
    }

    validate_control_boot_records(&records[..7])?;
    let rites = &records[7..];
    validate_rite_ready(&rites[0])?;
    validate_rite_task_entered(&rites[1])?;
    for (record, sequence, status) in [
        (&rites[2], 1, "bad_operation"),
        (&rites[3], 2, "denied"),
        (&rites[4], 3, "denied"),
        (&rites[5], 4, "limit_exceeded"),
        (&rites[6], 5, "bad_payload"),
        (&rites[9], 8, "bad_payload"),
    ] {
        validate_rite_rejection(record, sequence, status)?;
    }
    validate_rite_console_commit(&rites[7])?;
    validate_rite_input_empty(&rites[8])?;
    validate_rite_task_exit(&rites[10])?;
    validate_rite_released(&records[4], &rites[11])?;
    validate_rite_passed(&rites[12])
}

fn validate_rite_ready(record: &structured_record) -> Result<(), String> {
    let event = "kernel.rite_test.ready";
    require_rite_header(
        record,
        event,
        &["arch", "vector", "grant", "transport_limit"],
    )?;
    require_string(record, event, "arch", "x86_64")?;
    require_number(record, event, "vector", 243)?;
    require_number(record, event, "grant", 7)?;
    require_number(record, event, "transport_limit", 4056)
}

fn validate_rite_task_entered(record: &structured_record) -> Result<(), String> {
    let event = "kernel.rite_test.task.entered";
    require_rite_header(
        record,
        event,
        &[
            "origin",
            "mappings",
            "code_permissions",
            "mailbox_permissions",
            "stack_permissions",
            "guards_absent",
        ],
    )?;
    require_string(record, event, "origin", "user")?;
    require_number(record, event, "mappings", 3)?;
    require_string(record, event, "code_permissions", "user_rx")?;
    require_string(record, event, "mailbox_permissions", "user_rw_nx")?;
    require_string(record, event, "stack_permissions", "user_rw_nx")?;
    require_bool(record, event, "guards_absent", true)
}

fn validate_rite_rejection(
    record: &structured_record,
    sequence: u64,
    status: &str,
) -> Result<(), String> {
    let event = "kernel.rite_test.call.rejected";
    require_rite_header(record, event, &["sequence", "status", "state_unchanged"])?;
    require_number(record, event, "sequence", sequence)?;
    require_string(record, event, "status", status)?;
    require_bool(record, event, "state_unchanged", true)
}

fn validate_rite_console_commit(record: &structured_record) -> Result<(), String> {
    let event = "kernel.rite_test.console.committed";
    require_rite_header(record, event, &["sequence", "status", "bytes"])?;
    require_number(record, event, "sequence", 6)?;
    require_string(record, event, "status", "ok")?;
    require_number(record, event, "bytes", rite_private_sentinel.len() as u64)
}

fn validate_rite_input_empty(record: &structured_record) -> Result<(), String> {
    let event = "kernel.rite_test.input.empty";
    require_rite_header(record, event, &["sequence", "status", "state_unchanged"])?;
    require_number(record, event, "sequence", 7)?;
    require_string(record, event, "status", "would_block")?;
    require_bool(record, event, "state_unchanged", true)
}

fn validate_rite_task_exit(record: &structured_record) -> Result<(), String> {
    let event = "kernel.rite_test.task.exited";
    require_rite_header(
        record,
        event,
        &[
            "sequence",
            "status",
            "state",
            "returned_from_user",
            "reply_copied",
            "iret",
        ],
    )?;
    require_number(record, event, "sequence", 9)?;
    require_string(record, event, "status", "ok")?;
    require_string(record, event, "state", "exited")?;
    require_bool(record, event, "returned_from_user", true)?;
    require_bool(record, event, "reply_copied", false)?;
    require_bool(record, event, "iret", false)
}

fn validate_rite_released(
    memory_ready: &structured_record,
    record: &structured_record,
) -> Result<(), String> {
    let event = "kernel.rite_test.released";
    require_rite_header(
        record,
        event,
        &[
            "free_frames",
            "initial_free_frames",
            "data_owned_frames",
            "table_owned_frames",
            "active_circles",
            "active_tasks",
            "active_mappings",
            "channel_retired",
            "mailbox_wiped",
            "transition_stack_wiped",
        ],
    )?;
    let initial_free_frames = required_number(record, event, "initial_free_frames")?;
    if initial_free_frames == 0 {
        return Err("rite proof reported zero initial free frames".into());
    }
    require_number(
        memory_ready,
        "kernel.memory.ready",
        "free_frames",
        initial_free_frames,
    )?;
    require_number(record, event, "free_frames", initial_free_frames)?;
    for field in [
        "data_owned_frames",
        "table_owned_frames",
        "active_circles",
        "active_tasks",
        "active_mappings",
    ] {
        require_number(record, event, field, 0)?;
    }
    for field in ["channel_retired", "mailbox_wiped", "transition_stack_wiped"] {
        require_bool(record, event, field, true)?;
    }
    Ok(())
}

fn validate_rite_passed(record: &structured_record) -> Result<(), String> {
    let event = "kernel.rite_test.passed";
    require_rite_header(
        record,
        event,
        &["calls", "rejections", "commits", "would_block", "terminal"],
    )?;
    require_number(record, event, "calls", 9)?;
    require_number(record, event, "rejections", 6)?;
    require_number(record, event, "commits", 2)?;
    require_number(record, event, "would_block", 1)?;
    require_bool(record, event, "terminal", true)
}

fn require_rite_header(
    record: &structured_record,
    event: &str,
    event_fields: &[&str],
) -> Result<(), String> {
    require_control_header(record, event, event_fields)
}

fn verify_control_smoke_output(status: ExitStatus, transcript: &str) -> Result<(), String> {
    if status.code() != Some(qemu_success_status) {
        return Err(format!(
            "QEMU did not report control-test success (process status: {:?})",
            status.code()
        ));
    }
    verify_control_transcript(transcript)
}

fn verify_control_transcript(transcript: &str) -> Result<(), String> {
    for private in ["9ad4c1e76b208f35", "53b8f02da6914ce7"] {
        if transcript.to_ascii_lowercase().contains(private) {
            return Err("control smoke transcript exposed the private payload marker".into());
        }
    }
    let mut records = Vec::new();
    for (index, line) in transcript.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if !line.starts_with('{') {
            return Err(format!(
                "control smoke transcript contains unstructured serial content on line {}",
                index + 1
            ));
        }
        let record = json_parser::new(line).parse_record().map_err(|error| {
            format!(
                "malformed structured record on serial line {}: {error}",
                index + 1
            )
        })?;
        let event = record_string(&record, "event")
            .ok_or_else(|| format!("structured serial line {} has no event", index + 1))?;
        let level = record_string(&record, "level")
            .ok_or_else(|| format!("structured serial line {} has no level", index + 1))?;
        if matches!(level, "error" | "fatal")
            || event == control_failed_event
            || event.ends_with(".failed")
        {
            return Err(format!(
                "control smoke transcript contains forbidden {level} event `{event}`"
            ));
        }
        if level != "info" {
            return Err(format!(
                "control smoke transcript contains non-info event `{event}`"
            ));
        }
        if record_u64(&record, "schema") != Some(1) {
            return Err(format!("event `{event}` does not declare schema 1"));
        }
        if event.starts_with(control_event_prefix) {
            for field in record.keys() {
                if control_private_fields
                    .iter()
                    .any(|probe| field.contains(probe))
                {
                    return Err(format!(
                        "control event `{event}` exposes forbidden field `{field}`"
                    ));
                }
            }
        }
        records.push(record);
    }
    let events = records
        .iter()
        .map(|record| record_string(record, "event").unwrap_or("<missing>"))
        .collect::<Vec<_>>();
    if events != expected_control_serial_events {
        return Err(format!(
            "control lifecycle is missing, duplicated, unknown, or out of order\nexpected: {expected_control_serial_events:?}\nobserved: {events:?}"
        ));
    }

    validate_control_boot_records(&records[..7])?;
    let controls = &records[7..];
    validate_control_ready(&controls[0])?;
    validate_control_receiver_ready(&controls[1])?;
    validate_control_producer(&controls[2..7], "human_fixture", 1, 0, false)?;
    validate_control_producer(&controls[7..12], "machine_fixture", 2, 1, true)?;

    require_control_header(
        &controls[12],
        "kernel.control_test.receiver.entered",
        &["circle_generation", "task_generation", "origin", "mappings"],
    )?;
    require_number(
        &controls[12],
        "kernel.control_test.receiver.entered",
        "circle_generation",
        1,
    )?;
    require_number(
        &controls[12],
        "kernel.control_test.receiver.entered",
        "task_generation",
        1,
    )?;
    require_string(
        &controls[12],
        "kernel.control_test.receiver.entered",
        "origin",
        "user",
    )?;
    require_number(
        &controls[12],
        "kernel.control_test.receiver.entered",
        "mappings",
        3,
    )?;

    validate_control_denial(&controls[13], "receiver_fixture", "cross_namespace")?;
    validate_control_receive(&controls[14], "human_fixture", 0)?;
    validate_control_receive(&controls[15], "machine_fixture", 1)?;

    require_control_header(
        &controls[16],
        "kernel.control_test.queue.empty",
        &["status", "depth", "state_unchanged"],
    )?;
    require_string(
        &controls[16],
        "kernel.control_test.queue.empty",
        "status",
        "would_block",
    )?;
    require_number(&controls[16], "kernel.control_test.queue.empty", "depth", 0)?;
    require_bool(
        &controls[16],
        "kernel.control_test.queue.empty",
        "state_unchanged",
        true,
    )?;

    require_control_header(
        &controls[17],
        "kernel.control_test.lineage.revoked",
        &["revoked", "next_lookup_denied", "repeated_result"],
    )?;
    require_bool(
        &controls[17],
        "kernel.control_test.lineage.revoked",
        "revoked",
        true,
    )?;
    require_bool(
        &controls[17],
        "kernel.control_test.lineage.revoked",
        "next_lookup_denied",
        true,
    )?;
    require_string(
        &controls[17],
        "kernel.control_test.lineage.revoked",
        "repeated_result",
        "already_revoked",
    )?;

    require_control_header(
        &controls[18],
        "kernel.control_test.receiver.exited",
        &["state", "returned_from_user"],
    )?;
    require_string(
        &controls[18],
        "kernel.control_test.receiver.exited",
        "state",
        "exited",
    )?;
    require_bool(
        &controls[18],
        "kernel.control_test.receiver.exited",
        "returned_from_user",
        true,
    )?;

    require_control_header(
        &controls[19],
        "kernel.control_test.released",
        &[
            "free_frames",
            "initial_free_frames",
            "data_owned_frames",
            "table_owned_frames",
            "active_circles",
            "active_tasks",
            "active_mappings",
            "active_bindings",
            "active_sigils",
            "active_lineages",
            "active_endpoints",
            "queue_cleared",
            "transition_stack_wiped",
        ],
    )?;
    let initial_free = required_number(
        &controls[19],
        "kernel.control_test.released",
        "initial_free_frames",
    )?;
    if initial_free == 0 {
        return Err("control proof reported zero initial free frames".into());
    }
    require_number(
        &records[4],
        "kernel.memory.ready",
        "free_frames",
        initial_free,
    )?;
    require_number(
        &controls[19],
        "kernel.control_test.released",
        "free_frames",
        initial_free,
    )?;
    for field in [
        "data_owned_frames",
        "table_owned_frames",
        "active_circles",
        "active_tasks",
        "active_mappings",
        "active_bindings",
        "active_sigils",
        "active_lineages",
        "active_endpoints",
    ] {
        require_number(&controls[19], "kernel.control_test.released", field, 0)?;
    }
    require_bool(
        &controls[19],
        "kernel.control_test.released",
        "queue_cleared",
        true,
    )?;
    require_bool(
        &controls[19],
        "kernel.control_test.released",
        "transition_stack_wiped",
        true,
    )?;

    require_control_header(
        &controls[20],
        "kernel.control_test.passed",
        &["producer_runs", "messages", "denials", "coequal"],
    )?;
    require_number(
        &controls[20],
        "kernel.control_test.passed",
        "producer_runs",
        2,
    )?;
    require_number(&controls[20], "kernel.control_test.passed", "messages", 2)?;
    require_number(&controls[20], "kernel.control_test.passed", "denials", 3)?;
    require_bool(&controls[20], "kernel.control_test.passed", "coequal", true)
}

fn validate_control_boot_records(records: &[structured_record]) -> Result<(), String> {
    if records.len() != 7 {
        return Err("control proof does not contain exactly seven boot prerequisites".into());
    }
    for record in records {
        let event = required_string(record, "control boot prerequisite", "event")?;
        validate_interrupt_serial_record(record, event)?;
    }

    let usable_bytes = required_number(&records[3], "boot.memory.map", "usable_bytes")?;
    let free_frames = required_number(&records[4], "kernel.memory.ready", "free_frames")?;
    if free_frames.checked_mul(memory_frame_size) != Some(usable_bytes) {
        return Err(
            "control boot memory map does not match the allocator's exact free-frame count".into(),
        );
    }
    Ok(())
}

fn validate_control_ready(record: &structured_record) -> Result<(), String> {
    let event = "kernel.control_test.ready";
    require_control_header(
        record,
        event,
        &[
            "circle_capacity",
            "task_capacity",
            "endpoint_capacity",
            "sigil_capacity",
            "binding_capacity",
            "lineage_capacity",
            "queue_capacity",
            "mappings_per_circle",
            "scheduler",
            "user_interrupts",
            "nmi_masked",
        ],
    )?;
    for (field, expected) in [
        ("circle_capacity", 16),
        ("task_capacity", 16),
        ("endpoint_capacity", 4),
        ("sigil_capacity", 16),
        ("binding_capacity", 4),
        ("lineage_capacity", 4),
        ("queue_capacity", 2),
        ("mappings_per_circle", 3),
    ] {
        require_number(record, event, field, expected)?;
    }
    require_bool(record, event, "scheduler", false)?;
    require_bool(record, event, "user_interrupts", false)?;
    require_bool(record, event, "nmi_masked", true)
}

fn validate_control_receiver_ready(record: &structured_record) -> Result<(), String> {
    let event = "kernel.control_test.receiver.ready";
    require_control_header(
        record,
        event,
        &[
            "circle_generation",
            "task_generation",
            "endpoint_generation",
            "mappings",
            "rights",
            "code_permissions",
            "mailbox_permissions",
            "stack_permissions",
            "guards_absent",
        ],
    )?;
    require_number(record, event, "circle_generation", 1)?;
    require_number(record, event, "task_generation", 1)?;
    require_number(record, event, "endpoint_generation", 1)?;
    require_number(record, event, "mappings", 3)?;
    require_string(record, event, "rights", "receive_revoke")?;
    validate_control_mapping_evidence(record, event)
}

fn validate_control_producer(
    records: &[structured_record],
    role: &str,
    generation: u64,
    sequence: u64,
    replacement: bool,
) -> Result<(), String> {
    let entered = "kernel.control_test.producer.entered";
    require_control_header(
        &records[0],
        entered,
        &[
            "role",
            "circle_generation",
            "task_generation",
            "origin",
            "mappings",
            "code_identical",
            "storage_zero",
            "replacement",
            "code_permissions",
            "mailbox_permissions",
            "stack_permissions",
            "guards_absent",
        ],
    )?;
    require_string(&records[0], entered, "role", role)?;
    require_number(&records[0], entered, "circle_generation", generation)?;
    require_number(&records[0], entered, "task_generation", generation)?;
    require_string(&records[0], entered, "origin", "user")?;
    require_number(&records[0], entered, "mappings", 3)?;
    require_bool(&records[0], entered, "code_identical", true)?;
    require_bool(&records[0], entered, "storage_zero", true)?;
    require_bool(&records[0], entered, "replacement", replacement)?;
    validate_control_mapping_evidence(&records[0], entered)?;

    validate_control_denial(&records[1], role, "invalid_reference")?;

    let sent = "kernel.control_test.message.sent";
    require_control_header(
        &records[2],
        sent,
        &["role", "sequence", "attribution", "returned_to_user"],
    )?;
    require_string(&records[2], sent, "role", role)?;
    require_number(&records[2], sent, "sequence", sequence)?;
    require_string(&records[2], sent, "attribution", "kernel")?;
    require_bool(&records[2], sent, "returned_to_user", true)?;

    let exited = "kernel.control_test.producer.exited";
    require_control_header(&records[3], exited, &["role", "state"])?;
    require_string(&records[3], exited, "role", role)?;
    require_string(&records[3], exited, "state", "exited")?;

    let withdrawn = "kernel.control_test.producer.withdrawn";
    require_control_header(
        &records[4],
        withdrawn,
        &[
            "role",
            "authority_invalidated",
            "root_withdrawn",
            "storage_wiped",
            "transition_stack_wiped",
        ],
    )?;
    require_string(&records[4], withdrawn, "role", role)?;
    for field in [
        "authority_invalidated",
        "root_withdrawn",
        "storage_wiped",
        "transition_stack_wiped",
    ] {
        require_bool(&records[4], withdrawn, field, true)?;
    }
    Ok(())
}

fn validate_control_mapping_evidence(
    record: &structured_record,
    event: &str,
) -> Result<(), String> {
    require_string(record, event, "code_permissions", "user_rx")?;
    require_string(record, event, "mailbox_permissions", "user_rw_nx")?;
    require_string(record, event, "stack_permissions", "user_rw_nx")?;
    require_bool(record, event, "guards_absent", true)
}

fn validate_control_denial(
    record: &structured_record,
    role: &str,
    reason: &str,
) -> Result<(), String> {
    let event = "kernel.control_test.authority.denied";
    require_control_header(record, event, &["role", "reason", "state_unchanged"])?;
    require_string(record, event, "role", role)?;
    require_string(record, event, "reason", reason)?;
    require_bool(record, event, "state_unchanged", true)
}

fn validate_control_receive(
    record: &structured_record,
    source_role: &str,
    sequence: u64,
) -> Result<(), String> {
    let event = "kernel.control_test.message.received";
    require_control_header(
        record,
        event,
        &[
            "source_role",
            "sequence",
            "fifo",
            "body_equal",
            "attribution_verified",
        ],
    )?;
    require_string(record, event, "source_role", source_role)?;
    require_number(record, event, "sequence", sequence)?;
    require_bool(record, event, "fifo", true)?;
    require_bool(record, event, "body_equal", true)?;
    require_bool(record, event, "attribution_verified", true)
}

fn require_control_header(
    record: &structured_record,
    event: &str,
    event_fields: &[&str],
) -> Result<(), String> {
    let expected_count = event_fields.len() + 3;
    if record.len() != expected_count
        || !record.contains_key("schema")
        || !record.contains_key("event")
        || !record.contains_key("level")
        || event_fields
            .iter()
            .any(|field| !record.contains_key(*field))
    {
        let actual = record.keys().map(String::as_str).collect::<Vec<_>>();
        let mut expected = vec!["schema", "event", "level"];
        expected.extend_from_slice(event_fields);
        expected.sort_unstable();
        return Err(format!(
            "event `{event}` has fields {actual:?}; expected exactly {expected:?}"
        ));
    }
    require_number(record, event, "schema", 1)?;
    require_string(record, event, "event", event)?;
    require_string(record, event, "level", "info")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum circle_observation {
    fault_path_ready,
    protections_ready,
    generation_mapped(u64),
    generation_entered(u64),
    page_fault(u64),
    denial_contained(u64, &'static str),
    generation_torn_down(u64),
    passed,
}

const expected_circle_observations: &[circle_observation] = &[
    circle_observation::fault_path_ready,
    circle_observation::protections_ready,
    circle_observation::generation_mapped(1),
    circle_observation::generation_entered(1),
    circle_observation::page_fault(0x05),
    circle_observation::denial_contained(1, "supervisor_read"),
    circle_observation::generation_torn_down(1),
    circle_observation::generation_mapped(2),
    circle_observation::generation_entered(2),
    circle_observation::page_fault(0x15),
    circle_observation::denial_contained(2, "nx_stack"),
    circle_observation::generation_torn_down(2),
    circle_observation::generation_mapped(3),
    circle_observation::generation_entered(3),
    circle_observation::generation_torn_down(3),
    circle_observation::passed,
];

fn verify_circle_smoke_output(
    status: ExitStatus,
    transcript: &str,
    fault_addresses: circle_fault_addresses,
) -> Result<(), String> {
    if status.code() != Some(qemu_success_status) {
        return Err(format!(
            "QEMU did not report circle-test success (process status: {:?})",
            status.code()
        ));
    }
    verify_circle_transcript(transcript, fault_addresses)
}

fn verify_circle_transcript(
    transcript: &str,
    fault_addresses: circle_fault_addresses,
) -> Result<(), String> {
    let mut observations = Vec::new();
    for (index, line) in transcript.lines().enumerate() {
        let line = line.trim();
        if !line.starts_with('{') {
            continue;
        }
        let record = json_parser::new(line).parse_record().map_err(|error| {
            format!(
                "malformed structured record on serial line {}: {error}",
                index + 1
            )
        })?;
        if record_string(&record, "level") == Some("fatal") {
            let event = record_string(&record, "event").unwrap_or("<missing>");
            return Err(format!(
                "circle smoke transcript contains fatal record `{event}`"
            ));
        }
        let Some(event) = record_string(&record, "event") else {
            continue;
        };
        if event == nested_exception_event
            || event == circle_after_denial_event
            || event == circle_failed_event
            || event.ends_with(".failed")
        {
            return Err(format!(
                "circle smoke transcript contains forbidden event `{event}`"
            ));
        }
        if let Some(observation) = classify_circle_record(&record, fault_addresses)? {
            observations.push(observation);
        }
    }

    if observations != expected_circle_observations {
        return Err(format!(
            "circle lifecycle is missing, duplicated, or out of order\nexpected: {expected_circle_observations:?}\nobserved: {observations:?}"
        ));
    }
    Ok(())
}

fn classify_circle_record(
    record: &structured_record,
    fault_addresses: circle_fault_addresses,
) -> Result<Option<circle_observation>, String> {
    let Some(event) = record_string(record, "event") else {
        return Ok(None);
    };
    let tracked = event == "kernel.fault_path.ready"
        || event == "kernel.exception"
        || event.starts_with(circle_event_prefix);
    if !tracked {
        return Ok(None);
    }
    if record_u64(record, "schema") != Some(1) {
        return Err(format!("tracked event `{event}` does not declare schema 1"));
    }
    if record_string(record, "level") != Some("info") {
        return Err(format!(
            "tracked event `{event}` does not declare info level"
        ));
    }

    match event {
        "kernel.fault_path.ready" => {
            require_u64(record, event, "vectors", 32)?;
            require_bool(record, event, "tss", true)?;
            require_u64(record, event, "double_fault_ist", 1)?;
            require_u64(record, event, "nmi_ist", 2)?;
            require_bool(record, event, "nmi_masked", true)?;
            Ok(Some(circle_observation::fault_path_ready))
        }
        "kernel.circle_test.protections.ready" => {
            require_bool(record, event, "nx", true)?;
            require_bool(record, event, "write_protect", true)?;
            require_bool(record, event, "private_root", true)?;
            Ok(Some(circle_observation::protections_ready))
        }
        "kernel.circle_test.generation.mapped" => {
            let generation = required_generation(record, event)?;
            require_bool(record, event, "scratch_zero", true)?;
            require_bool(record, event, "private_root", true)?;
            require_bool(record, event, "replacement", generation != 1)?;
            Ok(Some(circle_observation::generation_mapped(generation)))
        }
        "kernel.circle_test.generation.entered" => {
            let generation = required_generation(record, event)?;
            require_string(record, event, "origin", "user")?;
            require_hex_or_u64(
                record,
                event,
                "scratch_marker",
                circle_generation_markers[generation as usize - 1],
            )?;
            Ok(Some(circle_observation::generation_entered(generation)))
        }
        "kernel.circle_test.generation.torn_down" => {
            let generation = required_generation(record, event)?;
            require_bool(record, event, "root_withdrawn", true)?;
            require_bool(record, event, "storage_wiped", true)?;
            require_bool(record, event, "transition_stack_wiped", true)?;
            Ok(Some(circle_observation::generation_torn_down(generation)))
        }
        "kernel.circle_test.denial.contained" => {
            let generation = required_generation(record, event)?;
            let (kind, error_code) = match generation {
                1 => ("supervisor_read", 0x05),
                2 => ("nx_stack", 0x15),
                _ => {
                    return Err(format!(
                        "event `{event}` cannot contain generation {generation}"
                    ));
                }
            };
            require_string(record, event, "kind", kind)?;
            require_u64(record, event, "vector", 14)?;
            require_hex_or_u64(record, event, "error_code", error_code)?;
            require_bool(record, event, "contained", true)?;
            Ok(Some(circle_observation::denial_contained(generation, kind)))
        }
        "kernel.exception" => {
            let error_code = validate_circle_page_fault(record, event, fault_addresses)?;
            Ok(Some(circle_observation::page_fault(error_code)))
        }
        "kernel.circle_test.passed" => {
            require_u64(record, event, "generations", 3)?;
            require_u64(record, event, "denials", 2)?;
            Ok(Some(circle_observation::passed))
        }
        _ => Err(format!(
            "circle smoke transcript contains unexpected event `{event}`"
        )),
    }
}

fn validate_circle_page_fault(
    record: &structured_record,
    event: &str,
    fault_addresses: circle_fault_addresses,
) -> Result<u64, String> {
    require_u64(record, event, "vector", 14)?;
    require_string(record, event, "kind", "page_fault")?;
    require_string(record, event, "origin", "user")?;
    require_string(record, event, "disposition", "contained_test")?;
    require_hex_or_u64(record, event, "code_segment", circle_user_code_selector)?;
    require_hex_or_u64(record, event, "stack_segment", circle_user_data_selector)?;
    let flags = required_hex_or_u64(record, event, "flags")?;
    if flags != circle_sanitized_flags && flags != circle_sanitized_flags | circle_fault_resume_flag
    {
        return Err(format!(
            "event `{event}` field `flags` is {flags:#x}; expected sanitized flags with optional RF"
        ));
    }
    require_bool(record, event, "stack_switched", true)?;
    require_bool(record, event, "protection_violation", true)?;
    require_bool(record, event, "write", false)?;
    require_bool(record, event, "user", true)?;
    require_bool(record, event, "reserved_bit", false)?;
    require_bool(record, event, "protection_key", false)?;
    require_bool(record, event, "shadow_stack", false)?;
    require_bool(record, event, "enclave", false)?;

    let stack_pointer = required_hex_or_u64(record, event, "stack_pointer")?;
    if stack_pointer <= circle_user_stack_address || stack_pointer > circle_user_stack_top {
        return Err(format!(
            "event `{event}` stack pointer {stack_pointer:#x} is outside the fixed user stack"
        ));
    }

    let error_code = required_hex_or_u64(record, event, "error_code")?;
    let instruction_pointer = required_hex_or_u64(record, event, "instruction_pointer")?;
    let fault_address = required_hex_or_u64(record, event, "fault_address")?;
    match error_code {
        0x05 => {
            require_bool(record, event, "instruction_fetch", false)?;
            if instruction_pointer != fault_addresses.generation_1_instruction_pointer {
                return Err(format!(
                    "supervisor-read fault RIP {instruction_pointer:#x} does not match the linked probe {:#x}",
                    fault_addresses.generation_1_instruction_pointer
                ));
            }
            if fault_address != fault_addresses.generation_1_fault_address {
                return Err(format!(
                    "supervisor-read CR2 {fault_address:#x} does not match the linked canary {:#x}",
                    fault_addresses.generation_1_fault_address
                ));
            }
        }
        0x15 => {
            require_bool(record, event, "instruction_fetch", true)?;
            if instruction_pointer != circle_user_stack_address
                || fault_address != circle_user_stack_address
            {
                return Err(format!(
                    "NX fault did not identify the exact stack probe: RIP={instruction_pointer:#x}, CR2={fault_address:#x}"
                ));
            }
        }
        _ => {
            return Err(format!(
                "circle smoke observed unexpected page-fault error code {error_code:#x}"
            ));
        }
    }
    Ok(error_code)
}

fn record_string<'a>(record: &'a structured_record, field: &str) -> Option<&'a str> {
    match record.get(field) {
        Some(json_scalar::string(value)) => Some(value),
        _ => None,
    }
}

fn record_u64(record: &structured_record, field: &str) -> Option<u64> {
    match record.get(field) {
        Some(json_scalar::number(value)) => value.parse().ok(),
        Some(json_scalar::string(value)) => value.parse().ok(),
        _ => None,
    }
}

fn record_bool(record: &structured_record, field: &str) -> Option<bool> {
    match record.get(field) {
        Some(json_scalar::boolean(value)) => Some(*value),
        _ => None,
    }
}

fn required_u64(record: &structured_record, event: &str, field: &str) -> Result<u64, String> {
    record_u64(record, field)
        .ok_or_else(|| format!("event `{event}` has missing or invalid numeric field `{field}`"))
}

fn required_generation(record: &structured_record, event: &str) -> Result<u64, String> {
    let generation = required_u64(record, event, "generation")?;
    if (1..=3).contains(&generation) {
        Ok(generation)
    } else {
        Err(format!(
            "event `{event}` has invalid generation {generation}"
        ))
    }
}

fn required_hex_or_u64(
    record: &structured_record,
    event: &str,
    field: &str,
) -> Result<u64, String> {
    match record.get(field) {
        Some(json_scalar::number(value)) => value
            .parse()
            .map_err(|_| format!("event `{event}` has invalid numeric field `{field}`")),
        Some(json_scalar::string(value)) => value
            .strip_prefix("0x")
            .and_then(|digits| u64::from_str_radix(digits, 16).ok())
            .ok_or_else(|| format!("event `{event}` has invalid hexadecimal field `{field}`")),
        _ => Err(format!("event `{event}` is missing field `{field}`")),
    }
}

fn required_string<'a>(
    record: &'a structured_record,
    event: &str,
    field: &str,
) -> Result<&'a str, String> {
    record_string(record, field)
        .ok_or_else(|| format!("event `{event}` has missing or invalid string field `{field}`"))
}

fn require_u64(
    record: &structured_record,
    event: &str,
    field: &str,
    expected: u64,
) -> Result<(), String> {
    let actual = required_u64(record, event, field)?;
    if actual == expected {
        Ok(())
    } else {
        Err(format!(
            "event `{event}` field `{field}` is {actual}; expected {expected}"
        ))
    }
}

fn require_hex_or_u64(
    record: &structured_record,
    event: &str,
    field: &str,
    expected: u64,
) -> Result<(), String> {
    let actual = required_hex_or_u64(record, event, field)?;
    if actual == expected {
        Ok(())
    } else {
        Err(format!(
            "event `{event}` field `{field}` is {actual:#x}; expected {expected:#x}"
        ))
    }
}

fn require_bool(
    record: &structured_record,
    event: &str,
    field: &str,
    expected: bool,
) -> Result<(), String> {
    let actual = record_bool(record, field)
        .ok_or_else(|| format!("event `{event}` has missing or invalid boolean field `{field}`"))?;
    if actual == expected {
        Ok(())
    } else {
        Err(format!(
            "event `{event}` field `{field}` is {actual}; expected {expected}"
        ))
    }
}

fn require_string(
    record: &structured_record,
    event: &str,
    field: &str,
    expected: &str,
) -> Result<(), String> {
    let actual = required_string(record, event, field)?;
    if actual == expected {
        Ok(())
    } else {
        Err(format!(
            "event `{event}` field `{field}` is `{actual}`; expected `{expected}`"
        ))
    }
}

fn fnv_1a(bytes: &[u8]) -> u32 {
    let mut hash = fnv_offset_basis;
    for byte in bytes {
        hash ^= u32::from(*byte);
        hash = hash.wrapping_mul(fnv_prime);
    }
    hash
}

fn write_u32(target: &mut [u8], offset: usize, value: u32) {
    target[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn write_u64(target: &mut [u8], offset: usize, value: u64) {
    target[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

fn run_checked<I, S>(directory: &Path, program: S, arguments: I) -> Result<(), String>
where
    I: IntoIterator<Item = OsString>,
    S: AsRef<OsStr>,
{
    let program = program.as_ref();
    let status = Command::new(program)
        .current_dir(directory)
        .args(arguments)
        .env("SOURCE_DATE_EPOCH", "0")
        .status()
        .map_err(|error| format!("launch {}: {error}", program.to_string_lossy()))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "{} exited with {status}",
            program.to_string_lossy()
        ))
    }
}

fn run_checked_with_environment<I, S, K, V>(
    directory: &Path,
    program: S,
    arguments: I,
    key: K,
    value: V,
) -> Result<(), String>
where
    I: IntoIterator<Item = OsString>,
    S: AsRef<OsStr>,
    K: AsRef<OsStr>,
    V: AsRef<OsStr>,
{
    let program = program.as_ref();
    let status = Command::new(program)
        .current_dir(directory)
        .args(arguments)
        .env("SOURCE_DATE_EPOCH", "0")
        .env(key, value)
        .status()
        .map_err(|error| format!("launch {}: {error}", program.to_string_lossy()))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "{} exited with {status}",
            program.to_string_lossy()
        ))
    }
}

fn run_output_checked<I, S>(directory: &Path, program: S, arguments: I) -> Result<Vec<u8>, String>
where
    I: IntoIterator<Item = OsString>,
    S: AsRef<OsStr>,
{
    let program = program.as_ref();
    let output = Command::new(program)
        .current_dir(directory)
        .args(arguments)
        .env("SOURCE_DATE_EPOCH", "0")
        .output()
        .map_err(|error| format!("launch {}: {error}", program.to_string_lossy()))?;
    if output.status.success() {
        Ok(output.stdout)
    } else {
        Err(format!(
            "{} exited with {}: {}",
            program.to_string_lossy(),
            output.status,
            String::from_utf8_lossy(&output.stderr)
        ))
    }
}

fn repository_root() -> Result<PathBuf, String> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .ok_or_else(|| "cannot locate repository root from xtask manifest".into())
}

fn reject_extra_arguments(mut arguments: impl Iterator<Item = OsString>) -> Result<(), String> {
    match arguments.next() {
        Some(value) => Err(format!("unexpected argument `{}`", value.to_string_lossy())),
        None => Ok(()),
    }
}

fn print_help() {
    println!(
        "Anomalous development tasks\n\n\
Usage:\n\
  cargo xtask program-manifest-pack INPUT_NIL OUTPUT.re package a bounded unsigned program image\n\
  cargo xtask program-manifest-inspect PACKAGE CEILING rederive claims under none|console_write\n\
  cargo xtask program-manifest-proof check bounded host-only admission and refusal fixtures\n\
  cargo xtask image        build a normal bootable raw disk\n\
  cargo xtask qemu         boot the normal image interactively in QEMU\n\
  cargo xtask smoke        build a test-only image and require boot.ready\n\
  cargo xtask fault-smoke  trigger and require an attributed invalid opcode\n\
  cargo xtask circle-smoke prove fixed ring-3 isolation and replacement\n\
  cargo xtask control-smoke prove attributable sigil-gated copied IPC\n\
  cargo xtask verify-control-transcript LOG verify a captured control log\n\
  cargo xtask rite-smoke prove the fixed x86-64 ring-3 rite lifecycle\n\
  cargo xtask verify-rite-transcript LOG verify a captured rite log\n\
  cargo xtask dispatcher-image build the sustained dispatcher proof image\n\
  cargo xtask dispatcher-smoke prove repeated preemption and survivor progress\n\
  cargo xtask native-durable-failure-build build the scoped durable failure proof\n\
  cargo xtask native-durable-failure-smoke prove service cuts and uncertain save recovery\n\
  cargo xtask native-interactive-build build the physical native editor/service pair\n\
  cargo xtask native-interactive-smoke prove physical editing, save outcomes and cold recovery\n\
  cargo xtask native-repeated-build build the repeated native editor proof\n\
  cargo xtask native-repeated-smoke prove repeated edits, saves and cold recovery\n\
  cargo xtask native-durable-build build the scoped native disk proof\n\
  cargo xtask native-durable-smoke prove exact disk saves and cold recovery\n\
  cargo xtask elf-relaunch-inspect inspect unchanged client ELF packaging and loader\n\
  cargo xtask build-elf-relaunch build the native/ELF retained-service comparison image\n\
  cargo xtask smoke-elf-relaunch prove both editor-replacement routes and rendering\n\
  cargo xtask verify-elf-relaunch-log <path> validate a retained comparison log\n\
  cargo xtask verify-elf-relaunch-frame <path> validate a retained RGB P6 capture\n\
  cargo xtask elf-document-inspect inspect zero-fill ELF packaging and loader\n\
  cargo xtask build-elf-document build the native/ELF document comparison image\n\
  cargo xtask smoke-elf-document prove both document routes and exact rendering\n\
  cargo xtask verify-elf-document-log <path> validate a retained comparison log\n\
  cargo xtask native-document-inspect inspect the native document client/service\n\
  cargo xtask build-native-document build the task-bound RAM document proof image\n\
  cargo xtask smoke-native-document prove open/save/reopen and uncertain save outcomes\n\
  cargo xtask native-runtime-inspect inspect the native startup and arena client\n\
  cargo xtask build-native-runtime build the guarded native arena proof image\n\
  cargo xtask smoke-native-runtime prove startup, arena copies, faults and reclamation\n\
  cargo xtask verify-native-runtime-log <log> validate native startup and arena evidence\n\
  cargo xtask linux-loader-entry-inspect package the pinned unchanged Linux interpreter startup\n\
  cargo xtask build-linux-loader-entry build the first interpreter service-boundary image\n\
  cargo xtask smoke-linux-loader-entry prove the first unchanged interpreter SYSCALL and retirement\n\
  cargo xtask verify-linux-loader-entry-transcript LOG verify retained boundary evidence and source bytes\n\
  cargo xtask shared-veil-inspect inspect the unchanged shared veil and client artifacts\n\
  cargo xtask build-shared-veil build the two-caller supervised veil image\n\
  cargo xtask smoke-shared-veil prove shared veil ownership and fault isolation\n\
  cargo xtask verify-shared-veil-log <log> validate shared veil comparison evidence\n\
  cargo xtask veil-application-inspect inspect the standalone userspace veil and retained clients\n\
  cargo xtask build-veil-application build the supervised userspace veil comparison image\n\
  cargo xtask smoke-veil-application prove the userspace veil and dependency-fault comparison\n\
  cargo xtask verify-veil-application-log <log> validate userspace veil comparison evidence\n\
  cargo xtask native-application-inspect inspect the bounded native client artifact\n\
  cargo xtask linux-application-inspect inspect the Linux producer and deployment image\n\
  cargo xtask linux-image-inspect PATH BIAS executable|shared-object inspect an unchanged Linux ELF\n\
  cargo xtask linux-process-inspect PROGRAM INTERPRETER RANDOM_16_BYTES NEW_OUTPUT_DIRECTORY plan an inert Linux startup\n\
  cargo xtask qualify-read-only-files PATH SHA256 NEW_OUTPUT_DIRECTORY qualify the host-only read-only file service\n\
  cargo xtask build-linux-application build the bounded Linux SYSCALL comparison image\n\
  cargo xtask smoke-linux-application prove the Linux producer/native service comparison\n\
  cargo xtask verify-linux-application-log <log> validate Linux comparison evidence\n\
  cargo xtask native-application-inspect inspect the bounded native client artifact\n\
  cargo xtask elf-application-inspect inspect the userspace ELF loader and deployment image\n\
  cargo xtask elf-application-image build the native-personality ELF comparison image\n\
  cargo xtask elf-application-smoke prove userspace ELF admission and native parity\n\
  cargo xtask verify-elf-application-transcript <log> validate ELF loader and comparison evidence\n\
  cargo xtask native-application-image build the native application service proof image\n\
  cargo xtask native-application-smoke prove task-owned channels, input and cleanup\n\
  cargo xtask verify-native-application-transcript <log> validate native service evidence\n\
  cargo xtask ny-object-inspect qualify the pinned Ny object and freestanding startup\n\
  cargo xtask ny-object-image build the bounded Ny userspace proof image\n\
  cargo xtask ny-object-smoke prove Ny startup, computation, faults and cleanup\n\
  cargo xtask verify-ny-object-transcript <log> validate Ny proof evidence\n\
  cargo xtask mixed-workspace-build build and inspect the three-role admission fixture\n\
  cargo xtask mixed-workspace-smoke verify TCG/KVM at 96M and 320M\n\
  cargo xtask native-editor-workspace-build inspect the three native workspace/editor/service payloads\n\
  cargo xtask native-editor-workspace-smoke prove physical editing, composed frames and cold saves\n\
  cargo xtask verify-mixed-workspace-log <path> verify retained mixed proof evidence\n\
  cargo xtask application-profile-image build the bounded x87/SSE2 application proof image\n\
  cargo xtask application-profile-smoke prove CPU isolation, checked copy and task faults\n\
  cargo xtask verify-application-profile-transcript <log> validate application-profile evidence\n\
  cargo xtask owned-dispatcher-image build the owned-root sustained dispatch proof image\n\
  cargo xtask owned-dispatcher-smoke prove owned-root scheduling and cleanup at two RAM sizes\n\
  cargo xtask owned-multi-region-image build the nine-region native launch proof\n\
  cargo xtask build-owned-source build the sealed source transfer proof\n\
  cargo xtask smoke-owned-source prove upload, producer retirement and sealed consumption\n\
  cargo xtask verify-owned-source-transcript LOG verify retained sealed-source evidence\n\
  cargo xtask build-owned-tls build the task-owned FS-base proof\n\
  cargo xtask smoke-owned-tls prove FS isolation across gates, switches and retirement\n\
  cargo xtask verify-owned-tls-transcript LOG verify retained task TLS evidence\n\
  cargo xtask owned-multi-region-smoke prove disjoint RX returns and owned cleanup\n\
  cargo xtask verify-owned-multi-region-transcript LOG verify retained native evidence\n\
  cargo xtask verify-owned-dispatcher-transcript <log> validate owned-root scheduling evidence\n\
  cargo xtask scheduler-smoke prove bounded preemption and user-fault containment\n\
  cargo xtask verify-scheduler-transcript LOG verify a captured scheduler log\n\
  cargo xtask supervision-smoke prove one bounded vessel restart\n\
  cargo xtask verify-supervision-transcript LOG verify a captured supervision log\n\
  cargo xtask shell-vessel-smoke prove fixed supervised shell-vessel rite composition\n\
  cargo xtask verify-shell-vessel-transcript LOG verify a captured shell-vessel log\n\
  cargo xtask input-wait-smoke prove exact semantic input wake and interruption\n\
  cargo xtask verify-input-wait-transcript LOG verify a captured input-wait log\n\
  cargo xtask display-vessel-smoke prove bounded ring-3 display presentation\n\
  cargo xtask display-demo boot the display-vessel fixture in a visible QEMU window\n\
  cargo xtask owned-workspace-cpu-inspect inspect the bounded legacy CPU probe artifact\n\
  cargo xtask owned-workspace-cpu-image build the eager CPU ownership workspace\n\
  cargo xtask owned-workspace-cpu-smoke verify CPU ownership and full desktop interaction\n\
  cargo xtask owned-workspace-inspect inspect the opt-in low-address workspace artifact\n\
  cargo xtask owned-workspace-build build its retained-owner kernel image\n\
  cargo xtask owned-workspace-smoke verify its entry, input, pixels and cleanup\n\
  cargo xtask workspace-image build the persistent native window service image\n\
  cargo xtask workspace-run launch native windows, composition and configuration\n\
  cargo xtask workspace-smoke verify native window interaction and fault withdrawal\n\
  cargo xtask workspace-editor-smoke verify physical editing, focus and exact frames\n\
  cargo xtask desktop-image build an inspected interactive development desktop image\n\
  cargo xtask desktop-run boot the development desktop; Enter opens the bootstrap shell\n\
  cargo xtask desktop-smoke verify keyboard shell entry, privacy, and exact desktop restoration\n\
  cargo xtask supervised-renderer-inspect build and inspect the fixed ring-3 renderer image\n\
  cargo xtask supervised-renderer-all-features-link link every kernel feature with renderer precedence\n\
  cargo xtask supervised-renderer-smoke prove supervised rendering and verify the captured frame\n\
  cargo xtask supervised-renderer-demo boot the supervised renderer in a visible QEMU window\n\
  cargo xtask semantic-presentation-inspect inspect the exact semantic source and renderer payloads\n\
  cargo xtask semantic-presentation-all-features-link link every kernel feature with presentation precedence\n\
  cargo xtask semantic-presentation-smoke prove exact native render, present, and host publication\n\
  cargo xtask verify-display-vessel-transcript LOG verify a captured display-vessel log\n\
  cargo xtask verify-display-vessel-frame PPM verify the complete public fixture frame\n\
  cargo xtask memory-smoke prove bounded memory ownership at two RAM sizes\n\
  cargo xtask interrupt-smoke prove bounded IRQ, time, input, and loss delivery\n\
  cargo xtask normal-exclusion verify normal images exclude feature probes\n\
  cargo xtask shell-smoke  inject commands and verify the bootstrap shell"
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering as atomic_ordering};

    static next_temporary_file: AtomicU64 = AtomicU64::new(1);

    const test_circle_fault_addresses: circle_fault_addresses = circle_fault_addresses {
        generation_1_instruction_pointer: 0x0000_4000_0000_1080,
        generation_1_fault_address: 0x0000_0000_0010_1000,
    };
    const test_scheduler_fault_address: scheduler_fault_address = scheduler_fault_address {
        instruction_pointer: 0x0000_7000_0000_1080,
    };
    const test_supervision_fault_address: supervision_fault_address = supervision_fault_address {
        instruction_pointer: 0x0000_7000_0000_1080,
    };
    const test_shell_vessel_fault_address: shell_vessel_fault_address =
        shell_vessel_fault_address {
            instruction_pointer: 0x0000_7000_0000_1080,
        };
    const test_semantic_presentation_source_pages: u64 = 3;
    const test_semantic_presentation_renderer_pages: u64 = 5;

    fn verify_test_circle_transcript(transcript: &str) -> Result<(), String> {
        verify_circle_transcript(transcript, test_circle_fault_addresses)
    }

    fn verify_test_scheduler_transcript(transcript: &str) -> Result<(), String> {
        verify_scheduler_transcript(transcript, test_scheduler_fault_address)
    }

    fn verify_test_supervision_transcript(transcript: &str) -> Result<(), String> {
        verify_supervision_transcript(transcript, test_supervision_fault_address)
    }

    fn verify_test_shell_vessel_transcript(transcript: &str) -> Result<(), String> {
        verify_shell_vessel_transcript(transcript, test_shell_vessel_fault_address)
    }

    fn temporary_file(label: &str) -> PathBuf {
        let sequence = next_temporary_file.fetch_add(1, atomic_ordering::Relaxed);
        env::temp_dir().join(format!(
            "anomalous-xtask-{label}-{}-{sequence}",
            std::process::id()
        ))
    }

    fn valid_display_vessel_transcript() -> String {
        [
            r#"{"schema":1,"event":"boot.kernel.enter","level":"info","arch":"x86_64"}"#,
            r#"{"schema":1,"event":"kernel.fault_path.ready","level":"info","vectors":32,"tss":true,"double_fault_ist":1,"nmi_ist":2,"nmi_masked":true}"#,
            r#"{"schema":1,"event":"boot.omen.accepted","level":"info","version":2,"boot_drive":128}"#,
            r#"{"schema":1,"event":"boot.memory.map","level":"info","regions":9,"usable_bytes":81920000}"#,
            r#"{"schema":1,"event":"kernel.memory.ready","level":"info","frame_size":4096,"address_limit":4294967296,"total_frames":1048576,"free_frames":20000,"unavailable_frames":1028576,"data_owned_frames":0,"table_owned_frames":0}"#,
            r#"{"schema":1,"event":"boot.display.ready","level":"info","width":1024,"height":768,"pitch":3072,"bpp":24}"#,
            r#"{"schema":1,"event":"boot.ready","level":"info","control_abi":"0.1","framebuffer":true}"#,
            r#"{"schema":1,"event":"kernel.display_vessel_test.ready","level":"info","arch":"x86_64","grant":20,"calls":772,"patches":768,"width":1024,"height":768,"format":"rgb8"}"#,
            r#"{"schema":1,"event":"kernel.display_vessel_test.task.entered","level":"info","origin":"user","mappings":3,"code_permissions":"user_rx","mailbox_permissions":"user_rw_nx","stack_permissions":"user_rw_nx","guards_absent":true,"aperture_user_accessible":false}"#,
            r#"{"schema":1,"event":"kernel.display_vessel_test.call.rejected","level":"info","sequence":1,"status":"denied","kind":"authority","state_unchanged":true,"surface_unchanged":true}"#,
            r#"{"schema":1,"event":"kernel.display_vessel_test.call.rejected","level":"info","sequence":2,"status":"bad_payload","kind":"malformed","state_unchanged":true,"surface_unchanged":true}"#,
            r#"{"schema":1,"event":"kernel.display_vessel_test.call.rejected","level":"info","sequence":3,"status":"bad_payload","kind":"bounds","state_unchanged":true,"surface_unchanged":true}"#,
            r#"{"schema":1,"event":"kernel.display_vessel_test.frame.committed","level":"info","patches":768,"width":1024,"height":768,"format":"rgb8"}"#,
            r#"{"schema":1,"event":"kernel.display_vessel_test.task.exited","level":"info","sequence":772,"status":"ok","state":"exited","returned_from_user":true,"reply_copied":false}"#,
            r#"{"schema":1,"event":"kernel.display_vessel_test.released","level":"info","free_frames":20000,"initial_free_frames":20000,"data_owned_frames":0,"table_owned_frames":0,"active_circles":0,"active_tasks":0,"active_mappings":0,"channel_retired":true,"mailbox_wiped":true,"transition_stack_wiped":true,"scratch_wiped":true}"#,
            r#"{"schema":1,"event":"kernel.display_vessel_test.passed","level":"info","calls":772,"rejections":3,"commits":768,"terminal":true}"#,
        ]
        .join("\n")
    }

    fn valid_supervised_renderer_transcript() -> String {
        let mut transcript = [
            r#"{"schema":1,"event":"boot.kernel.enter","level":"info","arch":"x86_64"}"#,
            r#"{"schema":1,"event":"kernel.fault_path.ready","level":"info","vectors":32,"tss":true,"double_fault_ist":1,"nmi_ist":2,"nmi_masked":true}"#,
            r#"{"schema":1,"event":"boot.omen.accepted","level":"info","version":2,"boot_drive":128}"#,
            r#"{"schema":1,"event":"boot.memory.map","level":"info","regions":9,"usable_bytes":81920000}"#,
            r#"{"schema":1,"event":"kernel.memory.ready","level":"info","frame_size":4096,"address_limit":4294967296,"total_frames":1048576,"free_frames":20000,"unavailable_frames":1028576,"data_owned_frames":0,"table_owned_frames":0}"#,
            r#"{"schema":1,"event":"boot.display.ready","level":"info","width":1024,"height":768,"pitch":3072,"bpp":24}"#,
            r#"{"schema":1,"event":"boot.ready","level":"info","control_abi":"0.1","framebuffer":true}"#,
            r#"{"schema":1,"event":"kernel.interrupts.ready","level":"info","backend":"legacy_pc","vector_base":32,"vector_count":16,"pit_input_hz":1193182,"pit_divisor":11932,"timebase_generation":1,"initial_master_mask":255,"initial_slave_mask":255,"master_mask":252,"slave_mask":255,"input_available":true,"input_source_generation":1,"input_queue_capacity":64,"nmi_masked":true,"if_enabled":true}"#,
            r#"{"schema":1,"event":"kernel.supervised_renderer_test.ready","level":"info","arch":"x86_64","grant":20,"image_pages":2,"mappings":3,"mailbox_pages":1,"stack_pages":4,"stage_frames":576,"supervisors":1,"renderer_limit":1,"restart_budget":1,"timer_budget":1,"nmi_masked":true}"#,
            r#"{"schema":1,"event":"kernel.supervised_renderer_test.renderer.entered","level":"info","generation":1,"origin":"user","image_pages":2,"mappings":3,"code_permissions":"user_rx","mailbox_permissions":"user_rw_nx","stack_permissions":"user_rw_nx","guards_absent":true,"stage_generation":1,"sequence_start":1}"#,
            r#"{"schema":1,"event":"kernel.supervised_renderer_test.renderer.preempted","level":"info","generation":1,"timer_interrupts":1,"timer_masked":true,"flags_normalized":true}"#,
            r#"{"schema":1,"event":"kernel.supervised_renderer_test.generation.staged","level":"info","generation":1,"rows":384,"complete":false,"surface_unchanged":true}"#,
            r#"{"schema":1,"event":"kernel.supervised_renderer_test.generation.faulted","level":"info","generation":1,"vector":6,"kind":"invalid_opcode","contained":true,"supervisor_woken":true,"fault_site_verified":true,"saved_frame_verified":true}"#,
            r#"{"schema":1,"event":"kernel.supervised_renderer_test.generation.released","level":"info","generation":1,"stage_cleared":true,"channel_retired":true,"mappings_removed":true,"root_withdrawn":true,"privilege_stack_wiped":true,"mailbox_wiped":true,"image_wiped":true,"scratch_wiped":true,"task_destroyed":true,"circle_destroyed":true,"surface_unchanged":true}"#,
            r#"{"schema":1,"event":"kernel.supervised_renderer_test.renderer.restarted","level":"info","generation":2,"fresh_task":true,"fresh_circle":true,"fresh_channel":true,"fresh_stage":true,"sequence_start":1}"#,
            r#"{"schema":1,"event":"kernel.supervised_renderer_test.generation.staged","level":"info","generation":2,"rows":768,"complete":true,"surface_unchanged":true}"#,
            r#"{"schema":1,"event":"kernel.supervised_renderer_test.renderer.exited","level":"info","generation":2,"terminal":true,"replied":false,"supervisor_woken":true}"#,
            r#"{"schema":1,"event":"kernel.supervised_renderer_test.supervisor.completed","level":"info","observations":2,"faults":1,"restarts":1,"completions":1,"success":true}"#,
            r#"{"schema":1,"event":"kernel.supervised_renderer_test.frame.committed","level":"info","frames":1,"rows":768,"surface_withdrawn":true,"preflight_complete":true}"#,
            r#"{"schema":1,"event":"kernel.supervised_renderer_test.released","level":"info","stages":2,"tasks":2,"circles":2,"channels":2,"free_frames":20000,"data_owned_frames":0,"table_owned_frames":0,"active_leases":0,"bound_leases":0,"active_circles":0,"active_tasks":0,"active_mappings":0,"active_endpoints":0,"active_sigils":0,"active_bindings":0,"active_lineages":0,"active_supervisions":0,"supervision_children":0,"scheduler_depth":0,"attached_contexts":0,"active_rite_channels":0,"stage_cleared":true,"resources_restored":true,"interrupts_masked":true,"supervisor_retired":true,"channels_retired":true,"mailboxes_wiped":true,"stacks_wiped":true,"images_wiped":true,"scratch_wiped":true,"kernel_cr3_restored":true,"tss_rsp0_restored":true,"ts_restored":true}"#,
            r#"{"schema":1,"event":"kernel.supervised_renderer_test.passed","level":"info","generations":2,"timer_interrupts":1,"rows_generation_1":384,"rows_generation_2":768,"faults":1,"restarts":1,"terminal_exits":1,"frame_commits":1,"resources_restored":true}"#,
        ]
        .join("\r\n");
        transcript.push_str("\r\n");
        transcript
    }

    pub(super) fn valid_semantic_presentation_transcript() -> String {
        let mut transcript = [
            r#"{"schema":1,"event":"boot.kernel.enter","level":"info","arch":"x86_64"}"#,
            r#"{"schema":1,"event":"kernel.fault_path.ready","level":"info","vectors":32,"tss":true,"double_fault_ist":1,"nmi_ist":2,"nmi_masked":true}"#,
            r#"{"schema":1,"event":"boot.omen.accepted","level":"info","version":2,"boot_drive":128}"#,
            r#"{"schema":1,"event":"boot.memory.map","level":"info","regions":9,"usable_bytes":81920000}"#,
            r#"{"schema":1,"event":"kernel.memory.ready","level":"info","frame_size":4096,"address_limit":4294967296,"total_frames":1048576,"free_frames":20000,"unavailable_frames":1028576,"data_owned_frames":0,"table_owned_frames":0}"#,
            r#"{"schema":1,"event":"boot.display.ready","level":"info","width":1024,"height":768,"pitch":3072,"bpp":24}"#,
            r#"{"schema":1,"event":"boot.ready","level":"info","control_abi":"0.1","framebuffer":true}"#,
            r#"{"schema":1,"event":"kernel.interrupts.ready","level":"info","backend":"legacy_pc","vector_base":32,"vector_count":16,"pit_input_hz":1193182,"pit_divisor":11932,"timebase_generation":1,"initial_master_mask":255,"initial_slave_mask":255,"master_mask":252,"slave_mask":255,"input_available":true,"input_source_generation":1,"input_queue_capacity":64,"nmi_masked":true,"if_enabled":true}"#,
            r#"{"schema":1,"event":"kernel.semantic_surface_presentation_test.ready","level":"info","source_image_pages":3,"renderer_image_pages":5,"supervisor_image_pages":1,"stage_frames":576,"source_mappings":3,"renderer_mappings":3,"supervisor_mappings":3}"#,
            r#"{"schema":1,"event":"kernel.semantic_surface_presentation_test.source_released","level":"info","queued":true,"task_released":true}"#,
            r#"{"schema":1,"event":"kernel.semantic_surface_presentation_test.render_acknowledged","level":"info","deliveries":1,"rows":768,"frame_bytes":2359296,"observations":1}"#,
            r#"{"schema":1,"event":"kernel.semantic_surface_presentation_test.prepared","level":"info","tasks":0,"circles":0,"channels":0,"endpoints":0,"supervisions":0,"retained_stage_frames":576,"framebuffer_unchanged":true}"#,
            r#"{"schema":1,"event":"kernel.semantic_surface_presentation_test.presented","level":"info","frame_commits":1,"framebuffer_verified":true,"stage_clears":1}"#,
            r#"{"schema":1,"event":"kernel.semantic_surface_presentation_test.released","level":"info","free_frames":20000,"data_owned_frames":0,"table_owned_frames":0,"active_leases":0}"#,
            r#"{"schema":1,"event":"kernel.semantic_surface_presentation_test.passed","level":"info","rite_calls":777,"body_copies":775,"body_bytes":2392160,"submissions":1,"deliveries":1,"backpressure":1,"denials":2,"malformed":1,"freshness_denials":1,"rows":768,"exits":2,"observations":1,"frame_commits":1}"#,
        ]
        .join("\r\n");
        transcript.push_str("\r\n");
        transcript
    }

    fn valid_input_wait_transcript() -> String {
        [
            r#"{"schema":1,"event":"boot.kernel.enter","level":"info","arch":"x86_64"}"#,
            r#"{"schema":1,"event":"kernel.fault_path.ready","level":"info","vectors":32,"tss":true,"double_fault_ist":1,"nmi_ist":2,"nmi_masked":true}"#,
            r#"{"schema":1,"event":"boot.omen.accepted","level":"info","version":2,"boot_drive":128}"#,
            r#"{"schema":1,"event":"boot.memory.map","level":"info","regions":9,"usable_bytes":81920000}"#,
            r#"{"schema":1,"event":"kernel.memory.ready","level":"info","frame_size":4096,"address_limit":4294967296,"total_frames":1048576,"free_frames":20000,"unavailable_frames":1028576,"data_owned_frames":0,"table_owned_frames":0}"#,
            r#"{"schema":1,"event":"boot.display.ready","level":"info","width":1024,"height":768,"pitch":3072,"bpp":24}"#,
            r#"{"schema":1,"event":"boot.ready","level":"info","control_abi":"0.1","framebuffer":true}"#,
            r#"{"schema":1,"event":"kernel.interrupts.ready","level":"info","backend":"legacy_pc","vector_base":32,"vector_count":16,"pit_input_hz":1193182,"pit_divisor":11932,"timebase_generation":1,"initial_master_mask":255,"initial_slave_mask":255,"master_mask":252,"slave_mask":255,"input_available":true,"input_source_generation":1,"input_queue_capacity":64,"nmi_masked":true,"if_enabled":true}"#,
            r#"{"schema":1,"event":"kernel.input_wait_test.ready","level":"info","arch":"x86_64","policy":"fixed_session_input","content_redacted":true,"nmi_masked":true,"bsp_only":true}"#,
            r#"{"schema":1,"event":"kernel.input_wait_test.immediate.completed","level":"info","body_copied":true,"source_consumed":true,"header_published_last":true,"content_redacted":true}"#,
            r#"{"schema":1,"event":"kernel.input_wait_test.wait.blocked","level":"info","pending_owned":true,"user_pointer_retained":false,"exact_reservation":true,"content_redacted":true}"#,
            r#"{"schema":1,"event":"kernel.input_wait_test.publication.woke","level":"info","same_semantic_shape":true,"publisher_remained_current":true,"fifo_tail":true,"source_consumed":false,"exact_wake":true,"content_redacted":true}"#,
            r#"{"schema":1,"event":"kernel.input_wait_test.delivery.selected","level":"info","reserved_selection":true,"body_copied":true,"source_consumed":true,"channel_settled":true,"header_published_last":true,"content_redacted":true}"#,
            r#"{"schema":1,"event":"kernel.input_wait_test.wait.blocked_again","level":"info","pending_owned":true,"user_pointer_retained":false,"exact_reservation":true,"content_redacted":true}"#,
            r#"{"schema":1,"event":"kernel.input_wait_test.route.closed","level":"info","route_queue_cleared":true,"exact_wake":true,"content_redacted":true}"#,
            r#"{"schema":1,"event":"kernel.input_wait_test.fault.selected","level":"info","kind":"invalid_opcode","peer_terminal":true,"reserved_selection":true,"content_redacted":true}"#,
            r#"{"schema":1,"event":"kernel.input_wait_test.interruption.completed","level":"info","payload_free":true,"channel_settled":true,"header_published_last":true,"content_redacted":true}"#,
            r#"{"schema":1,"event":"kernel.input_wait_test.released","level":"info","frame_accounting_restored":true,"tasks_retired":true,"contexts_withdrawn":true,"roots_withdrawn":true,"private_storage_cleared":true,"channel_retired":true,"endpoint_retired":true,"scratch_wiped":true,"kernel_cr3_restored":true,"tss_rsp0_restored":true,"ts_restored":true,"content_redacted":true}"#,
            r#"{"schema":1,"event":"kernel.input_wait_test.passed","level":"info","lifecycle_complete":true,"exact_wake":true,"copy_before_consume":true,"payload_free_interruption":true,"kernel_continuation_restored":true,"content_redacted":true}"#,
        ]
        .join("\n")
    }

    fn valid_circle_transcript() -> String {
        [
            r#"{"schema":1,"event":"boot.kernel.enter","level":"info"}"#,
            r#"{"schema":1,"event":"kernel.fault_path.ready","level":"info","vectors":32,"tss":true,"double_fault_ist":1,"nmi_ist":2,"nmi_masked":true}"#,
            r#"{"schema":1,"event":"kernel.circle_test.protections.ready","level":"info","nx":true,"write_protect":true,"private_root":true}"#,
            r#"{"schema":1,"event":"kernel.circle_test.generation.mapped","level":"info","generation":1,"scratch_zero":true,"private_root":true,"replacement":false}"#,
            r#"{"schema":1,"event":"kernel.circle_test.generation.entered","level":"info","generation":1,"origin":"user","scratch_marker":"0x1616161616161616"}"#,
            r#"{"schema":1,"event":"kernel.exception","level":"info","vector":14,"kind":"page_fault","error_code":"0x0000000000000005","instruction_pointer":"0x0000400000001080","stack_pointer":"0x0000400000006000","stack_segment":"0x002b","stack_switched":true,"code_segment":"0x0033","flags":"0x0000000000000002","origin":"user","disposition":"contained_test","fault_address":"0x0000000000101000","protection_violation":true,"write":false,"user":true,"reserved_bit":false,"instruction_fetch":false,"protection_key":false,"shadow_stack":false,"enclave":false}"#,
            r#"{"schema":1,"event":"kernel.circle_test.denial.contained","level":"info","generation":1,"kind":"supervisor_read","vector":14,"error_code":"0x5","contained":true}"#,
            r#"{"schema":1,"event":"kernel.circle_test.generation.torn_down","level":"info","generation":1,"root_withdrawn":true,"storage_wiped":true,"transition_stack_wiped":true}"#,
            r#"{"schema":1,"event":"kernel.circle_test.generation.mapped","level":"info","generation":2,"scratch_zero":true,"private_root":true,"replacement":true}"#,
            r#"{"schema":1,"event":"kernel.circle_test.generation.entered","level":"info","generation":2,"origin":"user","scratch_marker":"0x2626262626262626"}"#,
            r#"{"schema":1,"event":"kernel.exception","level":"info","vector":14,"kind":"page_fault","error_code":"0x0000000000000015","instruction_pointer":"0x0000400000005000","stack_pointer":"0x0000400000006000","stack_segment":"0x002b","stack_switched":true,"code_segment":"0x0033","flags":"0x0000000000000002","origin":"user","disposition":"contained_test","fault_address":"0x0000400000005000","protection_violation":true,"write":false,"user":true,"reserved_bit":false,"instruction_fetch":true,"protection_key":false,"shadow_stack":false,"enclave":false}"#,
            r#"{"schema":1,"event":"kernel.circle_test.denial.contained","level":"info","generation":2,"kind":"nx_stack","vector":14,"error_code":"0x15","contained":true}"#,
            r#"{"schema":1,"event":"kernel.circle_test.generation.torn_down","level":"info","generation":2,"root_withdrawn":true,"storage_wiped":true,"transition_stack_wiped":true}"#,
            r#"{"schema":1,"event":"kernel.circle_test.generation.mapped","level":"info","generation":3,"scratch_zero":true,"private_root":true,"replacement":true}"#,
            r#"{"schema":1,"event":"kernel.circle_test.generation.entered","level":"info","generation":3,"origin":"user","scratch_marker":"0x3636363636363636"}"#,
            r#"{"schema":1,"event":"kernel.circle_test.generation.torn_down","level":"info","generation":3,"root_withdrawn":true,"storage_wiped":true,"transition_stack_wiped":true}"#,
            r#"{"schema":1,"event":"kernel.circle_test.passed","level":"info","generations":3,"denials":2}"#,
        ]
        .join("\n")
    }

    fn valid_control_transcript() -> String {
        [
            r#"{"schema":1,"event":"boot.kernel.enter","level":"info","arch":"x86_64"}"#,
            r#"{"schema":1,"event":"kernel.fault_path.ready","level":"info","vectors":32,"tss":true,"double_fault_ist":1,"nmi_ist":2,"nmi_masked":true}"#,
            r#"{"schema":1,"event":"boot.omen.accepted","level":"info","version":2,"boot_drive":128}"#,
            r#"{"schema":1,"event":"boot.memory.map","level":"info","regions":9,"usable_bytes":81920000}"#,
            r#"{"schema":1,"event":"kernel.memory.ready","level":"info","frame_size":4096,"address_limit":4294967296,"total_frames":1048576,"free_frames":20000,"unavailable_frames":1028576,"data_owned_frames":0,"table_owned_frames":0}"#,
            r#"{"schema":1,"event":"boot.display.ready","level":"info","width":1024,"height":768,"pitch":3072,"bpp":24}"#,
            r#"{"schema":1,"event":"boot.ready","level":"info","control_abi":"0.1","framebuffer":true}"#,
            r#"{"schema":1,"event":"kernel.control_test.ready","level":"info","circle_capacity":16,"task_capacity":16,"endpoint_capacity":4,"sigil_capacity":16,"binding_capacity":4,"lineage_capacity":4,"queue_capacity":2,"mappings_per_circle":3,"scheduler":false,"user_interrupts":false,"nmi_masked":true}"#,
            r#"{"schema":1,"event":"kernel.control_test.receiver.ready","level":"info","circle_generation":1,"task_generation":1,"endpoint_generation":1,"mappings":3,"rights":"receive_revoke","code_permissions":"user_rx","mailbox_permissions":"user_rw_nx","stack_permissions":"user_rw_nx","guards_absent":true}"#,
            r#"{"schema":1,"event":"kernel.control_test.producer.entered","level":"info","role":"human_fixture","circle_generation":1,"task_generation":1,"origin":"user","mappings":3,"code_identical":true,"storage_zero":true,"replacement":false,"code_permissions":"user_rx","mailbox_permissions":"user_rw_nx","stack_permissions":"user_rw_nx","guards_absent":true}"#,
            r#"{"schema":1,"event":"kernel.control_test.authority.denied","level":"info","role":"human_fixture","reason":"invalid_reference","state_unchanged":true}"#,
            r#"{"schema":1,"event":"kernel.control_test.message.sent","level":"info","role":"human_fixture","sequence":0,"attribution":"kernel","returned_to_user":true}"#,
            r#"{"schema":1,"event":"kernel.control_test.producer.exited","level":"info","role":"human_fixture","state":"exited"}"#,
            r#"{"schema":1,"event":"kernel.control_test.producer.withdrawn","level":"info","role":"human_fixture","authority_invalidated":true,"root_withdrawn":true,"storage_wiped":true,"transition_stack_wiped":true}"#,
            r#"{"schema":1,"event":"kernel.control_test.producer.entered","level":"info","role":"machine_fixture","circle_generation":2,"task_generation":2,"origin":"user","mappings":3,"code_identical":true,"storage_zero":true,"replacement":true,"code_permissions":"user_rx","mailbox_permissions":"user_rw_nx","stack_permissions":"user_rw_nx","guards_absent":true}"#,
            r#"{"schema":1,"event":"kernel.control_test.authority.denied","level":"info","role":"machine_fixture","reason":"invalid_reference","state_unchanged":true}"#,
            r#"{"schema":1,"event":"kernel.control_test.message.sent","level":"info","role":"machine_fixture","sequence":1,"attribution":"kernel","returned_to_user":true}"#,
            r#"{"schema":1,"event":"kernel.control_test.producer.exited","level":"info","role":"machine_fixture","state":"exited"}"#,
            r#"{"schema":1,"event":"kernel.control_test.producer.withdrawn","level":"info","role":"machine_fixture","authority_invalidated":true,"root_withdrawn":true,"storage_wiped":true,"transition_stack_wiped":true}"#,
            r#"{"schema":1,"event":"kernel.control_test.receiver.entered","level":"info","circle_generation":1,"task_generation":1,"origin":"user","mappings":3}"#,
            r#"{"schema":1,"event":"kernel.control_test.authority.denied","level":"info","role":"receiver_fixture","reason":"cross_namespace","state_unchanged":true}"#,
            r#"{"schema":1,"event":"kernel.control_test.message.received","level":"info","source_role":"human_fixture","sequence":0,"fifo":true,"body_equal":true,"attribution_verified":true}"#,
            r#"{"schema":1,"event":"kernel.control_test.message.received","level":"info","source_role":"machine_fixture","sequence":1,"fifo":true,"body_equal":true,"attribution_verified":true}"#,
            r#"{"schema":1,"event":"kernel.control_test.queue.empty","level":"info","status":"would_block","depth":0,"state_unchanged":true}"#,
            r#"{"schema":1,"event":"kernel.control_test.lineage.revoked","level":"info","revoked":true,"next_lookup_denied":true,"repeated_result":"already_revoked"}"#,
            r#"{"schema":1,"event":"kernel.control_test.receiver.exited","level":"info","state":"exited","returned_from_user":true}"#,
            r#"{"schema":1,"event":"kernel.control_test.released","level":"info","free_frames":20000,"initial_free_frames":20000,"data_owned_frames":0,"table_owned_frames":0,"active_circles":0,"active_tasks":0,"active_mappings":0,"active_bindings":0,"active_sigils":0,"active_lineages":0,"active_endpoints":0,"queue_cleared":true,"transition_stack_wiped":true}"#,
            r#"{"schema":1,"event":"kernel.control_test.passed","level":"info","producer_runs":2,"messages":2,"denials":3,"coequal":true}"#,
        ]
        .join("\n")
    }

    fn valid_rite_transcript() -> String {
        [
            r#"{"schema":1,"event":"boot.kernel.enter","level":"info","arch":"x86_64"}"#,
            r#"{"schema":1,"event":"kernel.fault_path.ready","level":"info","vectors":32,"tss":true,"double_fault_ist":1,"nmi_ist":2,"nmi_masked":true}"#,
            r#"{"schema":1,"event":"boot.omen.accepted","level":"info","version":2,"boot_drive":128}"#,
            r#"{"schema":1,"event":"boot.memory.map","level":"info","regions":9,"usable_bytes":81920000}"#,
            r#"{"schema":1,"event":"kernel.memory.ready","level":"info","frame_size":4096,"address_limit":4294967296,"total_frames":1048576,"free_frames":20000,"unavailable_frames":1028576,"data_owned_frames":0,"table_owned_frames":0}"#,
            r#"{"schema":1,"event":"boot.display.ready","level":"info","width":1024,"height":768,"pitch":3072,"bpp":24}"#,
            r#"{"schema":1,"event":"boot.ready","level":"info","control_abi":"0.1","framebuffer":true}"#,
            r#"{"schema":1,"event":"kernel.rite_test.ready","level":"info","arch":"x86_64","vector":243,"grant":7,"transport_limit":4056}"#,
            r#"{"schema":1,"event":"kernel.rite_test.task.entered","level":"info","origin":"user","mappings":3,"code_permissions":"user_rx","mailbox_permissions":"user_rw_nx","stack_permissions":"user_rw_nx","guards_absent":true}"#,
            r#"{"schema":1,"event":"kernel.rite_test.call.rejected","level":"info","sequence":1,"status":"bad_operation","state_unchanged":true}"#,
            r#"{"schema":1,"event":"kernel.rite_test.call.rejected","level":"info","sequence":2,"status":"denied","state_unchanged":true}"#,
            r#"{"schema":1,"event":"kernel.rite_test.call.rejected","level":"info","sequence":3,"status":"denied","state_unchanged":true}"#,
            r#"{"schema":1,"event":"kernel.rite_test.call.rejected","level":"info","sequence":4,"status":"limit_exceeded","state_unchanged":true}"#,
            r#"{"schema":1,"event":"kernel.rite_test.call.rejected","level":"info","sequence":5,"status":"bad_payload","state_unchanged":true}"#,
            r#"{"schema":1,"event":"kernel.rite_test.console.committed","level":"info","sequence":6,"status":"ok","bytes":16}"#,
            r#"{"schema":1,"event":"kernel.rite_test.input.empty","level":"info","sequence":7,"status":"would_block","state_unchanged":true}"#,
            r#"{"schema":1,"event":"kernel.rite_test.call.rejected","level":"info","sequence":8,"status":"bad_payload","state_unchanged":true}"#,
            r#"{"schema":1,"event":"kernel.rite_test.task.exited","level":"info","sequence":9,"status":"ok","state":"exited","returned_from_user":true,"reply_copied":false,"iret":false}"#,
            r#"{"schema":1,"event":"kernel.rite_test.released","level":"info","free_frames":20000,"initial_free_frames":20000,"data_owned_frames":0,"table_owned_frames":0,"active_circles":0,"active_tasks":0,"active_mappings":0,"channel_retired":true,"mailbox_wiped":true,"transition_stack_wiped":true}"#,
            r#"{"schema":1,"event":"kernel.rite_test.passed","level":"info","calls":9,"rejections":6,"commits":2,"would_block":1,"terminal":true}"#,
        ]
        .join("\n")
    }

    fn valid_scheduler_transcript() -> String {
        [
            r#"{"schema":1,"event":"boot.kernel.enter","level":"info","arch":"x86_64"}"#,
            r#"{"schema":1,"event":"kernel.fault_path.ready","level":"info","vectors":32,"tss":true,"double_fault_ist":1,"nmi_ist":2,"nmi_masked":true}"#,
            r#"{"schema":1,"event":"boot.omen.accepted","level":"info","version":2,"boot_drive":128}"#,
            r#"{"schema":1,"event":"boot.memory.map","level":"info","regions":9,"usable_bytes":81920000}"#,
            r#"{"schema":1,"event":"kernel.memory.ready","level":"info","frame_size":4096,"address_limit":4294967296,"total_frames":1048576,"free_frames":20000,"unavailable_frames":1028576,"data_owned_frames":0,"table_owned_frames":0}"#,
            r#"{"schema":1,"event":"boot.display.ready","level":"info","width":1024,"height":768,"pitch":3072,"bpp":24}"#,
            r#"{"schema":1,"event":"boot.ready","level":"info","control_abi":"0.1","framebuffer":true}"#,
            r#"{"schema":1,"event":"kernel.interrupts.ready","level":"info","backend":"legacy_pc","vector_base":32,"vector_count":16,"pit_input_hz":1193182,"pit_divisor":11932,"timebase_generation":1,"initial_master_mask":255,"initial_slave_mask":255,"master_mask":252,"slave_mask":255,"input_available":true,"input_source_generation":1,"input_queue_capacity":64,"nmi_masked":true,"if_enabled":true}"#,
            r#"{"schema":1,"event":"kernel.scheduler_test.ready","level":"info","policy":"fixed_round_robin","circles":2,"tasks":2,"queue_capacity":2,"preemption_budget":2,"timer_vector":32,"master_mask":254,"slave_mask":255,"privilege_stack_pages":4,"data_frames":14,"table_frames":8,"free_frame_delta":22,"ts_set":true,"fpu_state_saved":false,"nmi_masked":true,"bsp_only":true}"#,
            r#"{"schema":1,"event":"kernel.scheduler_test.task.ready","level":"info","role":"faulting_fixture","circle_generation":1,"task_generation":1,"state":"ready","mappings":3,"privilege_stack_pages":4,"context_attached":true,"initial_if":false}"#,
            r#"{"schema":1,"event":"kernel.scheduler_test.task.ready","level":"info","role":"peer_fixture","circle_generation":1,"task_generation":1,"state":"ready","mappings":3,"privilege_stack_pages":4,"context_attached":true,"initial_if":false}"#,
            r#"{"schema":1,"event":"kernel.scheduler_test.task.dispatched","level":"info","role":"faulting_fixture","dispatch":0,"kind":"initial","state":"running","root_selected":true,"stack_selected":true,"initial_if":false}"#,
            r#"{"schema":1,"event":"kernel.scheduler_test.task.preempted","level":"info","role":"faulting_fixture","preemption":0,"tick":3,"state":"suspended","frame_retained":true,"eoi_sent":true}"#,
            r#"{"schema":1,"event":"kernel.scheduler_test.task.dispatched","level":"info","role":"peer_fixture","dispatch":1,"kind":"initial","state":"running","root_selected":true,"stack_selected":true,"initial_if":false}"#,
            r#"{"schema":1,"event":"kernel.scheduler_test.task.preempted","level":"info","role":"peer_fixture","preemption":1,"tick":7,"state":"suspended","frame_retained":true,"eoi_sent":true}"#,
            r#"{"schema":1,"event":"kernel.scheduler_test.task.resumed","level":"info","role":"faulting_fixture","dispatch":2,"resume":0,"state":"running","context_preserved":true,"frame_owned":true}"#,
            r#"{"schema":1,"event":"kernel.exception","level":"info","vector":6,"kind":"invalid_opcode","error_code":"0x0000000000000000","instruction_pointer":"0x0000700000001080","stack_pointer":"0x0000700000006000","stack_segment":"0x002b","stack_switched":true,"code_segment":"0x0033","flags":"0x0000000000000202","origin":"user","disposition":"contained_test","fault_address":null}"#,
            r#"{"schema":1,"event":"kernel.scheduler_test.task.faulted","level":"info","role":"faulting_fixture","state":"faulted","vector":6,"error_code":0,"contained":true,"frame_retained":true}"#,
            r#"{"schema":1,"event":"kernel.scheduler_test.task.resumed","level":"info","role":"peer_fixture","dispatch":3,"resume":1,"state":"running","context_preserved":true,"frame_owned":true}"#,
            r#"{"schema":1,"event":"kernel.scheduler_test.task.completed","level":"info","role":"peer_fixture","state":"exited","completion_gate":true,"peer_continued":true,"kernel_continuation_restored":true}"#,
            r#"{"schema":1,"event":"kernel.scheduler_test.interrupts.masked","level":"info","master_mask":255,"slave_mask":255,"if_enabled":false}"#,
            r#"{"schema":1,"event":"kernel.scheduler_test.task.withdrawn","level":"info","role":"faulting_fixture","context_withdrawn":true,"root_withdrawn":true,"storage_wiped":true,"privilege_stack_wiped":true}"#,
            r#"{"schema":1,"event":"kernel.scheduler_test.task.withdrawn","level":"info","role":"peer_fixture","context_withdrawn":true,"root_withdrawn":true,"storage_wiped":true,"privilege_stack_wiped":true}"#,
            r#"{"schema":1,"event":"kernel.scheduler_test.released","level":"info","free_frames":20000,"initial_free_frames":20000,"data_owned_frames":0,"table_owned_frames":0,"active_circles":0,"active_tasks":0,"active_mappings":0,"active_leases":0,"bound_leases":0,"scheduler_depth":0,"attached_contexts":0,"kernel_cr3_restored":true,"tss_rsp0_restored":true,"ts_restored":true}"#,
            r#"{"schema":1,"event":"kernel.scheduler_test.passed","level":"info","dispatches":4,"preemptions":2,"resumes":2,"faults":1,"completions":1,"peer_continued":true}"#,
        ]
        .join("\n")
    }

    fn valid_supervision_transcript() -> String {
        [
            r#"{"schema":1,"event":"boot.kernel.enter","level":"info","arch":"x86_64"}"#,
            r#"{"schema":1,"event":"kernel.fault_path.ready","level":"info","vectors":32,"tss":true,"double_fault_ist":1,"nmi_ist":2,"nmi_masked":true}"#,
            r#"{"schema":1,"event":"boot.omen.accepted","level":"info","version":2,"boot_drive":128}"#,
            r#"{"schema":1,"event":"boot.memory.map","level":"info","regions":9,"usable_bytes":81920000}"#,
            r#"{"schema":1,"event":"kernel.memory.ready","level":"info","frame_size":4096,"address_limit":4294967296,"total_frames":1048576,"free_frames":20000,"unavailable_frames":1028576,"data_owned_frames":0,"table_owned_frames":0}"#,
            r#"{"schema":1,"event":"boot.display.ready","level":"info","width":1024,"height":768,"pitch":3072,"bpp":24}"#,
            r#"{"schema":1,"event":"boot.ready","level":"info","control_abi":"0.1","framebuffer":true}"#,
            r#"{"schema":1,"event":"kernel.interrupts.ready","level":"info","backend":"legacy_pc","vector_base":32,"vector_count":16,"pit_input_hz":1193182,"pit_divisor":11932,"timebase_generation":1,"initial_master_mask":255,"initial_slave_mask":255,"master_mask":252,"slave_mask":255,"input_available":true,"input_source_generation":1,"input_queue_capacity":64,"nmi_masked":true,"if_enabled":true}"#,
            r#"{"schema":1,"event":"kernel.supervision_test.ready","level":"info","policy":"fixed_supervision","supervision_capacity":1,"relationships":1,"circles":2,"tasks":2,"queue_capacity":2,"restart_budget":1,"observation_capacity":1,"timer_vector":32,"master_mask":254,"slave_mask":255,"privilege_stack_pages":4,"data_frames":14,"table_frames":8,"free_frame_delta":22,"ts_set":true,"fpu_state_saved":false,"nmi_masked":true,"bsp_only":true}"#,
            r#"{"schema":1,"event":"kernel.supervision_test.vessel.generation.created","level":"info","role":"vessel_fixture","supervision_generation":1,"incarnation":1,"circle_generation":1,"task_generation":1,"state":"ready","mappings":3,"privilege_stack_pages":4,"context_attached":true,"storage_zero":true,"replacement":false,"restart_budget_remaining":1,"initial_if":false}"#,
            r#"{"schema":1,"event":"kernel.supervision_test.supervisor.ready","level":"info","role":"supervisor_fixture","supervision_generation":1,"circle_generation":1,"task_generation":1,"state":"ready","mappings":3,"privilege_stack_pages":4,"context_attached":true,"initial_if":false}"#,
            r#"{"schema":1,"event":"kernel.supervision_test.task.dispatched","level":"info","role":"vessel_fixture","incarnation":1,"dispatch":0,"kind":"initial","state":"running","root_selected":true,"stack_selected":true,"initial_if":false}"#,
            r#"{"schema":1,"event":"kernel.supervision_test.task.preempted","level":"info","role":"vessel_fixture","incarnation":1,"preemption":0,"tick":3,"state":"suspended","frame_retained":true,"eoi_sent":true}"#,
            r#"{"schema":1,"event":"kernel.supervision_test.task.dispatched","level":"info","role":"supervisor_fixture","dispatch":1,"kind":"initial","state":"running","root_selected":true,"stack_selected":true,"initial_if":false}"#,
            r#"{"schema":1,"event":"kernel.supervision_test.supervisor.blocked","level":"info","role":"supervisor_fixture","supervision_generation":1,"block":0,"wait":"await_fault","state":"blocked","frame_retained":true,"vessel_resumed":true}"#,
            r#"{"schema":1,"event":"kernel.supervision_test.interrupts.masked","level":"info","master_mask":255,"slave_mask":255,"if_enabled":false}"#,
            r#"{"schema":1,"event":"kernel.supervision_test.task.resumed","level":"info","role":"vessel_fixture","incarnation":1,"dispatch":2,"resume":0,"state":"running","context_preserved":true,"frame_owned":true}"#,
            r#"{"schema":1,"event":"kernel.exception","level":"info","vector":6,"kind":"invalid_opcode","error_code":"0x0000000000000000","instruction_pointer":"0x0000700000001080","stack_pointer":"0x0000700000006000","stack_segment":"0x002b","stack_switched":true,"code_segment":"0x0033","flags":"0x0000000000000202","origin":"user","disposition":"contained_test","fault_address":null}"#,
            r#"{"schema":1,"event":"kernel.supervision_test.vessel.generation.faulted","level":"info","role":"vessel_fixture","supervision_generation":1,"incarnation":1,"state":"faulted","outcome":"invalid_instruction_fault","event_sequence":1,"vector":6,"error_code":0,"observation_published":true,"supervisor_woken":true,"frame_retained":true}"#,
            r#"{"schema":1,"event":"kernel.supervision_test.task.resumed","level":"info","role":"supervisor_fixture","dispatch":3,"resume":1,"state":"running","context_preserved":true,"frame_owned":true}"#,
            r#"{"schema":1,"event":"kernel.supervision_test.observation.consumed","level":"info","role":"supervisor_fixture","supervision_generation":1,"event_sequence":1,"incarnation":1,"outcome":"invalid_instruction_fault","single_use":true,"pending":false}"#,
            r#"{"schema":1,"event":"kernel.supervision_test.restart.requested","level":"info","role":"supervisor_fixture","supervision_generation":1,"restart_sequence":1,"incarnation":1,"block":1,"wait":"restart_checkpoint","state":"blocked","restart_budget_remaining":0,"scheduler_empty":true,"kernel_continuation_restored":true}"#,
            r#"{"schema":1,"event":"kernel.supervision_test.vessel.generation.withdrawn","level":"info","role":"vessel_fixture","incarnation":1,"circle_generation":1,"task_generation":1,"outcome":"invalid_instruction_fault","context_withdrawn":true,"root_withdrawn":true,"storage_wiped":true,"privilege_stack_wiped":true,"task_identity_stale":true,"circle_identity_stale":true}"#,
            r#"{"schema":1,"event":"kernel.supervision_test.vessel.generation.created","level":"info","role":"vessel_fixture","supervision_generation":1,"incarnation":2,"circle_generation":2,"task_generation":2,"state":"ready","mappings":3,"privilege_stack_pages":4,"context_attached":true,"storage_zero":true,"replacement":true,"restart_budget_remaining":0,"initial_if":false}"#,
            r#"{"schema":1,"event":"kernel.supervision_test.task.dispatched","level":"info","role":"vessel_fixture","incarnation":2,"dispatch":4,"kind":"initial","state":"running","root_selected":true,"stack_selected":true,"initial_if":false}"#,
            r#"{"schema":1,"event":"kernel.supervision_test.vessel.generation.completed","level":"info","role":"vessel_fixture","supervision_generation":1,"incarnation":2,"state":"exited","outcome":"successful_exit","event_sequence":2,"completion_gate":true,"observation_published":true,"supervisor_woken":true}"#,
            r#"{"schema":1,"event":"kernel.supervision_test.task.resumed","level":"info","role":"supervisor_fixture","dispatch":5,"resume":2,"state":"running","context_preserved":true,"frame_owned":true}"#,
            r#"{"schema":1,"event":"kernel.supervision_test.observation.consumed","level":"info","role":"supervisor_fixture","supervision_generation":1,"event_sequence":2,"incarnation":2,"outcome":"successful_exit","single_use":true,"pending":false}"#,
            r#"{"schema":1,"event":"kernel.supervision_test.supervisor.completed","level":"info","role":"supervisor_fixture","state":"exited","completion_gate":true,"vessel_completed":true,"kernel_continuation_restored":true}"#,
            r#"{"schema":1,"event":"kernel.supervision_test.vessel.generation.withdrawn","level":"info","role":"vessel_fixture","incarnation":2,"circle_generation":2,"task_generation":2,"outcome":"successful_exit","context_withdrawn":true,"root_withdrawn":true,"storage_wiped":true,"privilege_stack_wiped":true,"task_identity_stale":true,"circle_identity_stale":true}"#,
            r#"{"schema":1,"event":"kernel.supervision_test.supervisor.withdrawn","level":"info","role":"supervisor_fixture","context_withdrawn":true,"root_withdrawn":true,"storage_wiped":true,"privilege_stack_wiped":true,"relationship_detached":true}"#,
            r#"{"schema":1,"event":"kernel.supervision_test.released","level":"info","free_frames":20000,"initial_free_frames":20000,"data_owned_frames":0,"table_owned_frames":0,"active_circles":0,"active_tasks":0,"active_mappings":0,"active_leases":0,"bound_leases":0,"scheduler_depth":0,"attached_contexts":0,"active_endpoints":0,"active_sigils":0,"active_bindings":0,"active_lineages":0,"active_supervisions":0,"supervision_children":0,"observation_cleared":true,"wait_cleared":true,"relationship_cleared":true,"kernel_cr3_restored":true,"tss_rsp0_restored":true,"ts_restored":true}"#,
            r#"{"schema":1,"event":"kernel.supervision_test.passed","level":"info","dispatches":6,"preemptions":1,"resumes":3,"blocks":2,"wakes":2,"observations":2,"faults":1,"restarts":1,"completions":1,"clean_replacement":true,"supervisor_continued":true}"#,
        ]
        .join("\n")
    }

    fn valid_shell_vessel_transcript() -> String {
        [
            r#"{"schema":1,"event":"boot.kernel.enter","level":"info","arch":"x86_64"}"#,
            r#"{"schema":1,"event":"kernel.fault_path.ready","level":"info","vectors":32,"tss":true,"double_fault_ist":1,"nmi_ist":2,"nmi_masked":true}"#,
            r#"{"schema":1,"event":"boot.omen.accepted","level":"info","version":2,"boot_drive":128}"#,
            r#"{"schema":1,"event":"boot.memory.map","level":"info","regions":9,"usable_bytes":81920000}"#,
            r#"{"schema":1,"event":"kernel.memory.ready","level":"info","frame_size":4096,"address_limit":4294967296,"total_frames":1048576,"free_frames":20000,"unavailable_frames":1028576,"data_owned_frames":0,"table_owned_frames":0}"#,
            r#"{"schema":1,"event":"boot.display.ready","level":"info","width":1024,"height":768,"pitch":3072,"bpp":24}"#,
            r#"{"schema":1,"event":"boot.ready","level":"info","control_abi":"0.1","framebuffer":true}"#,
            r#"{"schema":1,"event":"kernel.interrupts.ready","level":"info","backend":"legacy_pc","vector_base":32,"vector_count":16,"pit_input_hz":1193182,"pit_divisor":11932,"timebase_generation":1,"initial_master_mask":255,"initial_slave_mask":255,"master_mask":252,"slave_mask":255,"input_available":true,"input_source_generation":1,"input_queue_capacity":64,"nmi_masked":true,"if_enabled":true}"#,
            r#"{"schema":1,"event":"kernel.shell_vessel_test.ready","level":"info","arch":"x86_64","policy":"fixed_supervised_shell","supervision_capacity":1,"relationships":1,"circles":2,"tasks":2,"queue_capacity":2,"restart_budget":1,"observation_capacity":1,"scheduler_gate_vector":242,"rite_gate_vector":243,"rite_grant":7,"transport_limit":4056,"timer_vector":32,"master_mask":254,"slave_mask":255,"privilege_stack_pages":4,"data_frames":14,"table_frames":8,"free_frame_delta":22,"ts_set":true,"fpu_state_saved":false,"nmi_masked":true,"bsp_only":true}"#,
            r#"{"schema":1,"event":"kernel.shell_vessel_test.vessel.generation.created","level":"info","role":"shell_vessel_fixture","supervision_generation":1,"incarnation":1,"circle_generation":1,"task_generation":1,"state":"ready","mappings":3,"privilege_stack_pages":4,"context_attached":true,"storage_zero":true,"replacement":false,"restart_budget_remaining":1,"rite_grant":7,"rite_sequence_start":1,"transport_zero":true,"initial_if":false}"#,
            r#"{"schema":1,"event":"kernel.shell_vessel_test.supervisor.ready","level":"info","role":"supervisor_fixture","supervision_generation":1,"circle_generation":1,"task_generation":1,"state":"ready","mappings":3,"privilege_stack_pages":4,"context_attached":true,"initial_if":false}"#,
            r#"{"schema":1,"event":"kernel.shell_vessel_test.task.dispatched","level":"info","role":"shell_vessel_fixture","incarnation":1,"dispatch":0,"kind":"initial","state":"running","root_selected":true,"stack_selected":true,"initial_if":false}"#,
            r#"{"schema":1,"event":"kernel.shell_vessel_test.rite.console.committed","level":"info","incarnation":1,"sequence":1,"status":"ok","bytes":17,"projection":"fixed_human_source"}"#,
            r#"{"schema":1,"event":"kernel.shell_vessel_test.rite.input.empty","level":"info","incarnation":1,"sequence":2,"status":"would_block","state_unchanged":true}"#,
            r#"{"schema":1,"event":"kernel.shell_vessel_test.task.preempted","level":"info","role":"shell_vessel_fixture","incarnation":1,"preemption":0,"tick":3,"state":"suspended","frame_retained":true,"eoi_sent":true}"#,
            r#"{"schema":1,"event":"kernel.shell_vessel_test.task.dispatched","level":"info","role":"supervisor_fixture","dispatch":1,"kind":"initial","state":"running","root_selected":true,"stack_selected":true,"initial_if":false}"#,
            r#"{"schema":1,"event":"kernel.shell_vessel_test.supervisor.blocked","level":"info","role":"supervisor_fixture","supervision_generation":1,"block":0,"wait":"await_fault","state":"blocked","frame_retained":true,"vessel_resumed":true}"#,
            r#"{"schema":1,"event":"kernel.shell_vessel_test.interrupts.masked","level":"info","master_mask":255,"slave_mask":255,"if_enabled":false}"#,
            r#"{"schema":1,"event":"kernel.shell_vessel_test.task.resumed","level":"info","role":"shell_vessel_fixture","incarnation":1,"dispatch":2,"resume":0,"state":"running","context_preserved":true,"frame_owned":true}"#,
            r#"{"schema":1,"event":"kernel.exception","level":"info","vector":6,"kind":"invalid_opcode","error_code":"0x0000000000000000","instruction_pointer":"0x0000700000001080","stack_pointer":"0x0000700000006000","stack_segment":"0x002b","stack_switched":true,"code_segment":"0x0033","flags":"0x0000000000000202","origin":"user","disposition":"contained_test","fault_address":null}"#,
            r#"{"schema":1,"event":"kernel.shell_vessel_test.vessel.generation.faulted","level":"info","role":"shell_vessel_fixture","supervision_generation":1,"incarnation":1,"state":"faulted","outcome":"invalid_instruction_fault","event_sequence":1,"vector":6,"error_code":0,"observation_published":true,"supervisor_woken":true,"frame_retained":true}"#,
            r#"{"schema":1,"event":"kernel.shell_vessel_test.task.resumed","level":"info","role":"supervisor_fixture","dispatch":3,"resume":1,"state":"running","context_preserved":true,"frame_owned":true}"#,
            r#"{"schema":1,"event":"kernel.shell_vessel_test.observation.consumed","level":"info","role":"supervisor_fixture","supervision_generation":1,"event_sequence":1,"incarnation":1,"outcome":"invalid_instruction_fault","single_use":true,"pending":false}"#,
            r#"{"schema":1,"event":"kernel.shell_vessel_test.restart.requested","level":"info","role":"supervisor_fixture","supervision_generation":1,"restart_sequence":1,"incarnation":1,"block":1,"wait":"restart_checkpoint","state":"blocked","restart_budget_remaining":0,"scheduler_empty":true,"kernel_continuation_restored":true}"#,
            r#"{"schema":1,"event":"kernel.shell_vessel_test.vessel.generation.withdrawn","level":"info","role":"shell_vessel_fixture","incarnation":1,"circle_generation":1,"task_generation":1,"outcome":"invalid_instruction_fault","context_withdrawn":true,"root_withdrawn":true,"storage_wiped":true,"privilege_stack_wiped":true,"task_identity_stale":true,"circle_identity_stale":true,"channel_retired":true,"transport_wiped":true}"#,
            r#"{"schema":1,"event":"kernel.shell_vessel_test.vessel.generation.created","level":"info","role":"shell_vessel_fixture","supervision_generation":1,"incarnation":2,"circle_generation":2,"task_generation":2,"state":"ready","mappings":3,"privilege_stack_pages":4,"context_attached":true,"storage_zero":true,"replacement":true,"restart_budget_remaining":0,"rite_grant":7,"rite_sequence_start":1,"transport_zero":true,"initial_if":false}"#,
            r#"{"schema":1,"event":"kernel.shell_vessel_test.task.dispatched","level":"info","role":"shell_vessel_fixture","incarnation":2,"dispatch":4,"kind":"initial","state":"running","root_selected":true,"stack_selected":true,"initial_if":false}"#,
            r#"{"schema":1,"event":"kernel.shell_vessel_test.rite.console.committed","level":"info","incarnation":2,"sequence":1,"status":"ok","bytes":17,"projection":"machine_direct","semantic_equal":true}"#,
            r#"{"schema":1,"event":"kernel.shell_vessel_test.rite.input.empty","level":"info","incarnation":2,"sequence":2,"status":"would_block","state_unchanged":true}"#,
            r#"{"schema":1,"event":"kernel.shell_vessel_test.rite.task.exited","level":"info","incarnation":2,"sequence":3,"status":"ok","state":"exited","returned_from_user":true,"reply_copied":false,"iret_to_exited_vessel":false,"supervisor_selected":true}"#,
            r#"{"schema":1,"event":"kernel.shell_vessel_test.vessel.generation.completed","level":"info","role":"shell_vessel_fixture","supervision_generation":1,"incarnation":2,"state":"exited","outcome":"successful_exit","event_sequence":2,"completion_gate":true,"observation_published":true,"supervisor_woken":true}"#,
            r#"{"schema":1,"event":"kernel.shell_vessel_test.task.resumed","level":"info","role":"supervisor_fixture","dispatch":5,"resume":2,"state":"running","context_preserved":true,"frame_owned":true}"#,
            r#"{"schema":1,"event":"kernel.shell_vessel_test.observation.consumed","level":"info","role":"supervisor_fixture","supervision_generation":1,"event_sequence":2,"incarnation":2,"outcome":"successful_exit","single_use":true,"pending":false}"#,
            r#"{"schema":1,"event":"kernel.shell_vessel_test.supervisor.completed","level":"info","role":"supervisor_fixture","state":"exited","completion_gate":true,"vessel_completed":true,"kernel_continuation_restored":true}"#,
            r#"{"schema":1,"event":"kernel.shell_vessel_test.vessel.generation.withdrawn","level":"info","role":"shell_vessel_fixture","incarnation":2,"circle_generation":2,"task_generation":2,"outcome":"successful_exit","context_withdrawn":true,"root_withdrawn":true,"storage_wiped":true,"privilege_stack_wiped":true,"task_identity_stale":true,"circle_identity_stale":true,"channel_retired":true,"transport_wiped":true}"#,
            r#"{"schema":1,"event":"kernel.shell_vessel_test.supervisor.withdrawn","level":"info","role":"supervisor_fixture","context_withdrawn":true,"root_withdrawn":true,"storage_wiped":true,"privilege_stack_wiped":true,"relationship_detached":true}"#,
            r#"{"schema":1,"event":"kernel.shell_vessel_test.released","level":"info","free_frames":20000,"initial_free_frames":20000,"data_owned_frames":0,"table_owned_frames":0,"active_circles":0,"active_tasks":0,"active_mappings":0,"active_endpoints":0,"active_sigils":0,"active_bindings":0,"active_lineages":0,"active_supervisions":0,"supervision_children":0,"active_leases":0,"bound_leases":0,"scheduler_depth":0,"attached_contexts":0,"active_rite_channels":0,"observation_cleared":true,"wait_cleared":true,"relationship_cleared":true,"channels_retired":true,"transport_wiped":true,"scratch_wiped":true,"kernel_cr3_restored":true,"tss_rsp0_restored":true,"ts_restored":true}"#,
            r#"{"schema":1,"event":"kernel.shell_vessel_test.passed","level":"info","dispatches":6,"preemptions":1,"resumes":3,"blocks":2,"wakes":2,"observations":2,"faults":1,"restarts":1,"completions":1,"rite_calls":5,"console_commits":2,"would_block":2,"terminal_exits":1,"sequence_reset":true,"clean_replacement":true,"supervisor_continued":true}"#,
        ]
        .join("\n")
    }

    const test_memory_bounds: memory_image_bounds = memory_image_bounds {
        kernel_start: 0x0010_0000,
        kernel_end: 0x0018_0000,
    };

    fn valid_memory_transcript() -> String {
        [
            r#"Threshold stage 2"#,
            r#"{"schema":1,"event":"boot.kernel.enter","level":"info"}"#,
            r#"{"schema":1,"event":"boot.omen.accepted","level":"info","version":2,"boot_drive":128}"#,
            r#"{"schema":1,"event":"boot.memory.map","level":"info","regions":4,"usable_bytes":81920000}"#,
            r#"{"schema":1,"event":"kernel.memory.ready","level":"info","frame_size":4096,"address_limit":4294967296,"total_frames":1048576,"free_frames":20000,"unavailable_frames":1028576,"data_owned_frames":0,"table_owned_frames":0}"#,
            r#"{"schema":1,"event":"kernel.fault_path.ready","level":"info","vectors":32}"#,
            r#"{"schema":1,"event":"boot.ready","level":"info","control_abi":"0.1","framebuffer":true}"#,
            r#"{"schema":1,"event":"kernel.memory_test.leases.ready","level":"info","data_a":"0x0000000000200000","data_b":"0x0000000000201000","root_a":"0x0000000000202000","root_b":"0x0000000000206000","data_frames":2,"table_frames":8,"zeroed":true,"distinct":true}"#,
            r#"{"schema":1,"event":"kernel.memory_test.roots.ready","level":"info","virtual_address":"0x0000500000000000","writable":true,"executable":false,"supervisor":true,"exact_walk":true,"duplicate_binding_rejected":true,"overlap_rejected":true}"#,
            r#"{"schema":1,"event":"kernel.memory_test.switch.verified","level":"info","sequence":"a_b_a","distinct":true,"marker_a":"0xa11ca11ca11ca11c","marker_b":"0xb22db22db22db22d","active_root_rejected":true}"#,
            r#"{"schema":1,"event":"kernel.memory_test.roots.withdrawn","level":"info","kernel_cr3_restored":true,"mappings_removed":true,"roots_inactive":true}"#,
            r#"{"schema":1,"event":"kernel.memory_test.reuse.verified","level":"info","data_a":"0x0000000000200000","data_b":"0x0000000000201000","zeroed":true,"same_frames":true}"#,
            r#"{"schema":1,"event":"kernel.memory_test.released","level":"info","free_frames":20000,"initial_free_frames":20000,"data_owned_frames":0,"table_owned_frames":0}"#,
            r#"{"schema":1,"event":"kernel.memory_test.physical_window.ready","level":"info","base":"0xffff900000000000","length":4294967296,"supervisor":true,"writable":true,"executable":false,"bootstrap_root_unchanged":true}"#,
            r#"{"schema":1,"event":"kernel.memory_test.staged.verified","level":"info","regions":4,"data_frames":7,"table_frames":9,"low_code":"0x0000000000400000","exact_walk":true,"guards_absent":true,"distinct_ownership":true,"inactive":true,"allocation_failures":5,"population_failures":1,"cleanup_failures":6,"retry_restored":true}"#,
            r#"{"schema":1,"event":"kernel.memory_test.staged.released","level":"info","free_frames":20000,"initial_free_frames":20000,"data_owned_frames":0,"table_owned_frames":0,"active_leases":0,"bound_leases":0,"window_removed":true,"kernel_cr3_restored":true}"#,
            r#"{"schema":1,"event":"kernel.memory_test.resident.ready","level":"info","roots":2,"user_regions":4,"data_frames":14,"table_frames":20,"probe_frames":1,"active_leases":11,"bound_leases":10,"kernel_permissions_checked":true,"descriptor_residency_checked":true,"supervisor_stack_checked":true}"#,
            r#"{"schema":1,"event":"kernel.memory_test.resident.verified","level":"info","switches":4,"low_code":"0x0000000000400000","physical_collision":"0x0000000000400000","isolated_values":true,"physical_alias_preserved":true,"read_only_faults":2,"guard_faults":2,"no_execute_faults":2,"exact_fault_resume":true,"accessed_dirty_validated":true,"bootstrap_restored":true}"#,
            r#"{"schema":1,"event":"kernel.memory_test.resident.released","level":"info","free_frames":20000,"initial_free_frames":20000,"data_owned_frames":0,"table_owned_frames":0,"active_leases":0,"bound_leases":0,"window_removed":true,"kernel_cr3_restored":true}"#,
            r#"{"schema":1,"event":"kernel.memory_test.user.ready","level":"info","roots":2,"tasks":2,"circles":2,"regions_per_root":3,"data_frames":10,"table_frames":18,"active_leases":8,"bound_leases":8,"code_base":"0x0000000000400000","guarded_stack":true,"entry_admitted":true,"privilege_stack_resident":true,"gpr_only":true}"#,
            r#"{"schema":1,"event":"kernel.memory_test.user.verified","level":"info","entries":2,"resumed_reports":4,"root_switches":3,"faulted_tasks":1,"exited_tasks":1,"supervisor_read_denied":true,"peer_continued":true,"private_data_isolated":true,"noninitial_stack_return":true,"bootstrap_restored":true,"ts_restored":true,"tss_restored":true,"continuations_cleared":true}"#,
            r#"{"schema":1,"event":"kernel.memory_test.user.released","level":"info","free_frames":20000,"initial_free_frames":20000,"data_owned_frames":0,"table_owned_frames":0,"active_leases":0,"bound_leases":0,"active_tasks":0,"active_circles":0,"attached_contexts":0,"generations_retired":true,"window_removed":true,"kernel_cr3_restored":true}"#,
            r#"{"schema":1,"event":"kernel.memory_test.launch.verified","level":"info","allocation_failures":4,"table_write_failures":1,"payload_failures":1,"circle_failures":1,"prepared_cancellations":1,"unstarted_cancellations":5,"cleanup_failures":5,"cleanup_retries":5,"successful_commits":2,"prepared_without_tasks":true,"cancelled_before_cleanup":true,"commit_before_entry":true,"retry_restored":true}"#,
            r#"{"schema":1,"event":"kernel.memory_test.passed","level":"info","roots":2,"switches":3,"reused_frames":2}"#,
        ]
        .join("\n")
    }

    fn valid_interrupt_transcript() -> String {
        [
            r#"{"schema":1,"event":"boot.kernel.enter","level":"info","arch":"x86_64"}"#,
            r#"{"schema":1,"event":"kernel.fault_path.ready","level":"info","vectors":32,"tss":true,"double_fault_ist":1,"nmi_ist":2,"nmi_masked":true}"#,
            r#"{"schema":1,"event":"boot.omen.accepted","level":"info","version":2,"boot_drive":128}"#,
            r#"{"schema":1,"event":"boot.memory.map","level":"info","regions":9,"usable_bytes":81920000}"#,
            r#"{"schema":1,"event":"kernel.memory.ready","level":"info","frame_size":4096,"address_limit":4294967296,"total_frames":1048576,"free_frames":20000,"unavailable_frames":1028576,"data_owned_frames":0,"table_owned_frames":0}"#,
            r#"{"schema":1,"event":"boot.display.ready","level":"info","width":1024,"height":768,"pitch":3072,"bpp":24}"#,
            r#"{"schema":1,"event":"boot.ready","level":"info","control_abi":"0.1","framebuffer":true}"#,
            r#"{"schema":1,"event":"kernel.interrupts.ready","level":"info","backend":"legacy_pc","vector_base":32,"vector_count":16,"pit_input_hz":1193182,"pit_divisor":11932,"timebase_generation":1,"initial_master_mask":255,"initial_slave_mask":255,"master_mask":252,"slave_mask":255,"input_available":true,"input_source_generation":1,"input_queue_capacity":64,"nmi_masked":true,"if_enabled":true}"#,
            r#"{"schema":1,"event":"kernel.interrupt_test.timer.verified","level":"info","timebase_generation":1,"start_tick":2,"end_tick":10,"interrupts":8}"#,
            r#"{"schema":1,"event":"kernel.interrupt_test.input.verified","level":"info","source_generation":1,"first_sequence":0,"last_sequence":23,"transitions":24,"ordered":true,"monotonic":true}"#,
            r#"{"schema":1,"event":"kernel.interrupt_test.return.verified","level":"info","timer":true,"keyboard":true,"state_preserved":true}"#,
            r#"{"schema":1,"event":"kernel.interrupt_test.loss.rearmed","level":"info","old_source_generation":1,"new_source_generation":2,"contained_master_mask":254,"rearmed_master_mask":252,"input_available":true}"#,
            r#"{"schema":1,"event":"kernel.interrupt_test.loss.verified","level":"info","source_generation":1,"first_sequence":88,"last_sequence":88,"lost_count":1,"delivered_before":true,"delivered_after":true,"resynchronization_required":true,"new_source_generation":2,"post_rearm_first_sequence":0,"post_rearm_last_sequence":1,"post_rearm_transitions":2}"#,
            r#"{"schema":1,"event":"kernel.interrupt_test.masked","level":"info","master_mask":255,"slave_mask":255,"if_enabled":false}"#,
            r#"{"schema":1,"event":"kernel.interrupt_test.passed","level":"info","ticks":10,"transitions":24,"lost":1}"#,
        ]
        .join("\n")
    }

    #[test]
    fn fnv_1a_matches_reference_vectors() {
        assert_eq!(fnv_1a(b""), 0x811c_9dc5);
        assert_eq!(fnv_1a(b"a"), 0xe40c_292c);
        assert_eq!(fnv_1a(b"foobar"), 0xbf9c_f968);
    }

    #[test]
    fn manifest_hash_covers_every_payload_field() {
        let manifest = threshold_manifest {
            stage_2: image_extent {
                file_size: 1024,
                sector_count: 2,
                hash: 0x1122_3344,
            },
            kernel: image_extent {
                file_size: 4096,
                sector_count: 8,
                hash: 0x5566_7788,
            },
            kernel_memory_size: 8192,
            kernel_entry_address: kernel_load_address,
        }
        .encode();
        assert_eq!(&manifest[0..8], &manifest_magic);
        let stored_hash = u32::from_le_bytes(
            manifest[manifest_hash_offset..manifest_hash_offset + 4]
                .try_into()
                .expect("fixed hash field"),
        );
        assert_eq!(
            stored_hash,
            fnv_1a(&manifest[manifest_hashed_offset..manifest_size])
        );
        assert!(manifest[manifest_size..].iter().all(|byte| *byte == 0));
    }

    #[test]
    fn smoke_runner_reaps_a_stalled_process() {
        let mut command = Command::new("/bin/sleep");
        command.arg("10");
        let started = Instant::now();
        let error = run_qemu_smoke(command, Duration::from_millis(25), &[ready_event])
            .expect_err("sleep must exceed the smoke deadline");
        assert!(error.contains("deadline"));
        assert!(started.elapsed() < Duration::from_secs(2));
    }

    #[test]
    fn interrupt_exit_wait_reaps_a_stalled_process() {
        let mut child = Command::new("/bin/sleep")
            .arg("10")
            .spawn()
            .expect("launch sleeper");
        let started = Instant::now();
        let error = wait_for_interrupt_exit(&mut child, Instant::now() + Duration::from_millis(25))
            .expect_err("sleep must exceed the interrupt deadline");
        assert!(error.contains("deadline"));
        assert!(started.elapsed() < Duration::from_secs(2));
        assert!(child.try_wait().expect("poll reaped child").is_some());
    }

    #[test]
    fn display_vessel_exit_wait_reaps_a_stalled_process() {
        let mut child = Command::new("/bin/sleep")
            .arg("10")
            .spawn()
            .expect("launch sleeper");
        let started = Instant::now();
        let error =
            wait_for_display_vessel_exit(&mut child, Instant::now() + Duration::from_millis(25))
                .expect_err("sleep must exceed the display-vessel deadline");
        assert!(error.contains("deadline"));
        assert!(started.elapsed() < Duration::from_secs(2));
        assert!(child.try_wait().expect("poll reaped child").is_some());
    }

    #[test]
    fn display_fixture_oracle_applies_the_frozen_rectangle_order() {
        let pixels = render_display_vessel_fixture().expect("bounded public fixture");
        let pixel = |x: usize, y: usize| {
            let offset = (y * display_vessel_width + x) * display_vessel_channels;
            rgb8 {
                red: pixels[offset],
                green: pixels[offset + 1],
                blue: pixels[offset + 2],
            }
        };
        assert_eq!(pixel(0, 767), display_background);
        assert_eq!(pixel(10, 10), display_surface);
        assert_eq!(pixel(10, 70), display_border);
        assert_eq!(pixel(33, 25), display_accent);
        assert_eq!(pixel(46, 102), display_highlight);
        assert_eq!(pixel(290, 120), display_shadow);
        assert_eq!(pixel(50, 106), display_border);
        assert_eq!(pixel(52, 108), display_surface);
        assert_eq!(pixel(68, 178), display_accent);
        assert_eq!(pixel(400, 200), display_ink);
        assert_eq!(pixel(384, 220), display_terminal);
        assert_eq!(pixel(384, 292), display_accent);
        assert_eq!(pixel(480, 292), display_terminal);
        assert_eq!(pixel(368, 480), display_accent);
        assert_eq!(pixel(371, 483), display_surface);
        assert_eq!(pixel(374, 486), display_accent);
        assert_eq!(pixel(376, 488), display_highlight);
        assert_eq!(pixel(464, 514), display_ink);
        assert_eq!(pixel(896, 592), display_accent);
    }

    #[test]
    fn display_frame_verifier_requires_canonical_complete_p6_and_every_pixel() {
        let mut ppm = display_vessel_ppm_header.to_vec();
        ppm.extend(render_display_vessel_fixture().expect("bounded public fixture"));
        verify_display_vessel_frame(&ppm).expect("exact public fixture frame");

        let last = ppm.len() - 1;
        ppm[last] ^= 1;
        let mismatch = verify_display_vessel_frame(&ppm).expect_err("changed pixel must fail");
        assert!(mismatch.contains("(1023, 767)"));
        ppm[last] ^= 1;

        let mut noncanonical = ppm.clone();
        noncanonical.splice(0..2, b"P3".iter().copied());
        assert!(verify_display_vessel_frame(&noncanonical).is_err());
        assert!(verify_display_vessel_frame(&ppm[..ppm.len() - 1]).is_err());
        ppm.push(0);
        assert!(verify_display_vessel_frame(&ppm).is_err());
    }

    #[test]
    fn display_vessel_transcript_accepts_only_the_exact_private_free_lifecycle() {
        let transcript = valid_display_vessel_transcript();
        verify_display_vessel_transcript(&transcript).expect("valid display-vessel lifecycle");

        let frame = transcript
            .lines()
            .find(|line| line.contains(r#""event":"kernel.display_vessel_test.frame.committed""#))
            .expect("frame record");
        let duplicate = transcript.replacen(frame, &format!("{frame}\n{frame}"), 1);
        assert!(verify_display_vessel_transcript(&duplicate).is_err());
        assert!(verify_display_vessel_transcript(&transcript.replacen(frame, "", 1)).is_err());

        let malformed = transcript
            .lines()
            .find(|line| line.contains(r#""kind":"malformed""#))
            .expect("malformed rejection");
        let bounds = transcript
            .lines()
            .find(|line| line.contains(r#""kind":"bounds""#))
            .expect("bounds rejection");
        let reordered = transcript
            .replacen(malformed, "__malformed__", 1)
            .replacen(bounds, malformed, 1)
            .replacen("__malformed__", bounds, 1);
        assert!(verify_display_vessel_transcript(&reordered).is_err());
    }

    #[test]
    fn display_vessel_capture_waits_for_every_byte_of_the_final_record_and_newline() {
        let transcript = valid_display_vessel_transcript();
        let final_start = transcript.rfind('\n').expect("preceding records") + 1;
        let prefix = &transcript[..final_start];
        let final_record = &transcript[final_start..];
        assert!(final_record.contains(display_vessel_passed_event));
        for end in 0..=final_record.len() {
            let partial = format!("{prefix}{}", &final_record[..end]);
            assert!(
                !display_vessel_serial_event_is_complete(&partial, display_vessel_passed_event),
                "accepted unterminated final record at byte {end}"
            );
            if end < final_record.len() {
                assert!(
                    !display_vessel_serial_event_is_complete(
                        &format!("{partial}\n"),
                        display_vessel_passed_event
                    ),
                    "accepted truncated final record at byte {end}"
                );
            }
        }
        assert!(!display_vessel_serial_event_is_complete(
            &format!("{transcript}\r"),
            display_vessel_passed_event
        ));
        for complete in [
            format!("{transcript}\n"),
            format!("{}\r\n", transcript.replace('\n', "\r\n")),
            format!("{transcript}\n\n"),
        ] {
            assert!(display_vessel_serial_event_is_complete(
                &complete,
                display_vessel_passed_event
            ));
        }
    }

    #[test]
    fn display_vessel_capture_rejects_forged_malformed_and_unverified_completion() {
        let transcript = valid_display_vessel_transcript();
        let final_start = transcript.rfind('\n').expect("preceding records") + 1;
        let prefix = &transcript[..final_start];
        let final_record = &transcript[final_start..];
        for record in [
            final_record.replace(
                display_vessel_passed_event,
                &format!("{display_vessel_passed_event}.forged"),
            ),
            format!(
                r#"{{"schema":1,"event":"unrelated","level":"info","note":"{display_vessel_passed_event}"}}"#
            ),
            final_record.replace(
                r#""terminal":true"#,
                &format!(r#""terminal":true,"unknown":"{display_vessel_passed_event}""#),
            ),
            final_record.replace(r#""calls":772"#, r#""calls":771"#),
            final_record.replace(r#""terminal":true"#, r#""terminal":true,"terminal":true"#),
            final_record.replace(r#""terminal":true"#, r#""terminal":"#),
        ] {
            let forged = format!("{prefix}{record}\n");
            assert!(forged.contains(display_vessel_passed_event));
            assert!(!display_vessel_serial_event_is_complete(
                &forged,
                display_vessel_passed_event
            ));
        }
        // A complete final event alone cannot authorize capture without the
        // preceding verified boot, admission, rendering and release lifecycle.
        assert!(!display_vessel_serial_event_is_complete(
            &format!("{final_record}\n"),
            display_vessel_passed_event
        ));
        assert!(!display_vessel_serial_event_is_complete(
            &format!("{transcript}\n"),
            "unrelated"
        ));
    }

    #[test]
    fn display_vessel_transcript_rejects_mutated_private_and_failed_evidence() {
        for (original, replacement) in [
            (r#""grant":20"#, r#""grant":16"#),
            (r#""calls":772"#, r#""calls":771"#),
            (r#""patches":768"#, r#""patches":767"#),
            (
                r#""aperture_user_accessible":false"#,
                r#""aperture_user_accessible":true"#,
            ),
            (r#""kind":"authority""#, r#""kind":"bounds""#),
            (r#""state_unchanged":true"#, r#""state_unchanged":false"#),
            (
                r#""surface_unchanged":true"#,
                r#""surface_unchanged":false"#,
            ),
            (r#""reply_copied":false"#, r#""reply_copied":true"#),
            (r#""active_tasks":0"#, r#""active_tasks":1"#),
            (r#""commits":768"#, r#""commits":767"#),
        ] {
            let transcript = valid_display_vessel_transcript();
            assert!(transcript.contains(original));
            assert!(
                verify_display_vessel_transcript(&transcript.replacen(original, replacement, 1))
                    .is_err(),
                "accepted display-vessel mutation {original} -> {replacement}"
            );
        }

        let private = valid_display_vessel_transcript().replace(
            r#""arch":"x86_64""#,
            r#""arch":"x86_64","payload":"secret""#,
        );
        assert!(verify_display_vessel_transcript(&private).is_err());
        let marker = format!(
            "{}\n{}",
            valid_display_vessel_transcript(),
            std::str::from_utf8(display_vessel_fixture_marker).expect("ASCII fixture marker")
        );
        assert!(verify_display_vessel_transcript(&marker).is_err());
        let unstructured = format!("{}\nleak", valid_display_vessel_transcript());
        assert!(verify_display_vessel_transcript(&unstructured).is_err());
        let failed = valid_display_vessel_transcript().replace(
            r#""event":"kernel.display_vessel_test.passed","level":"info""#,
            r#""event":"kernel.display_vessel_test.failed","level":"fatal""#,
        );
        assert!(verify_display_vessel_transcript(&failed).is_err());
    }

    #[test]
    fn display_vessel_public_errors_redact_unverified_content() {
        let secret = "private-display-value";
        let transcript = valid_display_vessel_transcript().replace("rgb8", secret);
        let error = verify_display_vessel_transcript_redacted(&transcript)
            .expect_err("mutated transcript must fail");
        assert!(!error.contains(secret));
        assert!(error.contains("without echoing"));
    }

    #[test]
    fn supervised_renderer_transcript_accepts_only_the_exact_lifecycle() {
        let transcript = valid_supervised_renderer_transcript();
        verify_supervised_renderer_transcript(&transcript, 2)
            .expect("valid supervised-renderer lifecycle");
        assert!(verify_supervised_renderer_transcript(&transcript, 3).is_err());

        let committed = transcript
            .lines()
            .find(|line| {
                line.contains(r#""event":"kernel.supervised_renderer_test.frame.committed""#)
            })
            .expect("frame commit record");
        let duplicate = transcript.replacen(committed, &format!("{committed}\n{committed}"), 1);
        assert!(verify_supervised_renderer_transcript(&duplicate, 2).is_err());
        assert!(
            verify_supervised_renderer_transcript(&transcript.replacen(committed, "", 1), 2)
                .is_err()
        );

        let faulted = transcript
            .lines()
            .find(|line| {
                line.contains(r#""event":"kernel.supervised_renderer_test.generation.faulted""#)
            })
            .expect("fault record");
        let restarted = transcript
            .lines()
            .find(|line| {
                line.contains(r#""event":"kernel.supervised_renderer_test.renderer.restarted""#)
            })
            .expect("restart record");
        let reordered = transcript
            .replacen(faulted, "__faulted__", 1)
            .replacen(restarted, faulted, 1)
            .replacen("__faulted__", restarted, 1);
        assert!(verify_supervised_renderer_transcript(&reordered, 2).is_err());
    }

    #[test]
    fn supervised_renderer_transcript_rejects_mutated_private_and_failed_evidence() {
        for (original, replacement) in [
            (r#""grant":20"#, r#""grant":19"#),
            (r#""image_pages":2"#, r#""image_pages":3"#),
            (r#""timer_interrupts":1"#, r#""timer_interrupts":2"#),
            (r#""rows":384"#, r#""rows":383"#),
            (r#""contained":true"#, r#""contained":false"#),
            (r#""stage_cleared":true"#, r#""stage_cleared":false"#),
            (r#""fresh_task":true"#, r#""fresh_task":false"#),
            (r#""rows":768"#, r#""rows":767"#),
            (r#""replied":false"#, r#""replied":true"#),
            (r#""observations":2"#, r#""observations":1"#),
            (
                r#""surface_withdrawn":true"#,
                r#""surface_withdrawn":false"#,
            ),
            (r#""free_frames":20000"#, r#""free_frames":19999"#),
            (r#""frame_commits":1"#, r#""frame_commits":2"#),
        ] {
            let transcript = valid_supervised_renderer_transcript();
            assert!(transcript.contains(original));
            assert!(
                verify_supervised_renderer_transcript(
                    &transcript.replacen(original, replacement, 1),
                    2,
                )
                .is_err(),
                "accepted supervised-renderer mutation {original} -> {replacement}"
            );
        }

        let private = valid_supervised_renderer_transcript()
            .replace(r#""grant":20"#, r#""grant":20,"payload":"secret""#);
        assert!(verify_supervised_renderer_transcript(&private, 2).is_err());

        let protected = std::str::from_utf8(&renderer_protected_source(7, 19))
            .expect("protected source is ASCII")
            .to_string();
        let leaked = valid_supervised_renderer_transcript().replace("x86_64", &protected);
        assert!(verify_supervised_renderer_transcript(&leaked, 2).is_err());

        let unstructured = format!("{}\nleak", valid_supervised_renderer_transcript());
        assert!(verify_supervised_renderer_transcript(&unstructured, 2).is_err());
        let leading_whitespace = valid_supervised_renderer_transcript().replacen('{', " {", 1);
        assert!(verify_supervised_renderer_transcript(&leading_whitespace, 2).is_err());
        let internal_whitespace =
            valid_supervised_renderer_transcript().replacen(",\"event\"", ", \"event\"", 1);
        assert!(verify_supervised_renderer_transcript(&internal_whitespace, 2).is_err());
        let blank_record = valid_supervised_renderer_transcript().replacen("\r\n", "\r\n\r\n", 1);
        assert!(verify_supervised_renderer_transcript(&blank_record, 2).is_err());
        let lf_only = valid_supervised_renderer_transcript().replace("\r\n", "\n");
        assert!(verify_supervised_renderer_transcript(&lf_only, 2).is_err());
        let reordered_fields = valid_supervised_renderer_transcript().replacen(
            r#"{"schema":1,"event":"boot.kernel.enter","level":"info","arch":"x86_64"}"#,
            r#"{"event":"boot.kernel.enter","schema":1,"level":"info","arch":"x86_64"}"#,
            1,
        );
        assert!(verify_supervised_renderer_transcript(&reordered_fields, 2).is_err());
        let escaped_spelling = valid_supervised_renderer_transcript().replacen(
            r#""arch":"x86_64""#,
            r#""arch":"x86_\u0036\u0034""#,
            1,
        );
        assert!(verify_supervised_renderer_transcript(&escaped_spelling, 2).is_err());
        let failed = valid_supervised_renderer_transcript().replace(
            r#""event":"kernel.supervised_renderer_test.passed","level":"info""#,
            r#""event":"kernel.supervised_renderer_test.failed","level":"fatal""#,
        );
        assert!(verify_supervised_renderer_transcript(&failed, 2).is_err());
    }

    #[test]
    fn display_vessel_serial_reads_are_hard_bounded() {
        let path = temporary_file("bounded-serial");
        fs::write(&path, b"123456789").expect("write oversized serial fixture");
        assert!(read_bounded_optional_utf8_file(&path, 8).is_err());
        fs::write(&path, b"ok").expect("replace serial fixture");
        assert_eq!(
            read_bounded_optional_utf8_file(&path, 8).expect("bounded serial fixture"),
            "ok"
        );
        fs::remove_file(&path).expect("remove serial fixture");
    }

    #[test]
    fn normal_exclusion_rejects_display_vessel_symbols_strings_and_marker() {
        verify_display_vessel_symbols_absent("kernel_main T 1000").expect("normal symbols");
        assert!(
            verify_display_vessel_symbols_absent(
                "_ZN17anomalous_kernel4arch6x86_6419display_vessel_test3run T 1000"
            )
            .is_err()
        );
        for symbol in display_vessel_forbidden_symbols {
            assert!(verify_display_vessel_symbols_absent(&format!("{symbol} T 1000")).is_err());
        }
        verify_display_vessel_bytes_absent(b"ordinary normal kernel").expect("normal bytes");
        for bytes in display_vessel_forbidden_binary_strings {
            assert!(verify_display_vessel_bytes_absent(bytes).is_err());
        }

        let feature_symbols = display_vessel_required_symbols
            .iter()
            .map(|symbol| format!("{symbol} T 1000"))
            .collect::<Vec<_>>()
            .join("\n");
        verify_display_vessel_symbols_present(&feature_symbols).expect("feature symbols present");
        for symbol in display_vessel_required_symbols {
            let missing = feature_symbols
                .lines()
                .filter(|line| line.split_whitespace().next() != Some(*symbol))
                .collect::<Vec<_>>()
                .join("\n");
            assert!(verify_display_vessel_symbols_present(&missing).is_err());
        }

        let mut feature_bytes = Vec::new();
        for required in display_vessel_forbidden_binary_strings {
            feature_bytes.extend_from_slice(required);
            feature_bytes.push(0);
        }
        verify_display_vessel_bytes_present(&feature_bytes).expect("feature payloads present");
        for required in display_vessel_forbidden_binary_strings {
            let without = feature_bytes
                .windows(required.len())
                .position(|window| window == *required)
                .map(|offset| {
                    let mut bytes = feature_bytes.clone();
                    bytes[offset] ^= 1;
                    bytes
                })
                .expect("required feature payload");
            assert!(verify_display_vessel_bytes_present(&without).is_err());
        }
    }

    #[test]
    fn normal_exclusion_rejects_supervised_renderer_symbols_and_private_bytes() {
        verify_supervised_renderer_symbols_absent("kernel_main T 1000").expect("normal symbols");
        for symbol in [
            "anomalous_supervised_renderer_generation_1_entry",
            "x86_64_supervised_renderer_payload_start",
            "_ZN17anomalous_kernel4arch6x86_6424supervised_renderer_test3run17h1234567890abcdefE",
        ] {
            assert!(
                verify_supervised_renderer_symbols_absent(&format!("{symbol} T 1000")).is_err()
            );
        }

        verify_supervised_renderer_bytes_absent(b"ordinary normal kernel").expect("normal bytes");
        for bytes in supervised_renderer_forbidden_binary_strings {
            assert!(verify_supervised_renderer_bytes_absent(bytes).is_err());
        }
        for bytes in supervised_renderer_private_source_sentinels {
            assert!(verify_supervised_renderer_bytes_absent(bytes).is_err());
        }

        let mut feature_symbols = supervised_renderer_kernel_required_symbols
            .iter()
            .map(|symbol| format!("{symbol} R 1000"))
            .collect::<Vec<_>>();
        feature_symbols.push(
            "_ZN17anomalous_kernel4arch6x86_6424supervised_renderer_test3run17h1234567890abcdefE T 2000"
                .into(),
        );
        let feature_symbols = feature_symbols.join("\n");
        verify_supervised_renderer_symbols_present(&feature_symbols)
            .expect("feature runtime symbols present");
        for required in supervised_renderer_kernel_required_symbols {
            let missing = feature_symbols
                .lines()
                .filter(|line| line.split_whitespace().next() != Some(*required))
                .collect::<Vec<_>>()
                .join("\n");
            assert!(verify_supervised_renderer_symbols_present(&missing).is_err());
        }

        let mut feature_bytes = Vec::new();
        for required in supervised_renderer_forbidden_binary_strings {
            feature_bytes.extend_from_slice(required);
            feature_bytes.push(0);
        }
        verify_supervised_renderer_bytes_present(&feature_bytes)
            .expect("feature runtime bytes present");
        for required in supervised_renderer_forbidden_binary_strings {
            let offset = feature_bytes
                .windows(required.len())
                .position(|window| window == *required)
                .expect("required feature payload");
            let mut missing = feature_bytes.clone();
            missing[offset] ^= 1;
            assert!(verify_supervised_renderer_bytes_present(&missing).is_err());
        }
    }

    #[test]
    fn json_parser_requires_a_complete_flat_record() {
        let record =
            json_parser::new(r#" {"schema":1,"event":"test\u002eevent","ok":true,"empty":null} "#)
                .parse_record()
                .expect("valid record");
        assert_eq!(record_u64(&record, "schema"), Some(1));
        assert_eq!(record_string(&record, "event"), Some("test.event"));
        assert_eq!(record.get("ok"), Some(&json_scalar::boolean(true)));
        assert!(
            json_parser::new(r#"{"schema":1} trailing"#)
                .parse_record()
                .is_err()
        );
        assert!(
            json_parser::new(r#"{"schema":1,"schema":1}"#)
                .parse_record()
                .is_err()
        );
    }

    #[test]
    fn control_transcript_accepts_only_the_closed_world_lifecycle() {
        verify_control_transcript(&valid_control_transcript()).expect("valid control lifecycle");

        let transcript = valid_control_transcript();
        let human_send = transcript
            .lines()
            .find(|line| {
                line.contains(r#""event":"kernel.control_test.message.sent""#)
                    && line.contains(r#""role":"human_fixture""#)
            })
            .expect("human send")
            .to_string();
        let duplicate = transcript.replacen(&human_send, &format!("{human_send}\n{human_send}"), 1);
        assert!(verify_control_transcript(&duplicate).is_err());
        assert!(verify_control_transcript(&transcript.replacen(&human_send, "", 1)).is_err());

        let machine_send = transcript
            .lines()
            .find(|line| {
                line.contains(r#""event":"kernel.control_test.message.sent""#)
                    && line.contains(r#""role":"machine_fixture""#)
            })
            .expect("machine send")
            .to_string();
        let reordered = transcript
            .replacen(&human_send, "__human_send__", 1)
            .replacen(&machine_send, &human_send, 1)
            .replacen("__human_send__", &machine_send, 1);
        assert!(verify_control_transcript(&reordered).is_err());
    }

    #[test]
    fn control_transcript_rejects_private_unknown_and_mutated_evidence() {
        for (original, replacement) in [
            (r#""sequence":0"#, r#""sequence":2"#),
            (r#""code_identical":true"#, r#""code_identical":false"#),
            (r#""state_unchanged":true"#, r#""state_unchanged":false"#),
            (r#""coequal":true"#, r#""coequal":false"#),
            (r#""scheduler":false"#, r#""scheduler":true"#),
            (r#""endpoint_capacity":4"#, r#""endpoint_capacity":3"#),
            (r#""nmi_masked":true"#, r#""nmi_masked":false"#),
            (
                r#""code_permissions":"user_rx""#,
                r#""code_permissions":"user_rw""#,
            ),
            (r#""active_tasks":0"#, r#""active_tasks":1"#),
        ] {
            let transcript = valid_control_transcript();
            assert!(transcript.contains(original));
            assert!(
                verify_control_transcript(&transcript.replacen(original, replacement, 1)).is_err()
            );
        }

        let private = valid_control_transcript().replace(
            r#""state_unchanged":true"#,
            r#""state_unchanged":true,"payload":"secret""#,
        );
        assert!(verify_control_transcript(&private).is_err());

        let private_boot = valid_control_transcript().replace(
            r#""arch":"x86_64""#,
            r#""arch":"x86_64","payload":"secret""#,
        );
        assert!(verify_control_transcript(&private_boot).is_err());

        let unknown = format!(
            "{}\n{}",
            valid_control_transcript(),
            r#"{"schema":1,"event":"kernel.control_test.unknown","level":"info"}"#
        );
        assert!(verify_control_transcript(&unknown).is_err());
        let unstructured = format!("{}\nleak", valid_control_transcript());
        assert!(verify_control_transcript(&unstructured).is_err());
    }

    #[test]
    fn normal_exclusion_rejects_control_symbols_and_private_bytes() {
        assert!(verify_control_symbols_absent("kernel_main T 1000").is_ok());
        for symbol in control_forbidden_symbols {
            assert!(verify_control_symbols_absent(&format!("{symbol} T 1000")).is_err());
        }
        for bytes in control_forbidden_binary_strings {
            assert!(verify_control_bytes_absent(bytes).is_err());
        }
    }

    #[test]
    fn rite_transcript_accepts_only_the_exact_lifecycle() {
        let transcript = valid_rite_transcript();
        verify_rite_transcript(&transcript).expect("valid rite lifecycle");

        let console = transcript
            .lines()
            .find(|line| line.contains(r#""event":"kernel.rite_test.console.committed""#))
            .expect("console commit record");
        let duplicate = transcript.replacen(console, &format!("{console}\n{console}"), 1);
        assert!(verify_rite_transcript(&duplicate).is_err());
        assert!(verify_rite_transcript(&transcript.replacen(console, "", 1)).is_err());

        let input = transcript
            .lines()
            .find(|line| line.contains(r#""event":"kernel.rite_test.input.empty""#))
            .expect("input empty record");
        let reordered = transcript
            .replacen(console, "__console__", 1)
            .replacen(input, console, 1)
            .replacen("__console__", input, 1);
        assert!(verify_rite_transcript(&reordered).is_err());
    }

    #[test]
    fn rite_transcript_rejects_mutated_or_private_evidence() {
        for (original, replacement) in [
            (r#""vector":243"#, r#""vector":242"#),
            (r#""grant":7"#, r#""grant":15"#),
            (r#""transport_limit":4056"#, r#""transport_limit":4055"#),
            (r#""status":"bad_operation""#, r#""status":"denied""#),
            (r#""sequence":1"#, r#""sequence":0"#),
            (r#""state_unchanged":true"#, r#""state_unchanged":false"#),
            (r#""bytes":16"#, r#""bytes":15"#),
            (r#""status":"would_block""#, r#""status":"ok""#),
            (r#""reply_copied":false"#, r#""reply_copied":true"#),
            (r#""iret":false"#, r#""iret":true"#),
            (r#""active_tasks":0"#, r#""active_tasks":1"#),
            (r#""channel_retired":true"#, r#""channel_retired":false"#),
            (r#""calls":9"#, r#""calls":8"#),
        ] {
            let transcript = valid_rite_transcript();
            assert!(transcript.contains(original));
            assert!(
                verify_rite_transcript(&transcript.replacen(original, replacement, 1)).is_err()
            );
        }

        let trace = valid_rite_transcript()
            .replace(r#""vector":243"#, r#""vector":243,"trace_id":"forbidden""#);
        assert!(verify_rite_transcript(&trace).is_err());

        let private_boot = valid_rite_transcript().replace(
            r#""arch":"x86_64""#,
            r#""arch":"x86_64","payload":"secret""#,
        );
        assert!(verify_rite_transcript(&private_boot).is_err());

        let sentinel = format!("{}\n{rite_private_sentinel_text}", valid_rite_transcript());
        assert!(verify_rite_transcript(&sentinel).is_err());

        let unknown = format!(
            "{}\n{}",
            valid_rite_transcript(),
            r#"{"schema":1,"event":"kernel.rite_test.unknown","level":"info"}"#
        );
        assert!(verify_rite_transcript(&unknown).is_err());
        let unstructured = format!("{}\nleak", valid_rite_transcript());
        assert!(verify_rite_transcript(&unstructured).is_err());
    }

    #[test]
    fn normal_exclusion_rejects_rite_symbols_and_private_bytes() {
        assert!(verify_rite_symbols_absent("kernel_main T 1000").is_ok());
        assert!(
            verify_rite_symbols_absent(
                "_ZN17anomalous_kernel4arch6x86_649rite_test3run17h1234567890abcdefE t 1000"
            )
            .is_err()
        );
        for symbol in rite_forbidden_symbols {
            assert!(verify_rite_symbols_absent(&format!("{symbol} T 1000")).is_err());
        }
        assert!(verify_rite_bytes_absent(b"ordinary normal kernel").is_ok());
        for bytes in rite_forbidden_binary_strings {
            assert!(verify_rite_bytes_absent(bytes).is_err());
        }
    }

    #[test]
    fn scheduler_transcript_accepts_only_the_exact_lifecycle() {
        let transcript = valid_scheduler_transcript();
        verify_test_scheduler_transcript(&transcript).expect("valid scheduler lifecycle");

        let resumed_fault = transcript
            .lines()
            .find(|line| {
                line.contains(r#""event":"kernel.scheduler_test.task.resumed""#)
                    && line.contains(r#""role":"faulting_fixture""#)
            })
            .expect("faulting resume record");
        let duplicate = transcript.replacen(
            resumed_fault,
            &format!("{resumed_fault}\n{resumed_fault}"),
            1,
        );
        assert!(verify_test_scheduler_transcript(&duplicate).is_err());
        assert!(
            verify_test_scheduler_transcript(&transcript.replacen(resumed_fault, "", 1)).is_err()
        );

        let first_preemption = transcript
            .lines()
            .find(|line| {
                line.contains(r#""event":"kernel.scheduler_test.task.preempted""#)
                    && line.contains(r#""role":"faulting_fixture""#)
            })
            .expect("first preemption record");
        let peer_dispatch = transcript
            .lines()
            .find(|line| {
                line.contains(r#""event":"kernel.scheduler_test.task.dispatched""#)
                    && line.contains(r#""role":"peer_fixture""#)
            })
            .expect("peer dispatch record");
        let reordered = transcript
            .replacen(first_preemption, "__first_preemption__", 1)
            .replacen(peer_dispatch, first_preemption, 1)
            .replacen("__first_preemption__", peer_dispatch, 1);
        assert!(verify_test_scheduler_transcript(&reordered).is_err());

        let resumed_flags = transcript.replace(
            r#""flags":"0x0000000000000202""#,
            r#""flags":"0x0000000000010202""#,
        );
        verify_test_scheduler_transcript(&resumed_flags)
            .expect("RF is permitted on the processor-pushed fault frame");
    }

    #[test]
    fn scheduler_transcript_rejects_mutated_or_private_evidence() {
        for (original, replacement) in [
            (r#""policy":"fixed_round_robin""#, r#""policy":"priority""#),
            (r#""preemption_budget":2"#, r#""preemption_budget":3"#),
            (r#""preemption":1,"tick":7"#, r#""preemption":1,"tick":3"#),
            (r#""dispatch":3,"resume":1"#, r#""dispatch":4,"resume":1"#),
            (
                r#""instruction_pointer":"0x0000700000001080""#,
                r#""instruction_pointer":"0x0000700000001081""#,
            ),
            (r#""stack_segment":"0x002b""#, r#""stack_segment":"0x0010""#),
            (r#""code_segment":"0x0033""#, r#""code_segment":"0x0008""#),
            (r#""peer_continued":true"#, r#""peer_continued":false"#),
            (
                r#""kernel_cr3_restored":true"#,
                r#""kernel_cr3_restored":false"#,
            ),
        ] {
            let mutated = valid_scheduler_transcript().replacen(original, replacement, 1);
            assert!(
                verify_test_scheduler_transcript(&mutated).is_err(),
                "accepted scheduler mutation {original} -> {replacement}"
            );
        }

        let private_field = valid_scheduler_transcript().replace(
            r#""policy":"fixed_round_robin""#,
            r#""policy":"fixed_round_robin","private_address":"0x0000000000000000""#,
        );
        assert!(verify_test_scheduler_transcript(&private_field).is_err());
        let private_marker = format!(
            "{}\n{{\"schema\":1,\"event\":\"kernel.scheduler_test.note\",\"level\":\"info\",\"value\":\"{:#018x}\"}}",
            valid_scheduler_transcript(),
            scheduler_private_markers[0]
        );
        assert!(verify_test_scheduler_transcript(&private_marker).is_err());
        let unstructured = format!("{}\nleak", valid_scheduler_transcript());
        assert!(verify_test_scheduler_transcript(&unstructured).is_err());
        let malformed = format!("{}\n{{\"schema\":1", valid_scheduler_transcript());
        assert!(verify_test_scheduler_transcript(&malformed).is_err());
        let unknown = format!(
            "{}\n{{\"schema\":1,\"event\":\"kernel.scheduler_test.unknown\",\"level\":\"info\"}}",
            valid_scheduler_transcript()
        );
        assert!(verify_test_scheduler_transcript(&unknown).is_err());
        let failure = valid_scheduler_transcript().replace(
            r#""event":"kernel.scheduler_test.passed","level":"info""#,
            r#""event":"kernel.scheduler_test.failed","level":"fatal""#,
        );
        assert!(verify_test_scheduler_transcript(&failure).is_err());
    }

    #[test]
    fn supervision_transcript_accepts_only_the_exact_lifecycle() {
        let transcript = valid_supervision_transcript();
        verify_test_supervision_transcript(&transcript).expect("valid supervision lifecycle");

        let fault_resume = transcript
            .lines()
            .find(|line| {
                line.contains(r#""event":"kernel.supervision_test.task.resumed""#)
                    && line.contains(r#""incarnation":1"#)
            })
            .expect("faulted vessel resume record");
        let duplicate =
            transcript.replacen(fault_resume, &format!("{fault_resume}\n{fault_resume}"), 1);
        assert!(verify_test_supervision_transcript(&duplicate).is_err());
        assert!(
            verify_test_supervision_transcript(&transcript.replacen(fault_resume, "", 1)).is_err()
        );

        let restart = transcript
            .lines()
            .find(|line| line.contains(r#""event":"kernel.supervision_test.restart.requested""#))
            .expect("restart record");
        let replacement = transcript
            .lines()
            .find(|line| {
                line.contains(r#""event":"kernel.supervision_test.vessel.generation.created""#)
                    && line.contains(r#""incarnation":2"#)
            })
            .expect("replacement record");
        let reordered = transcript
            .replacen(restart, "__restart__", 1)
            .replacen(replacement, restart, 1)
            .replacen("__restart__", replacement, 1);
        assert!(verify_test_supervision_transcript(&reordered).is_err());
    }

    #[test]
    fn supervision_transcript_rejects_mutated_or_private_evidence() {
        for (original, replacement) in [
            (r#""policy":"fixed_supervision""#, r#""policy":"priority""#),
            (r#""restart_budget":1"#, r#""restart_budget":2"#),
            (r#""dispatch":5,"resume":2"#, r#""dispatch":6,"resume":2"#),
            (
                r#""instruction_pointer":"0x0000700000001080""#,
                r#""instruction_pointer":"0x0000700000001081""#,
            ),
            (
                r#""clean_replacement":true"#,
                r#""clean_replacement":false"#,
            ),
            (
                r#""supervisor_continued":true"#,
                r#""supervisor_continued":false"#,
            ),
        ] {
            let mutated = valid_supervision_transcript().replacen(original, replacement, 1);
            assert!(
                verify_test_supervision_transcript(&mutated).is_err(),
                "accepted supervision mutation {original} -> {replacement}"
            );
        }

        let private_field = valid_supervision_transcript().replace(
            r#""policy":"fixed_supervision""#,
            r#""policy":"fixed_supervision","private_address":"0x0000000000000000""#,
        );
        assert!(verify_test_supervision_transcript(&private_field).is_err());
        let private_marker = format!(
            "{}\n{{\"schema\":1,\"event\":\"kernel.supervision_test.note\",\"level\":\"info\",\"value\":\"{:#018x}\"}}",
            valid_supervision_transcript(),
            supervision_private_markers[0]
        );
        assert!(verify_test_supervision_transcript(&private_marker).is_err());
        let unstructured = format!("{}\nleak", valid_supervision_transcript());
        assert!(verify_test_supervision_transcript(&unstructured).is_err());
        let malformed = format!("{}\n{{\"schema\":1", valid_supervision_transcript());
        assert!(verify_test_supervision_transcript(&malformed).is_err());
        let unknown = format!(
            "{}\n{{\"schema\":1,\"event\":\"kernel.supervision_test.unknown\",\"level\":\"info\"}}",
            valid_supervision_transcript()
        );
        assert!(verify_test_supervision_transcript(&unknown).is_err());
        let failure = valid_supervision_transcript().replace(
            r#""event":"kernel.supervision_test.passed","level":"info""#,
            r#""event":"kernel.supervision_test.failed","level":"fatal""#,
        );
        assert!(verify_test_supervision_transcript(&failure).is_err());
    }

    #[test]
    fn shell_vessel_transcript_accepts_only_the_exact_lifecycle() {
        let transcript = valid_shell_vessel_transcript();
        assert_eq!(expected_shell_vessel_serial_events.len(), 38);
        assert_eq!(transcript.lines().count(), 38);
        verify_test_shell_vessel_transcript(&transcript)
            .expect("valid composed shell-vessel lifecycle");

        let first_console = transcript
            .lines()
            .find(|line| {
                line.contains(r#""event":"kernel.shell_vessel_test.rite.console.committed""#)
                    && line.contains(r#""incarnation":1"#)
            })
            .expect("generation-1 console record");
        let duplicate = transcript.replacen(
            first_console,
            &format!("{first_console}\n{first_console}"),
            1,
        );
        assert!(verify_test_shell_vessel_transcript(&duplicate).is_err());
        assert!(
            verify_test_shell_vessel_transcript(&transcript.replacen(first_console, "", 1))
                .is_err()
        );

        let restart = transcript
            .lines()
            .find(|line| line.contains(r#""event":"kernel.shell_vessel_test.restart.requested""#))
            .expect("restart record");
        let replacement = transcript
            .lines()
            .find(|line| {
                line.contains(r#""event":"kernel.shell_vessel_test.vessel.generation.created""#)
                    && line.contains(r#""incarnation":2"#)
            })
            .expect("replacement record");
        let reordered = transcript
            .replacen(restart, "__restart__", 1)
            .replacen(replacement, restart, 1)
            .replacen("__restart__", replacement, 1);
        assert!(verify_test_shell_vessel_transcript(&reordered).is_err());

        let resumed_flags = transcript.replace(
            r#""flags":"0x0000000000000202""#,
            r#""flags":"0x0000000000010202""#,
        );
        verify_test_shell_vessel_transcript(&resumed_flags)
            .expect("RF is permitted on the processor-pushed fault frame");
    }

    #[test]
    fn shell_vessel_transcript_rejects_mutated_or_private_evidence() {
        for (original, replacement) in [
            (
                r#""policy":"fixed_supervised_shell""#,
                r#""policy":"priority""#,
            ),
            (r#""rite_grant":7"#, r#""rite_grant":6"#),
            (r#""transport_limit":4056"#, r#""transport_limit":4055"#),
            (
                r#""projection":"fixed_human_source""#,
                r#""projection":"machine_direct""#,
            ),
            (r#""bytes":17"#, r#""bytes":16"#),
            (r#""semantic_equal":true"#, r#""semantic_equal":false"#),
            (r#""status":"would_block""#, r#""status":"ok""#),
            (r#""preemption":0,"tick":3"#, r#""preemption":0,"tick":0"#),
            (r#""dispatch":5,"resume":2"#, r#""dispatch":6,"resume":2"#),
            (
                r#""instruction_pointer":"0x0000700000001080""#,
                r#""instruction_pointer":"0x0000700000001081""#,
            ),
            (r#""stack_segment":"0x002b""#, r#""stack_segment":"0x0010""#),
            (r#""code_segment":"0x0033""#, r#""code_segment":"0x0008""#),
            (r#""reply_copied":false"#, r#""reply_copied":true"#),
            (
                r#""iret_to_exited_vessel":false"#,
                r#""iret_to_exited_vessel":true"#,
            ),
            (
                r#""supervisor_selected":true"#,
                r#""supervisor_selected":false"#,
            ),
            (
                r#""replacement":true,"restart_budget_remaining":0,"rite_grant":7,"rite_sequence_start":1"#,
                r#""replacement":true,"restart_budget_remaining":0,"rite_grant":7,"rite_sequence_start":2"#,
            ),
            (r#""active_rite_channels":0"#, r#""active_rite_channels":1"#),
            (r#""channels_retired":true"#, r#""channels_retired":false"#),
            (r#""rite_calls":5"#, r#""rite_calls":4"#),
            (r#""sequence_reset":true"#, r#""sequence_reset":false"#),
        ] {
            let transcript = valid_shell_vessel_transcript();
            assert!(transcript.contains(original));
            assert!(
                verify_test_shell_vessel_transcript(&transcript.replacen(original, replacement, 1))
                    .is_err(),
                "accepted shell-vessel mutation {original} -> {replacement}"
            );
        }

        let private_field = valid_shell_vessel_transcript().replace(
            r#""policy":"fixed_supervised_shell""#,
            r#""policy":"fixed_supervised_shell","trace_id":"forbidden""#,
        );
        assert!(verify_test_shell_vessel_transcript(&private_field).is_err());

        let private_marker = valid_shell_vessel_transcript().replace(
            r#""policy":"fixed_supervised_shell""#,
            &format!(
                r#""policy":"fixed_supervised_shell","evidence":"{:#018x}""#,
                shell_vessel_private_markers[0]
            ),
        );
        assert!(verify_test_shell_vessel_transcript(&private_marker).is_err());

        for private in [
            String::from_utf8(shell_vessel_private_output.to_vec()).expect("UTF-8 output probe"),
            String::from_utf8(shell_vessel_private_source.to_vec()).expect("UTF-8 source probe"),
        ] {
            let disclosure = format!("{}\n{private}", valid_shell_vessel_transcript());
            assert!(verify_test_shell_vessel_transcript(&disclosure).is_err());
        }

        let unstructured = format!("{}\nleak", valid_shell_vessel_transcript());
        assert!(verify_test_shell_vessel_transcript(&unstructured).is_err());
        let malformed = format!("{}\n{{\"schema\":1", valid_shell_vessel_transcript());
        assert!(verify_test_shell_vessel_transcript(&malformed).is_err());
        let unknown = format!(
            "{}\n{{\"schema\":1,\"event\":\"kernel.shell_vessel_test.unknown\",\"level\":\"info\"}}",
            valid_shell_vessel_transcript()
        );
        assert!(verify_test_shell_vessel_transcript(&unknown).is_err());
        let failure = valid_shell_vessel_transcript().replace(
            r#""event":"kernel.shell_vessel_test.passed","level":"info""#,
            r#""event":"kernel.shell_vessel_test.failed","level":"fatal""#,
        );
        assert!(verify_test_shell_vessel_transcript(&failure).is_err());
        let non_info =
            valid_shell_vessel_transcript().replacen(r#""level":"info""#, r#""level":"debug""#, 1);
        assert!(verify_test_shell_vessel_transcript(&non_info).is_err());
        let wrong_schema =
            valid_shell_vessel_transcript().replacen(r#""schema":1"#, r#""schema":2"#, 1);
        assert!(verify_test_shell_vessel_transcript(&wrong_schema).is_err());
    }

    #[test]
    fn input_wait_transcript_accepts_only_the_exact_lifecycle() {
        let transcript = valid_input_wait_transcript();
        assert_eq!(expected_input_wait_serial_events.len(), 19);
        assert_eq!(transcript.lines().count(), 19);
        verify_input_wait_transcript(&transcript).expect("valid input-wait lifecycle");
        verify_input_wait_transcript(&transcript.replace('\n', "\r\n"))
            .expect("valid unterminated CRLF-framed input-wait lifecycle");
        verify_input_wait_transcript(&format!("{}\r\n", transcript.replace('\n', "\r\n")))
            .expect("valid terminated CRLF-framed input-wait lifecycle");

        let blocked = transcript
            .lines()
            .find(|line| line.contains(r#""event":"kernel.input_wait_test.wait.blocked""#))
            .expect("blocked event");
        let duplicate = transcript.replacen(blocked, &format!("{blocked}\n{blocked}"), 1);
        assert!(verify_input_wait_transcript(&duplicate).is_err());
        assert!(verify_input_wait_transcript(&transcript.replacen(blocked, "", 1)).is_err());

        let publication = transcript
            .lines()
            .find(|line| line.contains(r#""event":"kernel.input_wait_test.publication.woke""#))
            .expect("publication event");
        let delivery = transcript
            .lines()
            .find(|line| line.contains(r#""event":"kernel.input_wait_test.delivery.selected""#))
            .expect("delivery event");
        let reordered = transcript
            .replacen(publication, "__publication__", 1)
            .replacen(delivery, publication, 1)
            .replacen("__publication__", delivery, 1);
        assert!(verify_input_wait_transcript(&reordered).is_err());

        for blank_record in [
            format!("\n{transcript}"),
            transcript.replacen('\n', "\n\n", 1),
            format!("{transcript}\n\n"),
            transcript.replacen('\n', "\n \t\n", 1),
        ] {
            assert!(verify_input_wait_transcript(&blank_record).is_err());
        }
        for whitespace_record in [
            format!(" {transcript}"),
            format!("{transcript} "),
            transcript.replacen(r#""schema":1"#, r#""schema": 1"#, 1),
            transcript.replacen(r#""schema":1"#, "\t\"schema\":1", 1),
            transcript.replacen('\n', "\r\n", 1),
            transcript.replacen(r#""schema":1"#, "\r\"schema\":1", 1),
        ] {
            assert!(verify_input_wait_transcript(&whitespace_record).is_err());
        }
    }

    #[test]
    fn input_wait_transcript_rejects_mutated_or_private_evidence() {
        for (original, replacement) in [
            (
                r#""policy":"fixed_session_input""#,
                r#""policy":"shared_input""#,
            ),
            (r#""body_copied":true"#, r#""body_copied":false"#),
            (r#""pending_owned":true"#, r#""pending_owned":false"#),
            (
                r#""user_pointer_retained":false"#,
                r#""user_pointer_retained":true"#,
            ),
            (
                r#""exact_reservation":true"#,
                r#""exact_reservation":false"#,
            ),
            (
                r#""same_semantic_shape":true"#,
                r#""same_semantic_shape":false"#,
            ),
            (
                r#""publisher_remained_current":true"#,
                r#""publisher_remained_current":false"#,
            ),
            (r#""fifo_tail":true"#, r#""fifo_tail":false"#),
            (r#""source_consumed":false"#, r#""source_consumed":true"#),
            (
                r#""reserved_selection":true"#,
                r#""reserved_selection":false"#,
            ),
            (
                r#""route_queue_cleared":true"#,
                r#""route_queue_cleared":false"#,
            ),
            (r#""kind":"invalid_opcode""#, r#""kind":"page_fault""#),
            (r#""peer_terminal":true"#, r#""peer_terminal":false"#),
            (r#""payload_free":true"#, r#""payload_free":false"#),
            (
                r#""private_storage_cleared":true"#,
                r#""private_storage_cleared":false"#,
            ),
            (
                r#""kernel_continuation_restored":true"#,
                r#""kernel_continuation_restored":false"#,
            ),
            (r#""content_redacted":true"#, r#""content_redacted":false"#),
        ] {
            let transcript = valid_input_wait_transcript();
            assert!(transcript.contains(original));
            assert!(
                verify_input_wait_transcript(&transcript.replacen(original, replacement, 1))
                    .is_err(),
                "accepted input-wait mutation {original} -> {replacement}"
            );
        }

        let extra_field = valid_input_wait_transcript().replace(
            r#""policy":"fixed_session_input""#,
            r#""policy":"fixed_session_input","trace_id":"forbidden""#,
        );
        assert!(verify_input_wait_transcript(&extra_field).is_err());
        for private_field in [
            r#""action":1"#,
            r#""origin":2"#,
            r#""text_size":5"#,
            r#""erase_count":1"#,
            r#""endpoint_generation":1"#,
            r#""session_generation":1"#,
            r#""consumer_generation":1"#,
            r#""route_epoch":1"#,
            r#""wait_sequence":1"#,
            r#""readiness_generation":1"#,
            r#""delivery_sequence":1"#,
            r#""rite_sequence":1"#,
            r#""address":4096"#,
            r#""pointer":4096"#,
            r#""timestamp":1"#,
        ] {
            let injected = valid_input_wait_transcript().replace(
                r#""policy":"fixed_session_input""#,
                &format!(r#""policy":"fixed_session_input",{private_field}"#),
            );
            assert!(verify_input_wait_transcript(&injected).is_err());
        }
        let wrong_type = valid_input_wait_transcript().replacen(
            r#""content_redacted":true"#,
            r#""content_redacted":"true""#,
            1,
        );
        assert!(verify_input_wait_transcript(&wrong_type).is_err());

        for disclosure in [
            "☃",
            r"\u2603",
            "e29883",
            "e2 98 83",
            "e2_98_83",
            "e2-98-83",
            "101112131415161718191a1b1c1d1e1f",
            "10 11 12 13 14 15 16 17 18 19 1a 1b 1c 1d 1e 1f",
            "10_11_12_13_14_15_16_17_18_19_1a_1b_1c_1d_1e_1f",
            "10-11-12-13-14-15-16-17-18-19-1a-1b-1c-1d-1e-1f",
            "202122232425262728292a2b2c2d2e2f",
            "303132333435363738393a3b3c3d3e3f",
        ] {
            assert!(
                verify_input_wait_transcript(&format!(
                    "{}\n{disclosure}",
                    valid_input_wait_transcript()
                ))
                .is_err()
            );
        }

        let split_trace = format!(
            "{}\n1011121314151617\n18191a1b1c1d1e1f",
            valid_input_wait_transcript()
        );
        assert!(verify_input_wait_transcript(&split_trace).is_err());

        let unknown = format!(
            "{}\n{{\"schema\":1,\"event\":\"kernel.input_wait_test.unknown\",\"level\":\"info\"}}",
            valid_input_wait_transcript()
        );
        assert!(verify_input_wait_transcript(&unknown).is_err());
        let unstructured = format!("{}\nleak", valid_input_wait_transcript());
        assert!(verify_input_wait_transcript(&unstructured).is_err());
        let malformed = format!("{}\n{{\"schema\":1", valid_input_wait_transcript());
        assert!(verify_input_wait_transcript(&malformed).is_err());
        let failure = valid_input_wait_transcript().replace(
            r#""event":"kernel.input_wait_test.passed","level":"info""#,
            r#""event":"kernel.input_wait_test.failed","level":"fatal""#,
        );
        assert!(verify_input_wait_transcript(&failure).is_err());
        let non_info =
            valid_input_wait_transcript().replacen(r#""level":"info""#, r#""level":"debug""#, 1);
        assert!(verify_input_wait_transcript(&non_info).is_err());
        let wrong_schema =
            valid_input_wait_transcript().replacen(r#""schema":1"#, r#""schema":2"#, 1);
        assert!(verify_input_wait_transcript(&wrong_schema).is_err());
    }

    #[test]
    fn normal_exclusion_rejects_input_wait_symbols_and_private_bytes() {
        verify_input_wait_symbols_absent("kernel_main T 1000").expect("normal symbols");
        assert!(
            verify_input_wait_symbols_absent(
                "_ZN17anomalous_kernel4arch6x86_6415input_wait_test3run T 1000"
            )
            .is_err()
        );
        for symbol in input_wait_forbidden_symbols {
            assert!(verify_input_wait_symbols_absent(&format!("{symbol} T 1000")).is_err());
        }
        verify_input_wait_bytes_absent(b"ordinary normal kernel").expect("normal bytes");
        for bytes in input_wait_forbidden_binary_strings {
            assert!(verify_input_wait_bytes_absent(bytes).is_err());
        }
    }

    #[test]
    fn normal_exclusion_rejects_shell_vessel_symbols_and_private_bytes() {
        verify_shell_vessel_symbols_absent("kernel_main T 1000").expect("normal symbols");
        assert!(
            verify_shell_vessel_symbols_absent(
                "_ZN17anomalous_kernel4arch6x86_6417shell_vessel_test3run T 1000"
            )
            .is_err()
        );
        for symbol in shell_vessel_forbidden_symbols {
            assert!(verify_shell_vessel_symbols_absent(&format!("{symbol} T 1000")).is_err());
        }
        verify_shell_vessel_bytes_absent(b"ordinary normal kernel").expect("normal bytes");
        for bytes in shell_vessel_forbidden_binary_strings {
            assert!(verify_shell_vessel_bytes_absent(bytes).is_err());
        }
    }

    #[test]
    fn linked_shell_vessel_symbols_define_the_exact_runtime_fault_address() {
        assert_eq!(
            shell_vessel_fault_address_from_symbols(0x102000, 0x102080)
                .expect("bounded shell-vessel payload symbols"),
            test_shell_vessel_fault_address
        );
        assert!(shell_vessel_fault_address_from_symbols(0x102000, 0x101fff).is_err());
        assert!(shell_vessel_fault_address_from_symbols(0x102000, 0x103000).is_err());
        assert!(shell_vessel_fault_address_from_symbols(0, u64::MAX).is_err());

        let wrong_expectation = shell_vessel_fault_address {
            instruction_pointer: test_shell_vessel_fault_address.instruction_pointer + 1,
        };
        assert!(
            verify_shell_vessel_transcript(&valid_shell_vessel_transcript(), wrong_expectation)
                .is_err()
        );
    }

    #[test]
    fn normal_exclusion_rejects_scheduler_symbols_and_private_bytes() {
        verify_scheduler_symbols_absent("kernel_main T 1000").expect("normal symbols");
        for symbol in scheduler_forbidden_symbols {
            assert!(verify_scheduler_symbols_absent(&format!("{symbol} T 1000")).is_err());
        }
        assert!(
            verify_scheduler_symbols_absent(
                "_ZN17anomalous_kernel4arch6x86_6414scheduler_test3run T 1000"
            )
            .is_err()
        );
        verify_scheduler_bytes_absent(b"ordinary normal kernel").expect("normal bytes");
        for bytes in scheduler_forbidden_binary_strings {
            assert!(verify_scheduler_bytes_absent(bytes).is_err());
        }
    }

    #[test]
    fn linked_scheduler_symbols_define_the_exact_runtime_fault_address() {
        assert_eq!(
            scheduler_fault_address_from_symbols(0x102000, 0x102080)
                .expect("bounded scheduler payload symbols"),
            test_scheduler_fault_address
        );
        assert!(scheduler_fault_address_from_symbols(0x102000, 0x101fff).is_err());
        assert!(scheduler_fault_address_from_symbols(0x102000, 0x103000).is_err());
        assert!(scheduler_fault_address_from_symbols(0, u64::MAX).is_err());

        let wrong_expectation = scheduler_fault_address {
            instruction_pointer: test_scheduler_fault_address.instruction_pointer + 1,
        };
        assert!(
            verify_scheduler_transcript(&valid_scheduler_transcript(), wrong_expectation).is_err()
        );
    }

    #[test]
    fn circle_transcript_accepts_only_the_exact_lifecycle() {
        verify_test_circle_transcript(&valid_circle_transcript()).expect("valid circle lifecycle");

        let resume_flag_transcript = valid_circle_transcript().replace(
            r#""flags":"0x0000000000000002""#,
            r#""flags":"0x0000000000010002""#,
        );
        verify_test_circle_transcript(&resume_flag_transcript)
            .expect("RF is permitted on processor-pushed fault frames");

        let transcript = valid_circle_transcript();
        let mapped = transcript
            .lines()
            .find(|line| {
                line.contains(r#""event":"kernel.circle_test.generation.mapped""#)
                    && line.contains(r#""generation":1"#)
            })
            .expect("generation-1 mapping")
            .to_string();
        let duplicate = transcript.replacen(&mapped, &format!("{mapped}\n{mapped}"), 1);
        assert!(verify_test_circle_transcript(&duplicate).is_err());

        let missing = transcript.replacen(&mapped, "", 1);
        assert!(verify_test_circle_transcript(&missing).is_err());

        let entered = transcript
            .lines()
            .find(|line| {
                line.contains(r#""event":"kernel.circle_test.generation.entered""#)
                    && line.contains(r#""generation":1"#)
            })
            .expect("generation-1 entry")
            .to_string();
        let reordered = transcript
            .replacen(&mapped, "__mapped__", 1)
            .replacen(&entered, &mapped, 1)
            .replacen("__mapped__", &entered, 1);
        assert!(verify_test_circle_transcript(&reordered).is_err());
    }

    #[test]
    fn circle_transcript_rejects_bad_faults_and_failure_markers() {
        let bad_fault =
            valid_circle_transcript().replacen("0x0000000000000005", "0x0000000000000004", 1);
        assert!(verify_test_circle_transcript(&bad_fault).is_err());

        for forbidden in [
            r#"{"schema":1,"event":"kernel.exception.nested","level":"fatal"}"#,
            r#"{"schema":1,"event":"kernel.circle_test.after_denial","level":"error"}"#,
            r#"{"schema":1,"event":"kernel.circle_test.failed","level":"error"}"#,
        ] {
            let transcript = format!("{}\n{forbidden}", valid_circle_transcript());
            assert!(verify_test_circle_transcript(&transcript).is_err());
        }
    }

    #[test]
    fn circle_transcript_rejects_mutated_evidence_fields() {
        let mutations = [
            (r#""vectors":32"#, r#""vectors":31"#),
            (r#""tss":true"#, r#""tss":false"#),
            (r#""nx":true"#, r#""nx":false"#),
            (r#""write_protect":true"#, r#""write_protect":false"#),
            (r#""private_root":true"#, r#""private_root":false"#),
            (r#""scratch_zero":true"#, r#""scratch_zero":false"#),
            (r#""replacement":false"#, r#""replacement":true"#),
            (
                r#""generation":1,"origin":"user""#,
                r#""generation":1,"origin":"kernel""#,
            ),
            (
                r#""scratch_marker":"0x1616161616161616""#,
                r#""scratch_marker":"0x1616161616161617""#,
            ),
            (
                r#""instruction_pointer":"0x0000400000001080""#,
                r#""instruction_pointer":"0x0000400000002000""#,
            ),
            (
                r#""fault_address":"0x0000000000101000""#,
                r#""fault_address":"0x0000000100000000""#,
            ),
            (
                r#""stack_pointer":"0x0000400000006000""#,
                r#""stack_pointer":"0x0000400000005000""#,
            ),
            (r#""stack_segment":"0x002b""#, r#""stack_segment":"0x0010""#),
            (r#""code_segment":"0x0033""#, r#""code_segment":"0x0008""#),
            (
                r#""flags":"0x0000000000000002""#,
                r#""flags":"0x0000000000000003""#,
            ),
            (
                r#""protection_violation":true"#,
                r#""protection_violation":false"#,
            ),
            (r#""stack_switched":true"#, r#""stack_switched":false"#),
            (r#""write":false"#, r#""write":true"#),
            (r#""user":true"#, r#""user":false"#),
            (r#""reserved_bit":false"#, r#""reserved_bit":true"#),
            (
                r#""instruction_fetch":false"#,
                r#""instruction_fetch":true"#,
            ),
            (r#""protection_key":false"#, r#""protection_key":true"#),
            (r#""shadow_stack":false"#, r#""shadow_stack":true"#),
            (r#""enclave":false"#, r#""enclave":true"#),
            (
                r#""instruction_pointer":"0x0000400000005000""#,
                r#""instruction_pointer":"0x0000400000005001""#,
            ),
            (
                r#""fault_address":"0x0000400000005000""#,
                r#""fault_address":"0x0000400000005001""#,
            ),
            (r#""contained":true"#, r#""contained":false"#),
            (r#""root_withdrawn":true"#, r#""root_withdrawn":false"#),
            (r#""storage_wiped":true"#, r#""storage_wiped":false"#),
            (
                r#""transition_stack_wiped":true"#,
                r#""transition_stack_wiped":false"#,
            ),
            (r#""generations":3"#, r#""generations":2"#),
            (r#""denials":2"#, r#""denials":1"#),
        ];

        for (original, replacement) in mutations {
            let valid = valid_circle_transcript();
            assert!(valid.contains(original), "missing fixture field {original}");
            let mutated = valid.replacen(original, replacement, 1);
            assert!(
                verify_test_circle_transcript(&mutated).is_err(),
                "accepted mutation {original} -> {replacement}"
            );
        }

        let bad_level = valid_circle_transcript().replacen(
            r#""event":"kernel.circle_test.protections.ready","level":"info""#,
            r#""event":"kernel.circle_test.protections.ready","level":"warn""#,
            1,
        );
        assert!(verify_test_circle_transcript(&bad_level).is_err());

        let bad_schema = valid_circle_transcript().replacen(
            r#""schema":1,"event":"kernel.fault_path.ready""#,
            r#""schema":2,"event":"kernel.fault_path.ready""#,
            1,
        );
        assert!(verify_test_circle_transcript(&bad_schema).is_err());

        let bad_replacement = valid_circle_transcript().replacen(
            r#""generation":2,"scratch_zero":true,"private_root":true,"replacement":true"#,
            r#""generation":2,"scratch_zero":true,"private_root":true,"replacement":false"#,
            1,
        );
        assert!(verify_test_circle_transcript(&bad_replacement).is_err());

        let bad_denial = valid_circle_transcript().replacen(
            r#""generation":1,"kind":"supervisor_read","vector":14,"error_code":"0x5""#,
            r#""generation":1,"kind":"supervisor_read","vector":14,"error_code":"0x15""#,
            1,
        );
        assert!(verify_test_circle_transcript(&bad_denial).is_err());
    }

    #[test]
    fn circle_transcript_rejects_malformed_structured_lines() {
        let transcript = format!("{}\n{{\"schema\":1", valid_circle_transcript());
        assert!(verify_test_circle_transcript(&transcript).is_err());
    }

    #[test]
    fn interrupt_transcript_accepts_only_the_exact_private_lifecycle() {
        assert_eq!(
            verify_interrupt_transcript(&valid_interrupt_transcript())
                .expect("valid interrupt lifecycle"),
            interrupt_run_evidence {
                timebase_generation: 1,
                start_tick: 2,
                end_tick: 10,
                timer_interrupts: 8,
                source_generation: 1,
                first_sequence: 0,
                last_sequence: 23,
                transitions: 24,
                lost_count: 1,
            }
        );

        let valid = valid_interrupt_transcript();
        let input = valid
            .lines()
            .find(|line| line.contains(r#""event":"kernel.interrupt_test.input.verified""#))
            .expect("input proof");
        let duplicate = valid.replacen(input, &format!("{input}\n{input}"), 1);
        assert!(verify_interrupt_transcript(&duplicate).is_err());
        let missing = valid.replacen(input, "", 1);
        assert!(verify_interrupt_transcript(&missing).is_err());

        let timer = valid
            .lines()
            .find(|line| line.contains(r#""event":"kernel.interrupt_test.timer.verified""#))
            .expect("timer proof");
        let reordered = valid
            .replacen(timer, "__timer__", 1)
            .replacen(input, timer, 1)
            .replacen("__timer__", input, 1);
        assert!(verify_interrupt_transcript(&reordered).is_err());

        let unexpected = valid.replacen(
            "kernel.interrupt_test.return.verified",
            "kernel.interrupt_test.return.unexpected",
            1,
        );
        assert!(verify_interrupt_transcript(&unexpected).is_err());
    }

    #[test]
    fn interrupt_transcript_rejects_mutated_proof_fields() {
        let mutations = [
            (r#""backend":"legacy_pc""#, r#""backend":"opaque""#),
            (r#""vector_base":32"#, r#""vector_base":33"#),
            (r#""vector_count":16"#, r#""vector_count":15"#),
            (r#""pit_input_hz":1193182"#, r#""pit_input_hz":1193181"#),
            (r#""pit_divisor":11932"#, r#""pit_divisor":11931"#),
            (
                r#""initial_master_mask":255"#,
                r#""initial_master_mask":254"#,
            ),
            (r#""initial_slave_mask":255"#, r#""initial_slave_mask":254"#),
            (r#""master_mask":252"#, r#""master_mask":254"#),
            (r#""input_available":true"#, r#""input_available":false"#),
            (
                r#""input_queue_capacity":64"#,
                r#""input_queue_capacity":63"#,
            ),
            (r#""start_tick":2"#, r#""start_tick":3"#),
            (r#""interrupts":8"#, r#""interrupts":7"#),
            (r#""last_sequence":23"#, r#""last_sequence":22"#),
            (r#""transitions":24"#, r#""transitions":3"#),
            (r#""ordered":true"#, r#""ordered":false"#),
            (r#""monotonic":true"#, r#""monotonic":false"#),
            (r#""timer":true"#, r#""timer":false"#),
            (r#""keyboard":true"#, r#""keyboard":false"#),
            (r#""state_preserved":true"#, r#""state_preserved":false"#),
            (
                r#""old_source_generation":1"#,
                r#""old_source_generation":2"#,
            ),
            (
                r#""new_source_generation":2"#,
                r#""new_source_generation":3"#,
            ),
            (
                r#""contained_master_mask":254"#,
                r#""contained_master_mask":252"#,
            ),
            (
                r#""rearmed_master_mask":252"#,
                r#""rearmed_master_mask":254"#,
            ),
            (r#""first_sequence":88"#, r#""first_sequence":89"#),
            (r#""last_sequence":88"#, r#""last_sequence":89"#),
            (r#""lost_count":1"#, r#""lost_count":2"#),
            (r#""delivered_before":true"#, r#""delivered_before":false"#),
            (r#""delivered_after":true"#, r#""delivered_after":false"#),
            (
                r#""resynchronization_required":true"#,
                r#""resynchronization_required":false"#,
            ),
            (
                r#""post_rearm_first_sequence":0"#,
                r#""post_rearm_first_sequence":1"#,
            ),
            (
                r#""post_rearm_last_sequence":1"#,
                r#""post_rearm_last_sequence":2"#,
            ),
            (
                r#""post_rearm_transitions":2"#,
                r#""post_rearm_transitions":3"#,
            ),
            (r#""master_mask":255"#, r#""master_mask":254"#),
            (r#""slave_mask":255"#, r#""slave_mask":254"#),
            (r#""if_enabled":false"#, r#""if_enabled":true"#),
            (r#""ticks":10"#, r#""ticks":9"#),
            (r#""lost":1"#, r#""lost":2"#),
        ];

        for (original, replacement) in mutations {
            let valid = valid_interrupt_transcript();
            assert!(valid.contains(original), "missing fixture field {original}");
            let mutated = valid.replacen(original, replacement, 1);
            assert!(
                verify_interrupt_transcript(&mutated).is_err(),
                "accepted mutation {original} -> {replacement}"
            );
        }

        let wrong_generation = valid_interrupt_transcript().replacen(
            r#""timebase_generation":1,"start_tick"#,
            r#""timebase_generation":2,"start_tick"#,
            1,
        );
        assert!(verify_interrupt_transcript(&wrong_generation).is_err());
        let early_loss = valid_interrupt_transcript().replacen(
            r#""first_sequence":88,"last_sequence":88"#,
            r#""first_sequence":20,"last_sequence":20"#,
            1,
        );
        assert!(verify_interrupt_transcript(&early_loss).is_err());
    }

    #[test]
    fn interrupt_transcript_rejects_failure_and_private_disclosure() {
        let valid = valid_interrupt_transcript();
        for forbidden in [
            r#"{"schema":1,"event":"kernel.interrupt_test.failed","level":"error"}"#,
            r#"{"schema":1,"event":"unrelated.notice","level":"fatal"}"#,
            r#"{"schema":1,"event":"unrelated.failed","level":"warn"}"#,
        ] {
            let transcript = format!("{valid}\n{forbidden}");
            assert!(verify_interrupt_transcript(&transcript).is_err());
        }

        for field in interrupt_private_fields {
            let disclosure = format!(
                "{{\"schema\":1,\"event\":\"unrelated.notice\",\"level\":\"info\",\"{field}\":\"redacted\"}}"
            );
            let transcript = format!("{valid}\n{disclosure}");
            assert!(
                verify_interrupt_transcript(&transcript).is_err(),
                "accepted private field {field}"
            );
        }
        assert!(verify_interrupt_transcript(&format!("{valid}\n{privacy_probe}")).is_err());
        assert!(verify_interrupt_transcript(&format!("{valid}\nscan=0x19")).is_err());
        assert!(verify_interrupt_transcript(&format!(
            "{valid}\n{{\"schema\":1,\"event\":\"unrelated.notice\",\"level\":\"info\",\"sample_value\":25}}"
        ))
        .is_err());
        let extra_known_field = valid.replacen(
            r#""arch":"x86_64""#,
            r#""arch":"x86_64","sample_value":25"#,
            1,
        );
        assert!(verify_interrupt_transcript(&extra_known_field).is_err());
        assert!(verify_interrupt_transcript(&format!("{valid}\n{{\"schema\":1")).is_err());
    }

    #[test]
    fn memory_transcript_accepts_exact_allocator_evidence() {
        assert_eq!(
            verify_memory_transcript(&valid_memory_transcript(), test_memory_bounds)
                .expect("valid memory lifecycle"),
            memory_run_evidence {
                total_frames: memory_total_frames,
                initial_free_frames: 20_000,
                unavailable_frames: 1_028_576,
                data_a: 0x0020_0000,
                data_b: 0x0020_1000,
                root_a: 0x0020_2000,
                root_b: 0x0020_6000,
            }
        );
    }

    #[test]
    fn memory_transcript_rejects_owned_mapping_lifecycle_mutations() {
        let valid = valid_memory_transcript();
        let lines = valid.lines().collect::<Vec<_>>();
        for event in [
            "kernel.memory_test.physical_window.ready",
            "kernel.memory_test.staged.verified",
            "kernel.memory_test.staged.released",
            "kernel.memory_test.resident.ready",
            "kernel.memory_test.resident.verified",
            "kernel.memory_test.resident.released",
            "kernel.memory_test.user.ready",
            "kernel.memory_test.user.verified",
            "kernel.memory_test.user.released",
            "kernel.memory_test.launch.verified",
        ] {
            let index = lines
                .iter()
                .position(|line| line.contains(&format!(r#""event":"{event}""#)))
                .expect("owned mapping proof event");
            let record = lines[index];
            for (case, replacement) in [
                ("missing", String::new()),
                ("duplicate", format!("{record}\n{record}")),
                (
                    "unknown",
                    record.replacen(event, &format!("{event}.unexpected"), 1),
                ),
            ] {
                let transcript = valid.replacen(record, &replacement, 1);
                assert!(
                    verify_memory_transcript(&transcript, test_memory_bounds).is_err(),
                    "accepted {case} event {event}"
                );
            }
            for other in [index - 1, index + 1] {
                let mut reordered = lines.clone();
                reordered.swap(index, other);
                assert!(
                    verify_memory_transcript(&reordered.join("\n"), test_memory_bounds).is_err(),
                    "accepted reordered event {event}"
                );
            }
        }
    }

    #[test]
    fn memory_transcript_rejects_every_owned_mapping_field_mutation() {
        let valid = valid_memory_transcript();
        for event in [
            "kernel.memory_test.physical_window.ready",
            "kernel.memory_test.staged.verified",
            "kernel.memory_test.staged.released",
            "kernel.memory_test.resident.ready",
            "kernel.memory_test.resident.verified",
            "kernel.memory_test.resident.released",
            "kernel.memory_test.user.ready",
            "kernel.memory_test.user.verified",
            "kernel.memory_test.user.released",
            "kernel.memory_test.launch.verified",
        ] {
            let line = valid
                .lines()
                .find(|line| line.contains(&format!(r#""event":"{event}""#)))
                .expect("owned mapping proof event");
            let record = json_parser::new(line)
                .parse_record()
                .expect("valid fixture");
            for (field, value) in record {
                let (original, changed_value, changed_type) = match value {
                    json_scalar::number(number) => {
                        let changed = number.parse::<u64>().expect("fixture number") + 1;
                        (
                            number.clone(),
                            changed.to_string(),
                            format!(r#""{number}""#),
                        )
                    }
                    json_scalar::boolean(value) => (
                        value.to_string(),
                        (!value).to_string(),
                        format!(r#""{value}""#),
                    ),
                    json_scalar::string(value) => {
                        (format!(r#""{value}""#), r#""mutated""#.into(), "0".into())
                    }
                    json_scalar::null => panic!("memory fixture must not contain null"),
                };
                let original = format!(r#""{field}":{original}"#);
                assert!(line.contains(&original), "missing fixture field {field}");
                for replacement in [changed_value, changed_type] {
                    let replacement = format!(r#""{field}":{replacement}"#);
                    let mutated_line = line.replacen(&original, &replacement, 1);
                    let transcript = valid.replacen(line, &mutated_line, 1);
                    assert!(
                        verify_memory_transcript(&transcript, test_memory_bounds).is_err(),
                        "accepted {event} field mutation {original} -> {replacement}"
                    );
                }
                let removed = if line.contains(&format!("{original},")) {
                    line.replacen(&format!("{original},"), "", 1)
                } else {
                    line.replacen(&format!(",{original}"), "", 1)
                };
                assert_ne!(removed, line, "field removal missed {event} {field}");
                assert!(
                    verify_memory_transcript(
                        &valid.replacen(line, &removed, 1),
                        test_memory_bounds
                    )
                    .is_err(),
                    "accepted missing field {event} {field}"
                );
                let duplicate = format!("{},{original}}}", line.trim_end_matches('}'));
                assert!(
                    verify_memory_transcript(
                        &valid.replacen(line, &duplicate, 1),
                        test_memory_bounds
                    )
                    .is_err(),
                    "accepted duplicate field {event} {field}"
                );
            }
        }
    }

    #[test]
    fn memory_transcript_rejects_owned_mapping_private_fields() {
        let valid = valid_memory_transcript();
        for event in [
            "kernel.memory_test.physical_window.ready",
            "kernel.memory_test.staged.verified",
            "kernel.memory_test.staged.released",
            "kernel.memory_test.resident.ready",
            "kernel.memory_test.resident.verified",
            "kernel.memory_test.resident.released",
            "kernel.memory_test.user.ready",
            "kernel.memory_test.user.verified",
            "kernel.memory_test.user.released",
            "kernel.memory_test.launch.verified",
        ] {
            let line = valid
                .lines()
                .find(|line| line.contains(&format!(r#""event":"{event}""#)))
                .expect("owned mapping proof event");
            for extra in [
                r#""private_root":"0x0000000000200000""#,
                r#""private_marker":123456789"#,
                r#""schema":1"#,
            ] {
                let mutated_line = format!("{},{extra}}}", line.trim_end_matches('}'));
                let transcript = valid.replacen(line, &mutated_line, 1);
                assert!(
                    verify_memory_transcript(&transcript, test_memory_bounds).is_err(),
                    "accepted {event} with extra field {extra}"
                );
            }
        }
    }

    #[test]
    fn memory_transcript_rejects_lifecycle_mutations() {
        let valid = valid_memory_transcript();
        for prerequisite in [
            "boot.omen.accepted",
            "boot.memory.map",
            "kernel.memory.ready",
            "boot.ready",
        ] {
            let record = valid
                .lines()
                .find(|line| line.contains(&format!(r#""event":"{prerequisite}""#)))
                .expect("bootstrap prerequisite");
            let missing = valid.replacen(record, "", 1);
            assert!(
                verify_memory_transcript(&missing, test_memory_bounds).is_err(),
                "accepted lifecycle missing {prerequisite}"
            );
        }

        let memory_map = valid
            .lines()
            .find(|line| line.contains(r#""event":"boot.memory.map""#))
            .expect("memory-map event");
        let ready = valid
            .lines()
            .find(|line| line.contains(r#""event":"kernel.memory.ready""#))
            .expect("memory-ready event");
        let reordered_bootstrap = valid
            .replacen(memory_map, "__memory_map__", 1)
            .replacen(ready, memory_map, 1)
            .replacen("__memory_map__", ready, 1);
        assert!(verify_memory_transcript(&reordered_bootstrap, test_memory_bounds).is_err());

        let leases = valid
            .lines()
            .find(|line| line.contains(r#""event":"kernel.memory_test.leases.ready""#))
            .expect("lease event");
        let duplicate = valid.replacen(leases, &format!("{leases}\n{leases}"), 1);
        assert!(verify_memory_transcript(&duplicate, test_memory_bounds).is_err());

        let missing = valid.replacen(leases, "", 1);
        assert!(verify_memory_transcript(&missing, test_memory_bounds).is_err());

        let boot_ready = valid
            .lines()
            .find(|line| line.contains(r#""event":"boot.ready""#))
            .expect("boot-ready event");
        let late_boot_ready = valid
            .replacen(boot_ready, "__boot_ready__", 1)
            .replacen(leases, boot_ready, 1)
            .replacen("__boot_ready__", leases, 1);
        assert!(verify_memory_transcript(&late_boot_ready, test_memory_bounds).is_err());

        let roots = valid
            .lines()
            .find(|line| line.contains(r#""event":"kernel.memory_test.roots.ready""#))
            .expect("roots event");
        let reordered = valid
            .replacen(leases, "__leases__", 1)
            .replacen(roots, leases, 1)
            .replacen("__leases__", roots, 1);
        assert!(verify_memory_transcript(&reordered, test_memory_bounds).is_err());

        let unexpected = valid.replacen(
            "kernel.memory_test.roots.ready",
            "kernel.memory_test.roots.unexpected",
            1,
        );
        assert!(verify_memory_transcript(&unexpected, test_memory_bounds).is_err());

        let extra_field = valid.replacen(
            r#""frame_size":4096,"#,
            r#""frame_size":4096,"regions":4,"#,
            1,
        );
        assert!(verify_memory_transcript(&extra_field, test_memory_bounds).is_err());

        let extra_boot_field = valid.replacen(r#""version":2,"#, r#""version":2,"size":96,"#, 1);
        assert!(verify_memory_transcript(&extra_boot_field, test_memory_bounds).is_err());

        for forbidden in [
            r#"{"schema":1,"event":"kernel.memory_test.failed","level":"error"}"#,
            r#"{"schema":1,"event":"unrelated.failed","level":"error"}"#,
            r#"{"schema":1,"event":"unrelated.notice","level":"error"}"#,
            r#"{"schema":1,"event":"unrelated.fatal","level":"fatal"}"#,
        ] {
            let transcript = format!("{valid}\n{forbidden}");
            assert!(verify_memory_transcript(&transcript, test_memory_bounds).is_err());
        }
    }

    #[test]
    fn memory_transcript_rejects_mutated_evidence_fields() {
        let mutations = [
            (r#""version":2"#, r#""version":1"#),
            (r#""boot_drive":128"#, r#""boot_drive":129"#),
            (r#""regions":4"#, r#""regions":0"#),
            (r#""regions":4"#, r#""regions":65"#),
            (r#""usable_bytes":81920000"#, r#""usable_bytes":81919999"#),
            (r#""control_abi":"0.1""#, r#""control_abi":"0.2""#),
            (r#""framebuffer":true"#, r#""framebuffer":"true""#),
            (r#""frame_size":4096"#, r#""frame_size":8192"#),
            (
                r#""address_limit":4294967296"#,
                r#""address_limit":4294963200"#,
            ),
            (r#""total_frames":1048576"#, r#""total_frames":1048575"#),
            (
                r#""unavailable_frames":1028576"#,
                r#""unavailable_frames":1028575"#,
            ),
            (r#""data_owned_frames":0"#, r#""data_owned_frames":1"#),
            (r#""table_owned_frames":0"#, r#""table_owned_frames":1"#),
            (
                r#""data_a":"0x0000000000200000""#,
                r#""data_a":"0x0000000000201000""#,
            ),
            (
                r#""root_a":"0x0000000000202000""#,
                r#""root_a":"0x0000000000201000""#,
            ),
            (r#""data_frames":2"#, r#""data_frames":1"#),
            (r#""table_frames":8"#, r#""table_frames":4"#),
            (r#""zeroed":true"#, r#""zeroed":false"#),
            (r#""distinct":true"#, r#""distinct":false"#),
            (
                r#""virtual_address":"0x0000500000000000""#,
                r#""virtual_address":"0x0000500000001000""#,
            ),
            (r#""writable":true"#, r#""writable":false"#),
            (r#""executable":false"#, r#""executable":true"#),
            (r#""supervisor":true"#, r#""supervisor":false"#),
            (r#""exact_walk":true"#, r#""exact_walk":false"#),
            (
                r#""duplicate_binding_rejected":true"#,
                r#""duplicate_binding_rejected":false"#,
            ),
            (r#""overlap_rejected":true"#, r#""overlap_rejected":false"#),
            (r#""sequence":"a_b_a""#, r#""sequence":"a_a_b""#),
            (
                r#""marker_a":"0xa11ca11ca11ca11c""#,
                r#""marker_a":"0xa11ca11ca11ca11d""#,
            ),
            (
                r#""marker_b":"0xb22db22db22db22d""#,
                r#""marker_b":"0xb22db22db22db22c""#,
            ),
            (
                r#""active_root_rejected":true"#,
                r#""active_root_rejected":false"#,
            ),
            (
                r#""kernel_cr3_restored":true"#,
                r#""kernel_cr3_restored":false"#,
            ),
            (r#""mappings_removed":true"#, r#""mappings_removed":false"#),
            (r#""roots_inactive":true"#, r#""roots_inactive":false"#),
            (r#""same_frames":true"#, r#""same_frames":false"#),
            (r#""roots":2"#, r#""roots":1"#),
            (r#""switches":3"#, r#""switches":2"#),
            (r#""reused_frames":2"#, r#""reused_frames":1"#),
        ];

        for (original, replacement) in mutations {
            let valid = valid_memory_transcript();
            assert!(valid.contains(original), "missing fixture field {original}");
            let mutated = valid.replacen(original, replacement, 1);
            assert!(
                verify_memory_transcript(&mutated, test_memory_bounds).is_err(),
                "accepted mutation {original} -> {replacement}"
            );
        }

        for noncanonical in [
            "0x000000000020000",
            "0X0000000000200000",
            "0x000000000020000A",
            "0x0000000000200001",
        ] {
            let mutated = valid_memory_transcript().replacen("0x0000000000200000", noncanonical, 1);
            assert!(verify_memory_transcript(&mutated, test_memory_bounds).is_err());
        }
    }

    #[test]
    fn memory_transcript_rejects_malformed_structured_lines() {
        let transcript = format!("{}\n{{\"schema\":1", valid_memory_transcript());
        assert!(verify_memory_transcript(&transcript, test_memory_bounds).is_err());

        let duplicate_field = valid_memory_transcript().replacen(
            r#""frame_size":4096"#,
            r#""frame_size":4096,"frame_size":4096"#,
            1,
        );
        assert!(verify_memory_transcript(&duplicate_field, test_memory_bounds).is_err());
    }

    #[test]
    fn memory_size_evidence_requires_the_configured_ram_delta() {
        let small = verify_memory_transcript(&valid_memory_transcript(), test_memory_bounds)
            .expect("small evidence");
        let added = (320u64 - 96) * 1024 * 1024 / memory_frame_size;
        let large = memory_run_evidence {
            initial_free_frames: small.initial_free_frames + added,
            unavailable_frames: small.unavailable_frames - added,
            ..small
        };
        verify_memory_size_evidence(small, large).expect("two-size evidence");
        assert!(
            verify_memory_size_evidence(
                small,
                memory_run_evidence {
                    initial_free_frames: large.initial_free_frames - 1,
                    unavailable_frames: large.unavailable_frames + 1,
                    ..large
                }
            )
            .is_err()
        );
        assert!(
            verify_memory_size_evidence(
                small,
                memory_run_evidence {
                    data_a: large.data_a + memory_frame_size,
                    ..large
                }
            )
            .is_err()
        );
    }

    #[test]
    fn normal_image_exclusion_checks_symbols_strings_and_payload_markers() {
        verify_circle_symbols_absent("kernel_main T 100000 10\n").expect("normal symbols");
        assert!(verify_circle_symbols_absent("x86_64_enter_circle T 100000 10\n").is_err());
        verify_circle_bytes_absent(b"ordinary normal kernel").expect("normal bytes");
        assert!(verify_circle_bytes_absent(b"kernel.circle_test.passed").is_err());
        assert!(
            verify_circle_bytes_absent(&[0x00, 0x26, 0x26, 0x26, 0x26, 0x26, 0x26, 0x26, 0x26])
                .is_err()
        );
        verify_memory_symbols_absent("kernel_main T 100000 10\n").expect("normal symbols");
        assert!(verify_memory_symbols_absent("run_memory_test T 100000 10\n").is_err());
        assert!(
            verify_memory_symbols_absent(
                "_ZN17anomalous_kernel17user_mapping_test3run17h1234567890abcdefE t 100000 10\n"
            )
            .is_err()
        );
        for symbol in [
            "x86_64_owned_user_enter",
            "x86_64_owned_user_resume",
            "owned_user_payload_start",
            "owned_user_report_one",
        ] {
            assert!(verify_memory_symbols_absent(&format!("{symbol} T 100000 10\n")).is_err());
        }
        verify_memory_bytes_absent(b"ordinary normal kernel").expect("normal bytes");
        for event in [
            "kernel.memory_test.passed",
            "kernel.memory_test.physical_window.ready",
            "kernel.memory_test.staged.verified",
            "kernel.memory_test.staged.released",
            "kernel.memory_test.resident.ready",
            "kernel.memory_test.resident.verified",
            "kernel.memory_test.resident.released",
            "kernel.memory_test.user.ready",
            "kernel.memory_test.user.verified",
            "kernel.memory_test.user.released",
            "kernel.memory_test.launch.verified",
        ] {
            assert!(verify_memory_bytes_absent(event.as_bytes()).is_err());
        }
        assert!(verify_memory_bytes_absent(&memory_test_marker_a.to_le_bytes()).is_err());
        assert!(verify_memory_bytes_absent(&memory_test_marker_b.to_le_bytes()).is_err());
        verify_interrupt_symbols_absent("kernel_main T 100000 10\n").expect("normal symbols");
        assert!(verify_interrupt_symbols_absent("run_interrupt_test T 100000 10\n").is_err());
        assert!(
            verify_interrupt_symbols_absent(
                "_ZN17anomalous_kernel14interrupt_test3run17h1234567890abcdefE t 100000 10\n"
            )
            .is_err()
        );
        assert!(
            verify_interrupt_symbols_absent("x86_64_interrupt_test_register_probe T 100000 10\n")
                .is_err()
        );
        assert!(
            verify_interrupt_symbols_absent("interrupt_test_force_loss T 100000 10\n").is_err()
        );
        assert!(
            verify_interrupt_symbols_absent("read_external_interrupt_health T 100000 10\n")
                .is_err()
        );
        assert!(
            verify_interrupt_symbols_absent("run_interrupt_return_probe T 100000 10\n").is_err()
        );
        assert!(
            verify_interrupt_symbols_absent("x86_64_timer_interrupt_count B 100000 8\n").is_err()
        );
        assert!(verify_interrupt_symbols_absent(
            "_ZN9anomalous5input32force_ps2_capacity_loss_for_test17h1234567890abcdefE t 100000 10\n"
        )
        .is_err());
        verify_interrupt_bytes_absent(b"ordinary normal kernel").expect("normal bytes");
        assert!(verify_interrupt_bytes_absent(b"kernel.interrupt_test.passed").is_err());
        assert!(verify_interrupt_bytes_absent(privacy_probe.as_bytes()).is_err());
    }

    #[test]
    fn linked_circle_symbols_define_the_exact_runtime_fault_addresses() {
        let listing = concat!(
            "circle_kernel_canary R 101000 8\n",
            "x86_64_circle_generation_1_denied_read R 102080 3\n",
            "x86_64_circle_generation_1_start R 102000 40\n",
        );
        assert_eq!(
            symbol_u64_from_listing(listing, "circle_kernel_canary").expect("canary symbol"),
            0x101000
        );
        assert_eq!(
            circle_fault_addresses_from_symbols(0x102000, 0x102080, 0x101000)
                .expect("bounded payload symbols"),
            test_circle_fault_addresses
        );
        assert!(circle_fault_addresses_from_symbols(0x102000, 0x101fff, 0x101000).is_err());
        assert!(circle_fault_addresses_from_symbols(0x102000, 0x103000, 0x101000).is_err());
        assert!(circle_fault_addresses_from_symbols(0x102000, 0x102080, 0x200000).is_err());

        let wrong_expectation = circle_fault_addresses {
            generation_1_instruction_pointer: test_circle_fault_addresses
                .generation_1_instruction_pointer
                + 1,
            ..test_circle_fault_addresses
        };
        assert!(verify_circle_transcript(&valid_circle_transcript(), wrong_expectation).is_err());
    }

    #[test]
    fn supervised_renderer_private_sources_are_distinct_and_never_echoed() {
        let first = renderer_protected_source(7, 19);
        let second = renderer_protected_source(11, 23);
        assert_eq!(first.len(), 37);
        assert_eq!(second.len(), 37);
        assert_ne!(first, second);
        assert_eq!(
            first.as_slice(),
            supervised_renderer_private_source_sentinels[0]
        );
        assert_eq!(
            second.as_slice(),
            supervised_renderer_private_source_sentinels[1]
        );
        assert!(first.iter().all(u8::is_ascii_lowercase));
        assert!(second.iter().all(u8::is_ascii_lowercase));
        reject_supervised_renderer_protected_sources(b"public renderer bytes", "test artifact")
            .expect("public artifact");
        assert!(reject_supervised_renderer_protected_sources(&first, "test artifact").is_err());
        assert!(reject_supervised_renderer_protected_sources(&second, "test artifact").is_err());
    }

    #[test]
    fn supervised_renderer_disassembly_allows_only_the_fixed_software_gates() {
        let valid = concat!(
            "0000000000000000: int $0xf2\n",
            "0000000000000002: int $0xf3\n",
            "0000000000000004: int $0xf3\n",
            "0000000000000006: ud2\n",
        );
        verify_supervised_renderer_disassembly(valid).expect("fixed gates");
        assert!(verify_supervised_renderer_disassembly(&valid.replace("ud2", "cli")).is_err());
        assert!(verify_supervised_renderer_disassembly(&valid.replace("$0xf2", "$0x80")).is_err());
        assert!(verify_supervised_renderer_disassembly(&valid.replace("%never", "%xmm0")).is_ok());
        assert!(
            verify_supervised_renderer_disassembly(&format!("{valid}0: mov %xmm0,%rax\n")).is_err()
        );
        for instruction in [
            "fldl (%rax)",
            "fildl (%rax)",
            "fstpl (%rax)",
            "fnstcw (%rax)",
            "fldcw (%rax)",
            "fsqrt",
            "fchs",
            "fabs",
            "fistpl (%rax)",
            "xsave64 (%rax)",
            "xrstors64 (%rax)",
            "fxsave64 (%rax)",
            "fxrstor64 (%rax)",
            "vzeroupper",
            "rdrand %rax",
            "rdtsc",
            "wrpkru",
            "mov %cr3,%rax",
            "rep insl (%dx),%es:(%rdi)",
            "tilezero %tmm0",
            "ldtilecfg (%rax)",
            "(bad)",
        ] {
            assert!(
                verify_supervised_renderer_disassembly(&format!("{valid}8: {instruction}\n"))
                    .is_err(),
                "accepted forbidden renderer instruction `{instruction}`"
            );
        }
        assert!(
            verify_supervised_renderer_disassembly(&valid.replacen("int $0xf3\n", "", 1)).is_err()
        );
    }

    #[test]
    fn supervised_renderer_gate_tail_binds_every_contract_byte() {
        let load = elf64_program_header {
            kind: 1,
            flags: 5,
            offset: 0,
            virtual_address: 0x1000,
            physical_address: 0x1000,
            file_size: 64,
            memory_size: 64,
            alignment: circle_page_size,
        };
        let expected = [
            0x49, 0xbf, 0x00, 0x00, 0x08, 0x00, 0x00, 0x60, 0x00, 0x00, 0x6a, 0x02, 0x9d, 0xcd,
            0xf3,
        ];
        let return_address = 0x1030;
        let start = return_address as usize - load.virtual_address as usize - expected.len();
        let mut bytes = vec![0_u8; load.file_size as usize];
        bytes[start..start + expected.len()].copy_from_slice(&expected);
        verify_supervised_renderer_gate_tail(&bytes, load, return_address, &expected, "fixture")
            .expect("exact gate tail");

        for index in 0..expected.len() {
            let mut mutated = bytes.clone();
            mutated[start + index] ^= 1;
            assert!(
                verify_supervised_renderer_gate_tail(
                    &mutated,
                    load,
                    return_address,
                    &expected,
                    "fixture",
                )
                .is_err(),
                "accepted mutation at gate byte {index}"
            );
        }
    }

    #[test]
    fn semantic_renderer_disassembly_allows_only_four_fixed_rite_gates() {
        let valid = concat!(
            "0000600000001000: int $0xf3\n",
            "0000600000001010: int $0xf3\n",
            "0000600000001020: int $0xf3\n",
            "0000600000001030: int $0xf3\n",
            "0000600000001040: ud2\n",
        );
        verify_semantic_renderer_disassembly(valid).expect("fixed Rite gates");
        assert!(
            verify_semantic_renderer_disassembly(&valid.replacen("int $0xf3\n", "", 1)).is_err()
        );
        assert!(
            verify_semantic_renderer_disassembly(&format!("{valid}0000600000001050: int $0xf3\n"))
                .is_err()
        );
        assert!(verify_semantic_renderer_disassembly(&valid.replace("$0xf3", "$0xf2")).is_err());
        assert!(
            verify_semantic_renderer_disassembly(&format!("{valid}0000600000001050: int3\n"))
                .is_err()
        );
        for instruction in [
            "fldl (%rax)",
            "fnstcw (%rax)",
            "xsave64 (%rax)",
            "fxrstor64 (%rax)",
            "vzeroupper",
            "rdrand %rax",
            "rdtscp",
            "wrpkru",
            "mov %cr3,%rax",
            "rep insl (%dx),%es:(%rdi)",
            "ljmp $0x8,$0x1000",
            "sgdt (%rax)",
            "tilezero %tmm0",
            "(bad)",
        ] {
            assert!(
                verify_semantic_renderer_disassembly(&format!(
                    "{valid}0000600000001050: {instruction}\n"
                ))
                .is_err(),
                "accepted forbidden semantic renderer instruction `{instruction}`"
            );
        }
    }

    #[test]
    fn semantic_renderer_gate_tail_binds_every_contract_byte() {
        let load = elf64_program_header {
            kind: 1,
            flags: 5,
            offset: 0,
            virtual_address: 0x1000,
            physical_address: 0x1000,
            file_size: 64,
            memory_size: 64,
            alignment: circle_page_size,
        };
        let mut expected = vec![0x49, 0xbf];
        expected.extend_from_slice(&semantic_surface_renderer_mailbox.to_le_bytes());
        expected.extend_from_slice(&[0x6a, 0x02, 0x9d, 0xcd, 0xf3]);
        let return_address = 0x1030;
        let start = return_address as usize - load.virtual_address as usize - expected.len();
        let mut bytes = vec![0_u8; load.file_size as usize];
        bytes[start..start + expected.len()].copy_from_slice(&expected);
        verify_semantic_renderer_gate_tail(&bytes, load, return_address, &expected, "fixture")
            .expect("exact gate tail");

        for index in 0..expected.len() {
            let mut mutated = bytes.clone();
            mutated[start + index] ^= 1;
            assert!(
                verify_semantic_renderer_gate_tail(
                    &mutated,
                    load,
                    return_address,
                    &expected,
                    "fixture",
                )
                .is_err(),
                "accepted mutation at semantic gate byte {index}"
            );
        }
    }

    #[test]
    fn semantic_renderer_artifacts_reject_adr_0026_magic_header_and_structure() {
        reject_adr_0026_capsule(b"public semantic renderer bytes", "test artifact")
            .expect("public artifact");
        assert!(
            reject_adr_0026_capsule(supervised_renderer_capsule_magic, "magic-only artifact")
                .is_err()
        );
        assert!(
            reject_adr_0026_capsule(&supervised_renderer_capsule_header, "header-only artifact")
                .is_err()
        );

        let capsule = anomalous_desktop_renderer::scene_conformance::build_fixture_capsule(
            "semantic renderer capsule scan secret",
        )
        .expect("canonical ADR 0026 capsule");
        let exact = *capsule.as_bytes();
        assert!(looks_like_adr_0026_capsule(&exact));
        assert!(reject_adr_0026_capsule(&exact, "exact capsule artifact").is_err());

        let mut magic_mutated = exact;
        magic_mutated[0] ^= 1;
        assert!(
            !magic_mutated
                .windows(supervised_renderer_capsule_magic.len())
                .any(|window| window == supervised_renderer_capsule_magic)
        );
        assert!(
            !magic_mutated
                .windows(supervised_renderer_capsule_header.len())
                .any(|window| window == supervised_renderer_capsule_header)
        );
        assert!(looks_like_adr_0026_capsule(&magic_mutated));
        assert!(reject_adr_0026_capsule(&magic_mutated, "structured capsule artifact").is_err());

        let mut malformed = magic_mutated;
        malformed[4] ^= 1;
        assert!(!looks_like_adr_0026_capsule(&malformed));
        reject_adr_0026_capsule(&malformed, "non-capsule artifact").expect("non-capsule structure");
    }

    #[test]
    fn elf64_parser_bounds_tables_and_section_names() {
        let mut bytes = vec![0_u8; 203];
        bytes[0..4].copy_from_slice(b"\x7fELF");
        bytes[4] = 2;
        bytes[5] = 1;
        bytes[6] = 1;
        bytes[16..18].copy_from_slice(&1_u16.to_le_bytes());
        bytes[18..20].copy_from_slice(&62_u16.to_le_bytes());
        write_u32(&mut bytes, 20, 1);
        write_u64(&mut bytes, 40, 64);
        bytes[52..54].copy_from_slice(&64_u16.to_le_bytes());
        bytes[58..60].copy_from_slice(&64_u16.to_le_bytes());
        bytes[60..62].copy_from_slice(&2_u16.to_le_bytes());
        bytes[62..64].copy_from_slice(&1_u16.to_le_bytes());
        write_u32(&mut bytes, 128, 1);
        write_u32(&mut bytes, 132, 3);
        write_u64(&mut bytes, 152, 192);
        write_u64(&mut bytes, 160, 11);
        bytes[192..203].copy_from_slice(b"\0.shstrtab\0");

        let parsed = parse_elf64(&bytes).expect("bounded ELF64 fixture");
        assert_eq!(parsed.kind, 1);
        assert_eq!(parsed.machine, 62);
        assert!(parsed.program_headers.is_empty());
        assert_eq!(parsed.section_headers[1].name, ".shstrtab");

        let mut truncated = bytes.clone();
        truncated.pop();
        assert!(parse_elf64(&truncated).is_err());
        let mut wrong_class = bytes;
        wrong_class[4] = 1;
        assert!(parse_elf64(&wrong_class).is_err());
    }

    #[test]
    fn supervised_renderer_occurrence_counter_is_exact_and_overlapping() {
        assert_eq!(count_subslice(b"aaaa", b"aa"), 3);
        assert_eq!(count_subslice(b"renderer", b"render"), 1);
        assert_eq!(count_subslice(b"renderer", b"capsule"), 0);
        assert_eq!(count_subslice(b"renderer", b""), 0);
    }

    #[test]
    fn elf_relaunch_selects_one_payload_and_preserves_privileged_precedence() {
        let feature = elf_relaunch_smoke::feature;
        let lower = [
            (
                native_relaunch_smoke::feature,
                native_relaunch_smoke::selected as fn(&[&str]) -> bool,
            ),
            (elf_document_smoke::feature, elf_document_smoke::selected),
            (
                native_document_smoke::feature,
                native_document_smoke::selected,
            ),
            (
                native_runtime_smoke::feature,
                native_runtime_smoke::selected,
            ),
            (
                elf_application_smoke::feature,
                elf_application_smoke::selected,
            ),
            (
                native_application_smoke::feature,
                native_application_smoke::selected,
            ),
            (ny_object_smoke::feature, ny_object_smoke::selected),
            (
                application_profile_smoke::feature,
                application_profile_smoke::selected,
            ),
            (
                owned_dispatcher_smoke::feature,
                owned_dispatcher_smoke::selected,
            ),
            (
                owned_multi_region_smoke::feature,
                owned_multi_region_smoke::selected,
            ),
            (owned_source_smoke::feature, owned_source_smoke::selected),
            (owned_tls_smoke::feature, owned_tls_smoke::selected),
            (dispatcher_smoke::feature, dispatcher_smoke::selected),
            (workspace_dev::feature, workspace_dev::selected),
            (owned_workspace::feature, owned_workspace::selected),
            (owned_workspace_cpu::feature, owned_workspace_cpu::selected),
        ];
        assert!(!elf_relaunch_smoke::selected(&[]));
        assert!(elf_relaunch_smoke::selected(&[feature]));
        for (name, selected) in lower {
            assert!(elf_relaunch_smoke::selected(&[name, feature]), "{name}");
            assert!(!selected(&[feature, name]), "{name}");
        }
        let combined = lower
            .iter()
            .map(|(name, _)| *name)
            .chain([feature])
            .collect::<Vec<_>>();
        assert!(elf_relaunch_smoke::selected(&combined));
        for (name, selected) in lower {
            assert!(!selected(&combined), "{name}");
        }
        for higher in [
            "fault_test",
            "circle_test",
            "control_test",
            "rite_test",
            "memory_test",
            "interrupt_test",
            "supervision_test",
            "shell_vessel_test",
            "input_wait_test",
            "display_vessel_test",
            "supervised_renderer_test",
            "semantic_surface_transport_test",
            "semantic_surface_presentation_test",
            "desktop_dev",
            "linux_application_test",
            "veil_application_test",
            "shared_veil_test",
            "linux_tls_test",
            "linux_loader_entry_test",
        ] {
            assert!(
                !elf_relaunch_smoke::selected(&[feature, higher]),
                "{higher}"
            );
            let mut mixed = combined.clone();
            mixed.push(higher);
            assert!(!elf_relaunch_smoke::selected(&mixed), "{higher}");
        }
        for (higher, selected) in [
            (
                linux_application_smoke::feature,
                linux_application_smoke::selected as fn(&[&str]) -> bool,
            ),
            (
                veil_application_smoke::feature,
                veil_application_smoke::selected,
            ),
            (shared_veil_smoke::feature, shared_veil_smoke::selected),
            (linux_tls_smoke::feature, linux_tls_smoke::selected),
            (
                linux_loader_entry_smoke::feature,
                linux_loader_entry_smoke::selected,
            ),
        ] {
            assert!(selected(&[feature, higher]), "{higher}");
            let mut mixed = combined.clone();
            mixed.push(higher);
            assert!(selected(&mixed), "{higher}");
        }
        assert!(!elf_relaunch_smoke::selected(all_kernel_features));
    }

    #[test]
    fn native_relaunch_selects_one_payload_without_overriding_earlier_privileged_proofs() {
        let feature = native_relaunch_smoke::feature;
        let lower = [
            native_application_smoke::feature,
            native_runtime_smoke::feature,
            native_document_smoke::feature,
            elf_application_smoke::feature,
            elf_document_smoke::feature,
            ny_object_smoke::feature,
            application_profile_smoke::feature,
            owned_dispatcher_smoke::feature,
            owned_multi_region_smoke::feature,
            owned_source_smoke::feature,
            owned_tls_smoke::feature,
            dispatcher_smoke::feature,
        ];
        let selectors: [fn(&[&str]) -> bool; 12] = [
            native_application_smoke::selected,
            native_runtime_smoke::selected,
            native_document_smoke::selected,
            elf_application_smoke::selected,
            elf_document_smoke::selected,
            ny_object_smoke::selected,
            application_profile_smoke::selected,
            owned_dispatcher_smoke::selected,
            owned_multi_region_smoke::selected,
            owned_source_smoke::selected,
            owned_tls_smoke::selected,
            dispatcher_smoke::selected,
        ];
        assert!(native_relaunch_smoke::selected(&[feature]));
        for (lower, selected) in lower.into_iter().zip(selectors) {
            assert!(native_relaunch_smoke::selected(&[feature, lower]));
            assert!(!selected(&[feature, lower]));
        }
        let all_lower = lower.into_iter().chain([feature]).collect::<Vec<_>>();
        assert!(native_relaunch_smoke::selected(&all_lower));
        for selected in selectors {
            assert!(!selected(&all_lower));
        }
        for higher in [
            "fault_test",
            "circle_test",
            "control_test",
            "rite_test",
            "memory_test",
            "interrupt_test",
            "supervision_test",
            "shell_vessel_test",
            "input_wait_test",
            "display_vessel_test",
            "supervised_renderer_test",
            "semantic_surface_transport_test",
            "semantic_surface_presentation_test",
            "desktop_dev",
            "linux_application_test",
            "veil_application_test",
            "shared_veil_test",
            "linux_tls_test",
            "linux_loader_entry_test",
        ] {
            assert!(
                !native_relaunch_smoke::selected(&[feature, higher]),
                "{higher}"
            );
        }
        for (higher, selected) in [
            (
                linux_application_smoke::feature,
                linux_application_smoke::selected as fn(&[&str]) -> bool,
            ),
            (
                veil_application_smoke::feature,
                veil_application_smoke::selected,
            ),
            (shared_veil_smoke::feature, shared_veil_smoke::selected),
            (linux_tls_smoke::feature, linux_tls_smoke::selected),
            (
                linux_loader_entry_smoke::feature,
                linux_loader_entry_smoke::selected,
            ),
        ] {
            assert!(selected(&[feature, higher]));
        }
        assert!(!native_relaunch_smoke::selected(all_kernel_features));
    }

    #[test]
    fn elf_document_integration_selects_one_payload_and_preserves_higher_profiles() {
        let document = elf_document_smoke::feature;
        assert!(elf_document_smoke::selected(&[document]));
        let lower = [
            (
                elf_application_smoke::feature,
                elf_application_smoke::selected as fn(&[&str]) -> bool,
            ),
            (
                native_document_smoke::feature,
                native_document_smoke::selected,
            ),
            (
                native_runtime_smoke::feature,
                native_runtime_smoke::selected,
            ),
            (
                native_application_smoke::feature,
                native_application_smoke::selected,
            ),
            (ny_object_smoke::feature, ny_object_smoke::selected),
            (
                application_profile_smoke::feature,
                application_profile_smoke::selected,
            ),
            (
                owned_dispatcher_smoke::feature,
                owned_dispatcher_smoke::selected,
            ),
            (
                owned_multi_region_smoke::feature,
                owned_multi_region_smoke::selected,
            ),
            (owned_source_smoke::feature, owned_source_smoke::selected),
            (owned_tls_smoke::feature, owned_tls_smoke::selected),
            (dispatcher_smoke::feature, dispatcher_smoke::selected),
            (workspace_dev::feature, workspace_dev::selected),
            (owned_workspace::feature, owned_workspace::selected),
            (owned_workspace_cpu::feature, owned_workspace_cpu::selected),
        ];
        for (feature, selected) in lower {
            let features = [document, feature];
            assert!(elf_document_smoke::selected(&features), "{feature}");
            assert!(!selected(&features), "{feature}");
        }
        let mut combined = lower
            .iter()
            .map(|(feature, _)| *feature)
            .collect::<Vec<_>>();
        combined.push(document);
        assert!(elf_document_smoke::selected(&combined));
        for (_, selected) in lower {
            assert!(!selected(&combined));
        }
        for (feature, selected) in [
            (
                linux_application_smoke::feature,
                linux_application_smoke::selected as fn(&[&str]) -> bool,
            ),
            (
                veil_application_smoke::feature,
                veil_application_smoke::selected,
            ),
            (shared_veil_smoke::feature, shared_veil_smoke::selected),
            (linux_tls_smoke::feature, linux_tls_smoke::selected),
            (
                linux_loader_entry_smoke::feature,
                linux_loader_entry_smoke::selected,
            ),
        ] {
            let mut features = combined.clone();
            features.push(feature);
            assert!(!elf_document_smoke::selected(&features), "{feature}");
            assert!(selected(&features), "{feature}");
            for (_, lower_selected) in lower {
                assert!(!lower_selected(&features));
            }
        }
        for higher in [
            "fault_test",
            "circle_test",
            "desktop_dev",
            "semantic_surface_presentation_test",
        ] {
            assert!(!elf_document_smoke::selected(&[document, higher]));
        }
        assert!(!elf_document_smoke::selected(all_kernel_features));
    }

    #[test]
    fn native_runtime_integration_keeps_each_application_profile_exclusive() {
        let runtime = native_runtime_smoke::feature;
        assert!(native_runtime_smoke::selected(&[runtime]));
        for lower in [
            native_application_smoke::feature,
            ny_object_smoke::feature,
            application_profile_smoke::feature,
            owned_dispatcher_smoke::feature,
            owned_multi_region_smoke::feature,
            owned_source_smoke::feature,
            owned_tls_smoke::feature,
            dispatcher_smoke::feature,
            workspace_dev::feature,
            owned_workspace_cpu::feature,
            owned_workspace::feature,
        ] {
            let features = [runtime, lower];
            assert!(native_runtime_smoke::selected(&features));
            assert!(!native_application_smoke::selected(&features));
            assert!(!ny_object_smoke::selected(&features));
            assert!(!application_profile_smoke::selected(&features));
            assert!(!owned_dispatcher_smoke::selected(&features));
            assert!(!owned_multi_region_smoke::selected(&features));
            assert!(!owned_source_smoke::selected(&features));
            assert!(!owned_tls_smoke::selected(&features));
            assert!(!dispatcher_smoke::selected(&features));
            assert!(!workspace_dev::selected(&features));
            assert!(!owned_workspace::selected(&features));
            assert!(!owned_workspace_cpu::selected(&features));
        }
        for (higher, selected) in [
            (
                elf_application_smoke::feature,
                elf_application_smoke::selected as fn(&[&str]) -> bool,
            ),
            (
                linux_application_smoke::feature,
                linux_application_smoke::selected,
            ),
            (
                veil_application_smoke::feature,
                veil_application_smoke::selected,
            ),
            (shared_veil_smoke::feature, shared_veil_smoke::selected),
            (linux_tls_smoke::feature, linux_tls_smoke::selected),
            (
                linux_loader_entry_smoke::feature,
                linux_loader_entry_smoke::selected,
            ),
        ] {
            let features = [runtime, higher];
            assert!(!native_runtime_smoke::selected(&features));
            assert!(selected(&features));
        }
        let applications = [
            runtime,
            native_application_smoke::feature,
            elf_application_smoke::feature,
            linux_application_smoke::feature,
            veil_application_smoke::feature,
            shared_veil_smoke::feature,
            linux_tls_smoke::feature,
            linux_loader_entry_smoke::feature,
        ];
        assert!(linux_loader_entry_smoke::selected(&applications));
        assert!(!linux_tls_smoke::selected(&applications));
        assert!(!shared_veil_smoke::selected(&applications));
        assert!(!native_runtime_smoke::selected(&applications));
        assert!(!native_application_smoke::selected(&applications));
        assert!(!elf_application_smoke::selected(&applications));
        assert!(!linux_application_smoke::selected(&applications));
        assert!(!veil_application_smoke::selected(&applications));
        assert!(!native_runtime_smoke::selected(all_kernel_features));
    }

    #[test]
    fn durable_profile_selects_one_payload_and_preserves_higher_proofs() {
        let durable = native_durable_smoke::feature;
        for lower in [
            native_document_smoke::feature,
            native_runtime_smoke::feature,
            native_application_smoke::feature,
            workspace_dev::feature,
            owned_workspace_cpu::feature,
            owned_workspace::feature,
        ] {
            let features = [durable, lower];
            assert!(native_durable_smoke::selected(&features));
            assert!(!native_document_smoke::selected(&features));
            assert!(!native_runtime_smoke::selected(&features));
            assert!(!native_application_smoke::selected(&features));
            assert!(!workspace_dev::selected(&features));
            assert!(!owned_workspace::selected(&features));
            assert!(!owned_workspace_cpu::selected(&features));
        }
        for (higher, selected) in [
            (
                native_durable_failure_smoke::feature,
                native_durable_failure_smoke::selected as fn(&[&str]) -> bool,
            ),
            (
                native_repeated_smoke::feature,
                native_repeated_smoke::selected,
            ),
            (
                native_relaunch_smoke::feature,
                native_relaunch_smoke::selected,
            ),
            (elf_document_smoke::feature, elf_document_smoke::selected),
            (elf_relaunch_smoke::feature, elf_relaunch_smoke::selected),
            (
                elf_application_smoke::feature,
                elf_application_smoke::selected,
            ),
            (
                linux_application_smoke::feature,
                linux_application_smoke::selected,
            ),
            (
                veil_application_smoke::feature,
                veil_application_smoke::selected,
            ),
            (shared_veil_smoke::feature, shared_veil_smoke::selected),
            (linux_tls_smoke::feature, linux_tls_smoke::selected),
            (
                linux_loader_entry_smoke::feature,
                linux_loader_entry_smoke::selected,
            ),
        ] {
            assert!(!native_durable_smoke::selected(&[durable, higher]));
            assert!(selected(&[durable, higher]), "higher {higher}");
        }
        assert!(!native_durable_smoke::selected(all_kernel_features));
    }

    #[test]
    fn durable_failure_selects_one_payload_and_preserves_higher_proofs() {
        let failure = native_durable_failure_smoke::feature;
        assert!(!native_durable_failure_smoke::selected(&[]));
        assert!(!native_durable_failure_smoke::selected(&[
            native_durable_smoke::feature
        ]));
        assert!(native_durable_failure_smoke::selected(&[failure]));
        let lower_profiles = [
            (
                native_durable_smoke::feature,
                native_durable_smoke::selected as fn(&[&str]) -> bool,
            ),
            (
                native_repeated_smoke::feature,
                native_repeated_smoke::selected,
            ),
            (
                native_document_smoke::feature,
                native_document_smoke::selected,
            ),
            (
                native_runtime_smoke::feature,
                native_runtime_smoke::selected,
            ),
            (
                native_application_smoke::feature,
                native_application_smoke::selected,
            ),
            (ny_object_smoke::feature, ny_object_smoke::selected),
            (
                application_profile_smoke::feature,
                application_profile_smoke::selected,
            ),
            (
                owned_dispatcher_smoke::feature,
                owned_dispatcher_smoke::selected,
            ),
            (
                owned_multi_region_smoke::feature,
                owned_multi_region_smoke::selected,
            ),
            (owned_source_smoke::feature, owned_source_smoke::selected),
            (owned_tls_smoke::feature, owned_tls_smoke::selected),
            (dispatcher_smoke::feature, dispatcher_smoke::selected),
            (workspace_dev::feature, workspace_dev::selected),
            (owned_workspace::feature, owned_workspace::selected),
            (owned_workspace_cpu::feature, owned_workspace_cpu::selected),
        ];
        let mut expanded = vec![failure];
        for (lower, selected) in lower_profiles {
            assert!(native_durable_failure_smoke::selected(&[failure, lower]));
            assert!(!selected(&[failure, lower]), "lower {lower}");
            expanded.push(lower);
        }
        assert!(native_durable_failure_smoke::selected(&expanded));
        for (lower, selected) in lower_profiles {
            assert!(!selected(&expanded), "expanded lower {lower}");
        }
        for (higher, selected) in [
            (
                native_relaunch_smoke::feature,
                native_relaunch_smoke::selected as fn(&[&str]) -> bool,
            ),
            (elf_document_smoke::feature, elf_document_smoke::selected),
            (elf_relaunch_smoke::feature, elf_relaunch_smoke::selected),
            (
                elf_application_smoke::feature,
                elf_application_smoke::selected,
            ),
            (
                linux_application_smoke::feature,
                linux_application_smoke::selected,
            ),
            (
                veil_application_smoke::feature,
                veil_application_smoke::selected,
            ),
            (shared_veil_smoke::feature, shared_veil_smoke::selected),
            (linux_tls_smoke::feature, linux_tls_smoke::selected),
            (
                linux_loader_entry_smoke::feature,
                linux_loader_entry_smoke::selected,
            ),
        ] {
            let features = [failure, native_durable_smoke::feature, higher];
            assert!(!native_durable_failure_smoke::selected(&features));
            assert!(!native_durable_smoke::selected(&features));
            assert!(selected(&features), "higher {higher}");
        }
        for privileged in [
            "fault_test",
            "circle_test",
            "control_test",
            "rite_test",
            "memory_test",
            "interrupt_test",
            "supervision_test",
            "shell_vessel_test",
            "input_wait_test",
            "display_vessel_test",
            "supervised_renderer_test",
            "semantic_surface_transport_test",
            "semantic_surface_presentation_test",
            "desktop_dev",
        ] {
            assert!(!native_durable_failure_smoke::selected(&[
                failure, privileged
            ]));
        }
        assert!(!native_durable_failure_smoke::selected(all_kernel_features));
    }

    #[test]
    fn repeated_profile_reuses_durable_runtime_and_preserves_selection_priority() {
        let repeated = native_repeated_smoke::feature;
        assert!(!native_repeated_smoke::selected(&[]));
        assert!(native_repeated_smoke::selected(&[repeated]));
        assert!(!native_repeated_smoke::selected(&[
            native_durable_smoke::feature
        ]));
        let lower_profiles = [
            (
                native_durable_smoke::feature,
                native_durable_smoke::selected as fn(&[&str]) -> bool,
            ),
            (
                native_document_smoke::feature,
                native_document_smoke::selected,
            ),
            (
                native_runtime_smoke::feature,
                native_runtime_smoke::selected,
            ),
            (
                native_application_smoke::feature,
                native_application_smoke::selected,
            ),
            (ny_object_smoke::feature, ny_object_smoke::selected),
            (
                application_profile_smoke::feature,
                application_profile_smoke::selected,
            ),
            (
                owned_dispatcher_smoke::feature,
                owned_dispatcher_smoke::selected,
            ),
            (
                owned_multi_region_smoke::feature,
                owned_multi_region_smoke::selected,
            ),
            (owned_source_smoke::feature, owned_source_smoke::selected),
            (owned_tls_smoke::feature, owned_tls_smoke::selected),
            (dispatcher_smoke::feature, dispatcher_smoke::selected),
            (workspace_dev::feature, workspace_dev::selected),
            (owned_workspace::feature, owned_workspace::selected),
            (owned_workspace_cpu::feature, owned_workspace_cpu::selected),
        ];
        let mut expanded = vec![repeated];
        for (lower, selected) in lower_profiles {
            assert!(
                native_repeated_smoke::selected(&[repeated, lower]),
                "lower {lower}"
            );
            assert!(!selected(&[repeated, lower]), "lower {lower}");
            expanded.push(lower);
        }
        assert!(native_repeated_smoke::selected(&expanded));
        for (lower, selected) in lower_profiles {
            assert!(!selected(&expanded), "expanded lower {lower}");
        }
        for (higher, selected) in [
            (
                native_durable_failure_smoke::feature,
                native_durable_failure_smoke::selected as fn(&[&str]) -> bool,
            ),
            (
                native_relaunch_smoke::feature,
                native_relaunch_smoke::selected,
            ),
            (elf_document_smoke::feature, elf_document_smoke::selected),
            (elf_relaunch_smoke::feature, elf_relaunch_smoke::selected),
            (
                elf_application_smoke::feature,
                elf_application_smoke::selected,
            ),
            (
                linux_application_smoke::feature,
                linux_application_smoke::selected,
            ),
            (
                veil_application_smoke::feature,
                veil_application_smoke::selected,
            ),
            (shared_veil_smoke::feature, shared_veil_smoke::selected),
            (linux_tls_smoke::feature, linux_tls_smoke::selected),
            (
                linux_loader_entry_smoke::feature,
                linux_loader_entry_smoke::selected,
            ),
        ] {
            let features = [repeated, native_durable_smoke::feature, higher];
            assert!(!native_repeated_smoke::selected(&features));
            assert!(!native_durable_smoke::selected(&features));
            assert!(selected(&features), "higher {higher}");
        }
        for privileged in [
            "fault_test",
            "circle_test",
            "control_test",
            "rite_test",
            "memory_test",
            "interrupt_test",
            "supervision_test",
            "shell_vessel_test",
            "input_wait_test",
            "display_vessel_test",
            "supervised_renderer_test",
            "semantic_surface_transport_test",
            "semantic_surface_presentation_test",
            "desktop_dev",
        ] {
            assert!(!native_repeated_smoke::selected(&[repeated, privileged]));
        }
        assert!(!native_repeated_smoke::selected(all_kernel_features));
    }

    #[test]
    fn all_feature_link_sets_are_exact_cumulative_terminal_tiers() {
        const older_features: &[&str] = &[
            "smoke_test",
            "fault_test",
            "circle_test",
            "control_test",
            "rite_test",
            "memory_test",
            "interrupt_test",
            "scheduler_test",
            "supervision_test",
            "shell_vessel_test",
            "input_wait_test",
            "display_vessel_test",
        ];
        assert_eq!(
            supervised_renderer_all_kernel_features
                .split_last()
                .expect("supervised renderer feature tier"),
            (&supervised_renderer_feature, older_features)
        );
        assert_eq!(
            semantic_surface_transport_all_kernel_features
                .split_last()
                .expect("semantic transport feature tier"),
            (
                &semantic_surface_transport_feature,
                supervised_renderer_all_kernel_features,
            )
        );
        const development_features: &[&str] = &[
            desktop_dev::feature,
            workspace_dev::feature,
            owned_workspace_cpu::feature,
            owned_workspace::feature,
            dispatcher_smoke::feature,
            owned_dispatcher_smoke::feature,
            owned_multi_region_smoke::feature,
            owned_source_smoke::feature,
            owned_tls_smoke::feature,
            application_profile_smoke::feature,
            mixed_workspace::feature,
            native_editor_workspace::feature,
            ny_object_smoke::feature,
            native_application_smoke::feature,
            native_runtime_smoke::feature,
            native_manifest_smoke::feature,
            native_program_smoke::feature,
            native_durable_smoke::feature,
            native_durable_failure_smoke::feature,
            native_repeated_smoke::feature,
            native_interactive_smoke::feature,
            native_document_smoke::feature,
            elf_document_smoke::feature,
            native_relaunch_smoke::feature,
            elf_relaunch_smoke::feature,
            elf_application_smoke::feature,
            linux_application_smoke::feature,
            veil_application_smoke::feature,
            shared_veil_smoke::feature,
            linux_tls_smoke::feature,
            linux_loader_entry_smoke::feature,
        ];
        let (proof_features, development_tail) =
            all_kernel_features.split_at(all_kernel_features.len() - development_features.len());
        assert_eq!(development_tail, development_features);
        assert_eq!(
            proof_features
                .split_last()
                .expect("semantic presentation feature tier"),
            (
                &semantic_surface_presentation_feature,
                semantic_surface_transport_all_kernel_features,
            )
        );
    }

    #[test]
    fn semantic_presentation_feature_contract_is_present_excluded_and_precedent() {
        let runtime_symbol =
            "_ZN16anomalous_kernel4arch6x86_6434semantic_surface_presentation_test3runE";
        let listing = semantic_presentation_required_symbols
            .iter()
            .copied()
            .chain(std::iter::once(runtime_symbol))
            .map(|symbol| format!("{symbol} T 1000 10"))
            .collect::<Vec<_>>()
            .join("\n");
        verify_semantic_presentation_symbols_present(&listing)
            .expect("complete presentation feature symbols");
        for required in semantic_presentation_required_symbols {
            let incomplete = listing
                .lines()
                .filter(|line| line.split_whitespace().next() != Some(*required))
                .collect::<Vec<_>>()
                .join("\n");
            assert!(
                verify_semantic_presentation_symbols_present(&incomplete).is_err(),
                "accepted feature symbols without `{required}`"
            );
        }
        assert!(verify_semantic_presentation_symbols_absent("kernel_main T 1000 10").is_ok());
        assert!(verify_semantic_presentation_symbols_absent(&listing).is_err());

        let required_bytes = semantic_presentation_required_binary_strings.concat();
        verify_semantic_presentation_bytes_present(&required_bytes)
            .expect("complete presentation feature events");
        for required in semantic_presentation_required_binary_strings {
            assert!(verify_semantic_presentation_bytes_present(required).is_err());
        }
        let exact_body = [0xa5; surface_snapshot_v1_byte_count];
        verify_semantic_presentation_bytes_absent(b"ordinary normal kernel", &exact_body)
            .expect("normal bytes");
        for forbidden in semantic_presentation_forbidden_binary_strings {
            assert!(verify_semantic_presentation_bytes_absent(forbidden, &exact_body).is_err());
        }

        let shared_support = concat!(
            "x86_64_scheduler_enter T 1000 10\n",
            "x86_64_scheduler_resume_kernel T 1010 10\n",
            "x86_64_semantic_surface_renderer_entry T 1020 10\n",
        );
        verify_semantic_presentation_precedence_symbols(shared_support)
            .expect("presentation may retain shared scheduler and renderer support");
        verify_semantic_presentation_precedence_bytes(
            b"kernel.semantic_surface_transport.failed kernel.supervised_renderer_test.failed",
        )
        .expect("presentation may retain generic failure labels from shared support");
        for old in semantic_surface_preceded_fixture_symbols
            .iter()
            .copied()
            .chain(std::iter::once("run_semantic_surface_transport_test"))
        {
            assert!(
                verify_semantic_presentation_precedence_symbols(&format!("{old} T 1000 10"))
                    .is_err(),
                "accepted old terminal fixture symbol `{old}`"
            );
        }
        for marker in semantic_surface_preceded_fixture_event_markers
            .iter()
            .copied()
            .chain(std::iter::once(
                b"kernel.semantic_surface_transport.passed".as_slice(),
            ))
        {
            assert!(
                verify_semantic_presentation_precedence_bytes(marker).is_err(),
                "accepted old terminal fixture event `{}`",
                display_probe(marker)
            );
        }
    }

    #[test]
    fn normal_exclusion_rejects_exact_ordinal_61_body_without_private_disclosure() {
        let host = semantic_presentation_host_run::prepare(Path::new(
            "/tmp/anomalous-semantic-presentation-normal-exclusion",
        ))
        .expect("retained ordinal-61 host transaction");
        assert!(host.protected_probes.iter().all(|probe| {
            !host
                .snapshot
                .windows(probe.len())
                .any(|window| window == probe)
        }));

        let mut body_image = b"normal kernel prefix".to_vec();
        body_image.extend_from_slice(&host.snapshot);
        body_image.extend_from_slice(b"normal kernel suffix");
        let body_error = verify_semantic_presentation_bytes_absent(&body_image, &host.snapshot)
            .expect_err("accepted exact ordinal-61 body in normal image");
        assert_eq!(
            body_error,
            "normal kernel unexpectedly contains the semantic-presentation ordinal-61 body"
        );

        let mut protected_image = b"normal kernel prefix".to_vec();
        protected_image.extend_from_slice(&host.protected_probes[0]);
        let probe_error = reject_semantic_presentation_probes(
            &protected_image,
            "normal kernel image",
            &host.protected_probes,
        )
        .expect_err("accepted protected source in normal image");
        for probe in &host.protected_probes {
            let probe = std::str::from_utf8(probe).expect("ASCII protected probe");
            assert!(!body_error.contains(probe));
            assert!(!probe_error.contains(probe));
        }
    }

    #[test]
    fn semantic_presentation_build_inputs_clean_up_success_and_every_error_exit() {
        let build = temporary_file("semantic-presentation-build-inputs");
        fs::create_dir_all(&build).expect("create build-input test directory");
        let host = semantic_presentation_host_run::prepare(&build)
            .expect("retained ordinal-61 host transaction");
        let first = build.join("semantic_presentation_snapshot_first.bin");
        let second = build.join("semantic_presentation_snapshot_second.bin");
        let intended_artifact = build.join("semantic_surface_source.bin");
        fs::write(&intended_artifact, b"intended packaged artifact")
            .expect("write intended artifact");

        {
            let inputs = semantic_presentation_build_inputs::prepare(&build, &host.snapshot)
                .expect("prepare successful build inputs");
            assert_eq!(fs::read(&first).expect("first snapshot"), host.snapshot);
            assert_eq!(fs::read(&second).expect("second snapshot"), host.snapshot);
            inputs.remove_all().expect("successful-path cleanup");
        }
        assert!(!first.exists());
        assert!(!second.exists());
        assert!(intended_artifact.exists());

        let downstream_error: Result<(), String> = (|| {
            let _inputs = semantic_presentation_build_inputs::prepare(&build, &host.snapshot)?;
            Err("forced downstream packaging failure".into())
        })();
        assert!(downstream_error.is_err());
        assert!(!first.exists());
        assert!(!second.exists());
        assert!(intended_artifact.exists());

        fs::create_dir(&second).expect("create second-path obstruction");
        assert!(semantic_presentation_build_inputs::prepare(&build, &host.snapshot).is_err());
        assert!(!first.exists());
        assert!(second.is_dir());
        assert!(intended_artifact.exists());

        fs::remove_dir(&second).expect("remove second-path obstruction");
        fs::remove_dir_all(&build).expect("remove build-input test directory");
    }

    #[test]
    fn semantic_presentation_capture_waits_for_every_byte_of_the_final_record() {
        let valid = valid_semantic_presentation_transcript();
        let final_start = valid
            .strip_suffix("\r\n")
            .expect("canonical fixture termination")
            .rfind("\r\n")
            .expect("fixture has preceding records")
            + 2;
        let (preceding, final_record) = valid.split_at(final_start);
        assert!(final_record.is_ascii());
        assert!(semantic_presentation_passed_record_is_complete(&valid));
        assert!(semantic_presentation_passed_record_is_complete(
            final_record
        ));
        for end in 0..final_record.len() {
            assert!(
                !semantic_presentation_passed_record_is_complete(&format!(
                    "{preceding}{}",
                    &final_record[..end]
                )),
                "capture accepted the final record after only {end} bytes"
            );
        }

        for record in [
            final_record.replace(".passed\"", ".failed\""),
            final_record.replace(".passed\"", ".passed.extra\""),
            final_record.replace("\"event\":\"", "\"event\":0,\"unexpected\":\""),
            final_record.replace("\"schema\":1", "\"schema\":2"),
            final_record.replace("\"schema\":1", "\"schema\":\"1\""),
            final_record.replace("\"level\":\"info\"", "\"level\":\"error\""),
            final_record.replace("\"rite_calls\":777", "\"rite_calls\":\"777\""),
            final_record.replace("\"rite_calls\":777", "\"rite_calls\":garbage"),
            final_record.replace("\"frame_commits\":1}", "\"frame_commits\":1,}"),
            final_record.replace("{\"schema\"", "{ \"schema\""),
            final_record.replace("\r\n", "\n"),
            format!("{final_record}\r\n"),
            format!("{final_record}{{\"event\":\"unfinished"),
        ] {
            assert!(
                !semantic_presentation_passed_record_is_complete(&format!("{preceding}{record}")),
                "capture accepted a malformed or wrong final record"
            );
        }
        // An earlier complete success record cannot hide a later wrong event.
        let later = final_record.replace(".passed\"", ".failed\"");
        assert!(!semantic_presentation_passed_record_is_complete(&format!(
            "{valid}{later}"
        )));
    }

    #[test]
    fn semantic_presentation_transcript_is_closed_world_and_private() {
        let probes = vec![
            b"unequal-protected-source-alpha".to_vec(),
            b"a-much-longer-protected-source-beta-that-must-remain-private".to_vec(),
        ];
        assert_ne!(probes[0].len(), probes[1].len());
        let valid = valid_semantic_presentation_transcript();
        verify_semantic_presentation_transcript(
            &valid,
            test_semantic_presentation_source_pages,
            test_semantic_presentation_renderer_pages,
            &probes,
        )
        .expect("valid semantic-presentation transcript");

        let records = valid
            .strip_suffix("\r\n")
            .expect("canonical framing")
            .split("\r\n")
            .map(str::to_string)
            .collect::<Vec<_>>();
        let verify_rejected = |candidate: &str| {
            assert!(
                verify_semantic_presentation_transcript(
                    candidate,
                    test_semantic_presentation_source_pages,
                    test_semantic_presentation_renderer_pages,
                    &probes,
                )
                .is_err()
            );
        };
        for index in 0..records.len() {
            let mut omitted = records.clone();
            omitted.remove(index);
            verify_rejected(&format!("{}\r\n", omitted.join("\r\n")));

            let mut duplicated = records.clone();
            duplicated.insert(index, records[index].clone());
            verify_rejected(&format!("{}\r\n", duplicated.join("\r\n")));

            let mut extended = records.clone();
            extended[index] = format!(
                "{},\"unexpected\":0}}",
                records[index].strip_suffix('}').expect("record terminator")
            );
            verify_rejected(&format!("{}\r\n", extended.join("\r\n")));
        }
        for index in 0..records.len() - 1 {
            let mut reordered = records.clone();
            reordered.swap(index, index + 1);
            verify_rejected(&format!("{}\r\n", reordered.join("\r\n")));
        }
        for mutated in [
            valid.trim_end_matches("\r\n").to_string(),
            valid.replace("\r\n", "\n"),
            format!("serial noise\r\n{valid}"),
            valid.replacen("\"schema\":1", "\"schema\":2", 1),
            valid.replacen("\"level\":\"info\"", "\"level\":\"error\"", 1),
            valid.replacen("\"source_image_pages\":3", "\"source_image_pages\":4", 1),
            valid.replacen(
                "\"renderer_image_pages\":5",
                "\"renderer_image_pages\":4",
                1,
            ),
            valid.replacen(
                "\"framebuffer_unchanged\":true",
                "\"framebuffer_unchanged\":false",
                1,
            ),
            valid.replacen(
                "\"framebuffer_verified\":true",
                "\"framebuffer_verified\":false",
                1,
            ),
            valid.replacen("\"body_bytes\":2392160", "\"body_bytes\":2392159", 1),
            valid.replacen(
                "kernel.semantic_surface_presentation_test.passed\",\"level\":\"info",
                "kernel.semantic_surface_presentation_test.failed\",\"level\":\"error",
                1,
            ),
        ] {
            verify_rejected(&mutated);
        }

        for field in semantic_presentation_private_fields {
            let mutated = valid.replacen(
                "\"queued\":true,\"task_released\":true}",
                &format!("\"queued\":true,\"task_released\":true,\"{field}\":0}}"),
                1,
            );
            verify_rejected(&mutated);
        }
        for probe in &probes {
            let probe = std::str::from_utf8(probe).expect("ASCII protected probe");
            let leaked = format!("{valid}{probe}");
            let error = verify_semantic_presentation_transcript(
                &leaked,
                test_semantic_presentation_source_pages,
                test_semantic_presentation_renderer_pages,
                &probes,
            )
            .expect_err("accepted protected probe in transcript");
            assert_eq!(
                error,
                "semantic-presentation protected source entered transcript"
            );
            assert!(!error.contains(probe));
        }
    }

    #[test]
    fn semantic_presentation_private_evidence_gate_controls_publication_without_qemu() {
        let valid = valid_semantic_presentation_transcript();
        let mut frame = supervised_renderer_oracle::ppm_header.to_vec();
        frame.extend(
            supervised_renderer_oracle::render_frame().expect("independent semantic frame"),
        );
        let host = semantic_presentation_host_run::prepare(Path::new(
            "/tmp/anomalous-semantic-presentation-verifier-gate",
        ))
        .expect("retained host transaction");

        let malformed = valid.replacen("\"frame_commits\":1", "\"frame_commits\":2", 1);
        assert_eq!(
            verify_semantic_presentation_evidence(
                &malformed,
                &frame,
                test_semantic_presentation_source_pages,
                test_semantic_presentation_renderer_pages,
                &host,
            )
            .err()
            .expect("accepted malformed transcript evidence"),
            semantic_presentation_evidence_error::transcript
        );
        assert!(host.transaction_is_pending());

        let mut altered_frame = frame.clone();
        let last = altered_frame.last_mut().expect("frame byte");
        *last ^= 1;
        assert_eq!(
            verify_semantic_presentation_evidence(
                &valid,
                &altered_frame,
                test_semantic_presentation_source_pages,
                test_semantic_presentation_renderer_pages,
                &host,
            )
            .err()
            .expect("accepted altered frame evidence"),
            semantic_presentation_evidence_error::frame
        );
        assert!(host.transaction_is_pending());

        let evidence = verify_semantic_presentation_evidence(
            &valid,
            &frame,
            test_semantic_presentation_source_pages,
            test_semantic_presentation_renderer_pages,
            &host,
        )
        .expect("exact evidence");
        assert!(host.transaction_is_pending());
        let accepted_directory = temporary_file("semantic-presentation-accepted-artifacts");
        fs::create_dir_all(&accepted_directory).expect("create accepted artifact directory");
        let mut accepted = semantic_presentation_capture_artifacts::prepare(&accepted_directory)
            .expect("prepare private accepted artifacts");
        let accepted_log = accepted.serial_log.clone();
        let accepted_frame = accepted.frame.clone();
        let accepted_log_part = accepted.serial_part.clone();
        let accepted_frame_part = accepted.frame_part.clone();
        assert!(!accepted_log.exists());
        assert!(!accepted_frame.exists());
        complete_semantic_presentation_and_publish(host, evidence, &mut accepted, &valid, &frame)
            .expect("verified evidence commits before artifact publication");
        drop(accepted);
        assert_eq!(
            fs::read(&accepted_log).expect("published transcript"),
            valid.as_bytes()
        );
        assert_eq!(fs::read(&accepted_frame).expect("published frame"), frame);
        assert!(!accepted_log_part.exists());
        assert!(!accepted_frame_part.exists());
        fs::remove_dir_all(&accepted_directory).expect("remove accepted artifact directory");

        let mut retired = semantic_presentation_host_run::prepare(Path::new(
            "/tmp/anomalous-semantic-presentation-retired-gate",
        ))
        .expect("second retained host transaction");
        retired
            .transaction
            .take()
            .expect("retained transaction")
            .retire_without_publication()
            .expect("retire without publication");
        assert_eq!(
            verify_semantic_presentation_evidence(
                &valid,
                &frame,
                test_semantic_presentation_source_pages,
                test_semantic_presentation_renderer_pages,
                &retired,
            )
            .err()
            .expect("accepted evidence without retained authority"),
            semantic_presentation_evidence_error::host_binding
        );
        let rejected_directory = temporary_file("semantic-presentation-rejected-artifacts");
        fs::create_dir_all(&rejected_directory).expect("create rejected artifact directory");
        let mut rejected = semantic_presentation_capture_artifacts::prepare(&rejected_directory)
            .expect("prepare private rejected artifacts");
        let rejected_paths = [
            rejected.serial_log.clone(),
            rejected.frame.clone(),
            rejected.serial_part.clone(),
            rejected.frame_part.clone(),
        ];
        assert!(
            complete_semantic_presentation_and_publish(
                retired,
                verified_semantic_presentation { _private: () },
                &mut rejected,
                &valid,
                &frame,
            )
            .is_err()
        );
        assert!(!rejected.serial_log.exists());
        assert!(!rejected.frame.exists());
        drop(rejected);
        assert!(rejected_paths.iter().all(|path| !path.exists()));
        fs::remove_dir_all(&rejected_directory).expect("remove rejected artifact directory");
    }

    #[test]
    fn semantic_surface_derived_private_probes_reject_artifact_and_normal_bytes() {
        let directory = std::env::temp_dir().join(format!(
            "anomalous-semantic-surface-probes-{}",
            next_temporary_file.fetch_add(1, atomic_ordering::Relaxed)
        ));
        fs::create_dir_all(&directory).expect("create probe OUT_DIR");
        let directories = vec![directory.clone()];
        let probes = semantic_surface_private_probes(&directories).expect("derive probes");
        let mut bytes = b"ordinary kernel prefix".to_vec();
        bytes.extend_from_slice(&probes.values[0]);
        assert!(
            reject_semantic_surface_private_sources(&bytes, "test artifact", &directories).is_err()
        );
        assert!(
            verify_normal_semantic_surface_bytes_absent(&bytes, b"snapshot", &directories).is_err()
        );
        drop(probes);
        fs::remove_dir_all(directory).expect("remove probe OUT_DIR");
    }

    #[test]
    fn semantic_surface_precedence_excludes_old_fixtures_not_shared_scheduler_support() {
        let shared_scheduler_support = concat!(
            "x86_64_scheduler_enter T 1000 10\n",
            "x86_64_scheduler_resume_kernel T 1010 10\n",
            "x86_64_scheduler_gate_entry T 1020 10\n",
            "x86_64_scheduler_gate_dispatch T 1030 10\n",
            "x86_64_scheduler_faulting_start R 1040 10\n",
            "x86_64_scheduler_peer_start R 1050 10\n",
            "_ZN16anomalous_kernel4arch6x86_6414scheduler_test15scheduler_frameE T 1040 10\n",
        );
        verify_semantic_surface_precedence_symbols(shared_scheduler_support)
            .expect("semantic transport may use shared scheduler support");
        verify_semantic_surface_precedence_bytes(b"kernel.scheduler_test.failed")
            .expect("shared scheduler support may retain a generic failure label");

        for symbol in semantic_surface_preceded_fixture_symbols {
            assert!(
                verify_semantic_surface_precedence_symbols(&format!("{symbol} T 1000 10\n"))
                    .is_err(),
                "accepted old fixture symbol `{symbol}`"
            );
        }
        for marker in semantic_surface_preceded_fixture_event_markers {
            assert!(
                verify_semantic_surface_precedence_bytes(marker).is_err(),
                "accepted old fixture event `{}`",
                display_probe(marker)
            );
        }
    }

    fn valid_semantic_surface_transcript() -> String {
        [
            r#"{"schema":1,"event":"boot.kernel.enter","level":"info","arch":"x86_64"}"#,
            r#"{"schema":1,"event":"kernel.fault_path.ready","level":"info","vectors":32,"tss":true,"double_fault_ist":1,"nmi_ist":2,"nmi_masked":true}"#,
            r#"{"schema":1,"event":"boot.omen.accepted","level":"info","version":2,"boot_drive":128}"#,
            r#"{"schema":1,"event":"boot.memory.map","level":"info","regions":9,"usable_bytes":409600}"#,
            r#"{"schema":1,"event":"kernel.memory.ready","level":"info","frame_size":4096,"address_limit":4294967296,"total_frames":1048576,"free_frames":100,"unavailable_frames":1048476,"data_owned_frames":0,"table_owned_frames":0}"#,
            r#"{"schema":1,"event":"boot.display.ready","level":"info","width":1024,"height":768,"pitch":3072,"bpp":24}"#,
            r#"{"schema":1,"event":"boot.ready","level":"info","control_abi":"0.1","framebuffer":true}"#,
            r#"{"schema":1,"event":"kernel.interrupts.ready","level":"info","backend":"legacy_pc","vector_base":32,"vector_count":16,"pit_input_hz":1193182,"pit_divisor":11932,"timebase_generation":1,"initial_master_mask":255,"initial_slave_mask":255,"master_mask":252,"slave_mask":255,"input_available":true,"input_source_generation":1,"input_queue_capacity":64,"nmi_masked":true,"if_enabled":true}"#,
            r#"{"schema":1,"event":"kernel.semantic_surface_transport.ready","level":"info","arch":"x86_64","source_image_pages":1,"consumer_image_pages":2,"mailbox_pages":1,"stack_pages":4,"grants_split":true,"content_redacted":true}"#,
            r#"{"schema":1,"event":"kernel.semantic_surface_transport.source.released","level":"info","queued_body_retained":true,"source_retired_before_delivery":true,"content_redacted":true}"#,
            r#"{"schema":1,"event":"kernel.semantic_surface_transport.consumer.released","level":"info","delivery_consumed_once":true,"content_redacted":true}"#,
            r#"{"schema":1,"event":"kernel.semantic_surface_transport.released","level":"info","free_frames":100,"resources_restored":true,"private_storage_cleared":true,"content_redacted":true}"#,
            r#"{"schema":1,"event":"kernel.semantic_surface_transport.passed","level":"info","lifecycle_complete":true,"rite_calls":11,"body_copies":9,"body_bytes":2176,"submissions":1,"deliveries":1,"backpressure":1,"denials":2,"malformed":2,"exits":2,"source_retired_before_delivery":true,"header_published_last":true,"stale_reply_cleared":true,"kernel_continuations":2,"source_image_pages":1,"consumer_image_pages":2,"free_frames":100,"content_redacted":true}"#,
        ].join("\n")
    }

    #[test]
    fn semantic_surface_transcript_is_closed_world_and_redacts_derived_probes() {
        let directory = std::env::temp_dir().join(format!(
            "anomalous-semantic-transcript-{}",
            next_temporary_file.fetch_add(1, atomic_ordering::Relaxed)
        ));
        fs::create_dir_all(&directory).expect("create synthetic OUT_DIR");
        let directories = vec![directory.clone()];
        let valid = valid_semantic_surface_transcript();
        verify_semantic_surface_transport_transcript_with_probe_directories(&valid, &directories)
            .expect("valid semantic transcript");
        let probes = semantic_surface_private_probes(&directories).expect("derive probes");
        let leaked = format!("{valid}\n{}", String::from_utf8_lossy(&probes.values[0]));
        assert!(
            verify_semantic_surface_transport_transcript_with_probe_directories(
                &leaked,
                &directories
            )
            .is_err()
        );
        for mutated in [
            valid.replacen("kernel.semantic_surface_transport.ready", "kernel.semantic_surface_transport.source.released", 1),
            valid.replacen("kernel.semantic_surface_transport.consumer.released", "kernel.semantic_surface_transport.consumer.released\n{\"schema\":1,\"event\":\"kernel.semantic_surface_transport.consumer.released\",\"level\":\"info\",\"delivery_consumed_once\":true,\"content_redacted\":true}", 1),
            valid.replacen("\"content_redacted\":true}", "\"content_redacted\":true,\"extra\":1}", 1),
            valid.replacen("\"rite_calls\":11", "\"rite_calls\":10", 1),
            valid.replacen("\"header_published_last\":true", "\"header_published_last\":false", 1),
            valid.replacen("{\"schema\":1", "{\"schema\":", 1),
            format!("serial noise\n{valid}"),
            valid.replacen("kernel.semantic_surface_transport.passed\",\"level\":\"info", "kernel.semantic_surface_transport.failed\",\"level\":\"error", 1),
        ] { assert!(verify_semantic_surface_transport_transcript_with_probe_directories(&mutated, &directories).is_err()); }
        for field in [
            "address",
            "attempt",
            "body",
            "buffer",
            "content",
            "endpoint",
            "generation",
            "label",
            "mailbox",
            "payload",
            "pointer",
            "private",
            "revision",
            "scene",
            "snapshot",
            "surface",
            "task",
            "trace",
        ] {
            let mutated = valid.replacen(
                "\"content_redacted\":true}",
                &format!("\"content_redacted\":true,\"{field}\":0}}"),
                1,
            );
            assert!(
                verify_semantic_surface_transport_transcript_with_probe_directories(
                    &mutated,
                    &directories
                )
                .is_err(),
                "accepted forbidden {field}"
            );
        }
        drop(probes);
        fs::remove_dir_all(directory).expect("remove synthetic OUT_DIR");
    }
}
