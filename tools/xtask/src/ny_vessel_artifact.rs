//! Build-time preparation of the Ny rite vessel's payload.
//!
//! Distinct from `ny_object_artifact`, which prepares ADR 0049's qualified
//! fixture and stays pinned to its own generation and its own emission shape.
//! This one prepares a vessel built on the freestanding ABI the toolchain now
//! ships: an object entered through `__ny_entry`, linked against the runtime
//! from the pinned generation, with no libc stubs and no reverse-engineered
//! hook signatures.
//!
//! This is a host inspector and payload builder. It is not an executable
//! loader, a general Ny runtime, or a validator of arbitrary objects.
use super::*;

/// The Ny runtime generation this vessel is built from, by manifest hash.
/// Refresh with `scripts/refresh-ny-runtime.sh` and re-pin deliberately.
const generation: &str = "dcf07d0caef1c7364a66b69b9b9beea834f0913b7d043dbf7324762b8253f614";
const pinned_tools: [(&str, &str); 3] = [
    (
        "nilcompiler",
        "0f530ea1ba02c01d27c9b9d0da7c973ddc32f6f19790be5db6c486d36859a116",
    ),
    (
        "nilassembler",
        "90828d62cb28b58912347abea7ddfa3b16c4df97f3f7ca146870a0e399c913cc",
    ),
    (
        "nilinterpreter",
        "a949a7445cdc5e0a8ec40f852fe72b28dfd0ad52b132dcb6e1f5626526dc6c35",
    ),
];
/// The archive the vessel links. Pinned separately from the manifest so a
/// generation that changed only this file is still rejected.
const pinned_runtime: (&str, &str) = (
    "nillib/native/libnilruntime_freestanding.a",
    "5963750e2f3506ff136f55e26206c60fc7ece65e92ddc445c8e12cc95cd4b469",
);

/// The vessel's memory map. Every address is below 4 GiB because the object is
/// not position-independent: a higher link address would overflow the 32-bit
/// relocations the small code model emits.
pub(super) const image_base: u64 = 0x40_0000;
pub(super) const state_base: u64 = 0x80_0000;
pub(super) const arena_base: u64 = 0x80_1000;
pub(super) const mailbox_base: u64 = 0x80_2000;
pub(super) const page_size: u64 = 0x1000;

/// What the vessel is expected to produce, measured by running the sequence
/// before the gate against this object and this archive on the build host.
pub(super) const expected_message: &[u8] = b"Anomalous vessel speaks through a rite";
pub(super) const expected_exit_code: i32 = 42;

/// Exactly what a freestanding Ny object is allowed to need. The first five
/// are the native runtime ABI; the last three appear only because this vessel
/// allocates, and no libc name may appear beside them.
const expected_object_imports: [&str; 8] = [
    "nyrt_abi_version",
    "nyrt_alloc",
    "nyrt_context_begin",
    "nyrt_context_end",
    "nyrt_fail",
    "nyrt_mem_copy",
    "nyrt_profile_site",
    "nyrt_str_length",
];

pub(super) struct artifact {
    /// The relocation-free wrapper the kernel link consumes.
    pub(super) object: PathBuf,
    /// The vessel's RX bytes, as they must appear in its single code page.
    pub(super) image: Vec<u8>,
    /// Addresses the kernel harness checks against its own mapping.
    pub(super) entry: u64,
    pub(super) failed: u64,
    pub(super) console_return: u64,
    pub(super) exit_return: u64,
}

pub(super) fn package_image(root: &Path, build: &Path) -> Result<artifact, String> {
    let output = build.join("ny-vessel");
    fs::create_dir_all(&output).map_err(|e| format!("create Ny vessel directory: {e}"))?;
    let source = root.join("vessels/anomalous-ny-vessel");
    let sdk = checked_sdk(root)?;

    let nil = output.join("vessel.nil");
    let vessel_object = output.join("vessel.o");
    let entry_object = output.join("entry.o");
    let elf = output.join("vessel.elf");
    let wrapper = output.join("payload.o");
    for path in [&nil, &vessel_object, &entry_object, &elf, &wrapper] {
        remove_optional_file(path)?;
    }

    run_stage(
        root,
        &sdk.join("nilcompiler"),
        &[
            "--no-bundle".into(),
            "--no-stdlib".into(),
            "--output".into(),
            nil.as_os_str().into(),
            source.join("vessel.ny").into_os_string(),
        ],
        &output,
        "compiler",
        0,
        120,
    )?;
    // --freestanding emits __ny_entry and no C main; --no-red-zone is separate
    // and is required here because this kernel's interrupt entry does not
    // switch stacks for the vessel's own frames.
    run_stage(
        root,
        &sdk.join("nilassembler"),
        &[
            "--unmanaged".into(),
            "--freestanding".into(),
            "--no-red-zone".into(),
            "--output".into(),
            vessel_object.as_os_str().into(),
            nil.as_os_str().into(),
        ],
        &output,
        "assembler",
        0,
        120,
    )?;

    // The emitted object must need the runtime ABI and nothing else. A libc
    // name here is the reverse-engineered-stub problem this vessel exists to
    // be free of, so it fails the build rather than being stubbed.
    check_object_imports(root, &vessel_object)?;

    // An independent semantic oracle: the same source through the interpreter,
    // whose exit status is Ny's returned value. This is not native execution.
    run_stage(
        root,
        &sdk.join("nilinterpreter"),
        &["--no-ai".into(), nil.as_os_str().into()],
        &output,
        "interpreter",
        expected_exit_code,
        300,
    )?;

    run_stage(
        root,
        Path::new("as"),
        &[
            "--64".into(),
            "--fatal-warnings".into(),
            "-o".into(),
            entry_object.as_os_str().into(),
            source.join("entry.S").into_os_string(),
        ],
        &output,
        "entry",
        0,
        120,
    )?;
    // --no-undefined and --orphan-handling=error together mean the vessel
    // cannot acquire an unnoticed dependency or an unplaced section: the
    // linker script names every section it admits and rejects the rest.
    run_stage(
        root,
        Path::new("ld"),
        &[
            "--build-id=none".into(),
            "--fatal-warnings".into(),
            "--no-undefined".into(),
            "--orphan-handling=error".into(),
            "-nostdlib".into(),
            "-static".into(),
            "-z".into(),
            "max-page-size=0x1000".into(),
            "-T".into(),
            source.join("vessel.ld").into_os_string(),
            "-o".into(),
            elf.as_os_str().into(),
            entry_object.as_os_str().into(),
            vessel_object.as_os_str().into(),
            sdk.join(pinned_runtime.0).into_os_string(),
        ],
        &output,
        "linker",
        0,
        120,
    )?;

    let symbols = listing(root, &elf)?;
    let artifact = artifact {
        object: wrapper,
        image: image_bytes(root, &elf)?,
        entry: symbol_u64_from_listing(&symbols, "anomalous_ny_vessel_entry")?,
        failed: symbol_u64_from_listing(&symbols, "anomalous_ny_vessel_failed")?,
        console_return: symbol_u64_from_listing(&symbols, "anomalous_ny_vessel_console_return")?,
        exit_return: symbol_u64_from_listing(&symbols, "anomalous_ny_vessel_exit_return")?,
    };
    check_layout(&artifact, &symbols)?;
    write_wrapper(root, &output, &artifact)?;
    Ok(artifact)
}

fn checked_sdk(root: &Path) -> Result<PathBuf, String> {
    let sdk = fs::canonicalize(root.join("runtime")).map_err(|_| {
        "Ny runtime missing; refresh it with scripts/refresh-ny-runtime.sh before this opt-in proof"
    })?;
    if sha256(root, &sdk.join("manifest.json"))? != generation {
        return Err("Ny runtime generation/manifest differs from the vessel pin".into());
    }
    for (name, expected) in pinned_tools {
        let path = sdk.join(name);
        if !fs::symlink_metadata(&path)
            .map_err(|e| e.to_string())?
            .file_type()
            .is_file()
            || sha256(root, &path)? != expected
        {
            return Err(format!("Ny pinned tool changed: {name}"));
        }
    }
    if sha256(root, &sdk.join(pinned_runtime.0))? != pinned_runtime.1 {
        return Err("Ny freestanding runtime archive differs from the vessel pin".into());
    }
    Ok(sdk)
}

fn check_object_imports(root: &Path, object: &Path) -> Result<(), String> {
    let text = String::from_utf8(run_output_checked(
        root,
        "nm",
        [
            "--undefined-only".into(),
            "--format=posix".into(),
            object.as_os_str().to_owned(),
        ],
    )?)
    .map_err(|_| "Ny object symbols are not UTF-8")?;
    let mut imports: Vec<&str> = text
        .lines()
        .filter_map(|line| line.split_whitespace().next())
        .collect();
    imports.sort_unstable();
    imports.dedup();
    if imports != expected_object_imports {
        return Err(format!(
            "Ny vessel object imports {imports:?}, not the expected runtime ABI \
             {expected_object_imports:?}"
        ));
    }
    Ok(())
}

fn check_layout(artifact: &artifact, symbols: &str) -> Result<(), String> {
    if artifact.entry != image_base {
        return Err("Ny vessel entry is not at the base of its code page".into());
    }
    if artifact.image.is_empty() || artifact.image.len() as u64 > page_size {
        return Err("Ny vessel RX image does not fit one page".into());
    }
    for (name, address) in [
        ("failed", artifact.failed),
        ("console_return", artifact.console_return),
        ("exit_return", artifact.exit_return),
    ] {
        if !(image_base..image_base + artifact.image.len() as u64).contains(&address) {
            return Err(format!("Ny vessel {name} lies outside its code page"));
        }
    }
    let state = symbol_u64_from_listing(symbols, "anomalous_ny_vessel_state_start")?;
    let arena = symbol_u64_from_listing(symbols, "anomalous_ny_vessel_arena_start")?;
    let arena_end = symbol_u64_from_listing(symbols, "anomalous_ny_vessel_arena_end")?;
    if state != state_base || arena != arena_base || arena_end != arena_base + page_size {
        return Err("Ny vessel writable layout differs from the mapped profile".into());
    }
    // The vessel allocates through the runtime, so the region must not reach
    // the mailbox the kernel owns.
    if arena_end > mailbox_base {
        return Err("Ny vessel region overlaps the kernel-owned mailbox".into());
    }
    Ok(())
}

/// The bytes of the single RX segment, which are exactly what the kernel
/// copies into the vessel's code page.
fn image_bytes(root: &Path, elf: &Path) -> Result<Vec<u8>, String> {
    let text = String::from_utf8(run_output_checked(
        root,
        "readelf",
        [
            "-lW".into(),
            elf.as_os_str().to_owned(),
        ],
    )?)
    .map_err(|_| "Ny vessel segment listing is not UTF-8")?;
    let loads: Vec<Vec<String>> = text
        .lines()
        .filter(|line| line.trim_start().starts_with("LOAD"))
        .map(|line| line.split_whitespace().map(str::to_owned).collect())
        .collect();
    if loads.len() != 2 {
        return Err(format!(
            "Ny vessel must have exactly two load segments, found {}",
            loads.len()
        ));
    }
    let executable = loads
        .iter()
        .filter(|fields| fields.iter().any(|field| field == "E"))
        .collect::<Vec<_>>();
    if executable.len() != 1 {
        return Err("Ny vessel must have exactly one executable segment".into());
    }
    let bytes = fs::read(elf).map_err(|e| format!("read Ny vessel image: {e}"))?;
    let number = |value: &str| -> Result<u64, String> {
        u64::from_str_radix(value.trim_start_matches("0x"), 16)
            .map_err(|_| format!("Ny vessel segment field is not hexadecimal: {value}"))
    };
    let offset = number(&executable[0][1])?;
    let address = number(&executable[0][2])?;
    let length = number(&executable[0][4])?;
    if address != image_base {
        return Err("Ny vessel executable segment is not at the image base".into());
    }
    let start = usize::try_from(offset).map_err(|_| "Ny vessel segment offset is too large")?;
    let end = start
        .checked_add(usize::try_from(length).map_err(|_| "Ny vessel segment is too large")?)
        .filter(|end| *end <= bytes.len())
        .ok_or("Ny vessel segment lies outside its file")?;
    Ok(bytes[start..end].to_vec())
}

fn write_wrapper(root: &Path, output: &Path, artifact: &artifact) -> Result<(), String> {
    fs::write(output.join("vessel_rx.bin"), &artifact.image)
        .map_err(|e| format!("write Ny vessel image: {e}"))?;
    // An opaque carrier: the kernel link gains bytes and two symbols, not a
    // second loader. Nothing here is executed in kernel context.
    fs::write(
        output.join("payload.S"),
        ".section .rodata.ny_vessel,\"a\",@progbits\n\
         .balign 16\n\
         .global x86_64_ny_vessel_payload_start\n\
         .global x86_64_ny_vessel_payload_end\n\
         x86_64_ny_vessel_payload_start:\n\
         .incbin \"vessel_rx.bin\"\n\
         x86_64_ny_vessel_payload_end:\n\
         .section .note.GNU-stack,\"\",@progbits\n",
    )
    .map_err(|e| format!("write Ny vessel wrapper source: {e}"))?;
    run_stage(
        output,
        Path::new("as"),
        &[
            "--64".into(),
            "--fatal-warnings".into(),
            "-o".into(),
            "payload.o".into(),
            "payload.S".into(),
        ],
        output,
        "wrapper",
        0,
        120,
    )?;
    let _ = root;
    Ok(())
}

fn listing(root: &Path, elf: &Path) -> Result<String, String> {
    let undefined = run_output_checked(
        root,
        "nm",
        [
            "--undefined-only".into(),
            "--format=posix".into(),
            elf.as_os_str().to_owned(),
        ],
    )?;
    if !undefined.iter().all(u8::is_ascii_whitespace) {
        return Err("Ny vessel has unresolved symbols".into());
    }
    String::from_utf8(run_output_checked(
        root,
        "nm",
        [
            "--defined-only".into(),
            "--format=posix".into(),
            elf.as_os_str().to_owned(),
        ],
    )?)
    .map_err(|_| "Ny vessel symbols are not UTF-8".into())
}

fn sha256(root: &Path, path: &Path) -> Result<String, String> {
    let output = run_output_checked(root, "sha256sum", [path.as_os_str().to_owned()])?;
    let text = String::from_utf8(output).map_err(|_| "sha256sum output is not UTF-8")?;
    let value = text
        .split_whitespace()
        .next()
        .ok_or("sha256sum omitted hash")?;
    if value.len() != 64 || !value.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("sha256sum hash malformed".into());
    }
    Ok(value.to_owned())
}

fn run_stage(
    directory: &Path,
    program: &Path,
    arguments: &[std::ffi::OsString],
    output: &Path,
    label: &str,
    expected_status: i32,
    timeout_seconds: u64,
) -> Result<(), String> {
    let mut command = std::process::Command::new("timeout");
    command.current_dir(directory);
    command.arg(format!("{timeout_seconds}"));
    command.arg(program);
    command.args(arguments);
    let result = command
        .output()
        .map_err(|e| format!("run Ny vessel {label}: {e}"))?;
    fs::write(output.join(format!("{label}.stdout")), &result.stdout)
        .map_err(|e| format!("record Ny vessel {label} output: {e}"))?;
    fs::write(output.join(format!("{label}.stderr")), &result.stderr)
        .map_err(|e| format!("record Ny vessel {label} diagnostics: {e}"))?;
    if result.status.code() != Some(expected_status) {
        return Err(format!(
            "Ny vessel {label} stage returned {:?}, expected {expected_status}",
            result.status.code()
        ));
    }
    Ok(())
}
