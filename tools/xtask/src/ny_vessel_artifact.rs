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
const generation: &str = "7292b787e330aae54f4fe172f03ea436cfba09f7dae70546545c2d4eb40370ea";
const pinned_tools: [(&str, &str); 3] = [
    (
        "nilcompiler",
        "6764709d059797bce815b51ea73b2adfd8d6404cbcfa0bf21c8b01e9f0a76f26",
    ),
    (
        "nilassembler",
        "df210e55b495fc1d72d4f06c384c504608b4b27561ae5bf890809c91a283929f",
    ),
    (
        "nilinterpreter",
        "ac259135567218c601a9751741e5ed5834eb29cf7cc779bdbb1558c38cb33e5d",
    ),
];
/// The archive the vessel links. Pinned separately from the manifest so a
/// generation that changed only this file is still rejected.
const pinned_runtime: (&str, &str) = (
    "nillib/native/libnilruntime_freestanding.a",
    "8bbcbd145b65b722d74a42b245b21bbaa35f3fb3e7078b364596b860f8b83e10",
);

/// The vessel's memory map, in the user window the kernel maps it into. It is
/// above the 512 GiB boundary because the first page-table entry is the
/// kernel's own branch and a user mapping there is refused, which is why the
/// object is emitted for the large code model below.
pub(super) const user_window: u64 = 0x0000_6000_0000_0000;
pub(super) const image_base: u64 = user_window + 0x1000;
/// The code occupies two pages. One sufficed until the vessel gained a
/// failure path; the kernel maps the pair as one extent, so this costs no
/// mapping against the three a circle admits.
pub(super) const image_pages: u64 = 2;
pub(super) const state_base: u64 = user_window + 0x3000;
pub(super) const arena_base: u64 = user_window + 0x4000;
pub(super) const mailbox_base: u64 = user_window + 0x5000;
pub(super) const page_size: u64 = 0x1000;

/// What the vessel is expected to produce, stated once. The vessel writes
/// three times: the message it composed, a line saying it saw the first rite
/// commit what it sent -- which it only does if the returned count matched --
/// and an answer composed around the line the kernel delivered to it. The
/// transcript oracle takes its byte counts from these rather than repeating
/// them, so a change to what the vessel says cannot pass by agreeing with
/// itself in one place and not the other.
pub(super) const expected_first_message: &[u8] = b"Anomalous vessel speaks through a rite";
pub(super) const expected_second_message: &[u8] = b"ny saw the rite succeed";
pub(super) const expected_third_message: &[u8] = b"ny heard: say it back";
/// The line the kernel hands the vessel. Its image cannot contain this, so the
/// third message's length is the evidence that it arrived.
pub(super) const expected_delivered_message: &[u8] = b"say it back";
pub(super) const expected_exit_code: i32 = 42;

/// Exactly what this vessel's object is allowed to need: the five native
/// runtime ABI hooks, three more because it allocates, and the three symbols
/// the vessel's own Anomalous half defines. No libc name may appear beside
/// them, and no further host symbol may arrive unnoticed -- the surface Ny can
/// reach is this list: two rites and one cell the host wrote.
const expected_object_imports: [&str; 11] = [
    "anomalous_rite_console_write",
    "anomalous_rite_input_read",
    "anomalous_vessel_mode",
    "nyrt_abi_version",
    "nyrt_alloc",
    "nyrt_context_begin",
    "nyrt_context_end",
    "nyrt_fail",
    "nyrt_mem_copy",
    "nyrt_profile_site",
    "nyrt_str_length",
];

/// Identifies the header the kernel parses, so a payload from some other
/// stage cannot be mistaken for this one.
pub(super) const header_magic: u64 = 0x4e59_5f56_4553_5331;
/// magic, image length, entry, console return, input return, exit return,
/// failure, mode cell.
pub(super) const header_bytes: usize = 8 * 8;

pub(super) struct artifact {
    /// The relocation-free wrapper the kernel link consumes.
    pub(super) object: PathBuf,
    /// The vessel's RX bytes, as they must appear in its single code page.
    pub(super) image: Vec<u8>,
    /// Addresses the kernel harness checks against its own mapping.
    pub(super) entry: u64,
    pub(super) failed: u64,
    pub(super) console_return: u64,
    pub(super) input_return: u64,
    pub(super) exit_return: u64,
    /// The cell in the vessel's own state page that the host writes before
    /// entry to choose the run. Not an address in the code page, so it is
    /// bounds-checked against the writable profile instead.
    pub(super) mode_cell: u64,
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
    // --freestanding emits __ny_entry and no C main. --no-red-zone is separate
    // and is required here because this kernel's interrupt entry does not
    // switch stacks for the vessel's own frames. --code-model large is what
    // lets the object be linked into the user window at all: the small model
    // materializes every address as a 32-bit absolute and the link fails on
    // the vessel's own string constants.
    run_stage(
        root,
        &sdk.join("nilassembler"),
        &[
            "--unmanaged".into(),
            "--freestanding".into(),
            "--no-red-zone".into(),
            "--code-model".into(),
            "large".into(),
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

    // The interpreter can no longer act as a semantic oracle for this vessel,
    // because the vessel reaches a symbol only a native object's host can
    // define. What it can do is state that limit rather than leave it implied:
    // the VM must refuse the host call by name, and a route that quietly
    // resolved it to something else would fail here.
    // Run it from the stage's own directory. The interpreter writes a crash
    // report beside its working directory when it refuses, and refusing is
    // what this stage requires, so running it from the repository root left a
    // stray report there on every build. The report is useful evidence; it
    // belongs with the other stage logs.
    run_stage(
        &output,
        &sdk.join("nilinterpreter"),
        &["--no-ai".into(), nil.as_os_str().into()],
        &output,
        "interpreter",
        1,
        300,
    )?;
    let refusal = fs::read_to_string(output.join("interpreter.stderr"))
        .map_err(|e| format!("read Ny vessel interpreter diagnostics: {e}"))?;
    if !refusal.contains("host-supplied symbol") {
        return Err(format!(
            "the interpreter must refuse the vessel's host symbols by name; \
             it said: {}",
            refusal.trim()
        ));
    }

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
        input_return: symbol_u64_from_listing(&symbols, "anomalous_ny_vessel_input_return")?,
        exit_return: symbol_u64_from_listing(&symbols, "anomalous_ny_vessel_exit_return")?,
        mode_cell: symbol_u64_from_listing(&symbols, "anomalous_ny_vessel_mode")?,
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
    if artifact.image.is_empty() || artifact.image.len() as u64 > image_pages * page_size {
        return Err("Ny vessel RX image does not fit its code extent".into());
    }
    for (name, address) in [
        ("failed", artifact.failed),
        ("console_return", artifact.console_return),
        ("input_return", artifact.input_return),
        ("exit_return", artifact.exit_return),
    ] {
        if !(image_base..image_base + artifact.image.len() as u64).contains(&address) {
            return Err(format!("Ny vessel {name} lies outside its code page"));
        }
    }
    // The mode cell is the one header address that is not code. It has to be
    // inside the vessel's own state page, because the host writes it and must
    // not be able to reach the region or the mailbox through this header.
    if !(state_base..state_base + page_size).contains(&artifact.mode_cell)
        || !artifact.mode_cell.is_multiple_of(8)
    {
        return Err("Ny vessel mode cell lies outside its state page".into());
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
        ["-lW".into(), elf.as_os_str().to_owned()],
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
    // An opaque carrier: the kernel link gains bytes and a few symbols, not a
    // second loader. Nothing here is executed in kernel context.
    //
    // The header states the addresses the kernel cannot derive for itself. For
    // a fixture assembled into the kernel these come from linker symbols, but
    // a Ny vessel's code is produced by the toolchain, so the addresses the
    // dispatcher validates each gate return against have to travel with the
    // bytes. The kernel checks the magic and bounds every one of them.
    // The array's length is `header_bytes`, so the words written here and the
    // extent the kernel requires are one statement rather than two.
    let words: [u64; header_bytes / 8] = [
        header_magic,
        artifact.image.len() as u64,
        artifact.entry,
        artifact.console_return,
        artifact.input_return,
        artifact.exit_return,
        artifact.failed,
        artifact.mode_cell,
    ];
    let mut header = String::from(
        ".section .rodata.ny_vessel_header,\"a\",@progbits\n\
         .balign 16\n\
         .global x86_64_ny_vessel_header_start\n\
         .global x86_64_ny_vessel_header_end\n\
         x86_64_ny_vessel_header_start:\n",
    );
    for word in words {
        header.push_str(&format!(".quad {word}\n"));
    }
    header.push_str("x86_64_ny_vessel_header_end:\n");
    fs::write(
        output.join("payload.S"),
        format!(
            "{header}\
             .section .rodata.ny_vessel,\"a\",@progbits\n\
             .balign 16\n\
             .global x86_64_ny_vessel_payload_start\n\
             .global x86_64_ny_vessel_payload_end\n\
             x86_64_ny_vessel_payload_start:\n\
             .incbin \"vessel_rx.bin\"\n\
             x86_64_ny_vessel_payload_end:\n\
             .section .note.GNU-stack,\"\",@progbits\n"
        ),
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
