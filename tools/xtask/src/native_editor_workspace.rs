//! Packaging and host oracles for the opt-in three-owner editor workspace.
use super::*;

pub(super) const feature: &str = "native_editor_workspace_test";
const prefix: &str = "x86_64_native_editor_workspace";
const entry: u64 = 0x400000;

pub(super) fn selected(features: &[&str]) -> bool {
    features.contains(&feature)
        && mixed_workspace::selected(
            &features
                .iter()
                .copied()
                .filter(|v| *v != feature)
                .chain([mixed_workspace::feature])
                .collect::<Vec<_>>(),
        )
}
pub(super) fn build() -> Result<built_image, String> {
    let image = build_image_with_context(&[feature], None)?;
    println!(
        "native editor workspace image inspected: {}",
        image.path.display()
    );
    Ok(image)
}
pub(super) fn smoke() -> Result<(), String> {
    let image = build()?;
    run_checked(
        &repository_root()?,
        "python3",
        [
            OsString::from("scripts/native-editor-workspace-proof.py"),
            OsString::from("--image"),
            image.path.into_os_string(),
        ],
    )
}

struct payload {
    role: &'static str,
    image: Vec<u8>,
    data: Vec<u8>,
    fault: u64,
}
pub(super) struct artifact {
    pub(super) object: PathBuf,
    payloads: [payload; 2],
}

pub(super) fn package_image(root: &Path, build: &Path) -> Result<artifact, String> {
    let output = build.join("native-editor-workspace");
    fs::create_dir_all(&output).map_err(|e| e.to_string())?;
    let output = fs::canonicalize(output).map_err(|e| e.to_string())?;
    let target = root.join("targets/x86_64-anomalous-workspace.json");
    verify_semantic_surface_target_cfg(root, &target)?;
    run_checked(
        root,
        env::var_os("CARGO").unwrap_or_else(|| "cargo".into()),
        [
            "-Zjson-target-spec".into(),
            "-Zbuild-std=core,compiler_builtins".into(),
            "-Zbuild-std-features=compiler-builtins-mem".into(),
            "build".into(),
            "--release".into(),
            "--package".into(),
            "anomalous-workspace-service".into(),
            "--target".into(),
            target.into_os_string(),
            "--locked".into(),
            "--no-default-features".into(),
            "--features".into(),
            "native_remote".into(),
        ],
    )?;
    let workspace_elf = output.join("workspace.elf");
    fs::copy(
        root.join("target/x86_64-anomalous-workspace/release/anomalous-workspace-service"),
        &workspace_elf,
    )
    .map_err(|e| e.to_string())?;
    let workspace = inspect_workspace(root, &workspace_elf)?;

    elf_application_artifact::build_vessel(
        root,
        "anomalous-native-durable-client",
        Some("remote_workspace_proof"),
        None,
    )?;
    let application_elf = elf_application_artifact::retain_vessel(
        root,
        &output,
        "anomalous-native-durable-client",
        "application.elf",
    )?;
    let inspected =
        elf_application_artifact::inspect_native_durable_vessel(root, &application_elf)?;
    let listing = native_application_artifact::listing(root, &application_elf)?;
    inspect_application_entry(&listing, &inspected.rx, &inspected.rw)?;
    let application = payload {
        role: "application",
        image: inspected.rx,
        data: inspected.rw,
        fault: inspected.fault,
    };
    let payloads = [workspace, application];
    let mut assembly = String::from(".section .rodata.native_editor_workspace,\"a\",@progbits\n");
    for value in &payloads {
        for (suffix, bytes) in [("payload", &value.image), ("data", &value.data)] {
            let file = format!("{}_{suffix}.bin", value.role);
            fs::write(output.join(&file), bytes).map_err(|e| e.to_string())?;
            let name = format!("{prefix}_{}_{suffix}", value.role);
            assembly.push_str(&format!(".balign 16\n.global {name}_start\n.global {name}_end\n{name}_start:\n.incbin \"{file}\"\n{name}_end:\n"));
        }
        assembly.push_str(&format!(
            ".global {prefix}_{}_fault_instruction\n.set {prefix}_{}_fault_instruction, 0x{:x}\n",
            value.role, value.role, value.fault
        ));
    }
    assembly.push_str(".section .note.GNU-stack,\"\",@progbits\n");
    fs::write(output.join("payload.S"), assembly).map_err(|e| e.to_string())?;
    let object = output.join("payload.o");
    run_checked(
        &output,
        "as",
        [
            "--64".into(),
            "-o".into(),
            object.clone().into_os_string(),
            "payload.S".into(),
        ],
    )?;
    let description = format!(
        "profile=native_editor_workspace_v1\nworkspace_rx_bytes={}\nworkspace_data_bytes={}\napplication_rx_bytes={}\napplication_data_bytes={}\nworkspace_rx_capacity=131072\napplication_rx_capacity=32768\nworkspace_stack_bytes=65536\napplication_stack_bytes=8192\nstack_review=required_separately_for_exact_emitted_artifacts\n",
        payloads[0].image.len(),
        payloads[0].data.len(),
        payloads[1].image.len(),
        payloads[1].data.len()
    );
    fs::write(output.join("artifact.txt"), description).map_err(|e| e.to_string())?;
    Ok(artifact { object, payloads })
}

fn inspect_workspace(root: &Path, path: &Path) -> Result<payload, String> {
    let bytes = fs::read(path).map_err(|e| e.to_string())?;
    let elf = parse_elf64(&bytes)?;
    let (image, data) = inspect_workspace_segments(&bytes, &elf)?;
    let listing = native_application_artifact::listing(root, path)?;
    let fault = inspect_workspace_contract(&listing, &image, &data)?;
    Ok(payload {
        role: "workspace",
        image,
        data,
        fault,
    })
}

fn inspect_workspace_contract(listing: &str, image: &[u8], data: &[u8]) -> Result<u64, String> {
    let symbol = |suffix: &str, kind: char| {
        native_durable_artifact::exact_symbol(
            listing,
            &format!("anomalous_workspace_service_{suffix}"),
            kind,
        )
    };
    if symbol("entry", 'T')? != entry
        || symbol("startup_return", 'T')? != entry + 10
        || image.get(..10) != Some(&[0xfc, 0x48, 0xc7, 0xc0, 0, 0, 0, 0, 0xcd, 0xf4])
        || symbol("data_start", 'D')? != 0x801000
        || data.len() < 192
        || data[..192].iter().any(|b| *b != 0)
    {
        return Err("workspace native startup descriptor/gate contract changed".into());
    }
    let fault = symbol("fault_instruction", 'T')?;
    let offset = fault.checked_sub(entry).ok_or("workspace fault below RX")? as usize;
    if image.get(offset..offset + 2) != Some(&[0x0f, 0x0b]) {
        return Err("workspace controlled fault is not an RX UD2".into());
    }
    inspect_call_continuation(image, 10, symbol("run", 'T')?)?;
    inspect_transport(
        listing,
        image,
        "anomalous_workspace_native_call",
        "anomalous_workspace_native_call_return",
    )?;
    Ok(fault)
}

fn inspect_workspace_segments(
    bytes: &[u8],
    elf: &parsed_elf64,
) -> Result<(Vec<u8>, Vec<u8>), String> {
    if elf.kind != 2 || elf.machine != 62 || elf.entry != entry || elf.program_headers.len() != 2 {
        return Err("remote workspace requires static two-load x86-64 ELF".into());
    }
    let rx = elf.program_headers[0];
    let rw = elf.program_headers[1];
    for (load, address, flags, capacity) in [(rx, entry, 5, 0x20000), (rw, 0x801000, 6, 0x7000)] {
        if load.kind != 1
            || load.flags != flags
            || load.virtual_address != address
            || load.physical_address != address
            || load.alignment != 4096
            || load.memory_size == 0
            || load.memory_size > capacity
            || load.file_size > load.memory_size
            || load.offset % 4096 != address % 4096
            || load
                .offset
                .checked_add(load.file_size)
                .is_none_or(|end| end > bytes.len() as u64)
        {
            return Err("remote workspace load geometry/W^X invalid".into());
        }
    }
    if rx.file_size != rx.memory_size || rx.offset + rx.file_size > rw.offset {
        return Err("workspace RX is uninitialized or overlaps RW file data".into());
    }
    for section in &elf.section_headers {
        if section.size == 0 {
            continue;
        }
        if matches!(section.kind, 4 | 6 | 9 | 11)
            || section.flags & 0x400 != 0
            || section.name.starts_with(".got")
            || section.name.starts_with(".plt")
            || section.name == ".interp"
        {
            return Err("workspace relocations/dynamic/TLS metadata forbidden".into());
        }
        if section.flags & 2 == 0 {
            continue;
        }
        let load = if section.flags & 1 == 0 { rx } else { rw };
        let end = section
            .address
            .checked_add(section.size)
            .ok_or("section overflow")?;
        if section.address < load.virtual_address
            || end > load.virtual_address + load.memory_size
            || section.flags & 5 == 5
            || !matches!(section.kind, 1 | 8)
            || section.kind == 8 && load.flags != 6
        {
            return Err("workspace allocated section outside owned load".into());
        }
        if section.kind != 8 {
            let relative = section.address - load.virtual_address;
            if relative + section.size > load.file_size || section.offset != load.offset + relative
            {
                return Err("workspace section lacks exact initialized backing".into());
            }
        }
    }
    Ok((
        elf_virtual_bytes(bytes, rx, entry, rx.file_size as usize)?.to_vec(),
        elf_virtual_bytes(bytes, rw, 0x801000, rw.file_size as usize)?.to_vec(),
    ))
}

fn inspect_application_entry(symbols: &str, image: &[u8], data: &[u8]) -> Result<(), String> {
    let symbol = |suffix: &str, kind| {
        native_durable_artifact::exact_symbol(
            symbols,
            &format!("anomalous_native_durable_client_{suffix}"),
            kind,
        )
    };
    if symbol("entry", 'T')? != entry
        || symbol("startup_return", 'T')? != entry + 10
        || image.get(..10) != Some(&[0xfc, 0x48, 0xc7, 0xc0, 0, 0, 0, 0, 0xcd, 0xf4])
        || symbol("descriptor_start", 'D')? != 0x801000
        || symbol("descriptor_end", 'D')? != 0x8010c0
        || data.len() < 4288
        || data.iter().any(|b| *b != 0)
    {
        return Err("remote application startup/gate contract changed".into());
    }
    inspect_call_continuation(image, 10, symbol("run", 'T')?)?;
    inspect_transport(
        symbols,
        image,
        "anomalous_native_durable_client_call",
        "anomalous_native_durable_client_syscall_return",
    )
}
fn inspect_call_continuation(image: &[u8], offset: usize, target: u64) -> Result<(), String> {
    let bytes = image
        .get(offset..offset + 7)
        .ok_or("startup call truncated")?;
    let displacement = i32::from_le_bytes(bytes[1..5].try_into().unwrap());
    if bytes[0] != 0xe8
        || bytes[5..] != [0x0f, 0x0b]
        || (entry + offset as u64 + 5).checked_add_signed(i64::from(displacement)) != Some(target)
    {
        return Err("startup does not call exact sole Rust entry".into());
    }
    Ok(())
}
fn inspect_transport(
    symbols: &str,
    image: &[u8],
    start: &str,
    returned: &str,
) -> Result<(), String> {
    let address = native_durable_artifact::exact_symbol(symbols, start, 'T')?;
    let end = native_durable_artifact::exact_symbol(symbols, returned, 'T')?;
    let offset = address.checked_sub(entry).ok_or("transport outside RX")? as usize;
    let expected = [
        0x53, 0x48, 0x89, 0xfb, 0x48, 0x8b, 0x03, 0x48, 0x8b, 0x7b, 0x08, 0x48, 0x8b, 0x73, 0x10,
        0x48, 0x8b, 0x53, 0x18, 0x48, 0x8b, 0x4b, 0x20, 0xcd, 0xf4, 0x48, 0x89, 0x43, 0x28, 0x48,
        0x89, 0x4b, 0x30, 0x48, 0x89, 0x53, 0x38, 0x5b, 0xc3,
    ];
    if end != address + 25
        || image.get(offset..offset + expected.len()) != Some(expected.as_slice())
    {
        return Err("native copied-call register contract changed".into());
    }
    Ok(())
}

pub(super) fn inspect_linked(root: &Path, path: &Path, artifact: &artifact) -> Result<(), String> {
    let bytes = fs::read(path).map_err(|e| e.to_string())?;
    let elf = parse_elf64(&bytes)?;
    let symbols = native_application_artifact::listing(root, path)?;
    let mut ranges = Vec::new();
    for value in &artifact.payloads {
        for (suffix, expected) in [("payload", &value.image), ("data", &value.data)] {
            let name = format!("{prefix}_{}_{suffix}", value.role);
            let start =
                native_durable_artifact::exact_symbol(&symbols, &format!("{name}_start"), 'R')?;
            let end = native_durable_artifact::exact_symbol(&symbols, &format!("{name}_end"), 'R')?;
            if end.checked_sub(start) != Some(expected.len() as u64)
                || ranges.iter().any(|(a, b)| start < *b && *a < end)
            {
                return Err("native workspace linked payload extents changed/overlapped".into());
            }
            ranges.push((start, end));
            let matching = elf
                .program_headers
                .iter()
                .filter(|load| {
                    load.kind == 1
                        && start < load.virtual_address.saturating_add(load.memory_size)
                        && load.virtual_address < end
                })
                .collect::<Vec<_>>();
            if matching.len() != 1
                || matching[0].flags & 6 != 4
                || start < matching[0].virtual_address
                || end
                    > matching[0]
                        .virtual_address
                        .saturating_add(matching[0].file_size)
                || elf_virtual_bytes(&bytes, *matching[0], start, expected.len())? != expected
            {
                return Err("native workspace payload lost unique readonly backing".into());
            }
            let sections = elf
                .section_headers
                .iter()
                .filter(|section| {
                    section.flags & 2 != 0
                        && start < section.address.saturating_add(section.size)
                        && section.address < end
                })
                .collect::<Vec<_>>();
            if sections.len() != 1
                || sections[0].kind != 1
                || sections[0].flags & 3 != 2
                || start < sections[0].address
                || end > sections[0].address.saturating_add(sections[0].size)
            {
                return Err("native workspace payload lost unique readonly section".into());
            }
            if suffix == "payload"
                && bytes
                    .windows(expected.len())
                    .filter(|b| *b == expected.as_slice())
                    .count()
                    != 1
            {
                return Err("native workspace executable payload duplicated".into());
            }
        }
        if native_durable_artifact::exact_symbol(
            &symbols,
            &format!("{prefix}_{}_fault_instruction", value.role),
            'A',
        )? != value.fault
        {
            return Err("native workspace fault alias changed".into());
        }
    }
    Ok(())
}
pub(super) fn verify_absent(symbols: &str, bytes: &[u8]) -> Result<(), String> {
    for marker in [
        "native_editor_workspace",
        "workspace_native_call",
        "remote_workspace_proof",
    ] {
        if symbols.contains(marker) || bytes.windows(marker.len()).any(|s| s == marker.as_bytes()) {
            return Err("normal image retained native editor workspace".into());
        }
    }
    Ok(())
}

use anomalous_desktop_workspace::native_bridge::native_remote::{
    remote_editor_bridge, remote_route_binding,
};
use anomalous_desktop_workspace::{client_endpoint, desktop};

struct expected_desktop {
    model: desktop,
    human: client_endpoint,
    remote: Option<remote_editor_bridge>,
}

const render_history_limit: usize = 128 * 1024;

/// Host-only input history, not a guest format: `anui <hex>` or `pointer x y`.
/// Replaying actual publications retains desktop revision, surface sequence,
/// focus and pointer history. Literal expected ANUI controls content; no editor
/// session/actions or guest-produced pixels, counters or observations are used.
fn replay_observations(history: &str) -> Result<expected_desktop, String> {
    use anomalous_desktop_workspace::native_bridge::{editor_disclosure, native_editor_frame};
    use anomalous_desktop_workspace::{client_grants, client_origin, desktop_command};
    use anomalous_editor_contract::remote_protocol::remote_message;
    if history.len() > render_history_limit
        || history.lines().count() > 256
        || (!history.is_empty() && !history.ends_with('\n'))
    {
        return Err("invalid or oversized render history".into());
    }
    let (mut model, control, initial) =
        desktop::new_demo(1024, 768).map_err(|e| format!("desktop: {e:?}"))?;
    let human = model
        .bind_client(&control, client_origin::human, client_grants::all)
        .map_err(|e| format!("client: {e:?}"))?;
    let publish = |model: &mut desktop, ticket| -> Result<(), String> {
        model
            .begin(&control, &ticket)
            .map_err(|e| format!("begin: {e:?}"))?;
        model
            .attest_presented(&control, &ticket)
            .map_err(|e| format!("attest: {e:?}"))?;
        model
            .publish(&control, ticket)
            .map_err(|e| format!("publish: {:?}", e.error()))?;
        Ok(())
    };
    publish(&mut model, initial)?;
    let route = remote_route_binding::try_new(1, [1, 1], [2, 1], [1, 901], [2, 901])
        .map_err(|e| format!("host route: {e:?}"))?;
    let mut remote: Option<remote_editor_bridge> = None;
    let mut delivery = 1;
    let mut closed = false;
    for line in history.lines() {
        if closed {
            return Err("render history continues after editor close".into());
        }
        let revision = model
            .observe(&human)
            .map_err(|e| format!("observe: {e:?}"))?
            .revision();
        match line.split(' ').collect::<Vec<_>>().as_slice() {
            ["anui", encoded] => {
                let mut audit = [0; 128];
                if encoded.len() != audit.len() * 2 {
                    return Err("render ANUI must contain exactly 128 hexadecimal bytes".into());
                }
                let nibble = |byte| match byte {
                    b'0'..=b'9' => Ok(byte - b'0'),
                    b'a'..=b'f' => Ok(byte - b'a' + 10),
                    _ => Err("render ANUI requires lowercase hexadecimal"),
                };
                for (byte, pair) in audit.iter_mut().zip(encoded.as_bytes().chunks_exact(2)) {
                    *byte = nibble(pair[0])? * 16 + nibble(pair[1])?;
                }
                let frame =
                    native_editor_frame::from_audit(&audit).map_err(|e| format!("ANUI: {e:?}"))?;
                closed = frame.observation().closed;
                let message = remote_message::try_observation(1, &audit)
                    .map_err(|e| format!("host observation: {e:?}"))?
                    .encode();
                if let Some(bridge) = remote.as_mut() {
                    bridge
                        .admit_checked_message(&mut model, route, delivery, &message)
                        .map_err(|e| format!("host observation: {e:?}"))?;
                    let update = bridge
                        .prepare_update(&mut model, revision, frame)
                        .map_err(|e| format!("update: {:?}", e.error()))?;
                    bridge
                        .begin(&mut model, &control, &update)
                        .map_err(|e| format!("begin update: {e:?}"))?;
                    bridge
                        .attest_presented(&mut model, &control, &update)
                        .map_err(|e| format!("attest update: {e:?}"))?;
                    bridge
                        .publish_recycling(&mut model, &control, update)
                        .map_err(|e| format!("publish update: {:?}", e.error()))?;
                } else {
                    let mut port = model
                        .provision_remote_editor(&control, route)
                        .map_err(|e| format!("host port: {e:?}"))?;
                    port.admit_initial_checked(route, delivery, &message)
                        .map_err(|e| format!("host initial observation: {e:?}"))?;
                    let ticket = model
                        .prepare(
                            &human,
                            revision,
                            desktop_command::open_native_editor(frame.descriptor()),
                        )
                        .map_err(|e| format!("open: {e:?}"))?;
                    let window = model
                        .candidate(&ticket)
                        .map_err(|e| format!("open candidate: {e:?}"))?
                        .windows()
                        .find(|w| {
                            w.content() == anomalous_desktop_workspace::demo_content::native_editor
                        })
                        .ok_or("no native window")?
                        .id();
                    publish(&mut model, ticket)?;
                    remote = Some(
                        model
                            .bind_remote_editor(
                                &control,
                                &human,
                                window,
                                port,
                                frame,
                                editor_disclosure::public_fixture,
                            )
                            .map_err(|e| format!("host binding: {:?}", e.error()))?,
                    );
                }
                delivery += 1;
            }
            ["pointer", x, y] => {
                let (x, y) = (
                    x.parse::<u32>().map_err(|_| "invalid pointer x")?,
                    y.parse::<u32>().map_err(|_| "invalid pointer y")?,
                );
                if x >= 1024 || y >= 768 {
                    return Err("pointer outside proof viewport".into());
                }
                let ticket = model
                    .prepare_batch(
                        &human,
                        revision,
                        &[
                            desktop_command::set_pointer { x, y },
                            desktop_command::set_pointer_buttons(0),
                        ],
                    )
                    .map_err(|e| format!("pointer: {e:?}"))?;
                publish(&mut model, ticket)?;
            }
            _ => return Err("unknown render history operation".into()),
        }
    }
    Ok(expected_desktop {
        model,
        human,
        remote,
    })
}

fn render_history(history: &str) -> Result<Vec<u8>, String> {
    use anomalous_desktop_compositor::{composition_frame, surface_binding};
    let expected = replay_observations(history)?;
    let view = expected
        .model
        .observe(&expected.human)
        .map_err(|e| format!("observe: {e:?}"))?;
    let native = view
        .all_windows()
        .find(|w| w.content() == anomalous_desktop_workspace::demo_content::native_editor);
    let pixels = match (native, expected.remote.as_ref()) {
        (Some(_), Some(remote)) => Some(
            remote
                .presented_frame(&expected.model, &expected.human)
                .map_err(|e| format!("presented frame: {e:?}"))?,
        ),
        _ => None,
    };
    let bindings = match (native, pixels.as_ref()) {
        (Some(window), Some(pixels)) => vec![surface_binding::new(window.id(), pixels)],
        _ => Vec::new(),
    };
    let composition =
        composition_frame::try_new(&view, &bindings).map_err(|e| format!("composition: {e:?}"))?;
    let mut row = [0; 3072];
    let mut output = Vec::with_capacity(1024 * 768 * 3);
    for y in 0..768 {
        composition
            .compose_row(y, &mut row)
            .map_err(|e| format!("row: {e:?}"))?;
        output.extend_from_slice(&row);
    }
    Ok(output)
}

pub(super) fn render_observation(arguments: Vec<OsString>) -> Result<(), String> {
    use std::io::{Read, Write};
    if !arguments.is_empty() {
        return Err("render accepts expected desktop history on stdin only".into());
    }
    let mut history = String::new();
    std::io::stdin()
        .take(render_history_limit as u64 + 1)
        .read_to_string(&mut history)
        .map_err(|e| e.to_string())?;
    std::io::stdout()
        .lock()
        .write_all(&render_history(&history)?)
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn history_observation(sequence: u64, text: &[u8], closed: bool) -> String {
        let audit = anomalous_editor_contract::interactive_audit {
            sequence,
            revision: if text.is_empty() { 1 } else { 2 },
            version: 1,
            pending_request: 0,
            pending_revision: 0,
            anchor: text.len() as u8,
            head: text.len() as u8,
            undo_depth: u8::from(!text.is_empty()),
            dirty: !text.is_empty(),
            outcome: 0,
            feedback: if closed {
                13
            } else {
                u8::from(!text.is_empty())
            },
            close_armed: false,
            closed,
            text,
        }
        .encode()
        .unwrap();
        let hex = audit
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        format!("anui {hex}\n")
    }

    #[test]
    fn render_history_retains_both_pointer_publications_at_the_same_final_position() {
        let initial = history_observation(1, b"", false);
        let full = format!("{initial}pointer 528 384\npointer 512 384\n");
        let collapsed = format!("{initial}pointer 512 384\n");
        let full_desktop = replay_observations(&full).unwrap();
        let collapsed_desktop = replay_observations(&collapsed).unwrap();
        let full_view = full_desktop.model.observe(&full_desktop.human).unwrap();
        let collapsed_view = collapsed_desktop
            .model
            .observe(&collapsed_desktop.human)
            .unwrap();
        assert_eq!(full_view.pointer(), collapsed_view.pointer());
        assert_eq!(full_view.revision(), 4);
        assert_eq!(collapsed_view.revision(), 3);
        assert_ne!(
            render_history(&full).unwrap(),
            render_history(&collapsed).unwrap()
        );
    }

    #[test]
    fn render_history_updates_existing_native_surface_without_reopening() {
        use anomalous_desktop_workspace::demo_content;
        let initial = history_observation(1, b"", false);
        let edited = history_observation(2, b"n", false);
        let latest = history_observation(3, b"n", false);
        let history = format!("{initial}{edited}{latest}");
        let expected = replay_observations(&history).unwrap();
        let view = expected.model.observe(&expected.human).unwrap();
        assert_eq!(view.revision(), 4);
        assert_eq!(view.all_windows().count(), 3);
        let native = view.focused_window().unwrap();
        assert_eq!(native.content(), demo_content::native_editor);
        assert_eq!(native.surface_sequence(), 3);
        assert_ne!(
            render_history(&history).unwrap(),
            render_history(&latest).unwrap()
        );
    }

    #[test]
    fn render_history_publishes_confirmed_close_and_restores_prior_focus() {
        use anomalous_desktop_workspace::demo_content;
        let history = format!(
            "{}pointer 528 384\npointer 512 384\n{}",
            history_observation(1, b"", false),
            history_observation(2, b"", true)
        );
        let expected = replay_observations(&history).unwrap();
        let view = expected.model.observe(&expected.human).unwrap();
        assert_eq!(view.revision(), 5);
        assert_eq!(view.all_windows().count(), 2);
        assert_eq!(
            view.focused_window().unwrap().content(),
            demo_content::inspector
        );
        assert!(view.pointer().visible());
        assert_eq!((view.pointer().x(), view.pointer().y()), (512, 384));
        assert_ne!(
            render_history(&history).unwrap(),
            render_history("pointer 512 384\n").unwrap()
        );
        assert!(replay_observations(&(history + "pointer 512 384\n")).is_err());
    }

    #[test]
    fn render_history_rejects_malformed_truncated_replayed_and_terminal_observations() {
        let initial = history_observation(1, b"", false);
        for history in [
            "anui 00\n".to_string(),
            format!("anui {}\n", "00".repeat(128)),
            initial.trim_end().to_string(),
            format!("{initial}{initial}"),
            history_observation(1, b"", true),
            "pointer 1024 384\n".to_string(),
            "pointer 512 768\n".to_string(),
            "pointer -1 0\n".to_string(),
            "pointer 512 384 extra\n".to_string(),
            "unknown\n".to_string(),
            "pointer 0 0\n".repeat(257),
        ] {
            assert!(replay_observations(&history).is_err());
        }
    }

    fn workspace_fixture() -> (Vec<u8>, parsed_elf64) {
        let load = |flags, offset, address, size| elf64_program_header {
            kind: 1,
            flags,
            offset,
            virtual_address: address,
            physical_address: address,
            file_size: size,
            memory_size: size,
            alignment: 4096,
        };
        (
            vec![0; 0x2100],
            parsed_elf64 {
                kind: 2,
                machine: 62,
                entry,
                program_headers: vec![load(5, 0x1000, entry, 128), load(6, 0x2000, 0x801000, 256)],
                section_headers: vec![
                    elf64_section_header {
                        name: ".text".into(),
                        kind: 1,
                        flags: 6,
                        address: entry,
                        offset: 0x1000,
                        size: 128,
                    },
                    elf64_section_header {
                        name: ".data".into(),
                        kind: 1,
                        flags: 3,
                        address: 0x801000,
                        offset: 0x2000,
                        size: 256,
                    },
                ],
            },
        )
    }

    #[test]
    fn workspace_loads_reject_permission_capacity_section_and_file_alias_mutations() {
        let (bytes, elf) = workspace_fixture();
        assert_eq!(
            inspect_workspace_segments(&bytes, &elf).unwrap(),
            (vec![0; 128], vec![0; 256])
        );
        for index in 0..2 {
            for flags in [0, 4, 7] {
                let mut changed = elf.clone();
                changed.program_headers[index].flags = flags;
                assert!(inspect_workspace_segments(&bytes, &changed).is_err());
            }
            for mutation in 0..6 {
                let mut changed = elf.clone();
                let load = &mut changed.program_headers[index];
                match mutation {
                    0 => load.physical_address += 4096,
                    1 => load.virtual_address += 4096,
                    2 => load.memory_size = if index == 0 { 0x20001 } else { 0x7001 },
                    3 => load.file_size = load.memory_size + 1,
                    4 => load.offset = u64::MAX,
                    _ => load.alignment = 8192,
                }
                assert!(
                    inspect_workspace_segments(&bytes, &changed).is_err(),
                    "load {index}, mutation {mutation}"
                );
            }
        }
        for mutation in 0..8 {
            let mut changed = elf.clone();
            match mutation {
                0 => changed.section_headers[0].flags = 7,
                1 => changed.section_headers[0].kind = 8,
                2 => changed.section_headers[1].address += 256,
                3 => changed.section_headers[1].offset += 1,
                4 => changed.section_headers[0].name = ".got.plt".into(),
                5 => changed.section_headers[0].kind = 4,
                6 => changed.section_headers[0].flags |= 0x400,
                _ => changed.program_headers[0].memory_size += 1,
            }
            assert!(
                inspect_workspace_segments(&bytes, &changed).is_err(),
                "section mutation {mutation}"
            );
        }
        assert!(inspect_workspace_segments(&bytes[..0x20ff], &elf).is_err());
    }

    fn startup_fixture(workspace: bool) -> (String, Vec<u8>, Vec<u8>) {
        let mut image = vec![0x90; 128];
        // Every role preserves arithmetic flags until the startup gate.
        image[..10].copy_from_slice(&[0xfc, 0x48, 0xc7, 0xc0, 0, 0, 0, 0, 0xcd, 0xf4]);
        let startup = 10;
        image[startup] = 0xe8;
        image[startup + 1..startup + 5]
            .copy_from_slice(&(96_i32 - startup as i32 - 5).to_le_bytes());
        image[startup + 5..startup + 7].copy_from_slice(&[0x0f, 0x0b]);
        let transport = [
            0x53, 0x48, 0x89, 0xfb, 0x48, 0x8b, 0x03, 0x48, 0x8b, 0x7b, 0x08, 0x48, 0x8b, 0x73,
            0x10, 0x48, 0x8b, 0x53, 0x18, 0x48, 0x8b, 0x4b, 0x20, 0xcd, 0xf4, 0x48, 0x89, 0x43,
            0x28, 0x48, 0x89, 0x4b, 0x30, 0x48, 0x89, 0x53, 0x38, 0x5b, 0xc3,
        ];
        image[32..71].copy_from_slice(&transport);
        image[100..102].copy_from_slice(&[0x0f, 0x0b]);
        let symbol_prefix = if workspace {
            "anomalous_workspace_service"
        } else {
            "anomalous_native_durable_client"
        };
        let mut symbols = format!(
            "{symbol_prefix}_entry T 400000\n{symbol_prefix}_startup_return T {:x}\n{symbol_prefix}_run T 400060\n",
            entry + startup as u64
        );
        if workspace {
            symbols.push_str(&format!("{symbol_prefix}_data_start D 801000\n{symbol_prefix}_fault_instruction T 400064\nanomalous_workspace_native_call T 400020\nanomalous_workspace_native_call_return T 400039\n"));
        } else {
            symbols.push_str(&format!("{symbol_prefix}_descriptor_start D 801000\n{symbol_prefix}_descriptor_end D 8010c0\n{symbol_prefix}_call T 400020\n{symbol_prefix}_syscall_return T 400039\n"));
        }
        (symbols, image, vec![0; if workspace { 192 } else { 4288 }])
    }

    #[test]
    fn both_startup_profiles_bind_each_gate_byte_descriptor_and_exact_entry_target() {
        for workspace in [true, false] {
            let (symbols, image, data) = startup_fixture(workspace);
            let inspect = |s: &str, i: &[u8], d: &[u8]| {
                if workspace {
                    inspect_workspace_contract(s, i, d).map(|_| ())
                } else {
                    inspect_application_entry(s, i, d)
                }
            };
            inspect(&symbols, &image, &data).unwrap();
            let startup = 10;
            for offset in
                (0..startup + 7)
                    .chain(32..71)
                    .chain(if workspace { 100..102 } else { 0..0 })
            {
                let mut changed = image.clone();
                changed[offset] ^= 1;
                assert!(
                    inspect(&symbols, &changed, &data).is_err(),
                    "workspace {workspace}, byte {offset}"
                );
            }
            let descriptor = if workspace { 0 } else { 4096 };
            for offset in descriptor..descriptor + 192 {
                let mut changed = data.clone();
                changed[offset] = 1;
                assert!(inspect(&symbols, &image, &changed).is_err());
            }
            assert!(inspect(&symbols, &image, &data[..data.len() - 1]).is_err());
            assert!(inspect(&symbols.replace("400060", "400061"), &image, &data).is_err());
            assert!(inspect(&symbols.replace("400039", "40003a"), &image, &data).is_err());
            assert!(inspect(&format!("{symbols}{symbols}"), &image, &data).is_err());
            assert!(inspect(&symbols.replace(" T ", " R "), &image, &data).is_err());
        }
    }

    #[test]
    fn new_profile_is_exclusive_and_preserves_higher_diagnostic_priority() {
        assert!(selected(&[feature]));
        for higher in [
            "fault_test",
            "ny_object_test",
            "native_application_test",
            "semantic_surface_presentation_test",
            "desktop_dev",
        ] {
            assert!(!selected(&[feature, higher]), "{higher}");
        }
        assert!(!mixed_workspace::selected(&[
            feature,
            mixed_workspace::feature
        ]));
        assert!(verify_absent("ordinary", b"ordinary").is_ok());
        assert!(verify_absent("x86_64_native_editor_workspace_dispatch", b"").is_err());
        assert!(verify_absent("", b"remote_workspace_proof").is_err());
    }
}
