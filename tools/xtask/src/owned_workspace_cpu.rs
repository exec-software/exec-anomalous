//! Explicit eager legacy CPU ownership for the retained workspace. The Rust
//! target stays GPR-only; only the inspected assembly probe may use FP/SIMD.
use super::*;

pub(super) const feature: &str = "owned_workspace_cpu_test";
const prefix: &str = "x86_64_owned_workspace_cpu_service";
const ready_event: &str = "owned_workspace_cpu.ready";
const released_event: &str = "owned_workspace_cpu.released";

pub(super) fn selected(features: &[&str]) -> bool {
    features.contains(&feature)
        && owned_workspace::selected(
            &features
                .iter()
                .copied()
                .filter(|v| *v != feature)
                .chain([owned_workspace::feature])
                .collect::<Vec<_>>(),
        )
}

pub(super) fn build() -> Result<built_image, String> {
    build_image_with_context(
        &[workspace_dev::feature, owned_workspace::feature, feature],
        None,
    )
}

pub(super) fn package_image(
    root: &Path,
    build: &Path,
) -> Result<owned_workspace::artifact, String> {
    owned_workspace::package_profile(root, build, true)
}

pub(super) fn smoke() -> Result<(), String> {
    let image = build()?;
    workspace_dev::smoke_image(&image, workspace_dev::workspace_profile::owned_cpu)?;
    workspace_dev::editor_smoke_image(&image, workspace_dev::workspace_profile::owned_cpu)
}

pub(super) fn verify_absent(symbols: &str, bytes: &[u8]) -> Result<(), String> {
    for marker in [
        "owned_workspace_cpu",
        "anomalous_workspace_cpu_probe",
        prefix,
    ] {
        if symbols.contains(marker) || bytes.windows(marker.len()).any(|s| s == marker.as_bytes()) {
            return Err("normal image retains owned workspace CPU proof".into());
        }
    }
    Ok(())
}

pub(super) fn verify_transcript(
    transcript: &str,
    pages: u64,
    frames: usize,
    fallback: Option<&str>,
) -> Result<(), String> {
    let normalized = normalize(transcript)?.ok_or("workspace CPU lifecycle is incomplete")?;
    owned_workspace::verify_transcript(&normalized, pages, frames, fallback)
}

/// Strip only fully validated CPU evidence. The underlying desktop oracle still
/// checks every other record byte for byte. `None` means a writer stopped between
/// two required adjacent records; incremental readers must wait, never accept it.
pub(super) fn normalize(transcript: &str) -> Result<Option<String>, String> {
    if transcript.len() > 128 * 1024 {
        return Err("CPU transcript exceeds bound".into());
    }
    let Some(body) = transcript.strip_suffix("\r\n") else {
        return Ok(None);
    };
    let mut output = Vec::new();
    let mut active: Option<String> = None;
    let mut need_ready = false;
    let mut release: Option<structured_record> = None;
    let mut vendor: Option<String> = None;
    for line in body.split("\r\n") {
        let record = json_parser::new(line).parse_record().ok();
        let event = record.as_ref().and_then(|r| record_string(r, "event"));
        if need_ready {
            let record = record.as_ref().ok_or("CPU readiness record malformed")?;
            validate_ready(record, active.as_deref().unwrap(), &mut vendor)?;
            need_ready = false;
            continue;
        }
        if let Some(cpu) = release.take() {
            let record = record.as_ref().ok_or("CPU release has no owner release")?;
            if event != Some("owned_workspace.released")
                || record_string(record, "scenario") != active.as_deref()
            {
                return Err("CPU release must immediately precede its exact owner release".into());
            }
            validate_release(&cpu, record)?;
            active = None;
            output.push(line);
            continue;
        }
        match event {
            Some("owned_workspace.prepared") => {
                if active.is_some() {
                    return Err("CPU owner was replaced before retirement".into());
                }
                let scenario = record_string(record.as_ref().unwrap(), "scenario")
                    .ok_or("CPU scenario missing")?;
                if !matches!(scenario, "guard" | "nx" | "desktop") {
                    return Err("unknown CPU scenario".into());
                }
                active = Some(scenario.into());
                need_ready = true;
                output.push(line);
            }
            Some(value) if value == released_event => {
                if active.is_none() {
                    return Err("CPU release without owner".into());
                }
                release = record;
            }
            Some("owned_workspace.released") => {
                return Err("owner release omitted CPU evidence".into());
            }
            Some(value) if value.starts_with("owned_workspace_cpu.") => {
                return Err("unexpected CPU evidence".into());
            }
            _ => output.push(line),
        }
    }
    if need_ready || release.is_some() {
        return Ok(None);
    }
    Ok(Some(output.join("\r\n") + "\r\n"))
}

fn validate_ready(
    record: &structured_record,
    scenario: &str,
    vendor: &mut Option<String>,
) -> Result<(), String> {
    require_control_header(
        record,
        ready_event,
        &[
            "scenario",
            "task_images",
            "image_bytes",
            "observation_bytes",
            "cpu_vendor",
            "initial_clean",
        ],
    )?;
    require_string(record, ready_event, "scenario", scenario)?;
    for (field, value) in [
        ("task_images", 1),
        ("image_bytes", 512),
        ("observation_bytes", 512),
    ] {
        require_number(record, ready_event, field, value)?;
    }
    require_bool(record, ready_event, "initial_clean", true)?;
    let actual = record_string(record, "cpu_vendor").ok_or("CPU vendor missing")?;
    if !matches!(actual, "GenuineIntel" | "AuthenticAMD")
        || vendor.as_deref().is_some_and(|v| v != actual)
    {
        return Err("CPU vendor is unsupported or changed during one boot".into());
    }
    *vendor = Some(actual.into());
    Ok(())
}

fn validate_release(record: &structured_record, owner: &structured_record) -> Result<(), String> {
    require_control_header(
        record,
        released_event,
        &[
            "scenario",
            "gates",
            "timers",
            "inputs",
            "other_irqs",
            "faults",
            "probes",
            "clean_checks",
            "user_checks",
            "x87_timers",
            "mmx_timers",
            "probe_complete",
            "controls_restored",
            "storage_cleared",
        ],
    )?;
    let scenario = record_string(owner, "scenario").ok_or("owner scenario missing")?;
    require_string(record, released_event, "scenario", scenario)?;
    for field in ["probe_complete", "controls_restored", "storage_cleared"] {
        require_bool(record, released_event, field, true)?;
    }
    require_number(record, released_event, "probes", 4)?;
    let get = |field| required_number(record, released_event, field);
    let gates = get("gates")?;
    let timers = get("timers")?;
    let inputs = get("inputs")?;
    let other_irqs = get("other_irqs")?;
    // The current source policy fail-stops CPL3 IRQ7/15. Count capture for
    // diagnostic attribution, but no completed lifecycle can resume one.
    if other_irqs != 0 {
        return Err("completed workspace resumed an unsupported IRQ source".into());
    }

    let faults = get("faults")?;
    let captures = [gates, timers, inputs, other_irqs, faults]
        .into_iter()
        .try_fold(0_u64, |n, v| n.checked_add(v))
        .ok_or("CPU counter overflow")?;
    let x87 = get("x87_timers")?;
    let mmx = get("mmx_timers")?;
    let checks = get("user_checks")?;
    let reason = record_string(owner, "reason").ok_or("owner retirement reason missing")?;
    // The four sealed probes and every ordinary Rite gate observe the saved
    // user image. A terminal invalid return/mailbox can stop before that check;
    // only the non-exception user_fault path may omit its one final gate.
    let unchecked_gate = u64::from(scenario == "desktop" && reason == "user_fault" && faults == 0);
    let checked_gates = gates
        .checked_sub(unchecked_gate)
        .ok_or("CPU gate count underflow")?;
    let required_checks = [checked_gates, x87, mmx, faults]
        .into_iter()
        .try_fold(0_u64, |n, value| n.checked_add(value))
        .ok_or("CPU required-check count overflow")?;
    // Watchdog withdrawal happens in a newly captured timer before its stable
    // image check. It cannot be one of the already counted startup-phase ticks.
    let stable_ticks = x87
        .checked_add(mmx)
        .ok_or("CPU stable timer count overflow")?;
    let maximum_checks = captures
        .checked_sub(u64::from(reason == "watchdog"))
        .ok_or("CPU terminal capture missing")?;
    if get("clean_checks")? != captures
        || !(required_checks..=maximum_checks).contains(&checks)
        || x87 < 2
        || mmx < 2
        || stable_ticks > timers
        || reason == "watchdog" && stable_ticks == timers
        || captures > 10_000_000
    {
        return Err(
            "CPU capture and timer evidence is inconsistent or exceeds the bounded run".into(),
        );
    }
    if scenario == "desktop" {
        // The only successful display operation copies one complete 1024-pixel
        // row. A published frame requires 768 distinct row gates; no probe gate
        // can present pixels and task_exit is a separate Rite operation.
        let frames = required_number(owner, "owned_workspace.released", "frames_presented")?;
        let minimum_gates = frames
            .checked_mul(768)
            .and_then(|rows| rows.checked_add(4))
            .and_then(|gates| gates.checked_add(u64::from(reason == "task_exit")))
            .ok_or("CPU frame gate count overflow")?;
        let fault_matches = match reason {
            "task_exit" | "watchdog" => faults == 0,
            "controlled_fault" => faults == 1,
            // A malformed mailbox is rejected at its gate; malformed user
            // flags are rejected at #UD. Both are existing user_fault paths.
            "user_fault" => faults <= 1,
            _ => return Err("unexpected desktop retirement".into()),
        };
        if gates < minimum_gates || !fault_matches {
            return Err("desktop CPU terminal counters differ".into());
        }
    } else if gates != 4
        || faults != 1
        || reason
            != if scenario == "guard" {
                "guard_fault"
            } else {
                "nx_fault"
            }
    {
        return Err("guard/NX CPU counters differ from exact four-phase proof".into());
    }
    Ok(())
}

pub(super) fn inspect_probe(
    root: &Path,
    output: &Path,
    elf_path: &Path,
    _bytes: &[u8],
    elf: &parsed_elf64,
    listing: &str,
    image: &[u8],
) -> Result<Vec<(&'static str, u64)>, String> {
    let symbol = |name| owned_workspace::exact_symbol(listing, name);
    let start = symbol("anomalous_workspace_cpu_probe_start")?;
    let end = symbol("anomalous_workspace_cpu_probe_end")?;
    let returned = symbol("anomalous_workspace_cpu_probe_return")?;
    if symbol("anomalous_workspace_service_data_end")? > 0x802000 {
        return Err("CPU scratch or timer page overlaps live workspace data".into());
    }
    let sections = elf
        .section_headers
        .iter()
        .filter(|s| s.flags & 4 != 0 && s.size != 0)
        .collect::<Vec<_>>();
    validate_code_sections(&sections, start, end)?;
    validate_probe_extent(image, start, end, returned)?;
    let disassembly = run_output_checked(
        root,
        "objdump",
        [
            "--disassemble".into(),
            "--disassemble-zeroes".into(),
            "--wide".into(),
            "--insn-width=16".into(),
            "--section=.entry".into(),
            "--section=.text".into(),
            elf_path.as_os_str().to_owned(),
        ],
    )?;
    fs::write(output.join("workspace_service.disassembly"), &disassembly)
        .map_err(|e| e.to_string())?;
    let disassembly = String::from_utf8(disassembly).map_err(|_| "CPU disassembly is not UTF-8")?;
    inspect_instruction_ranges(
        &disassembly,
        image,
        &sections
            .iter()
            .map(|s| (s.address, s.size))
            .collect::<Vec<_>>(),
        start,
        end,
        returned,
    )?;
    Ok(vec![
        ("cpu_probe_start", start - 0x400000),
        ("cpu_probe_end", end - 0x400000),
        ("cpu_probe_return", returned - 0x400000),
    ])
}

fn validate_code_sections(
    sections: &[&elf64_section_header],
    start: u64,
    end: u64,
) -> Result<(), String> {
    // SHF_X86_64_LARGE is the selected compiler target's addressing metadata,
    // not another page permission. Do not broaden the legacy inspector or
    // admit arbitrary section flags (especially WRITE/TLS/MERGE on code).
    if sections.len() != 2
        || sections.iter().filter(|s| s.name == ".entry").count() != 1
        || sections.iter().filter(|s| s.name == ".text").count() != 1
        || sections.iter().any(|s| {
            s.kind != 1
                || match s.name.as_str() {
                    ".entry" => s.flags != 6 || s.address != 0x400000,
                    ".text" => s.flags != 0x1000_0006,
                    _ => true,
                }
        })
        || !sections.iter().any(|s| {
            s.name == ".text"
                && start >= s.address
                && start < end
                && s.address
                    .checked_add(s.size)
                    .is_some_and(|limit| end <= limit)
        })
    {
        return Err("CPU probe requires exact large-target entry and text sections".into());
    }
    Ok(())
}

fn validate_probe_extent(image: &[u8], start: u64, end: u64, returned: u64) -> Result<(), String> {
    let image_end = 0x400000_u64
        .checked_add(image.len() as u64)
        .ok_or("CPU payload overflow")?;
    if start < 0x400000
        || start >= end
        || end > image_end
        || end - start > 16384
        || returned < start + 2
        || returned >= end
        || image.get((returned - 0x400000 - 2) as usize..(returned - 0x400000) as usize)
            != Some(&[0xcd, 0xf3])
    {
        return Err("CPU probe range or exact INT f3 return marker is invalid".into());
    }
    Ok(())
}

// Only the hand-written probe forms are admitted: no restore instruction,
// arbitrary arithmetic, AVX, segment bases, or compiler-generated SIMD. RIP
// displacements belong to immutable in-image constants; their values are also
// checked by the guest/kernel register observation contract.
fn exact_fp_instruction(mnemonic: &str, operands: &str, raw: &[u8]) -> bool {
    let operands = operands.split('#').next().unwrap().trim();
    match mnemonic {
        "fninit" => raw == [0xdb, 0xe3] && operands.is_empty(),
        "fld1" => raw == [0xd9, 0xe8] && operands.is_empty(),
        "fxsave64" => {
            raw == [0x48, 0x0f, 0xae, 0x04, 0x25, 0x00, 0x20, 0x80, 0x00] && operands == "0x802000"
        }
        "fldcw" => raw.len() == 6 && raw[..2] == [0xd9, 0x2d] && operands.ends_with("(%rip)"),
        "fldt" => raw.len() == 6 && raw[..2] == [0xdb, 0x2d] && operands.ends_with("(%rip)"),
        "ldmxcsr" => {
            raw.len() == 7 && raw[..3] == [0x0f, 0xae, 0x15] && operands.ends_with("(%rip)")
        }
        "movdqu" => {
            let (opcode, register) = if raw.len() == 8 && raw[..3] == [0xf3, 0x0f, 0x6f] {
                (3, 0)
            } else if raw.len() == 9 && raw[..4] == [0xf3, 0x44, 0x0f, 0x6f] {
                (4, 8)
            } else {
                return false;
            };
            let modrm = raw[opcode];
            modrm & 0xc7 == 0x05
                && operands.ends_with(&format!("(%rip),%xmm{}", register + ((modrm >> 3) & 7)))
        }
        "movq" => {
            raw.len() == 7
                && raw[..2] == [0x0f, 0x6f]
                && raw[2] & 0xc7 == 0x05
                && operands.ends_with(&format!("(%rip),%mm{}", (raw[2] >> 3) & 7))
        }
        _ => false,
    }
}

// Complete executable-section byte coverage. This is a bounded CPU instruction
// admission check, not a general hostile-code validator or a control-flow proof.
fn inspect_instruction_ranges(
    disassembly: &str,
    image: &[u8],
    sections: &[(u64, u64)],
    start: u64,
    end: u64,
    returned: u64,
) -> Result<(), String> {
    validate_probe_extent(image, start, end, returned)?;
    let scalar = "cld std mov movabs movb movw movl movq movslq movsbl movsbq movswl movswq movzbl movzbq movzwl movzwq lea leaq push pushq pop popq pushf pushfq popf popfq add addb addw addl addq adc adcl adcq sub subb subw subl subq sbb sbbl sbbq inc incb incl incq dec decb decl decq and andb andw andl andq or orb orl orq xor xorb xorl xorq not notb notl notq neg negl negq cmp cmpb cmpw cmpl cmpq test testb testw testl testq shl shlb shll shlq shr shrb shrl shrq sar sarl sarq shld shrd rol ror rcl rcr bsf bsr bswap bt btl btq bts btr btc imul imull imulq mul mull mulq idiv idivl idivq div divl divq cltq cqto cltd cwtl cbtw seta setae setb setbe sete setne setg setge setl setle setnp setp seto setno sets setns cmova cmovae cmovb cmovbe cmove cmovne cmovg cmovge cmovl cmovle cmovnp cmovp cmovo cmovno cmovs cmovns xchg xadd cmpxchg nop nopl nopw pause call callq ret retq jmp je jne jb jbe ja jae jg jge jl jle jp jnp js jns jo jno jrcxz jecxz loop loope loopne movs movsb movsw movsl movsq stos stosb stosw stosl stosq cmps cmpsb cmpsw cmpsl cmpsq scas scasb scasw scasl scasq int int3 ud2";
    let mut cursors = sections
        .iter()
        .map(|(address, _)| *address)
        .collect::<Vec<_>>();
    let mut probe_gate = false;
    let mut fp_sites = 0;
    for line in disassembly.lines() {
        let Some((address, body)) = line.split_once(':') else {
            continue;
        };
        let address = address.trim();
        if address.is_empty() || !address.bytes().all(|b| b.is_ascii_hexdigit()) {
            continue;
        }
        let address =
            u64::from_str_radix(address, 16).map_err(|_| "CPU instruction address malformed")?;
        let mut columns = body.split('\t').filter(|c| !c.trim().is_empty());
        let raw = columns
            .next()
            .ok_or("CPU instruction bytes missing")?
            .split_whitespace()
            .map(|b| {
                if b.len() == 2 {
                    u8::from_str_radix(b, 16).map_err(|_| "CPU instruction byte invalid")
                } else {
                    Err("CPU byte width invalid")
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        let instruction = columns.next().ok_or("CPU instruction missing")?;
        let mut words = instruction.split_whitespace();
        let mut mnemonic = words.next().ok_or("CPU mnemonic missing")?;
        let mut prefixes = Vec::new();
        while matches!(
            mnemonic,
            "data16" | "cs" | "rep" | "repz" | "repnz" | "lock" | "rex.W" | "rex"
        ) {
            prefixes.push(mnemonic);
            mnemonic = words.next().ok_or("dangling CPU prefix")?;
        }
        let operands = words.collect::<Vec<_>>().join(" ");
        let instruction_end = address
            .checked_add(raw.len() as u64)
            .ok_or("CPU instruction overflow")?;
        let section = sections
            .iter()
            .position(|(base, size)| {
                address >= *base && instruction_end <= base.saturating_add(*size)
            })
            .ok_or("instruction outside executable sections")?;
        if address != cursors[section]
            || raw.is_empty()
            || raw.len() > 15
            || address < 0x400000
            || image.get((address - 0x400000) as usize..(instruction_end - 0x400000) as usize)
                != Some(raw.as_slice())
        {
            return Err("CPU disassembly has gaps, duplicates, or different bytes".into());
        }
        let is_fp = operands.contains("%xmm")
            || operands.contains("%mm")
            || operands.contains("%st")
            || matches!(
                mnemonic,
                "fxsave" | "fxsave64" | "fldcw" | "fldt" | "ldmxcsr" | "fninit" | "fld1" | "emms"
            );
        if is_fp {
            if address < start
                || instruction_end > end
                || !prefixes.is_empty()
                || !exact_fp_instruction(mnemonic, &operands, &raw)
            {
                return Err(format!(
                    "FP instruction outside exact owned probe contract at 0x{address:x}: {instruction}"
                ));
            }
            if matches!(mnemonic, "fldcw" | "fldt" | "ldmxcsr" | "movdqu" | "movq") {
                let displacement = i32::from_le_bytes(raw[raw.len() - 4..].try_into().unwrap());
                let target = instruction_end
                    .checked_add_signed(i64::from(displacement))
                    .ok_or("CPU literal address overflow")?;
                let length = match mnemonic {
                    "fldcw" => 2,
                    "fldt" => 10,
                    "ldmxcsr" => 4,
                    "movdqu" => 16,
                    _ => 8,
                };
                if target < 0x400000
                    || target
                        .checked_add(length)
                        .is_none_or(|end| end > 0x400000 + image.len() as u64)
                {
                    return Err("CPU literal load leaves the immutable image".into());
                }
            }
            fp_sites += 1;
        } else if !scalar.split_ascii_whitespace().any(|m| m == mnemonic)
            || [
                "%fs", "%gs", "%ymm", "%zmm", "%k", "%bnd", "%tmm", "%cr", "%dr",
            ]
            .iter()
            .any(|r| operands.contains(r))
        {
            return Err(format!(
                "instruction outside scalar workspace profile at 0x{address:x}: {instruction}"
            ));
        }
        if prefixes.iter().any(|p| match *p {
            "data16" | "cs" | "rex" | "rex.W" => {
                !matches!(mnemonic, "nop" | "nopl" | "nopw" | "xchg")
            }
            "rep" | "repz" | "repnz" => !matches!(
                mnemonic,
                "movs"
                    | "movsb"
                    | "movsw"
                    | "movsl"
                    | "movsq"
                    | "stos"
                    | "stosb"
                    | "stosw"
                    | "stosl"
                    | "stosq"
                    | "cmps"
                    | "cmpsb"
                    | "cmpsw"
                    | "cmpsl"
                    | "cmpsq"
                    | "scas"
                    | "scasb"
                    | "scasw"
                    | "scasl"
                    | "scasq"
                    | "ret"
                    | "retq"
            ),
            "lock" => !matches!(
                mnemonic,
                "cmpxchg" | "xadd" | "add" | "sub" | "and" | "or" | "xor" | "inc" | "dec"
            ),
            _ => true,
        }) {
            return Err("workspace CPU instruction prefix is unsupported".into());
        }
        if mnemonic == "int" {
            if raw != [0xcd, 0xf3] || operands != "$0xf3" {
                return Err("workspace CPU software gate differs".into());
            }
            if instruction_end == returned {
                probe_gate = true;
            }
        }
        cursors[section] = instruction_end;
    }
    if cursors
        .iter()
        .zip(sections)
        .any(|(cursor, (base, size))| base.checked_add(*size) != Some(*cursor))
        || !probe_gate
        || fp_sites == 0
    {
        return Err("workspace CPU executable coverage or probe is incomplete".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ready(scenario: &str) -> String {
        format!(
            r#"{{"schema":1,"event":"owned_workspace_cpu.ready","level":"info","scenario":"{scenario}","task_images":1,"image_bytes":512,"observation_bytes":512,"cpu_vendor":"GenuineIntel","initial_clean":true}}"#
        )
    }
    fn release(scenario: &str, faults: u64) -> String {
        let gates = if scenario == "desktop" { 773 } else { 4 };
        // Four stable startup ticks plus one unchecked setup/terminal capture.
        let captures = gates + 5 + faults;
        let checks = gates + 4 + faults;
        format!(
            r#"{{"schema":1,"event":"owned_workspace_cpu.released","level":"info","scenario":"{scenario}","gates":{gates},"timers":5,"inputs":0,"other_irqs":0,"faults":{faults},"probes":4,"clean_checks":{captures},"user_checks":{checks},"x87_timers":2,"mmx_timers":2,"probe_complete":true,"controls_restored":true,"storage_cleared":true}}"#
        )
    }
    fn transcript(reason: Option<&str>, desktop_faults: u64) -> String {
        let mut lines = crate::tests::valid_semantic_presentation_transcript()
            .split("\r\n")
            .take(8)
            .map(str::to_owned)
            .collect::<Vec<_>>();
        for (scenario, fault) in [("guard", "guard_fault"), ("nx", "nx_fault")] {
            lines.extend([
                owned_workspace::prepared_record(scenario),
                ready(scenario),
                release(scenario, 1),
                owned_workspace::released_record(scenario, fault, 0),
            ]);
        }
        lines.extend([
            owned_workspace::prepared_record("desktop"),
            ready("desktop"),
        ]);
        lines.push(r#"{"schema":1,"event":"workspace_pointer.ready","level":"info","available":true,"generation":1}"#.into());
        lines.push(r#"{"schema":1,"event":"workspace_service.ready","level":"info","image_pages":32,"data_pages":8,"stack_pages":16,"stage_frames":576,"rights":30}"#.into());
        lines.push(r#"{"schema":1,"event":"workspace_service.frame_presented","level":"info","sequence":1,"rows":768,"width":1024,"height":768}"#.into());
        if let Some(reason) = reason {
            lines.extend([
                release("desktop", desktop_faults),
                owned_workspace::released_record("desktop", reason, 1),
            ]);
            lines.extend(workspace_dev::fallback_records(1, reason));
        }
        lines.join("\r\n") + "\r\n"
    }

    #[test]
    fn code_section_flags_admit_only_the_selected_large_target_metadata() {
        let entry = elf64_section_header {
            name: ".entry".into(),
            kind: 1,
            flags: 6,
            address: 0x400000,
            offset: 0x1000,
            size: 16,
        };
        let text = elf64_section_header {
            name: ".text".into(),
            kind: 1,
            flags: 0x1000_0006,
            address: 0x400010,
            offset: 0x1010,
            size: 64,
        };
        validate_code_sections(&[&entry, &text], 0x400010, 0x400050).unwrap();
        for flags in [0, 2, 6, 7, 0x1000_0007, 0x1000_0016, 0x1000_0406] {
            let mut changed = text.clone();
            changed.flags = flags;
            assert!(validate_code_sections(&[&entry, &changed], 0x400010, 0x400050).is_err());
        }
        let mut changed = entry.clone();
        changed.flags |= 0x1000_0000;
        assert!(validate_code_sections(&[&changed, &text], 0x400010, 0x400050).is_err());
        assert!(validate_code_sections(&[&text, &text], 0x400010, 0x400050).is_err());
        assert!(validate_code_sections(&[&entry, &text], 0x40000f, 0x400050).is_err());
        assert!(validate_code_sections(&[&entry, &text], 0x400010, 0x400051).is_err());
        let mut changed = text.clone();
        changed.address = u64::MAX - 8;
        changed.size = 64;
        assert!(validate_code_sections(&[&entry, &changed], u64::MAX - 8, u64::MAX).is_err());
    }

    #[test]
    fn fp_forms_are_narrow_and_reject_restore_arithmetic_and_changed_registers() {
        for (name, operands, bytes) in [
            ("fninit", "", vec![0xdb, 0xe3]),
            ("fld1", "", vec![0xd9, 0xe8]),
            (
                "fxsave64",
                "0x802000",
                vec![0x48, 0x0f, 0xae, 0x04, 0x25, 0x00, 0x20, 0x80, 0x00],
            ),
            ("fldcw", "0x10(%rip)", vec![0xd9, 0x2d, 0x10, 0, 0, 0]),
            ("fldt", "0x10(%rip)", vec![0xdb, 0x2d, 0x10, 0, 0, 0]),
            (
                "ldmxcsr",
                "0x10(%rip)",
                vec![0x0f, 0xae, 0x15, 0x10, 0, 0, 0],
            ),
            (
                "movdqu",
                "0x10(%rip),%xmm0",
                vec![0xf3, 0x0f, 0x6f, 0x05, 0x10, 0, 0, 0],
            ),
            (
                "movdqu",
                "0x10(%rip),%xmm15",
                vec![0xf3, 0x44, 0x0f, 0x6f, 0x3d, 0x10, 0, 0, 0],
            ),
            (
                "movq",
                "0x10(%rip),%mm7",
                vec![0x0f, 0x6f, 0x3d, 0x10, 0, 0, 0],
            ),
        ] {
            assert!(
                exact_fp_instruction(name, operands, &bytes),
                "{name} {operands}"
            );
            for length in 0..bytes.len() {
                assert!(!exact_fp_instruction(name, operands, &bytes[..length]));
            }
            let mut changed = bytes.clone();
            changed[0] ^= 1;
            assert!(!exact_fp_instruction(name, operands, &changed));
            assert!(!exact_fp_instruction(
                name,
                &format!("{operands},%ymm0"),
                &bytes
            ));
        }
        assert!(!exact_fp_instruction(
            "fxrstor64",
            "0x802000",
            &[0x48, 0x0f, 0xae, 0x0c, 0x25, 0x00, 0x20, 0x80, 0x00]
        ));
        assert!(!exact_fp_instruction(
            "pxor",
            "%xmm0,%xmm0",
            &[0x66, 0x0f, 0xef, 0xc0]
        ));
        assert!(!exact_fp_instruction(
            "movdqu",
            "0x10(%rip),%xmm14",
            &[0xf3, 0x44, 0x0f, 0x6f, 0x3d, 0x10, 0, 0, 0]
        ));
    }

    #[test]
    fn extended_precision_seed_requires_the_full_immutable_ten_byte_extent() {
        let mut image = vec![0xdb, 0x2d, 3, 0, 0, 0, 0xcd, 0xf3, 0xc3];
        image.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 0xc0, 0xff, 0xff]);
        let text = "400000:\tdb 2d 03 00 00 00\tfldt 0x3(%rip)\n400006:\tcd f3\tint $0xf3\n400008:\tc3\tret\n";
        let inspect = |listing: &str, bytes: &[u8]| {
            inspect_instruction_ranges(
                listing,
                bytes,
                &[(0x400000, 9)],
                0x400000,
                0x400009,
                0x400008,
            )
        };
        inspect(text, &image).unwrap();
        // Nine in-image bytes are insufficient even though the low MMX word
        // fits: the seed reads the entire 80-bit representation.
        assert!(inspect(text, &image[..image.len() - 1]).is_err());
        let mut below_image = image.clone();
        below_image[2..6].copy_from_slice(&(-7_i32).to_le_bytes());
        let below_listing = text
            .replace("03 00 00 00", "f9 ff ff ff")
            .replace("0x3(%rip)", "-0x7(%rip)");
        assert!(inspect(&below_listing, &below_image).is_err());
        let mut wrong_form = image.clone();
        wrong_form[1] = 0x25;
        assert!(inspect(&text.replace("db 2d", "db 25"), &wrong_form).is_err());
    }

    #[test]
    fn instruction_ranges_require_exact_bytes_and_exclude_fp_elsewhere() {
        let image = [0xdb, 0xe3, 0xcd, 0xf3, 0xc3, 0xc3];
        let text = "400000:\tdb e3\tfninit\n400002:\tcd f3\tint $0xf3\n400004:\tc3\tret\n400005:\tc3\tret\n";
        inspect_instruction_ranges(text, &image, &[(0x400000, 6)], 0x400000, 0x400005, 0x400004)
            .unwrap();
        assert!(
            inspect_instruction_ranges(
                text,
                &image,
                &[(0x400000, 6)],
                0x400002,
                0x400005,
                0x400004
            )
            .is_err()
        );
        assert!(
            inspect_instruction_ranges(
                text,
                &image,
                &[(0x400000, 6)],
                0x400000,
                0x400005,
                0x400003
            )
            .is_err()
        );
        assert!(
            inspect_instruction_ranges(
                &text.replace("400005:\tc3\tret\n", ""),
                &image,
                &[(0x400000, 6)],
                0x400000,
                0x400005,
                0x400004
            )
            .is_err()
        );
        assert!(
            inspect_instruction_ranges(
                &(text.to_string() + "400005:\tc3\tret\n"),
                &image,
                &[(0x400000, 6)],
                0x400000,
                0x400005,
                0x400004
            )
            .is_err()
        );
        assert!(
            inspect_instruction_ranges(
                &text.replace("db e3", "db e2"),
                &image,
                &[(0x400000, 6)],
                0x400000,
                0x400005,
                0x400004
            )
            .is_err()
        );
        for mnemonic in [
            "fxrstor64 0x802000",
            "vzeroupper",
            "ldmxcsr 0x802000",
            "syscall",
            "wrmsr",
        ] {
            assert!(
                inspect_instruction_ranges(
                    &text.replace("fninit", mnemonic),
                    &image,
                    &[(0x400000, 6)],
                    0x400000,
                    0x400005,
                    0x400004
                )
                .is_err(),
                "{mnemonic}"
            );
        }
        for (start, end, returned) in [
            (0x3fffff, 0x400005, 0x400004),
            (0x400000, 0x400007, 0x400004),
            (0x400005, 0x400005, 0x400004),
            (u64::MAX - 1, u64::MAX, u64::MAX),
        ] {
            assert!(validate_probe_extent(&image, start, end, returned).is_err());
        }
    }

    #[test]
    fn cpu_lifecycle_is_exact_and_keeps_the_desktop_oracle() {
        for (reason, faults) in [
            (None, 0),
            (Some("task_exit"), 0),
            (Some("controlled_fault"), 1),
            (Some("user_fault"), 0),
            (Some("user_fault"), 1),
            (Some("watchdog"), 0),
        ] {
            let valid = transcript(reason, faults);
            verify_transcript(&valid, 32, 1, reason).unwrap();
            let normalized = normalize(&valid).unwrap().unwrap();
            owned_workspace::verify_transcript(&normalized, 32, 1, reason).unwrap();
            assert!(owned_workspace::verify_transcript(&valid, 32, 1, reason).is_err());
            for (from, to) in [
                ("\"task_images\":1", "\"task_images\":2"),
                ("\"image_bytes\":512", "\"image_bytes\":511"),
                ("\"observation_bytes\":512", "\"observation_bytes\":0"),
                ("GenuineIntel", "unrecognized"),
                ("\"initial_clean\":true", "\"initial_clean\":false"),
                ("\"probes\":4", "\"probes\":3"),
                ("\"timers\":5", "\"timers\":3"),
                ("\"inputs\":0", "\"inputs\":1"),
                ("\"other_irqs\":0", "\"other_irqs\":1"),
                ("\"clean_checks\":10", "\"clean_checks\":9"),
                ("\"user_checks\":9", "\"user_checks\":8"),
                ("\"user_checks\":9", "\"user_checks\":18446744073709551615"),
                ("\"x87_timers\":2", "\"x87_timers\":1"),
                ("\"mmx_timers\":2", "\"mmx_timers\":3"),
                ("\"probe_complete\":true", "\"probe_complete\":false"),
                ("\"controls_restored\":true", "\"controls_restored\":false"),
                ("\"storage_cleared\":true", "\"storage_cleared\":false"),
                ("\"width\":1024", "\"width\":1023"),
            ] {
                assert!(
                    verify_transcript(&valid.replace(from, to), 32, 1, reason).is_err(),
                    "{from}"
                );
            }
            for index in 0..valid.lines().count() {
                let mut lines = valid
                    .trim_end_matches("\r\n")
                    .split("\r\n")
                    .collect::<Vec<_>>();
                lines.remove(index);
                assert!(
                    verify_transcript(&(lines.join("\r\n") + "\r\n"), 32, 1, reason).is_err(),
                    "missing {index}"
                );
            }
            assert!(verify_transcript(&(valid.clone() + "{}\r\n"), 32, 1, reason).is_err());
            assert!(verify_transcript(valid.trim_end(), 32, 1, reason).is_err());
        }
        for (reason, faults) in [
            ("task_exit", 1),
            ("watchdog", 1),
            ("controlled_fault", 0),
            ("user_fault", 2),
        ] {
            assert!(
                verify_transcript(&transcript(Some(reason), faults), 32, 1, Some(reason)).is_err()
            );
        }
    }

    #[test]
    fn every_gate_stable_timer_and_exception_requires_a_user_check() {
        for (reason, faults) in [
            ("task_exit", 0),
            ("controlled_fault", 1),
            ("user_fault", 0),
            ("user_fault", 1),
            ("watchdog", 0),
        ] {
            let owner = json_parser::new(&owned_workspace::released_record("desktop", reason, 1))
                .parse_record()
                .unwrap();
            let valid = release("desktop", faults);
            let accepts = |text: &str| {
                validate_release(&json_parser::new(text).parse_record().unwrap(), &owner).is_ok()
            };
            assert!(accepts(&valid));
            let minimum = 777 + faults - u64::from(reason == "user_fault" && faults == 0);
            for checks in [0, 4, 8 + faults, minimum - 1] {
                let bad = valid.replace(
                    &format!("\"user_checks\":{}", 777 + faults),
                    &format!("\"user_checks\":{checks}"),
                );
                assert!(!accepts(&bad), "{reason} {checks}");
            }
            let at_boundary = valid.replace(
                &format!("\"user_checks\":{}", 777 + faults),
                &format!("\"user_checks\":{minimum}"),
            );
            assert!(accepts(&at_boundary), "{reason} minimum");
            let unsupported_irq = valid
                .replace("\"other_irqs\":0", "\"other_irqs\":1")
                .replace(
                    &format!("\"clean_checks\":{}", 778 + faults),
                    &format!("\"clean_checks\":{}", 779 + faults),
                );
            assert!(!accepts(&unsupported_irq));
            let extra_tick = at_boundary
                .replace("\"timers\":5", "\"timers\":6")
                .replace("\"x87_timers\":2", "\"x87_timers\":3")
                .replace(
                    &format!("\"clean_checks\":{}", 778 + faults),
                    &format!("\"clean_checks\":{}", 779 + faults),
                );
            assert!(!accepts(&extra_tick));
            let checked_tick = extra_tick.replace(
                &format!("\"user_checks\":{minimum}"),
                &format!("\"user_checks\":{}", minimum + 1),
            );
            assert!(accepts(&checked_tick));
        }
    }

    #[test]
    fn complete_frames_require_row_gates_and_exit_requires_another_gate() {
        for (reason, faults, minimum) in [("task_exit", 0, 773), ("controlled_fault", 1, 772)] {
            let owner = json_parser::new(&owned_workspace::released_record("desktop", reason, 1))
                .parse_record()
                .unwrap();
            let valid = release("desktop", faults);
            for gates in [4_u64, 10, minimum - 1, minimum] {
                let changed = valid
                    .replace("\"gates\":773", &format!("\"gates\":{gates}"))
                    .replace(
                        &format!("\"clean_checks\":{}", 778 + faults),
                        &format!("\"clean_checks\":{}", gates + 5 + faults),
                    )
                    .replace(
                        &format!("\"user_checks\":{}", 777 + faults),
                        &format!("\"user_checks\":{}", gates + 4 + faults),
                    );
                assert_eq!(
                    validate_release(&json_parser::new(&changed).parse_record().unwrap(), &owner)
                        .is_ok(),
                    gates == minimum,
                    "{reason} {gates}"
                );
            }
            let overflow = json_parser::new(
                &owned_workspace::released_record("desktop", reason, 1).replace(
                    "\"frames_presented\":1",
                    &format!("\"frames_presented\":{}", u64::MAX),
                ),
            )
            .parse_record()
            .unwrap();
            assert!(
                validate_release(&json_parser::new(&valid).parse_record().unwrap(), &overflow)
                    .is_err()
            );
        }
    }

    #[test]
    fn watchdog_requires_a_separate_unchecked_terminal_timer() {
        let owner = json_parser::new(&owned_workspace::released_record("desktop", "watchdog", 1))
            .parse_record()
            .unwrap();
        let valid = release("desktop", 0);
        let checked_terminal = valid.replace("\"user_checks\":777", "\"user_checks\":778");
        assert!(
            validate_release(
                &json_parser::new(&checked_terminal).parse_record().unwrap(),
                &owner
            )
            .is_err()
        );
        let missing_terminal = valid
            .replace("\"timers\":5", "\"timers\":4")
            .replace("\"clean_checks\":778", "\"clean_checks\":777");
        assert!(
            validate_release(
                &json_parser::new(&missing_terminal).parse_record().unwrap(),
                &owner
            )
            .is_err()
        );
    }

    #[test]
    fn readiness_release_and_restart_are_ordered_and_vendor_bound() {
        let valid = transcript(Some("task_exit"), 0);
        for needle in [ready("guard"), release("guard", 1), ready("desktop")] {
            let doubled = valid.replacen(&needle, &format!("{needle}\r\n{needle}"), 1);
            assert!(verify_transcript(&doubled, 32, 1, Some("task_exit")).is_err());
        }
        let changed = valid.replacen(
            &ready("nx"),
            &ready("nx").replace("GenuineIntel", "AuthenticAMD"),
            1,
        );
        assert!(verify_transcript(&changed, 32, 1, Some("task_exit")).is_err());
        let end = valid.find(&release("desktop", 0)).unwrap() + release("desktop", 0).len() + 2;
        assert!(normalize(&valid[..end]).unwrap().is_none());
        for length in valid.len() - 2..valid.len() {
            assert!(normalize(&valid[..length]).unwrap().is_none());
        }
        let restarted = format!(
            "{valid}{}\r\n{}\r\n",
            owned_workspace::prepared_record("desktop"),
            ready("desktop")
        );
        assert!(
            normalize(&restarted)
                .unwrap()
                .unwrap()
                .ends_with(&(owned_workspace::prepared_record("desktop") + "\r\n"))
        );
        assert!(
            normalize(&(restarted + &owned_workspace::prepared_record("desktop") + "\r\n"))
                .is_err()
        );
    }

    #[test]
    fn cpu_profile_precedes_owned_but_no_higher_proof() {
        assert!(selected(&[feature]));
        for lower in [
            owned_workspace::feature,
            workspace_dev::feature,
            "scheduler_test",
            "smoke_test",
        ] {
            let features = [feature, lower];
            assert!(selected(&features));
            assert!(!owned_workspace::selected(&features));
            assert!(!workspace_dev::selected(&features));
        }
        for higher in all_kernel_features.iter().copied().filter(|v| {
            !matches!(
                *v,
                "owned_workspace_cpu_test"
                    | "owned_workspace_test"
                    | "workspace_dev"
                    | "scheduler_test"
                    | "smoke_test"
            )
        }) {
            assert!(!selected(&[feature, higher]), "{higher}");
        }
        assert!(!selected(all_kernel_features));
        assert!(!selected(&[]));
        verify_absent("ordinary", b"ordinary").unwrap();
        for marker in [
            "owned_workspace_cpu",
            "anomalous_workspace_cpu_probe",
            prefix,
        ] {
            assert!(verify_absent(marker, b"").is_err());
            assert!(verify_absent("", marker.as_bytes()).is_err());
        }
    }
}
