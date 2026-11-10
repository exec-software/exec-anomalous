//! Host-generated inert native plan for unchanged main/interpreter source bytes.
//! The external provisioning blob is data, never an execution or ownership grant.
use super::*;
use anomalous_elf_adapter::{linux_image, linux_startup};

pub(super) const plan_magic: u64 = u64::from_le_bytes(*b"ANLEPLN1");
pub(super) const plan_word_count: usize = 74;
pub(super) const source_limit: usize = 512 * 1024;
pub(super) const program_bias: u64 = 0x4000_0000;
pub(super) const interpreter_bias: u64 = 0x6000_0000;
/// Pages the two installed images and the stack map between them. This one
/// moves when the host's packages do -- 7 for the main image, 58 for the
/// loader and 16 for the stack as of the re-pin below -- so it is named here
/// and used by both the live check and the synthetic fixture, rather than
/// written twice and discovered to disagree by a failing test.
pub(super) const mapped_page_total: u64 = 81;
pub(super) const stack_start: u64 = 0x7fff_0000_0000;
pub(super) const stack_end: u64 = stack_start + 65536;
// The two host binaries this proof reads, pinned so a changed input is
// reviewed rather than absorbed. They are provisioning, not authority: the
// proof is inert and executes neither file, and the hashes travel into the
// recorded evidence so a reader can see exactly which bytes were planned
// against.
//
// Re-pinned 2026-11-09 for Ubuntu's coreutils 9.4-3ubuntu6.3 and libc6
// 2.39-0ubuntu8.9 on this build host, after `dpkg --verify` reported both
// files identical to their package manifests. The previous pair dated from
// before those packages were updated, and the fixture refused -- which is
// what it is for.
const program_hash: &str = "8a63c98320173f79e263115e10cefc170155e47d1fd4b5a70e4429d2699cae28";
const interpreter_hash: &str = "c20a2dc8917c755f02b94049356320fe1f62ac7d9f8994731f807d9df39302da";
pub(super) const file_source_limit: usize = 8 * 1024 * 1024;
pub(super) const file_source_grant: u64 = 0x4c45_0002;
pub(super) const file_source_name: &str = "opt/anomalous/linux-loader-file-source";
// The configured libc this proof reads as a file source, pinned for the same
// reason and re-pinned with the two above: libc6 2.39-0ubuntu8.9 on this
// build host, `dpkg --verify` clean.
const file_source_hash: &str = "3a15d66867d83762c7f2f1e37359cb8f6c5743edb369c65285cb0b1c4f7498bf";
const file_binding_offset: usize = 136;
const file_binding_size: usize = 32;
const prefix: &str = "x86_64_linux_loader_entry";
const policy_magic: u64 = u64::from_le_bytes(*b"ANLPOL03");
const policy_size: usize = 128;
const policy_absent_path: &[u8; 18] = b"/etc/ld.so.preload";
const policy_cache_absent_path: &[u8; 16] = b"/etc/ld.so.cache";
const budget: linux_image::limits = linux_image::limits {
    image_bytes: source_limit as u64,
    program_headers: 32,
    load_segments: 4,
    mapped_pages: 128,
};

pub(super) struct artifact {
    pub(super) object: PathBuf,
    blobs: [Vec<u8>; 3],
    fault: u64,
    file_faults: [u64; 2],
}

fn file_fault_sites(symbols: &str, rx: &[u8]) -> Result<[u64; 2], String> {
    let mut sites = [0; 2];
    for (site, name) in sites.iter_mut().zip([
        "anomalous_linux_file_stage_fault",
        "anomalous_linux_file_unknown_fault",
    ]) {
        *site = native_durable_artifact::exact_symbol(symbols, name, 'T')?;
        let offset = site
            .checked_sub(0x400000)
            .and_then(|value| usize::try_from(value).ok())
            .ok_or("file fault site lies outside native RX")?;
        if rx.get(offset..offset.saturating_add(2)) != Some(&[0x0f, 0x0b]) {
            return Err("file fault site is not an exact initialized UD2".into());
        }
    }
    if sites[0] == sites[1] {
        return Err("file fault scenarios alias the same instruction".into());
    }
    Ok(sites)
}

pub(super) fn retained_file_faults(root: &Path) -> Result<[u64; 2], String> {
    let output = root.join("build/linux-loader-entry");
    let path = output.join("native.elf");
    let native =
        elf_application_artifact::inspect_vessel(root, &path, "anomalous_linux_veil", true)?;
    if native.rx != held_bytes(&output.join("payload.bin"), 32768)? {
        return Err("retained native RX differs from its ELF".into());
    }
    file_fault_sites(
        &native_application_artifact::listing(root, &path)?,
        &native.rx,
    )
}

struct process_plan {
    words: [u64; plan_word_count],
    source: Vec<u8>,
    stack: Vec<u8>,
    stack_used: usize,
}

// This userspace personality state is inert launch data, not permission to map
// or allocate memory. Only the kernel-supplied role word changes at startup.
fn policy_record(initial_break: u64) -> Result<[u8; policy_size], String> {
    if initial_break == 0 || initial_break & 4095 != 0 || initial_break >= 1 << 47 {
        return Err("loader policy break is not a nonzero lower-canonical page boundary".into());
    }
    let mut bytes = [0; policy_size];
    for (slot, value) in bytes[..56].chunks_exact_mut(8).zip([
        0,
        policy_magic,
        3,
        policy_size as u64,
        initial_break,
        1,
        18,
    ]) {
        slot.copy_from_slice(&value.to_le_bytes());
    }
    // Declared launch namespace policy, never inferred from the host filesystem.
    bytes[56..74].copy_from_slice(policy_absent_path);
    bytes[80..88].copy_from_slice(&16u64.to_le_bytes());
    bytes[88..104].copy_from_slice(policy_cache_absent_path);
    Ok(bytes)
}

fn validate_policy(bytes: &[u8], initial_break: u64) -> Result<(), String> {
    if bytes != policy_record(initial_break)? {
        return Err("loader policy differs from the exact versioned host plan facts".into());
    }
    Ok(())
}

fn install_policy(data: &mut [u8], initial_break: u64) -> Result<(), String> {
    let policy = policy_record(initial_break)?;
    let policy_prefix = data
        .get_mut(..policy_size)
        .ok_or("native loader data lacks the complete policy prefix")?;
    if policy_prefix.iter().any(|byte| *byte != 0) {
        return Err("native loader policy prefix was not reserved zero data".into());
    }
    policy_prefix.copy_from_slice(&policy);
    Ok(())
}

// Independent host encoding of the native immutable-source deployment facts.
// This does not modify the Linux policy or grant access by itself.
fn file_binding(bytes: &[u8]) -> Result<[u8; file_binding_size], String> {
    if bytes.is_empty() || bytes.len() > file_source_limit {
        return Err("file source is outside its explicit8MiB provisioning budget".into());
    }
    let mut binding = [0; file_binding_size];
    for (slot, value) in binding.chunks_exact_mut(8).zip([
        u64::from_le_bytes(*b"ANFSB001"),
        file_source_grant,
        bytes.len() as u64,
        fnv64(bytes),
    ]) {
        slot.copy_from_slice(&value.to_le_bytes());
    }
    Ok(binding)
}

fn install_file_binding(data: &mut [u8], bytes: &[u8]) -> Result<(), String> {
    let binding = file_binding(bytes)?;
    if data.len() != file_binding_offset + file_binding_size
        || data[file_binding_offset..].iter().any(|byte| *byte != 0)
    {
        return Err("native file binding requires its exact reserved zero deployment area".into());
    }
    data[file_binding_offset..].copy_from_slice(&binding);
    Ok(())
}

pub(super) fn retained_file_source(root: &Path) -> Result<Vec<u8>, String> {
    let output = root.join("build/linux-loader-entry");
    let file = held_bytes(&output.join("file-source.bin"), file_source_limit)?;
    if sha256(&file)? != file_source_hash {
        return Err("retained file source differs from its pinned SHA-256".into());
    }
    let expected = file_binding(&file)?;
    let data = held_bytes(&output.join("data.bin"), 8192)?;
    if data.len() != file_binding_offset + file_binding_size
        || data[file_binding_offset..] != expected
        || held_bytes(&output.join("file-binding.bin"), file_binding_size)? != expected
    {
        return Err("retained file binding differs from independently rebuilt source facts".into());
    }
    Ok(file)
}

pub(super) fn fnv64(bytes: &[u8]) -> u64 {
    bytes.iter().fold(14_695_981_039_346_656_037, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(1_099_511_628_211)
    })
}

fn held_bytes(path: &Path, limit: usize) -> Result<Vec<u8>, String> {
    let file = File::open(path).map_err(|error| format!("open {}: {error}", path.display()))?;
    let metadata = file.metadata().map_err(|e| e.to_string())?;
    if !metadata.is_file() || metadata.len() > limit as u64 {
        return Err("loader input is not a regular file within its operation budget".into());
    }
    let mut bytes = Vec::new();
    file.take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > limit {
        return Err("loader input grew beyond its operation budget".into());
    }
    Ok(bytes)
}

fn sha256(bytes: &[u8]) -> Result<String, String> {
    let mut child = Command::new("sha256sum")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;
    let write = child
        .stdin
        .take()
        .ok_or("hash stdin missing")?
        .write_all(bytes);
    let output = child.wait_with_output().map_err(|e| e.to_string())?;
    write.map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err("hashing held loader input failed".into());
    }
    let digest = output.stdout.get(..64).ok_or("short SHA-256 output")?;
    if !digest
        .iter()
        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
    {
        return Err("invalid SHA-256 output".into());
    }
    String::from_utf8(digest.to_vec()).map_err(|e| e.to_string())
}

fn retained_random(output: &Path) -> Result<[u8; 16], String> {
    let retained = output.join("random.bin");
    let bytes = if let Some(input) = env::var_os("ANOMALOUS_LINUX_LOADER_RANDOM") {
        held_bytes(Path::new(&input), 16)?
    } else if retained.exists() {
        held_bytes(&retained, 16)?
    } else {
        let mut bytes = [0; 16];
        File::open("/dev/urandom")
            .and_then(|mut source| source.read_exact(&mut bytes))
            .map_err(|e| format!("read initial host random bytes: {e}"))?;
        bytes.to_vec()
    };
    let random: [u8; 16] = bytes
        .try_into()
        .map_err(|_| "random input requires exactly16 bytes")?;
    // A subsequent build reuses these exact bytes. An explicit replacement is
    // intentional input and necessarily changes the source/plan artifact hashes.
    fs::write(retained, random).map_err(|e| e.to_string())?;
    Ok(random)
}

fn process(program: &[u8], interpreter: &[u8], random: &[u8; 16]) -> Result<process_plan, String> {
    let main = linux_image::image::parse(program, budget)
        .and_then(|image| image.plan(program_bias, linux_image::image_role::executable))
        .map_err(|error| format!("main Linux image: {error:?}"))?;
    let loader = linux_image::image::parse(interpreter, budget)
        .and_then(|image| image.plan(interpreter_bias, linux_image::image_role::executable))
        .map_err(|error| format!("interpreter Linux image: {error:?}"))?;
    // The shape this proof plans against: two position-independent images, the
    // main one naming the loader as its interpreter, neither carrying TLS.
    // These are what make the plan meaningful at all, so a change here is not
    // a re-pin -- it is a different kind of input.
    if main.elf_type() != linux_image::elf_type::shared
        || loader.elf_type() != linux_image::elf_type::shared
        || main.interp() != Some(b"/lib64/ld-linux-x86-64.so.2".as_slice())
        || main.dynamic().is_none()
        || loader.dynamic().is_none()
        || loader.interp().is_some()
        || main.tls().is_some()
        || loader.tls().is_some()
    {
        return Err("installed inputs are not the two-image shape this plan assumes".into());
    }
    // And the measurements, which do move when the host's packages do. The
    // mismatch is reported with both numbers: a pin that only says "differs"
    // makes the reviewer rediscover the installed values before they can
    // judge whether the change is ordinary, which is the opposite of the
    // explicit review it is asking for.
    //
    // Re-pinned 2026-11-09 with the hashes above, and cross-checked against
    // `readelf` rather than taken from the parser alone: /usr/bin/true enters
    // at 0x19f0 with 13 program headers and 4 LOADs, and the loader enters at
    // 0x1f540 with 4 LOADs, both still position-independent.
    let measured: [(&str, u64, u64); 7] = [
        ("main loads", 4, main.loads().count() as u64),
        ("loader loads", 4, loader.loads().count() as u64),
        ("main mapped pages", 7, main.mapped_pages()),
        ("loader mapped pages", 58, loader.mapped_pages()),
        ("main entry", 0x4000_19f0, main.entry().unwrap_or_default()),
        (
            "loader entry",
            0x6001_f540,
            loader.entry().unwrap_or_default(),
        ),
        (
            "main program headers",
            13,
            main.program_header_count() as u64,
        ),
    ];
    let differing = measured
        .iter()
        .filter(|(_, pinned, installed)| pinned != installed)
        .map(|(name, pinned, installed)| {
            format!("{name}: pinned {pinned:#x}, installed {installed:#x}")
        })
        .collect::<Vec<_>>();
    if !differing.is_empty() || main.phdr_address() != Some(0x4000_0040) {
        return Err(format!(
            "installed inputs differ from the pinned two-image entry profile: {}; main PHDR \
             pinned 0x40000040, installed {:#x}",
            differing.join(", "),
            main.phdr_address().unwrap_or_default()
        ));
    }
    let auxiliary = [
        (11, 1000),
        (12, 1000),
        (13, 1000),
        (14, 1000),
        (23, 0),
        (17, 100),
        (16, 0),
        (26, 0),
    ]
    .map(|(kind, value)| linux_startup::auxiliary_entry {
        kind,
        value: linux_startup::auxiliary_value::scalar(value),
    });
    let argv: &[&[u8]] = &[b"/usr/bin/true"];
    let context = linux_startup::startup_context {
        image: linux_startup::image_facts {
            program_headers: main.phdr_address().ok_or("main PHDR missing")?,
            program_header_entry_size: linux_image::program_header_size,
            program_header_count: 14,
            entry: main.entry().ok_or("main entry missing")?,
            interpreter_base: Some(interpreter_bias),
            page_size: 4096,
        },
        stack: linux_startup::stack_range {
            base: stack_start,
            end: stack_end,
        },
        argv,
        envp: &[],
        auxiliary: &auxiliary,
        random,
        execfn: argv[0],
    };
    let mut stack = vec![0; (stack_end - stack_start) as usize];
    let layout = linux_startup::build(&context, &mut stack)
        .map_err(|error| format!("Linux startup template: {error:?}"))?;
    let initial_source_bytes = program
        .len()
        .checked_add(interpreter.len())
        .ok_or("source size overflow")?;
    let total = initial_source_bytes
        .checked_add(layout.used_bytes)
        .ok_or("source size overflow")?;
    if total > source_limit {
        return Err("loader source exceeds explicit512KiB import budget".into());
    }
    let mut source = Vec::with_capacity(total);
    source.extend_from_slice(program);
    source.extend_from_slice(interpreter);
    source.extend_from_slice(&stack[stack.len() - layout.used_bytes..]);
    let mut regions = Vec::new();
    let mut spans = Vec::new();
    let mut executable = Vec::new();
    for (image, offset) in [(main, 0), (loader, program.len() as u64)] {
        for segment in image.loads() {
            let mapped = segment.mapped();
            let flags =
                1 | (u64::from(segment.writable()) << 1) | (u64::from(segment.executable()) << 2);
            regions.push([mapped.start(), mapped.len() / 4096, flags]);
            let copy = segment.copy_file();
            let destination = segment.copy_destination();
            if copy.is_empty() || copy.len() != destination.len() {
                return Err("pinned LOAD has no exact nonempty source span".into());
            }
            spans.push([
                destination.start(),
                offset
                    .checked_add(copy.start())
                    .ok_or("source offset overflow")?,
                copy.len(),
            ]);
            if segment.executable() {
                executable.push([segment.initialized().start(), segment.initialized().end()]);
            }
        }
    }
    regions.push([stack_start, 16, 3]);
    spans.push([
        layout.stack_pointer,
        initial_source_bytes as u64,
        layout.used_bytes as u64,
    ]);
    let brk = main
        .loads()
        .map(|segment| segment.memory().end())
        .max()
        .ok_or("main loads missing")?
        .checked_add(4095)
        .ok_or("main break overflow")?
        & !4095;
    let mut words = [0; plan_word_count];
    words[..16].copy_from_slice(&[
        plan_magic,
        1,
        source.len() as u64,
        fnv64(&source),
        loader.entry().ok_or("loader entry missing")?,
        layout.stack_pointer,
        9,
        9,
        2,
        8,
        stack_start,
        stack_end,
        main.entry().ok_or("main entry missing")?,
        main.phdr_address().ok_or("PHDR missing")?,
        interpreter_bias,
        brk,
    ]);
    if regions.len() != 9 || spans.len() != 9 || executable.len() != 2 {
        return Err("pinned native plan record counts changed".into());
    }
    for (slot, record) in words[16..43].chunks_exact_mut(3).zip(regions) {
        slot.copy_from_slice(&record);
    }
    for (slot, record) in words[43..70].chunks_exact_mut(3).zip(spans) {
        slot.copy_from_slice(&record);
    }
    for (slot, record) in words[70..74].chunks_exact_mut(2).zip(executable) {
        slot.copy_from_slice(&record);
    }
    validate_plan(&words, &source)?;
    Ok(process_plan {
        words,
        source,
        stack,
        stack_used: layout.used_bytes,
    })
}

fn validate_plan(words: &[u64; plan_word_count], source: &[u8]) -> Result<(), String> {
    let fail = || "inert native plan failed exact bounds/permission validation".to_string();
    if words[0] != plan_magic
        || words[1] != 1
        || source.is_empty()
        || source.len() > source_limit
        || words[2] != source.len() as u64
        || words[3] != fnv64(source)
        || words[6..10] != [9, 9, 2, 8]
        || words[10] != stack_start
        || words[11] != stack_end
        || words[5] & 15 != 0
        || words[5] < stack_start
        || words[5] >= stack_end
    {
        return Err(fail());
    }
    let regions = words[16..43].chunks_exact(3).collect::<Vec<_>>();
    let mut ends = Vec::new();
    let mut pages = 0u64;
    for (index, region) in regions.iter().enumerate() {
        let length = region[1].checked_mul(4096).ok_or_else(fail)?;
        let end = region[0].checked_add(length).ok_or_else(fail)?;
        if region[0] == 0
            || region[0] & 4095 != 0
            || length == 0
            || end > 1 << 47
            || !matches!(region[2], 1 | 3 | 5)
            || regions[..index]
                .iter()
                .zip(&ends)
                .any(|(old, previous_end)| region[0] < *previous_end && old[0] < end)
        {
            return Err(fail());
        }
        ends.push(end);
        pages = pages.checked_add(region[1]).ok_or_else(fail)?;
    }
    // Unlike the structural checks around it, the total moves when the host's
    // packages do, so it reports both numbers rather than only refusing.
    if pages != mapped_page_total {
        return Err(format!(
            "inert native plan maps {pages} pages, not the pinned {mapped_page_total}; the \
             installed images changed and the profile above needs reviewing with it"
        ));
    }
    if regions[8] != [stack_start, 16, 3] {
        return Err(fail());
    }
    let spans = words[43..70].chunks_exact(3).collect::<Vec<_>>();
    let mut destinations = Vec::new();
    for span in &spans {
        let end = span[0].checked_add(span[2]).ok_or_else(fail)?;
        let source_end = span[1].checked_add(span[2]).ok_or_else(fail)?;
        if span[2] == 0
            || source_end > source.len() as u64
            || !regions
                .iter()
                .zip(&ends)
                .any(|(region, bound)| span[0] >= region[0] && end <= *bound)
            || destinations
                .iter()
                .any(|(start, limit)| span[0] < *limit && *start < end)
        {
            return Err(fail());
        }
        destinations.push((span[0], end));
    }
    let executable = words[70..74].chunks_exact(2).collect::<Vec<_>>();
    for (index, interval) in executable.iter().enumerate() {
        if interval[0] >= interval[1]
            || !regions.iter().zip(&ends).any(|(region, end)| {
                region[2] == 5 && interval[0] >= region[0] && interval[1] <= *end
            })
            || !destinations
                .iter()
                .any(|(start, end)| interval[0] >= *start && interval[1] <= *end)
            || executable[..index]
                .iter()
                .any(|old| interval[0] < old[1] && old[0] < interval[1])
        {
            return Err(fail());
        }
    }
    if !executable
        .iter()
        .any(|interval| words[4] >= interval[0] && words[4] < interval[1])
    {
        return Err(fail());
    }
    Ok(())
}

pub(super) fn retained_plan(root: &Path) -> Result<([u64; plan_word_count], Vec<u8>), String> {
    let output = root.join("build/linux-loader-entry");
    let program = held_bytes(&output.join("true.elf"), source_limit)?;
    let interpreter = held_bytes(&output.join("interpreter.elf"), source_limit)?;
    if sha256(&program)? != program_hash || sha256(&interpreter)? != interpreter_hash {
        return Err("retained loader input hashes changed".into());
    }
    let random: [u8; 16] = held_bytes(&output.join("random.bin"), 16)?
        .try_into()
        .map_err(|_| "retained random input is not16 bytes")?;
    let expected = process(&program, &interpreter, &random)?;
    let source = held_bytes(&output.join("source.bin"), source_limit)?;
    let encoded = held_bytes(&output.join("plan.bin"), plan_word_count * 8)?;
    let expected_encoded = expected
        .words
        .iter()
        .flat_map(|word| word.to_le_bytes())
        .collect::<Vec<_>>();
    if source != expected.source
        || encoded != expected_encoded
        || held_bytes(&output.join("stack.bin"), 65536)? != expected.stack
    {
        return Err(
            "retained plan/source/stack differ from independently rebuilt inert facts".into(),
        );
    }
    let data = held_bytes(&output.join("data.bin"), 8192)?;
    let policy = data
        .get(..policy_size)
        .ok_or("retained native data lacks the complete policy prefix")?;
    validate_policy(policy, expected.words[15])?;
    if held_bytes(&output.join("policy.bin"), policy_size)? != policy {
        return Err("retained policy bytes differ from the native data prefix".into());
    }
    Ok((expected.words, source))
}

pub(super) fn package_image(root: &Path, build: &Path) -> Result<artifact, String> {
    let output = build.join("linux-loader-entry");
    fs::create_dir_all(&output).map_err(|e| e.to_string())?;
    let output = fs::canonicalize(output).map_err(|e| e.to_string())?;
    let program = held_bytes(Path::new("/usr/bin/true"), source_limit)?;
    let interpreter = held_bytes(Path::new("/lib64/ld-linux-x86-64.so.2"), source_limit)?;
    if sha256(&program)? != program_hash || sha256(&interpreter)? != interpreter_hash {
        return Err(
            "installed true/interpreter hashes changed; review new inputs explicitly".into(),
        );
    }
    let configured_file = env::var_os("ANOMALOUS_LINUX_LOADER_LIBC")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/lib/x86_64-linux-gnu/libc.so.6"));
    let file_source = held_bytes(&configured_file, file_source_limit)?;
    if sha256(&file_source)? != file_source_hash {
        return Err("configured libc hash changed; review new input explicitly".into());
    }
    let random = retained_random(&output)?;
    let process = process(&program, &interpreter, &random)?;
    let plan = process
        .words
        .iter()
        .flat_map(|word| word.to_le_bytes())
        .collect::<Vec<_>>();
    for (name, bytes) in [
        ("true.elf", &program),
        ("interpreter.elf", &interpreter),
        ("source.bin", &process.source),
        ("stack.bin", &process.stack),
        ("plan.bin", &plan),
        ("file-source.bin", &file_source),
    ] {
        fs::write(output.join(name), bytes).map_err(|e| e.to_string())?;
    }
    elf_application_artifact::build_vessel(
        root,
        "anomalous-linux-veil",
        Some("loader-entry"),
        None,
    )?;
    let native_path = elf_application_artifact::retain_vessel(
        root,
        &output,
        "anomalous-linux-loader-entry-veil",
        "native.elf",
    )?;
    let mut native =
        elf_application_artifact::inspect_vessel(root, &native_path, "anomalous_linux_veil", true)?;
    let native_symbols = native_application_artifact::listing(root, &native_path)?;
    let file_faults = file_fault_sites(&native_symbols, &native.rx)?;
    for (suffix, expected) in [
        ("file_source_binding_start", 0x800088),
        ("file_source_binding_end", 0x8000a8),
        ("file_source_report_start", 0x8000b0),
        ("file_source_report_end", 0x8000d0),
        ("file_source_buffer_start", 0x8000d0),
        ("file_source_buffer_end", 0x8010d0),
        ("file_service_report_start", 0x8010d0),
        ("file_service_report_end", 0x801120),
    ] {
        if symbol_u64_from_listing(&native_symbols, &format!("anomalous_linux_veil_{suffix}"))?
            != expected
        {
            return Err("native file binding/buffer/report deployment geometry changed".into());
        }
    }
    // Inspect the original ELF first, then populate only its reserved deployment
    // data prefix. The original canary and every later initialized byte survive.
    install_policy(&mut native.rw, process.words[15])?;
    install_file_binding(&mut native.rw, &file_source)?;
    validate_policy(&native.rw[..policy_size], process.words[15])?;
    fs::write(output.join("policy.bin"), &native.rw[..policy_size]).map_err(|e| e.to_string())?;
    fs::write(
        output.join("file-binding.bin"),
        &native.rw[file_binding_offset..],
    )
    .map_err(|e| e.to_string())?;
    let artifact = artifact {
        object: output.join("payload.o"),
        blobs: [plan, native.rx, native.rw],
        fault: native.fault,
        file_faults,
    };
    let mut assembly = String::from(".section .rodata.linux_loader_entry,\"a\",@progbits\n");
    for (name, bytes) in ["plan", "payload", "data"].into_iter().zip(&artifact.blobs) {
        fs::write(output.join(format!("{name}.bin")), bytes).map_err(|e| e.to_string())?;
        assembly.push_str(&format!(".balign 16\n.global {prefix}_{name}_start\n.global {prefix}_{name}_end\n{prefix}_{name}_start:\n.incbin \"{name}.bin\"\n{prefix}_{name}_end:\n"));
    }
    assembly.push_str(&format!(
        ".global {prefix}_user_fault_instruction\n.set {prefix}_user_fault_instruction, 0x{:x}\n",
        artifact.fault
    ));
    for (name, address) in ["stage_fault", "unknown_fault"]
        .into_iter()
        .zip(artifact.file_faults)
    {
        assembly.push_str(&format!(
            ".global {prefix}_{name}\n.set {prefix}_{name}, 0x{address:x}\n"
        ));
    }
    assembly.push_str(".section .note.GNU-stack,\"\",@progbits\n");
    fs::write(output.join("payload.S"), assembly).map_err(|e| e.to_string())?;
    run_checked(
        &output,
        "as",
        [
            "--64".into(),
            "-o".into(),
            artifact.object.clone().into_os_string(),
            "payload.S".into(),
        ],
    )?;
    let numbers = process
        .words
        .iter()
        .map(u64::to_string)
        .collect::<Vec<_>>()
        .join(",");
    let report = format!(
        "{{\"scope\":\"host-generated-inert-native-loader-plan\",\"executed\":false,\"admitted\":false,\"plan_words\":[{numbers}],\"program_sha256\":\"{program_hash}\",\"interpreter_sha256\":\"{interpreter_hash}\",\"source_sha256\":\"{}\",\"plan_sha256\":\"{}\",\"random_sha256\":\"{}\",\"stack_used\":{},\"policy_magic\":\"ANLPOL03\",\"policy_version\":3,\"policy_bytes\":128,\"policy_initial_break\":{},\"policy_sha256\":\"{}\",\"policy_scope\":\"userspace initial break and declared absent paths only; no heap or general filesystem authority\",\"policy_namespace_kind\":1,\"policy_absent_path\":\"/etc/ld.so.preload\",\"policy_path_bytes\":18,\"policy_cache_absent_path\":\"/etc/ld.so.cache\",\"policy_cache_path_bytes\":16,\"policy_read_budget\":256,\"initial_rdx\":0,\"argv\":[\"/usr/bin/true\"],\"envp\":[],\"uid\":1000,\"euid\":1000,\"gid\":1000,\"egid\":1000,\"secure\":0,\"clktck\":100,\"hwcap\":0,\"hwcap2\":0,\"identity_policy\":\"explicit unprivileged Linux personality IDs; no host credential inference\",\"clock_policy\":\"personality accounting unit100; no clock syscall supplied\",\"cpu_policy\":\"conservative auxv; guest gates OSXSAVE,FSGSBASE,CET,PKE,PKS off; no vDSO\",\"source_trust\":\"trusted host provisioning; FNV64 is integrity bookkeeping, not authentication\"}}\n",
        sha256(&process.source)?,
        sha256(&artifact.blobs[0])?,
        sha256(&random)?,
        process.stack_used,
        process.words[15],
        sha256(&artifact.blobs[2][..policy_size])?
    );
    fs::write(output.join("report.json"), report).map_err(|e| e.to_string())?;
    let report = format!(
        "{{\"scope\":\"host-provisioned-native-file-source\",\"executed\":false,\"mapped_into_linux\":false,\"fw_cfg_name\":\"{file_source_name}\",\"grant\":{file_source_grant},\"bytes\":{},\"pages\":{},\"sha256\":\"{file_source_hash}\",\"fnv64\":{},\"binding_sha256\":\"{}\",\"binding_offset\":136,\"binding_bytes\":32,\"report_offset\":176,\"report_bytes\":32,\"source_budget_bytes\":{file_source_limit},\"read_budget_bytes\":4096,\"file_service_report_magic\":\"ANFSPR01\",\"file_service_report_offset\":4304,\"file_service_report_bytes\":80,\"file_service_context_magic\":\"ANFCTX01\",\"file_service_context_bytes\":80,\"namespace_prefix\":\"/lib/x86_64-linux-gnu\",\"namespace_file\":\"/lib/x86_64-linux-gnu/libc.so.6\",\"trust\":\"trusted host provisioning; FNV64 is integrity bookkeeping, not authentication\"}}\n",
        file_source.len(),
        file_source.len().div_ceil(4096),
        fnv64(&file_source),
        sha256(&artifact.blobs[2][file_binding_offset..])?
    );
    fs::write(output.join("file-source-report.json"), report).map_err(|e| e.to_string())?;
    Ok(artifact)
}

pub(super) fn inspect_image(root: &Path, path: &Path, artifact: &artifact) -> Result<(), String> {
    if artifact.blobs[0].len() != plan_word_count * 8 {
        return Err("loader linked plan has an invalid length".into());
    }
    let initial_break = u64::from_le_bytes(
        artifact.blobs[0][15 * 8..16 * 8]
            .try_into()
            .map_err(|_| "loader initial break word is incomplete")?,
    );
    validate_policy(
        artifact.blobs[2]
            .get(..policy_size)
            .ok_or("loader linked native data lacks its policy prefix")?,
        initial_break,
    )?;
    let file_source = retained_file_source(root)?;
    if artifact.blobs[2].get(file_binding_offset..) != Some(file_binding(&file_source)?.as_slice())
    {
        return Err("linked file source binding differs from retained provisioning".into());
    }
    let bytes = fs::read(path).map_err(|e| e.to_string())?;
    let elf = parse_elf64(&bytes)?;
    let symbols = native_application_artifact::listing(root, path)?;
    let mut ranges = Vec::new();
    for (name, expected) in ["plan", "payload", "data"].into_iter().zip(&artifact.blobs) {
        let start = native_durable_artifact::exact_symbol(
            &symbols,
            &format!("{prefix}_{name}_start"),
            'R',
        )?;
        let end =
            native_durable_artifact::exact_symbol(&symbols, &format!("{prefix}_{name}_end"), 'R')?;
        if end.checked_sub(start) != Some(expected.len() as u64)
            || ranges.iter().any(|(a, b)| start < *b && *a < end)
        {
            return Err("loader linked blob extent changed/overlapped".into());
        }
        ranges.push((start, end));
        let loads = elf
            .program_headers
            .iter()
            .filter(|load| {
                load.kind == 1
                    && start < load.virtual_address.saturating_add(load.memory_size)
                    && load.virtual_address < end
            })
            .collect::<Vec<_>>();
        if loads.len() != 1
            || loads[0].flags & 6 != 4
            || start < loads[0].virtual_address
            || end > loads[0].virtual_address.saturating_add(loads[0].file_size)
            || elf_virtual_bytes(&bytes, *loads[0], start, expected.len())? != expected
        {
            return Err("loader blob lost unique readonly backing or exact bytes".into());
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
            return Err("loader blob lost unique readonly section".into());
        }
    }
    if native_durable_artifact::exact_symbol(
        &symbols,
        &format!("{prefix}_user_fault_instruction"),
        'A',
    )? != artifact.fault
    {
        return Err("loader native failure alias changed".into());
    }
    for (name, address) in ["stage_fault", "unknown_fault"]
        .into_iter()
        .zip(artifact.file_faults)
    {
        if native_durable_artifact::exact_symbol(&symbols, &format!("{prefix}_{name}"), 'A')?
            != address
        {
            return Err("loader file fault alias changed".into());
        }
    }
    Ok(())
}

pub(super) fn verify_absent(symbols: &str, bytes: &[u8]) -> Result<(), String> {
    if symbols.contains("linux_loader_entry")
        || bytes.windows(8).any(|value| value == b"ANLEPLN1")
        || bytes
            .windows(8)
            .any(|value| value == b"ANLPOL02" || value == b"ANLPOL03")
        || bytes
            .windows(8)
            .any(|value| matches!(value, b"ANFSB001" | b"ANFSR001" | b"ANFCTX01" | b"ANFSPR01"))
        || bytes
            .windows(b"kernel.linux_loader_entry_test".len())
            .any(|value| value == b"kernel.linux_loader_entry_test")
    {
        return Err("loader-entry profile leaked into an unselected image".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_faults_require_distinct_initialized_exact_ud2_symbols() {
        let symbols = "anomalous_linux_file_stage_fault T 400000 2\nanomalous_linux_file_unknown_fault T 400002 2\n";
        let bytes = [0x0f, 0x0b, 0x0f, 0x0b];
        assert_eq!(
            file_fault_sites(symbols, &bytes).unwrap(),
            [0x400000, 0x400002]
        );
        for bad_symbols in [
            symbols.replace("400002", "400000"),
            symbols.replace("400002", "400003"),
            symbols.replace("400002", "3fffff"),
            symbols.replace(" T ", " D "),
            format!("{symbols}{symbols}"),
        ] {
            assert!(file_fault_sites(&bad_symbols, &bytes).is_err());
        }
        assert!(file_fault_sites(symbols, &[0x0f, 0x0b, 0x90, 0x90]).is_err());
        assert!(file_fault_sites(symbols, &bytes[..3]).is_err());
    }

    fn fixture() -> ([u64; plan_word_count], Vec<u8>) {
        let source = vec![0x53; 128];
        let mut words = [0; plan_word_count];
        words[..16].copy_from_slice(&[
            plan_magic,
            1,
            128,
            fnv64(&source),
            0x40002000,
            stack_end - 32,
            9,
            9,
            2,
            8,
            stack_start,
            stack_end,
            0x40002000,
            0x40000040,
            interpreter_bias,
            0x40009000,
        ]);
        // Eight regions of fixed size plus one that takes up the remainder of
        // the pinned total, so this synthetic plan satisfies the page check by
        // construction and a re-pin does not have to be made twice.
        let fixed_pages = 2 + 4 + 1 + 2 + 1 + 11 + 4 + 16;
        let regions = [
            [0x40000000, 2, 1],
            [0x40002000, 4, 5],
            [0x40006000, 1, 1],
            [0x40007000, 2, 3],
            [0x60000000, 1, 1],
            [0x60001000, mapped_page_total - fixed_pages, 5],
            [0x60030000, 11, 1],
            [0x6003b000, 4, 3],
            [stack_start, 16, 3],
        ];
        for (index, region) in regions.iter().enumerate() {
            words[16 + index * 3..19 + index * 3].copy_from_slice(region);
            words[43 + index * 3..46 + index * 3].copy_from_slice(&[region[0], 0, 16]);
        }
        words[70..74].copy_from_slice(&[0x40002000, 0x40002010, 0x60001000, 0x60001010]);
        (words, source)
    }

    #[test]
    fn plan_rejects_overflow_aliases_permissions_and_uninitialized_entries() {
        let (valid, source) = fixture();
        validate_plan(&valid, &source).unwrap();
        for (index, value) in [
            (0, 0),
            (1, 2),
            (2, 127),
            (3, 0),
            (6, 8),
            (7, 10),
            (8, 1),
            (9, 7),
            (5, stack_end - 31),
            (16, 1 << 47),
            (17, u64::MAX),
            (18, 7),
            (19, 0x40001000),
            (44, u64::MAX),
            (45, 129),
            (46, 0x40000008),
            (70, 0x40001000),
            (71, 0x40002020),
            (72, 0x40002008),
            (4, 0x40002010),
        ] {
            let mut changed = valid;
            changed[index] = value;
            assert!(
                validate_plan(&changed, &source).is_err(),
                "accepted changed word{index}"
            );
        }
        let mut changed = source.clone();
        changed[0] ^= 1;
        assert!(validate_plan(&valid, &changed).is_err());
    }

    #[test]
    fn source_hash_has_known_vectors_and_plan_encoding_is_exact() {
        assert_eq!(fnv64(b""), 0xcbf29ce484222325);
        assert_eq!(fnv64(b"a"), 0xaf63dc4c8601ec8c);
        let (words, _) = fixture();
        let bytes = words
            .iter()
            .flat_map(|word| word.to_le_bytes())
            .collect::<Vec<_>>();
        assert_eq!(bytes.len(), 592);
        assert_eq!(&bytes[..8], b"ANLEPLN1");
        let decoded = bytes
            .chunks_exact(8)
            .map(|value| u64::from_le_bytes(value.try_into().unwrap()))
            .collect::<Vec<_>>();
        assert_eq!(decoded, words);
    }

    #[test]
    fn policy_encodes_full_width_personality_fact_and_zero_reserved_words() {
        let initial_break = 0x7fff_1234_5000;
        let policy = policy_record(initial_break).unwrap();
        assert_eq!(policy.len(), 128);
        assert_eq!(&policy[..8], &[0; 8]);
        assert_eq!(&policy[8..16], b"ANLPOL03");
        assert_eq!(&policy[16..24], &3u64.to_le_bytes());
        assert_eq!(&policy[24..32], &128u64.to_le_bytes());
        assert_eq!(&policy[32..40], &initial_break.to_le_bytes());
        assert_eq!(&policy[40..48], &1u64.to_le_bytes());
        assert_eq!(&policy[48..56], &18u64.to_le_bytes());
        assert_eq!(&policy[56..74], b"/etc/ld.so.preload");
        assert_eq!(&policy[74..80], &[0; 6]);
        assert_eq!(&policy[80..88], &16u64.to_le_bytes());
        assert_eq!(&policy[88..104], b"/etc/ld.so.cache");
        assert_eq!(&policy[104..], &[0; 24]);
        validate_policy(&policy, initial_break).unwrap();
    }

    #[test]
    fn policy_rejects_every_mutated_byte_wrong_length_and_unbound_break() {
        let initial_break = 0x4000_9000;
        let policy = policy_record(initial_break).unwrap();
        for index in 0..policy_size {
            for bit in 0..8 {
                let mut changed = policy;
                changed[index] ^= 1 << bit;
                assert!(
                    validate_policy(&changed, initial_break).is_err(),
                    "accepted changed policy byte{index} bit{bit}"
                );
            }
        }
        assert!(validate_policy(&policy[..policy_size - 1], initial_break).is_err());
        let mut oversized = policy.to_vec();
        oversized.push(0);
        assert!(validate_policy(&oversized, initial_break).is_err());
        assert!(validate_policy(&policy, initial_break + 4096).is_err());
        // Runtime-assigned roles are never accepted as host artifact state.
        for role in [1u64, 2] {
            let mut changed = policy;
            changed[..8].copy_from_slice(&role.to_le_bytes());
            assert!(validate_policy(&changed, initial_break).is_err());
        }
    }

    #[test]
    fn policy_install_preserves_canary_and_rejects_atomically() {
        let initial_break = 0x4000_9000;
        let mut original = vec![0; policy_size];
        original.extend_from_slice(&0x454c_465f_4441_5441u64.to_le_bytes());
        original.extend_from_slice(&[0x73; 37]);
        let mut valid = original.clone();
        install_policy(&mut valid, initial_break).unwrap();
        validate_policy(&valid[..policy_size], initial_break).unwrap();
        assert_eq!(&valid[policy_size..], &original[policy_size..]);
        for invalid_break in [0, 1, initial_break + 1, 1 << 47, u64::MAX] {
            let mut changed = original.clone();
            assert!(install_policy(&mut changed, invalid_break).is_err());
            assert_eq!(changed, original);
        }
        for length in [0, 8, policy_size - 1] {
            let mut short = vec![0; length];
            let before = short.clone();
            assert!(install_policy(&mut short, initial_break).is_err());
            assert_eq!(short, before);
        }
        for index in 0..policy_size {
            let mut occupied = original.clone();
            occupied[index] = 0x5a;
            let before = occupied.clone();
            assert!(install_policy(&mut occupied, initial_break).is_err());
            assert_eq!(occupied, before);
        }
        let before = valid.clone();
        assert!(install_policy(&mut valid, initial_break).is_err());
        assert_eq!(valid, before);
    }

    #[test]
    fn file_binding_is_exact_separate_and_rejects_reserved_data_changes() {
        let file = [0x37; 8193];
        let binding = file_binding(&file).unwrap();
        assert_eq!(&binding[..8], b"ANFSB001");
        assert_eq!(&binding[8..16], &file_source_grant.to_le_bytes());
        assert_eq!(&binding[16..24], &8193u64.to_le_bytes());
        assert_eq!(&binding[24..], &fnv64(&file).to_le_bytes());
        assert!(file_binding(&[]).is_err());
        assert!(file_binding(&vec![0; file_source_limit + 1]).is_err());
        let mut original = vec![0x53; file_binding_offset];
        original.resize(file_binding_offset + file_binding_size, 0);
        let mut data = original.clone();
        install_file_binding(&mut data, &file).unwrap();
        assert_eq!(
            &data[..file_binding_offset],
            &original[..file_binding_offset]
        );
        assert_eq!(&data[file_binding_offset..], &binding);
        for index in file_binding_offset..original.len() {
            let mut occupied = original.clone();
            occupied[index] = 1;
            let before = occupied.clone();
            assert!(install_file_binding(&mut occupied, &file).is_err());
            assert_eq!(occupied, before);
        }
        for length in [
            0,
            file_binding_offset,
            original.len() - 1,
            original.len() + 1,
        ] {
            let mut wrong = original.clone();
            wrong.resize(length, 0);
            let before = wrong.clone();
            assert!(install_file_binding(&mut wrong, &file).is_err());
            assert_eq!(wrong, before);
        }
    }

    #[test]
    fn default_image_rejects_new_profile_markers() {
        verify_absent("ordinary", b"ordinary").unwrap();
        assert!(verify_absent("x86_64_linux_loader_entry_payload_start", b"").is_err());
        assert!(verify_absent("", b"ANLEPLN1").is_err());
        assert!(verify_absent("", b"ANLPOL02").is_err());
        assert!(verify_absent("", b"ANLPOL03").is_err());
        assert!(verify_absent("", b"ANFSB001").is_err());
        assert!(verify_absent("", b"ANFSR001").is_err());
        assert!(verify_absent("", b"kernel.linux_loader_entry_test.ready").is_err());
    }
}
