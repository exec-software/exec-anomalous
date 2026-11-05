#[path = "line_editor.rs"]
mod editing;
mod language;

use crate::console::{screen_console, serial_port};
use crate::input::{bootstrap_input_decoder, bootstrap_input_delivery, key_event};
use anomalous_abi::{abi_major, abi_minor};
use core::fmt::Write;
use editing::{line_capacity, line_editor};
use language::{builtin_kind, classify_for_session, parse};

const shell_version: &str = "0.1.0";

/// Run the intentionally narrow, ring-0 bootstrap command environment.
///
/// This loop exists only to bring up input, rendering, parsing, and observable
/// command dispatch. It is not the future user-space shell or a security
/// boundary, and every milestone-zero built-in is a local display operation.
pub fn run(screen: screen_console, serial: &mut serial_port, input_available: bool) -> ! {
    let mut input = bootstrap_input_decoder::new();
    run_session(screen, serial, input_available, &mut input, false);
    crate::arch::halt_forever()
}

/// Borrow the launcher's decoder so held modifiers and input-loss state have
/// one continuous lifetime across the temporary shell surface.
#[cfg(feature = "desktop_dev")]
pub fn run_desktop_session(
    screen: screen_console,
    serial: &mut serial_port,
    input: &mut bootstrap_input_decoder,
) {
    run_session(screen, serial, true, input, true);
}

/// Start a fresh bootstrap input adapter after the native service has retired.
/// Returning consumes the console owner before the next native presenter starts.
#[cfg(feature = "workspace_dev")]
pub fn run_workspace_session(
    screen: screen_console,
    serial: &mut serial_port,
    input_available: bool,
) {
    let mut input = bootstrap_input_decoder::new();
    run_session(screen, serial, input_available, &mut input, input_available);
}

fn run_session(
    mut screen: screen_console,
    serial: &mut serial_port,
    input_available: bool,
    input: &mut bootstrap_input_decoder,
    return_to_desktop: bool,
) {
    let mut editor = line_editor::new();
    let mut sequence = 0u64;
    let mut capacity_reported = false;

    screen.initialize();
    let _ = writeln!(screen, "Anomalous bootstrap shell {shell_version}");
    if input_available {
        let _ = writeln!(screen, "Input: interrupt-driven physical keyboard");
    } else {
        let _ = writeln!(screen, "Input unavailable; timer diagnostics remain active");
    }
    let _ = writeln!(screen, "Type help for available commands.\n");
    #[cfg(any(feature = "desktop_dev", feature = "workspace_dev"))]
    if return_to_desktop {
        let _ = writeln!(screen, "Type desktop to open the desktop.\n");
    }
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"bootstrap_shell.ready","level":"info","input":"interrupt_ps2_set_1","input_available":{input_available},"input_source_generation":{},"input_queue_capacity":{},"line_capacity":{line_capacity}}}"#,
        crate::input::ps2_source_generation,
        crate::input::ps2_queue_capacity
    );
    write_prompt(&mut screen, serial, sequence);

    loop {
        let delivery = crate::arch::with_external_interrupts_disabled(|| {
            // SAFETY: The architecture critical section holds IF clear on the
            // BSP while NMI remains masked and no other CPU exists.
            unsafe { crate::input::poll_physical_input_if_disabled() }
        });
        let Some(delivery) = delivery else {
            input_failure(&mut screen, serial, "critical_section_unavailable");
        };
        let Some(delivery) = delivery else {
            let waited = crate::arch::wait_for_interrupt_if_idle(|| {
                // SAFETY: The architecture invokes this predicate with IF
                // clear under the same single-BSP exclusion contract.
                unsafe { crate::input::ps2_delivery_ready_if_disabled() }
            });
            if !waited {
                input_failure(&mut screen, serial, "interrupt_wait_unavailable");
            }
            continue;
        };
        let Some(delivery) = input.consume(delivery) else {
            continue;
        };
        let event = match delivery {
            bootstrap_input_delivery::key(record) => record.into_payload(),
            bootstrap_input_delivery::loss(loss) => {
                editor.clear();
                capacity_reported = false;
                if loss.terminal_reason().is_some() {
                    input_failure(&mut screen, serial, "source_identity_exhausted");
                }
                let first_sequence = loss
                    .first_sequence()
                    .unwrap_or_else(|| input_failure(&mut screen, serial, "loss_identity_missing"))
                    .get();
                let last_sequence = loss
                    .last_sequence()
                    .unwrap_or_else(|| input_failure(&mut screen, serial, "loss_identity_missing"))
                    .get();
                let rearmed_generation = crate::arch::with_external_interrupts_disabled(|| {
                    crate::arch::rearm_keyboard_input_after_loss()
                })
                .flatten();
                let rearmed = rearmed_generation.is_some();
                let _ = writeln!(screen);
                let _ = writeln!(
                    screen,
                    "Keyboard delivery was interrupted; unfinished input was discarded."
                );
                let _ = writeln!(
                    serial,
                    r#"{{"schema":1,"event":"bootstrap_shell.input.loss","level":"warn","source_generation":{},"first_sequence":{first_sequence},"last_sequence":{last_sequence},"lost_count":{},"rearmed":{rearmed},"new_source_generation":{}}}"#,
                    loss.source().generation(),
                    loss.lost_count(),
                    rearmed_generation.unwrap_or(0)
                );
                if !rearmed {
                    input_failure(&mut screen, serial, "source_rearm_failed");
                }
                write_prompt(&mut screen, serial, sequence);
                continue;
            }
        };
        match event {
            key_event::character(byte) => {
                if editor.push(byte) {
                    screen.write_input(byte);
                } else if !capacity_reported {
                    let _ = writeln!(screen);
                    let _ = writeln!(screen, "Input limit reached: {line_capacity} bytes.");
                    let _ = write!(screen, "anomalous> {}", editor.as_str());
                    let _ = writeln!(
                        serial,
                        r#"{{"schema":1,"event":"bootstrap_shell.input.rejected","level":"warn","reason":"line_capacity"}}"#
                    );
                    capacity_reported = true;
                }
            }
            key_event::backspace => {
                if editor.backspace() {
                    screen.backspace();
                }
            }
            key_event::enter => {
                let _ = writeln!(screen);
                if let Some(line) = parse(editor.as_str()) {
                    sequence = sequence.checked_add(1).unwrap_or_else(|| {
                        input_failure(&mut screen, serial, "sequence_exhausted")
                    });
                    if dispatch(
                        line.command,
                        line.arguments,
                        sequence,
                        &mut screen,
                        serial,
                        return_to_desktop,
                    ) {
                        editor.clear();
                        return;
                    }
                }
                editor.clear();
                capacity_reported = false;
                write_prompt(&mut screen, serial, sequence);
            }
        }
    }
}

fn input_failure(screen: &mut screen_console, serial: &mut serial_port, reason: &str) -> ! {
    let _ = writeln!(screen);
    let _ = writeln!(screen, "Input path halted. See serial diagnostics.");
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"bootstrap_shell.input.failed","level":"fatal","reason":"{reason}"}}"#
    );
    crate::arch::disable_external_interrupts();
    crate::arch::qemu_exit_failure();
    crate::arch::halt_forever()
}

fn write_prompt(screen: &mut screen_console, serial: &mut serial_port, sequence: u64) {
    let _ = write!(screen, "anomalous> ");
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"bootstrap_shell.prompt","level":"debug","after_sequence":{sequence}}}"#
    );
}

fn dispatch(
    command: &str,
    arguments: &str,
    sequence: u64,
    screen: &mut screen_console,
    serial: &mut serial_port,
    return_to_desktop: bool,
) -> bool {
    let kind = classify_for_session(command, return_to_desktop);
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"bootstrap_shell.command.received","level":"info","sequence":{sequence},"kind":"{}"}}"#,
        kind.name()
    );

    let status = match kind {
        builtin_kind::help => {
            let _ = writeln!(screen, "help       list bootstrap commands");
            let _ = writeln!(screen, "version    show kernel and control ABI versions");
            let _ = writeln!(screen, "about      describe this environment");
            let _ = writeln!(screen, "clear      clear the display");
            let _ = writeln!(screen, "echo TEXT  display literal text");
            #[cfg(any(feature = "desktop_dev", feature = "workspace_dev"))]
            if return_to_desktop {
                let _ = writeln!(screen, "desktop    open the development desktop");
            }
            #[cfg(any(feature = "desktop_dev", feature = "workspace_dev"))]
            if return_to_desktop {
                let _ = writeln!(
                    serial,
                    r#"{{"schema":1,"event":"bootstrap_shell.result","level":"info","sequence":{sequence},"kind":"help","commands":["help","version","about","clear","echo","desktop"]}}"#
                );
            }
            if !return_to_desktop {
                let _ = writeln!(
                    serial,
                    r#"{{"schema":1,"event":"bootstrap_shell.result","level":"info","sequence":{sequence},"kind":"help","commands":["help","version","about","clear","echo"]}}"#
                );
            }
            "ok"
        }
        builtin_kind::version => {
            let _ = writeln!(
                screen,
                "Anomalous kernel 0.1.0 | control ABI {abi_major}.{abi_minor}"
            );
            let _ = writeln!(
                serial,
                r#"{{"schema":1,"event":"bootstrap_shell.result","level":"info","sequence":{sequence},"kind":"version","kernel":"{shell_version}","control_abi":"{abi_major}.{abi_minor}"}}"#
            );
            "ok"
        }
        builtin_kind::about => {
            let _ = writeln!(
                screen,
                "Anomalous is an operating system for people and machines."
            );
            let _ = writeln!(
                screen,
                "This bootstrap shell has no persistence or authority model."
            );
            let _ = writeln!(
                serial,
                r#"{{"schema":1,"event":"bootstrap_shell.result","level":"info","sequence":{sequence},"kind":"about","environment":"kernel_bootstrap","persistent":false,"authority_model":false}}"#
            );
            "ok"
        }
        builtin_kind::clear => {
            screen.clear();
            let _ = writeln!(
                serial,
                r#"{{"schema":1,"event":"bootstrap_shell.result","level":"info","sequence":{sequence},"kind":"clear","display_cleared":true}}"#
            );
            "ok"
        }
        builtin_kind::echo => {
            let _ = writeln!(screen, "{arguments}");
            let _ = writeln!(
                serial,
                r#"{{"schema":1,"event":"bootstrap_shell.result","level":"info","sequence":{sequence},"kind":"echo","displayed":true,"content":"redacted"}}"#
            );
            "ok"
        }
        #[cfg(any(feature = "desktop_dev", feature = "workspace_dev"))]
        builtin_kind::desktop => {
            let _ = writeln!(
                serial,
                r#"{{"schema":1,"event":"bootstrap_shell.result","level":"info","sequence":{sequence},"kind":"desktop","returned":true}}"#
            );
            "ok"
        }
        builtin_kind::unknown => {
            let _ = writeln!(screen, "Unknown command. Type help.");
            let _ = writeln!(
                serial,
                r#"{{"schema":1,"event":"bootstrap_shell.result","level":"warn","sequence":{sequence},"kind":"unknown","reason":"unknown_command"}}"#
            );
            "unknown_command"
        }
    };

    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"bootstrap_shell.command.completed","level":"info","sequence":{sequence},"kind":"{}","status":"{status}"}}"#,
        kind.name()
    );
    #[cfg(any(feature = "desktop_dev", feature = "workspace_dev"))]
    if kind == builtin_kind::desktop {
        return true;
    }
    false
}
