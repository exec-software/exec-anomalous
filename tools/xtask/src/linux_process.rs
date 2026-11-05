//! Host-only inspection of unchanged Linux artifacts. Nothing here maps or runs
//! the inspected bytes; the reports are review evidence, never admission tokens.

use std::ffi::OsString;
use std::fmt::Write as _;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};

use anomalous_elf_adapter::linux_image::{self, byte_range, elf_type, image_role, load_plan};
use anomalous_elf_adapter::linux_startup;

const inspection_limits: linux_image::limits = linux_image::limits {
    image_bytes: 512 * 1024 * 1024,
    program_headers: 128,
    load_segments: 32,
    mapped_pages: 131072,
};
const program_bias: u64 = 0x4000_0000;
const interpreter_bias: u64 = 0x6000_0000;
const stack_base: u64 = 0x7fff_0000_0000;
const stack_bytes: usize = 64 * 1024;

pub fn inspect_image(arguments: Vec<OsString>) -> Result<(), String> {
    if arguments.len() != 3 {
        return Err("linux-image-inspect requires PATH BIAS executable|shared-object".into());
    }
    let path = Path::new(&arguments[0]);
    let bias = parse_bias(&arguments[1])?;
    let role = match arguments[2].to_str() {
        Some("executable") => image_role::executable,
        Some("shared-object") => image_role::shared_object,
        _ => return Err("image role must be executable or shared-object".into()),
    };
    let bytes = read_bounded(path, inspection_limits.image_bytes)?;
    let plan = linux_image::image::parse(&bytes, inspection_limits)
        .and_then(|image| image.plan(bias, role))
        .map_err(|error| format!("Linux image rejected: {error:?}"))?;
    println!("{}", image_report(path, plan)?);
    Ok(())
}

pub fn inspect_process(arguments: Vec<OsString>) -> Result<(), String> {
    if arguments.len() != 4 {
        return Err("linux-process-inspect requires PROGRAM INTERPRETER RANDOM_16_BYTES NEW_OUTPUT_DIRECTORY".into());
    }
    let program_path = Path::new(&arguments[0]);
    let interpreter_path = Path::new(&arguments[1]);
    let output = Path::new(&arguments[3]);
    let program = read_bounded(program_path, inspection_limits.image_bytes)?;
    let interpreter = read_bounded(interpreter_path, inspection_limits.image_bytes)?;
    let random = read_bounded(Path::new(&arguments[2]), 16)?;
    let random: &[u8; 16] = random
        .as_slice()
        .try_into()
        .map_err(|_| "random input must contain exactly 16 bytes".to_string())?;
    let main_image = linux_image::image::parse(&program, inspection_limits)
        .map_err(|error| format!("program rejected: {error:?}"))?;
    let main = main_image
        .plan(
            if main_image.elf_type() == elf_type::shared {
                program_bias
            } else {
                0
            },
            image_role::executable,
        )
        .map_err(|error| format!("program layout rejected: {error:?}"))?;
    let loader_image = linux_image::image::parse(&interpreter, inspection_limits)
        .map_err(|error| format!("interpreter rejected: {error:?}"))?;
    let loader = loader_image
        .plan(
            if loader_image.elf_type() == elf_type::shared {
                interpreter_bias
            } else {
                0
            },
            image_role::executable,
        )
        .map_err(|error| format!("interpreter layout rejected: {error:?}"))?;
    validate_pair(main, loader)?;
    let entry = main.entry().ok_or("program has no entry")?;
    let initial_rip = loader.entry().ok_or("interpreter has no entry")?;
    let stack_range = linux_startup::stack_range {
        base: stack_base,
        end: stack_base + stack_bytes as u64,
    };
    let argv = [program_path.as_os_str().as_encoded_bytes()];
    let context = linux_startup::startup_context {
        image: linux_startup::image_facts {
            program_headers: main
                .phdr_address()
                .ok_or("program headers are not loaded; cannot emit AT_PHDR")?,
            program_header_entry_size: linux_image::program_header_size,
            program_header_count: main
                .program_header_count()
                .try_into()
                .map_err(|_| "too many program headers")?,
            entry,
            interpreter_base: Some(loader.load_bias()),
            page_size: linux_image::page_size,
        },
        stack: stack_range,
        argv: &argv,
        envp: &[],
        auxiliary: &[],
        random,
        execfn: argv[0],
    };
    let mut stack = vec![0; stack_bytes];
    let layout = linux_startup::build(&context, &mut stack)
        .map_err(|error| format!("initial stack rejected: {error:?}"))?;
    let report = format!(
        "{{\"scope\":\"host-only-inert-linux-process-plan\",\"executed\":false,\"admitted\":false,\"profile\":\"explicit-layout-empty-env-core-auxv\",\"program\":{},\"interpreter\":{},\"initial_rip\":{initial_rip},\"initial_rdx\":0,\"initial_rsp\":{},\"stack\":{{\"base\":{stack_base},\"end\":{},\"bytes\":{stack_bytes},\"sha256\":{}}},\"argv_hex\":[{}],\"envp_hex\":[],\"requirements\":[\"interpreter identity and namespace policy\",\"owned image mapping and executable interval set\",\"CPU and GNU property policy\",\"TLS and Linux syscall environment\",\"dynamic linker and library execution\",\"truthful additional auxv identity and capability facts\"],\"random_source\":\"caller-supplied; entropy quality not attested\"}}\n",
        image_report(program_path, main)?,
        image_report(interpreter_path, loader)?,
        layout.stack_pointer,
        stack_range.end,
        quoted(&sha256(&stack)?),
        quoted(&hex(argv[0])),
    );
    // All semantic validation and hashing precedes publication. create_dir refuses
    // an existing destination (including a symlink), preserving earlier evidence.
    publish(output, report.as_bytes(), &stack)?;
    println!(
        "Linux process layout and stack written to {} (not executed or admitted)",
        output.display()
    );
    Ok(())
}

fn validate_pair(main: load_plan<'_>, loader: load_plan<'_>) -> Result<(), String> {
    if main.interp().is_none() || main.dynamic().is_none() {
        return Err(
            "process review profile requires a dynamically linked program with PT_INTERP".into(),
        );
    }
    if loader.interp().is_some() || loader.dynamic().is_none() {
        return Err("interpreter must have PT_DYNAMIC and no nested PT_INTERP".into());
    }
    let mut ranges = Vec::new();
    for plan in [main, loader] {
        ranges.extend(
            plan.loads()
                .map(|segment| (segment.mapped().start(), segment.mapped().end())),
        );
    }
    ranges.push((stack_base, stack_base + stack_bytes as u64));
    disjoint(&ranges)
}

fn disjoint(ranges: &[(u64, u64)]) -> Result<(), String> {
    for (index, &(start, end)) in ranges.iter().enumerate() {
        if start > end {
            return Err("reversed process range".into());
        }
        if start == end {
            continue;
        }
        for &(other_start, other_end) in &ranges[..index] {
            if other_start != other_end && start < other_end && other_start < end {
                return Err("program, interpreter, and stack mappings overlap".into());
            }
        }
    }
    Ok(())
}

fn publish(output: &Path, report: &[u8], stack: &[u8]) -> Result<(), String> {
    fs::create_dir(output).map_err(|error| {
        format!(
            "create new evidence directory {}: {error}",
            output.display()
        )
    })?;
    // A failed write leaves an explicitly incomplete evidence directory, never
    // deletes unrelated files or substitutes old evidence for the new attempt.
    for (name, bytes) in [("stack.bin", stack), ("report.json", report)] {
        let mut file = File::create_new(output.join(name))
            .map_err(|error| format!("create {name}: {error}"))?;
        file.write_all(bytes)
            .and_then(|()| file.sync_all())
            .map_err(|error| format!("write {name}: {error}"))?;
    }
    Ok(())
}

fn image_report(path: &Path, plan: load_plan<'_>) -> Result<String, String> {
    let loads = plan.loads().map(|segment| format!(
        "{{\"flags\":{},\"file\":{},\"memory\":{},\"initialized\":{},\"zero_tail\":{},\"mapped\":{},\"copy_file\":{},\"copy_destination\":{},\"zero_fill\":{}}}",
        segment.flags(), range_json(segment.file()), range_json(segment.memory()), range_json(segment.initialized()),
        range_json(segment.zero_tail()), range_json(segment.mapped()), range_json(segment.copy_file()),
        range_json(segment.copy_destination()), range_json(segment.zero_fill()),
    )).collect::<Vec<_>>().join(",");
    let headers = plan.program_headers().map(|header| format!(
        "{{\"kind\":{},\"flags\":{},\"file_offset\":{},\"virtual_address\":{},\"physical_address\":{},\"file_size\":{},\"memory_size\":{},\"alignment\":{}}}",
        header.kind, header.flags, header.file_offset, header.virtual_address, header.physical_address,
        header.file_size, header.memory_size, header.alignment,
    )).collect::<Vec<_>>().join(",");
    Ok(format!(
        "{{\"scope\":\"host-only-inert-linux-image-plan\",\"path\":{},\"path_hex\":{},\"source_bytes\":{},\"sha256\":{},\"elf_type\":{},\"role\":{},\"load_bias\":{},\"entry\":{},\"phdr_address\":{},\"program_header_count\":{},\"mapped_pages\":{},\"interpreter_hex\":{},\"loads\":[{loads}],\"program_headers\":[{headers}],\"limits\":{{\"image_bytes\":{},\"program_headers\":{},\"load_segments\":{},\"mapped_pages\":{}}},\"metadata_policy\":\"GNU properties, notes, dynamic data, TLS and RELRO are not runtime-qualified\"}}",
        quoted(&path.to_string_lossy()),
        quoted(&hex(path.as_os_str().as_encoded_bytes())),
        plan.source().len(),
        quoted(&sha256(plan.source())?),
        quoted(match plan.elf_type() {
            elf_type::executable => "ET_EXEC",
            elf_type::shared => "ET_DYN",
        }),
        quoted(match plan.role() {
            image_role::executable => "executable",
            image_role::shared_object => "shared-object",
        }),
        plan.load_bias(),
        optional_number(plan.entry()),
        optional_number(plan.phdr_address()),
        plan.program_header_count(),
        plan.mapped_pages(),
        plan.interp()
            .map_or_else(|| "null".into(), |value| quoted(&hex(value))),
        inspection_limits.image_bytes,
        inspection_limits.program_headers,
        inspection_limits.load_segments,
        inspection_limits.mapped_pages,
    ))
}

fn range_json(range: byte_range) -> String {
    format!("[{},{}]", range.start(), range.end())
}
fn optional_number(value: Option<u64>) -> String {
    value.map_or_else(|| "null".into(), |value| value.to_string())
}

fn parse_bias(value: &OsString) -> Result<u64, String> {
    let text = value.to_str().ok_or("load bias is not UTF-8")?;
    let result = match text.strip_prefix("0x") {
        Some(digits) => u64::from_str_radix(digits, 16),
        None => text.parse(),
    };
    result.map_err(|_| {
        "load bias must be an unsigned decimal or 0x-prefixed hexadecimal integer".into()
    })
}

fn read_bounded(path: &Path, limit: u64) -> Result<Vec<u8>, String> {
    let file = File::open(path).map_err(|error| format!("open {}: {error}", path.display()))?;
    let metadata = file
        .metadata()
        .map_err(|error| format!("stat {}: {error}", path.display()))?;
    if !metadata.is_file() || metadata.len() > limit {
        return Err(format!(
            "{} must be a regular file within the {limit}-byte inspection budget",
            path.display()
        ));
    }
    let mut bytes = Vec::new();
    file.take(limit.checked_add(1).ok_or("invalid read limit")?)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("read {}: {error}", path.display()))?;
    if bytes.len() as u64 > limit {
        return Err("source grew beyond inspection budget".into());
    }
    Ok(bytes)
}

fn sha256(bytes: &[u8]) -> Result<String, String> {
    // Hash the held bytes, never reopen a possibly replaced path. The digest is
    // evidence binding only; it does not authenticate or authorize the source.
    let mut child = Command::new("sha256sum")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("start sha256sum: {error}"))?;
    let write_result = child
        .stdin
        .take()
        .ok_or("sha256sum stdin missing")?
        .write_all(bytes);
    let output = child
        .wait_with_output()
        .map_err(|error| format!("wait for sha256sum: {error}"))?;
    write_result.map_err(|error| format!("hash input: {error}"))?;
    if !output.status.success() {
        return Err("sha256sum failed".into());
    }
    let digest = output.stdout.get(..64).ok_or("short SHA-256 result")?;
    if !digest
        .iter()
        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
    {
        return Err("invalid SHA-256 result".into());
    }
    String::from_utf8(digest.to_vec()).map_err(|error| error.to_string())
}

fn hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(output, "{byte:02x}").unwrap();
    }
    output
}

fn quoted(value: &str) -> String {
    let mut output = String::from("\"");
    for character in value.chars() {
        match character {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\u{0}'..='\u{1f}' => {
                write!(output, "\\u{:04x}", character as u32).unwrap();
            }
            other => output.push(other),
        }
    }
    output.push('"');
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn process_ranges_reject_cross_image_aliases_and_allow_adjacent_or_empty_ranges() {
        assert!(disjoint(&[(4096, 8192), (8192, 12288), (7000, 7000)]).is_ok());
        assert!(disjoint(&[(4096, 8192), (8191, 12288)]).is_err());
        assert!(disjoint(&[(4096, 8192), (5000, 6000)]).is_err());
        assert!(disjoint(&[(8192, 4096)]).is_err());
    }

    #[test]
    fn report_escapes_paths_and_bias_rejects_ambiguous_input() {
        assert_eq!(quoted("a\n\"\\\0λ"), "\"a\\u000a\\\"\\\\\\u0000λ\"");
        assert_eq!(hex(&[0, 255, b'/']), "00ff2f");
        assert_eq!(parse_bias(&OsString::from("0x40000000")), Ok(program_bias));
        assert!(parse_bias(&OsString::from("-1")).is_err());
        assert!(parse_bias(&OsString::from("0x10000000000000000")).is_err());
    }

    #[test]
    fn source_budget_and_publication_preserve_existing_evidence() {
        let output =
            std::env::temp_dir().join(format!("anomalous-linux-plan-test-{}", std::process::id()));
        fs::create_dir(&output).unwrap();
        let source = output.join("source");
        fs::write(&source, [1, 2, 3, 4]).unwrap();
        assert!(read_bounded(&source, 3).is_err());
        assert_eq!(read_bounded(&source, 4).unwrap(), [1, 2, 3, 4]);
        let evidence = output.join("evidence");
        publish(&evidence, b"first", b"stack").unwrap();
        assert!(publish(&evidence, b"second", b"replacement").is_err());
        assert_eq!(fs::read(evidence.join("report.json")).unwrap(), b"first");
        assert_eq!(fs::read(evidence.join("stack.bin")).unwrap(), b"stack");
        fs::remove_dir_all(output).unwrap();
    }
}
