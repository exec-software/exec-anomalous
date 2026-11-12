//! Host-only packaging of the fixed native client, not an ELF execution ABI.
use super::*;

const package: &str = "anomalous-native-client";
const image_base: u64 = 0x400000;
const data_base: u64 = 0x800000;
const image_limit: u64 = 8 * 4096;
// Independent five-page bound for the measured native manifest loader.
// Legacy application/runtime entry points retain their eight-page allowance.
const manifest_image_limit: u64 = 5 * 4096;
const data_limit: u64 = 2 * 4096;
const fault_alias: &str = "x86_64_native_application_user_fault_instruction";

pub(super) struct artifact {
    pub(super) object: PathBuf,
    image: Vec<u8>,
    fault: u64,
}

pub(super) fn package_image(root: &Path, build: &Path) -> Result<artifact, String> {
    let output = build.join("native-application");
    fs::create_dir_all(&output)
        .map_err(|error| format!("create native artifact directory: {error}"))?;
    remove_optional_file(&output.join("qualification.txt"))?;
    let elf = output.join("client.elf");
    let object = output.join("payload.o");
    remove_optional_file(&elf)?;
    remove_optional_file(&object)?;
    // Rebuild core with the same small model used by this low-address image.
    // PLT-style object calls let the static linker resolve compiler libcalls
    // directly; the final artifact still must contain no GOT, PLT or imports.
    run_checked_with_environment(
        root,
        env::var_os("CARGO").unwrap_or_else(|| "cargo".into()),
        [
            "build".into(),
            "-Zbuild-std=core,compiler_builtins".into(),
            "-Zbuild-std-features=compiler-builtins-mem".into(),
            "--release".into(),
            "--package".into(),
            package.into(),
            "--target".into(),
            "x86_64-unknown-none".into(),
            "--target-dir".into(),
            root.join("target").into_os_string(),
            "--locked".into(),
            "--no-default-features".into(),
        ],
        "CARGO_ENCODED_RUSTFLAGS",
        "-C\u{1f}code-model=small\u{1f}-C\u{1f}relocation-model=static\u{1f}-C\u{1f}force-frame-pointers=yes\u{1f}-Z\u{1f}plt=yes",
    )?;
    fs::copy(
        root.join("target/x86_64-unknown-none/release")
            .join(package),
        &elf,
    )
    .map_err(|error| format!("retain native client ELF: {error}"))?;
    let bytes = fs::read(&elf).map_err(|error| error.to_string())?;
    let parsed = parse_elf64(&bytes)?;
    let image = inspect_image(&bytes, &parsed)?;
    let symbols = listing(root, &elf)?;
    if symbol_u64_from_listing(&symbols, "anomalous_native_client_entry")? != image_base {
        return Err("native client entry symbol differs from fixed entry".into());
    }
    if symbol_u64_from_listing(&symbols, "anomalous_native_client_data_start")? != data_base
        || symbol_u64_from_listing(&symbols, "anomalous_native_client_data_end")?
            != data_base + parsed.program_headers[1].memory_size
        || symbol_u64_from_listing(&symbols, "anomalous_native_client_bss_start")? < data_base + 128
        || symbol_u64_from_listing(&symbols, "anomalous_native_client_bss_end")?
            != data_base + parsed.program_headers[1].memory_size
    {
        return Err("native client zeroed data/BSS markers changed".into());
    }
    let fault = symbol_u64_from_listing(&symbols, "anomalous_native_client_fault_instruction")?;
    let text = parsed
        .section_headers
        .iter()
        .find(|section| section.name == ".text")
        .ok_or("native client .text missing")?;
    if fault < text.address
        || fault
            .checked_add(2)
            .is_none_or(|end| end > text.address + text.size)
        || elf_virtual_bytes(&bytes, parsed.program_headers[0], fault, 2)? != [0x0f, 0x0b]
    {
        return Err("native client fault marker is not exact executable UD2".into());
    }
    let disassembly = run_output_checked(
        root,
        "objdump",
        [
            "--disassemble".into(),
            "--disassemble-zeroes".into(),
            "--wide".into(),
            "--insn-width=16".into(),
            "--section=.text".into(),
            elf.as_os_str().to_owned(),
        ],
    )?;
    fs::write(output.join("client.disassembly"), &disassembly)
        .map_err(|error| error.to_string())?;
    let disassembly =
        String::from_utf8(disassembly).map_err(|_| "native disassembly is not UTF-8")?;
    inspect_instructions(&disassembly, &image, text.size, fault)?;
    fs::write(output.join("client_rx.bin"), &image).map_err(|error| error.to_string())?;
    let assembly = format!(
        ".section .rodata.native_application,\"a\",@progbits\n.balign 16\n.global x86_64_native_application_payload_start\n.global x86_64_native_application_payload_end\nx86_64_native_application_payload_start:\n.incbin \"client_rx.bin\"\nx86_64_native_application_payload_end:\n.global {fault_alias}\n.set {fault_alias}, 0x{fault:x}\n.section .note.GNU-stack,\"\",@progbits\n"
    );
    fs::write(output.join("payload.S"), assembly).map_err(|error| error.to_string())?;
    run_checked(
        &output,
        "as",
        [
            "--64".into(),
            "--fatal-warnings".into(),
            "-o".into(),
            "payload.o".into(),
            "payload.S".into(),
        ],
    )?;
    let artifact = artifact {
        object,
        image,
        fault,
    };
    inspect_wrapper(root, &artifact)?;
    let mut record = format!(
        "profile=native_application_client_v1\nentry=0x{image_base:x}\nrx_bytes={}\nrw_bytes={}\nfault=0x{fault:x}\n",
        artifact.image.len(),
        parsed.program_headers[1].memory_size
    );
    for name in ["client.elf", "client_rx.bin", "payload.o"] {
        let digest = run_output_checked(root, "sha256sum", [output.join(name).into_os_string()])?;
        let digest = String::from_utf8(digest).map_err(|_| "native hash is not UTF-8")?;
        let value = digest
            .split_whitespace()
            .next()
            .ok_or("native hash missing")?;
        if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err("native hash malformed".into());
        }
        record.push_str(&format!("{name}_sha256={value}\n"));
    }
    fs::write(output.join("qualification.txt"), record).map_err(|error| error.to_string())?;
    Ok(artifact)
}

pub(super) fn inspect_image(bytes: &[u8], elf: &parsed_elf64) -> Result<Vec<u8>, String> {
    inspect_image_profile(bytes, elf, None, rodata_metadata::plain, image_limit)
}

pub(super) fn inspect_elf_loader_image(
    bytes: &[u8],
    elf: &parsed_elf64,
) -> Result<Vec<u8>, String> {
    inspect_image_profile(bytes, elf, None, rodata_metadata::merge, image_limit)
}

/// The ELF deployment fixture permits exactly one initialized u64 following
/// the unchanged 128-byte zeroed boot-argument prefix. Native stays all-zero.
pub(super) fn inspect_image_with_canary(
    bytes: &[u8],
    elf: &parsed_elf64,
    canary: u64,
) -> Result<Vec<u8>, String> {
    if canary == 0 {
        return Err("ELF data canary must exercise nonzero initialization".into());
    }
    inspect_image_profile(
        bytes,
        elf,
        Some(canary),
        rodata_metadata::merge,
        image_limit,
    )
}

/// This profile alone permits the new runtime client's merged string-only
/// readonly output. These ELF linker flags grant neither write nor execute.
pub(super) fn inspect_native_runtime_image(
    bytes: &[u8],
    elf: &parsed_elf64,
) -> Result<Vec<u8>, String> {
    inspect_image_profile(
        bytes,
        elf,
        None,
        rodata_metadata::merge_strings,
        image_limit,
    )
}

/// Explicit five-page profile for the separate native manifest loader.
/// It preserves the private zeroed RW and ordinary CPU contracts.
pub(super) fn inspect_native_manifest_image(
    bytes: &[u8],
    elf: &parsed_elf64,
) -> Result<Vec<u8>, String> {
    inspect_image_profile(
        bytes,
        elf,
        None,
        rodata_metadata::merge_strings,
        manifest_image_limit,
    )
}

#[derive(Clone, Copy)]
enum rodata_metadata {
    plain,
    merge,
    merge_strings,
}

fn inspect_image_profile(
    bytes: &[u8],
    elf: &parsed_elf64,
    canary: Option<u64>,
    metadata: rodata_metadata,
    rx_limit: u64,
) -> Result<Vec<u8>, String> {
    if elf.kind != 2
        || elf.machine != 62
        || elf.entry != image_base
        || elf.program_headers.len() != 2
    {
        return Err("native client requires exact static x86-64 RX/RW loads".into());
    }
    let rx = elf.program_headers[0];
    let rw = elf.program_headers[1];
    for (load, base, flags, limit) in [
        (rx, image_base, 5, rx_limit),
        (rw, data_base, 6, data_limit),
    ] {
        if load.kind != 1
            || load.flags != flags
            || load.virtual_address != base
            || load.physical_address != base
            || load.alignment != 4096
            || load.memory_size == 0
            || load.memory_size > limit
            || load.file_size > load.memory_size
            || !load.offset.is_multiple_of(4096)
            || load
                .offset
                .checked_add(load.file_size)
                .is_none_or(|end| end > bytes.len() as u64)
        {
            return Err("native client load permissions/geometry changed".into());
        }
    }
    if rx.file_size != rx.memory_size
        || rx.offset + rx.file_size > rw.offset
        || rw.memory_size < 128
    {
        return Err("native client load bounds or boot argument storage changed".into());
    }
    let mut allocated = Vec::new();
    for section in &elf.section_headers {
        if matches!(section.kind, 4 | 6 | 9 | 11)
            || section.flags & 0x400 != 0
            || [
                ".got",
                ".plt",
                ".interp",
                ".init",
                ".fini",
                ".preinit",
                ".ctors",
                ".dtors",
                ".eh_frame",
            ]
            .iter()
            .any(|prefix| section.name.starts_with(prefix))
        {
            return Err(
                "native client retains relocations, dynamic linkage, TLS, unwind or constructors"
                    .into(),
            );
        }
        if section.flags & 2 == 0 || section.size == 0 {
            continue;
        }
        let (load, kind, flags) = match section.name.as_str() {
            ".text" => (rx, 1, 6),
            ".rodata" => (rx, 1, 2),
            ".data" => (rw, 1, 3),
            ".bss" => (rw, 8, 3),
            _ => return Err("native client has an undeclared allocated section".into()),
        };
        // Admit exact readonly linker metadata only for explicitly selected
        // profiles. The original native/ELF/Linux accepted flags stay unchanged.
        let allowed_metadata = match metadata {
            rodata_metadata::plain => false,
            rodata_metadata::merge => section.flags == 0x12,
            rodata_metadata::merge_strings => matches!(section.flags, 0x12 | 0x32),
        };
        let flags = if section.name == ".rodata" && allowed_metadata {
            section.flags
        } else {
            flags
        };
        let end = section
            .address
            .checked_add(section.size)
            .ok_or("native section overflow")?;
        if section.kind != kind
            || section.flags != flags
            || section.address < load.virtual_address
            || end > load.virtual_address + load.memory_size
            || (kind != 8
                && (end > load.virtual_address + load.file_size
                    || section.offset != load.offset + section.address - load.virtual_address))
            || (section.name == ".text" && section.address != image_base)
            || (section.name == ".data" && (section.address != data_base || section.size < 128))
            || (section.name == ".bss" && section.address < data_base + 128)
            || allocated.iter().any(|(start, old_end, name)| {
                section.address < *old_end && *start < end || *name == section.name
            })
        {
            return Err("native client allocated sections violate their protection domains".into());
        }
        allocated.push((section.address, end, section.name.as_str()));
    }
    if !allocated.iter().any(|(_, _, name)| *name == ".text")
        || !allocated.iter().any(|(_, _, name)| *name == ".data")
    {
        return Err("native client code or zeroed data section missing".into());
    }
    let initialized = elf_virtual_bytes(bytes, rw, data_base, rw.file_size as usize)?;
    let mut expected = vec![0; initialized.len()];
    if let Some(canary) = canary {
        if !elf
            .section_headers
            .iter()
            .any(|section| section.name == ".data" && section.size >= 136)
        {
            return Err("ELF data canary is outside the initialized data section".into());
        }
        expected
            .get_mut(128..136)
            .ok_or("ELF data canary storage missing")?
            .copy_from_slice(&canary.to_le_bytes());
    }
    if initialized != expected {
        return Err("native client requires unsupported nonzero data initialization".into());
    }
    Ok(elf_virtual_bytes(bytes, rx, image_base, rx.file_size as usize)?.to_vec())
}

pub(super) fn listing(root: &Path, elf: &Path) -> Result<String, String> {
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
        return Err("native client artifact has unresolved or weak imports".into());
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
    .map_err(|_| "native symbols are not UTF-8".into())
}

pub(super) fn inspect_instructions(
    disassembly: &str,
    image: &[u8],
    text_size: u64,
    fault: u64,
) -> Result<(), String> {
    inspect_instruction_profile(
        disassembly,
        image,
        text_size,
        fault,
        None,
        None,
        image_limit,
    )
}

pub(super) fn inspect_native_manifest_instructions(
    disassembly: &str,
    image: &[u8],
    text_size: u64,
    fault: u64,
) -> Result<(), String> {
    inspect_instruction_profile(
        disassembly,
        image,
        text_size,
        fault,
        None,
        None,
        manifest_image_limit,
    )
}

pub(super) fn inspect_linux_instructions(
    disassembly: &str,
    image: &[u8],
    text_size: u64,
    fault: u64,
    syscalls: &[u64],
) -> Result<(), String> {
    if syscalls.len() != 2 || syscalls[0] == syscalls[1] {
        return Err("Linux instruction profile requires two distinct syscall markers".into());
    }
    inspect_instruction_profile(
        disassembly,
        image,
        text_size,
        fault,
        Some(syscalls),
        None,
        image_limit,
    )
}

/// Exact feature-private additions to the ordinary Linux instruction profile.
/// This is a fixture profile, not a general TLS/FP executable admission policy.
pub(super) struct linux_tls_instruction_profile {
    sites: Vec<linux_tls_instruction_site>,
}
struct linux_tls_instruction_site {
    address: u64,
    bytes: Vec<u8>,
    mnemonic: &'static str,
    fs: bool,
}
impl linux_tls_instruction_profile {
    pub(super) fn from_symbols(
        symbols: &str,
        image: &[u8],
        text_size: u64,
        bss_start: u64,
        bss_end: u64,
    ) -> Result<Self, String> {
        let text_end = image_base
            .checked_add(text_size)
            .ok_or("TLS text overflow")?;
        if text_size == 0
            || text_size > image.len() as u64
            || bss_start < 0x800088
            || bss_start.checked_add(8).is_none_or(|end| end > bss_end)
            || bss_end > 0x802000
        {
            return Err("TLS helper data geometry invalid".into());
        }
        let mut spans = Vec::new();
        for (suffix, expected_size) in [("load", 31), ("check", 128), ("canary", 62), ("spin", 78)]
        {
            let name = format!("anomalous_linux_client_tls_{suffix}");
            let matching = symbols
                .lines()
                .filter_map(|line| {
                    let fields = line.split_whitespace().collect::<Vec<_>>();
                    (fields.first().copied() == Some(name.as_str())).then_some(fields)
                })
                .collect::<Vec<_>>();
            if matching.len() != 1 {
                return Err("TLS helper symbol missing or duplicated".into());
            }
            let fields = &matching[0];
            if fields.len() != 4 || !matches!(fields[1], "T" | "t") {
                return Err("TLS helper lacks sized text symbol".into());
            }
            let start =
                u64::from_str_radix(fields[2], 16).map_err(|_| "TLS helper address malformed")?;
            let size =
                u64::from_str_radix(fields[3], 16).map_err(|_| "TLS helper size malformed")?;
            let end = start.checked_add(size).ok_or("TLS helper overflow")?;
            if size != expected_size
                || start < image_base
                || end > text_end
                || spans
                    .iter()
                    .any(|&(other_start, other_end)| start < other_end && other_start < end)
            {
                return Err("TLS helper extent changed or overlaps".into());
            }
            spans.push((start, end));
        }
        let mut profile = Self { sites: Vec::new() };
        // Only these seven extra legacy-FP sites are admitted. RIP-relative
        // loads must address the literal qualified constants beyond .text;
        // stores must target the beginning of the inspected writable BSS.
        for (helper, offset, prefix, value, mnemonic) in [
            (0, 2, &[0xd9, 0x2d][..], &[0x7f, 0x07][..], "fldcw"),
            (
                0,
                8,
                &[0x0f, 0xae, 0x15][..],
                &[0x80, 0x3f, 0, 0][..],
                "ldmxcsr",
            ),
            (
                0,
                15,
                &[0xdd, 0x05][..],
                &[0, 0, 0, 0, 0, 0, 0xf0, 0x3f][..],
                "fldl",
            ),
            (1, 0, &[0xd9, 0x3d][..], &[][..], "fnstcw"),
            (1, 21, &[0x0f, 0xae, 0x1d][..], &[][..], "stmxcsr"),
            (1, 44, &[0xdd, 0x15][..], &[][..], "fstl"),
        ] {
            let address = spans[helper].0 + offset;
            let start = (address - image_base) as usize;
            let length = prefix.len() + 4;
            let bytes = image
                .get(start..start + length)
                .ok_or("TLS FP instruction missing")?;
            if &bytes[..prefix.len()] != prefix {
                return Err("TLS FP instruction encoding changed".into());
            }
            let displacement = i32::from_le_bytes(bytes[prefix.len()..].try_into().unwrap()) as i64;
            let target = (address + length as u64)
                .checked_add_signed(displacement)
                .ok_or("TLS FP target overflow")?;
            if value.is_empty() {
                if target != bss_start {
                    return Err("TLS FP scratch target changed".into());
                }
            } else if target < text_end
                || image.get(
                    (target - image_base) as usize..(target - image_base) as usize + value.len(),
                ) != Some(value)
            {
                return Err("TLS FP constant target changed".into());
            }
            profile.sites.push(linux_tls_instruction_site {
                address,
                bytes: bytes.to_vec(),
                mnemonic,
                fs: false,
            });
        }
        for (helper, offset, bytes, mnemonic, fs) in [
            (0, 0, &[0xdb, 0xe3][..], "fninit", false),
            (
                2,
                0,
                &[0x64, 0x48, 0x83, 0x3c, 0x25, 0, 0, 0, 0, 0][..],
                "cmpq",
                true,
            ),
            (
                2,
                16,
                &[0x64, 0x48, 0x83, 0x3c, 0x25, 8, 0, 0, 0, 0][..],
                "cmpq",
                true,
            ),
            (
                2,
                28,
                &[0x64, 0x48, 0x89, 0x3c, 0x25, 0, 0, 0, 0][..],
                "mov",
                true,
            ),
            (
                2,
                37,
                &[0x64, 0x48, 0xc7, 0x04, 0x25, 8, 0, 0, 0, 1, 0, 0, 0][..],
                "movq",
                true,
            ),
            (
                2,
                50,
                &[0x64, 0x48, 0x39, 0x3c, 0x25, 0, 0, 0, 0][..],
                "cmp",
                true,
            ),
            (
                3,
                31,
                &[0x64, 0x48, 0x39, 0x04, 0x25, 0, 0, 0, 0][..],
                "cmp",
                true,
            ),
            (
                3,
                42,
                &[0x64, 0x48, 0xff, 0x04, 0x25, 8, 0, 0, 0][..],
                "incq",
                true,
            ),
        ] {
            let address = spans[helper].0 + offset;
            let start = (address - image_base) as usize;
            if image.get(start..start + bytes.len()) != Some(bytes) {
                return Err("TLS helper encoding changed".into());
            }
            profile.sites.push(linux_tls_instruction_site {
                address,
                bytes: bytes.to_vec(),
                mnemonic,
                fs,
            });
        }
        Ok(profile)
    }
}

pub(super) fn inspect_linux_tls_instructions(
    disassembly: &str,
    image: &[u8],
    text_size: u64,
    fault: u64,
    syscalls: &[u64],
    profile: &linux_tls_instruction_profile,
) -> Result<(), String> {
    if syscalls.len() != 2 || syscalls[0] == syscalls[1] {
        return Err("TLS Linux profile requires two distinct syscall markers".into());
    }
    inspect_instruction_profile(
        disassembly,
        image,
        text_size,
        fault,
        Some(syscalls),
        Some(profile),
        image_limit,
    )
}

fn inspect_instruction_profile(
    disassembly: &str,
    image: &[u8],
    text_size: u64,
    fault: u64,
    syscalls: Option<&[u64]>,
    linux_tls: Option<&linux_tls_instruction_profile>,
    text_limit: u64,
) -> Result<(), String> {
    // This is a CPU instruction-profile check, not control-flow integrity or an
    // arbitrary hostile executable validator. Rust may use internal jump tables.
    let allowed = "cld mov movabs movb movw movl movq movslq movsbl movsbq movswl movswq movzbl movzbq movzwl
        movzwq lea leaq push pushq pop popq pushf pushfq popf popfq add addb addw addl
        addq adc adcl adcq sub subb subw subl subq sbb sbbl sbbq inc incb incl
        incq dec decb decl decq and andb andw andl andq or orb orl orq xor
        xorb xorl xorq not notb notl notq neg negl negq cmp cmpb cmpw cmpl cmpq
        test testb testw testl testq shl shlb shll shlq shr shrb shrl shrq sar sarl
        sarq shld shrd rol ror rcl rcr bsf bsr bswap bt btl btq bts btr
        btc imul imull imulq mul mull mulq idiv idivl idivq div divl divq cltq cqto
        cltd cwtl cbtw seta setae setb setbe sete setne setg setge setl setle setnp setp
        seto setno sets setns cmova cmovae cmovb cmovbe cmove cmovne cmovg cmovge cmovl cmovle cmovnp
        cmovp cmovo cmovno cmovs cmovns xchg xadd cmpxchg nop nopl nopw pause call callq ret
        retq jmp je jne jb jbe ja jae jg jge jl jle jp jnp js
        jns jo jno jrcxz jecxz loop loope loopne movs movsb movsw movsl movsq stos stosb
        stosw stosl stosq cmps cmpsb cmpsw cmpsl cmpsq scas scasb scasw scasl scasq movsd movss
        movaps movups movapd movupd movdqa movdqu movd movlps movhps movlhps movhlps movlpd movhpd movmskps movmskpd
        addsd addss addpd addps subsd subss subpd subps mulsd mulss mulpd mulps divsd divss divpd
        divps ucomisd ucomiss comisd comiss cvtsi2sd cvtsi2ss cvttsd2si cvttss2si cvtsd2ss cvtss2sd xorpd xorps andpd andps
        orpd orps andnpd andnps shufps shufpd unpcklps unpckhps unpcklpd unpckhpd pxor pand pandn por pshufd
        pshuflw pshufhw psllw pslld psllq pslldq psrlw psrld psrlq psrldq psraw psrad paddb paddw paddd
        paddq psubb psubw psubd psubq pcmpeqb pcmpeqw pcmpeqd pcmpgtb pcmpgtw pcmpgtd punpcklbw punpcklwd punpckldq punpcklqdq
        punpckhbw punpckhwd punpckhdq punpckhqdq packsswb packssdw packuswb pmovmskb pmullw pmulhw pmulhuw pmuludq pmaddwd pminub pmaxub
        pminsw pmaxsw pavgb pavgw psadbw pextrw pinsrw int ud2 syscall";
    let end = image_base
        .checked_add(text_size)
        .ok_or("native text extent overflow")?;
    if text_size == 0 || text_size > image.len() as u64 || text_size > text_limit {
        return Err("native text bounds invalid".into());
    }
    let mut cursor = image_base;
    let mut gate_count = 0;
    let mut found_fault = false;
    let mut tls_sites_seen = 0;
    for line in disassembly.lines() {
        let Some((address, body)) = line.split_once(':') else {
            continue;
        };
        let address = address.trim();
        if address.is_empty() || !address.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            continue;
        }
        let address =
            u64::from_str_radix(address, 16).map_err(|_| "native instruction address malformed")?;
        let mut columns = body.split('\t').filter(|value| !value.trim().is_empty());
        let raw = columns
            .next()
            .ok_or("native disassembler omitted bytes")?
            .split_whitespace()
            .map(|byte| {
                if byte.len() == 2 {
                    u8::from_str_radix(byte, 16).map_err(|_| "native instruction byte malformed")
                } else {
                    Err("native instruction byte malformed")
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        let instruction = columns
            .next()
            .ok_or("native disassembler omitted instruction")?;
        let mut words = instruction.split_whitespace();
        let mut mnemonic = words.next().ok_or("native mnemonic missing")?;
        let mut prefixes = Vec::new();
        while matches!(
            mnemonic,
            "data16" | "cs" | "rep" | "repz" | "repnz" | "lock"
        ) {
            prefixes.push(mnemonic);
            mnemonic = words.next().ok_or("native dangling prefix")?;
        }
        let operands = words.collect::<Vec<_>>().join(" ");
        let instruction_end = address
            .checked_add(raw.len() as u64)
            .ok_or("native instruction overflow")?;
        let tls_site = linux_tls.is_some_and(|profile| {
            profile.sites.iter().any(|site| {
                site.address == address
                    && site.bytes == raw
                    && site.mnemonic == mnemonic
                    && site.fs == operands.contains("%fs:")
                    && prefixes.is_empty()
            })
        });
        if tls_site {
            tls_sites_seen += 1;
        }
        if address != cursor
            || raw.is_empty()
            || raw.len() > 15
            || instruction_end > end
            || image.get((address - image_base) as usize..(instruction_end - image_base) as usize)
                != Some(raw.as_slice())
            || (!tls_site
                && !allowed
                    .split_ascii_whitespace()
                    .any(|value| value == mnemonic))
            || [
                "%gs", "%ymm", "%zmm", "%k", "%bnd", "%tmm", "%cr", "%dr", "%mm",
            ]
            .iter()
            .any(|register| operands.contains(register))
            || (operands.contains("%fs") && !tls_site)
            || prefixes.iter().any(|prefix| match *prefix {
                "data16" | "cs" => !matches!(mnemonic, "nop" | "nopl" | "nopw"),
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
            })
        {
            return Err(format!(
                "native instruction at 0x{address:x} outside baseline profile: {mnemonic}"
            ));
        }
        if mnemonic == "int" {
            if syscalls.is_some() || operands != "$0xf4" || raw != [0xcd, 0xf4] {
                return Err("native client contains an unsupported software gate".into());
            }
            gate_count += 1;
        }
        if mnemonic == "syscall" {
            if !syscalls.is_some_and(|markers| markers.contains(&address))
                || raw != [0x0f, 0x05]
                || !operands.is_empty()
            {
                return Err("Linux syscall is outside its exact instruction markers".into());
            }
            gate_count += 1;
        }
        if address == fault {
            if mnemonic != "ud2" || raw != [0x0f, 0x0b] {
                return Err("native fault symbol does not identify UD2".into());
            }
            found_fault = true;
        }
        cursor = instruction_end;
    }
    if cursor != end
        || gate_count == 0
        || !found_fault
        || syscalls.is_some_and(|markers| gate_count != markers.len())
        || linux_tls.is_some_and(|profile| tls_sites_seen != profile.sites.len())
    {
        return Err("native disassembly coverage or required gate/fault missing".into());
    }
    Ok(())
}

pub(super) fn section_bytes<'a>(
    bytes: &'a [u8],
    section: &elf64_section_header,
) -> Result<&'a [u8], String> {
    let start = usize::try_from(section.offset).map_err(|_| "native section offset overflow")?;
    let end = start
        .checked_add(usize::try_from(section.size).map_err(|_| "native section size overflow")?)
        .ok_or("native section extent overflow")?;
    bytes
        .get(start..end)
        .ok_or_else(|| "native section backing truncated".into())
}

fn check_symbols(symbols: &str, artifact: &artifact, start: u64) -> Result<(), String> {
    let end = start
        .checked_add(artifact.image.len() as u64)
        .ok_or("native payload symbol overflow")?;
    for (name, value, kind) in [
        ("x86_64_native_application_payload_start", start, 'R'),
        ("x86_64_native_application_payload_end", end, 'R'),
        (fault_alias, artifact.fault, 'A'),
    ] {
        if symbol_u64_from_listing(symbols, name)? != value
            || symbol_kind_from_listing(symbols, name)? != kind
        {
            return Err("native payload symbols differ from inspected source".into());
        }
    }
    Ok(())
}

fn inspect_wrapper(root: &Path, artifact: &artifact) -> Result<(), String> {
    let bytes = fs::read(&artifact.object).map_err(|error| error.to_string())?;
    let elf = parse_elf64(&bytes)?;
    let allocated = elf
        .section_headers
        .iter()
        .filter(|section| section.flags & 2 != 0 && section.size != 0)
        .collect::<Vec<_>>();
    let notes = elf
        .section_headers
        .iter()
        .filter(|section| section.name == ".note.GNU-stack")
        .collect::<Vec<_>>();
    if elf.kind != 1
        || elf.machine != 62
        || elf.entry != 0
        || !elf.program_headers.is_empty()
        || elf
            .section_headers
            .iter()
            .any(|section| matches!(section.kind, 4 | 9))
        || allocated.len() != 1
        || allocated[0].name != ".rodata.native_application"
        || allocated[0].kind != 1
        || allocated[0].flags != 2
        || allocated[0].address != 0
        || section_bytes(&bytes, allocated[0])? != artifact.image
        || notes.len() != 1
        || notes[0].kind != 1
        || notes[0].flags != 0
        || notes[0].size != 0
    {
        return Err(
            "native wrapper has changed bytes, relocations, permissions or stack declaration"
                .into(),
        );
    }
    let symbols = listing(root, &artifact.object)?;
    check_symbols(&symbols, artifact, 0)?;
    if symbols.lines().count() != 3 {
        return Err("native wrapper symbol set is not exact".into());
    }
    Ok(())
}

pub(super) fn inspect_linked(
    root: &Path,
    kernel: &Path,
    artifact: &artifact,
) -> Result<(), String> {
    let bytes = fs::read(kernel).map_err(|error| error.to_string())?;
    let elf = parse_elf64(&bytes)?;
    let symbols = listing(root, kernel)?;
    inspect_embedding(&bytes, &elf, &symbols, artifact)
}

fn inspect_embedding(
    bytes: &[u8],
    elf: &parsed_elf64,
    symbols: &str,
    artifact: &artifact,
) -> Result<(), String> {
    let start = symbol_u64_from_listing(symbols, "x86_64_native_application_payload_start")?;
    check_symbols(symbols, artifact, start)?;
    let end = start
        .checked_add(artifact.image.len() as u64)
        .ok_or("native payload overflow")?;
    let sections = elf
        .section_headers
        .iter()
        .filter(|section| {
            section.flags & 2 != 0
                && section.address < end
                && section
                    .address
                    .checked_add(section.size)
                    .is_none_or(|limit| start < limit)
        })
        .collect::<Vec<_>>();
    let loads = elf
        .program_headers
        .iter()
        .copied()
        .filter(|load| {
            load.kind == 1
                && load.virtual_address < end
                && load
                    .virtual_address
                    .checked_add(load.memory_size)
                    .is_none_or(|limit| start < limit)
        })
        .collect::<Vec<_>>();
    if start < symbol_u64_from_listing(symbols, "kernel_rodata_start")?
        || end > symbol_u64_from_listing(symbols, "kernel_rodata_end")?
        || sections.len() != 1
        || sections[0].kind != 1
        || sections[0].flags != 2
        || start < sections[0].address
        || sections[0]
            .address
            .checked_add(sections[0].size)
            .is_none_or(|limit| end > limit)
        || loads.len() != 1
        || loads[0].flags != 5
        || elf_virtual_bytes(bytes, loads[0], start, artifact.image.len())? != artifact.image
        || count_subslice(bytes, &artifact.image) != 1
    {
        return Err("native linked payload changed or lacks unique readonly backing".into());
    }
    Ok(())
}

pub(super) fn verify_absent(symbols: &str, bytes: &[u8]) -> Result<(), String> {
    for marker in ["native_application", "anomalous_native_client"] {
        if symbols.contains(marker)
            || bytes
                .windows(marker.len())
                .any(|window| window == marker.as_bytes())
        {
            return Err("normal image retains native application fixture".into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image_fixture() -> (Vec<u8>, parsed_elf64) {
        let bytes = vec![0; 0x2080];
        let program_headers = [
            (image_base, 5, 0x1000, 64, 64),
            (data_base, 6, 0x2000, 128, 512),
        ]
        .map(
            |(base, flags, offset, file_size, memory_size)| elf64_program_header {
                kind: 1,
                flags,
                offset,
                virtual_address: base,
                physical_address: base,
                file_size,
                memory_size,
                alignment: 4096,
            },
        )
        .to_vec();
        let section_headers = [
            (".text", 1, 6, image_base, 0x1000, 32),
            (".rodata", 1, 2, image_base + 32, 0x1020, 32),
            (".data", 1, 3, data_base, 0x2000, 128),
            (".bss", 8, 3, data_base + 128, 0x2080, 384),
        ]
        .map(
            |(name, kind, flags, address, offset, size)| elf64_section_header {
                name: name.into(),
                kind,
                flags,
                address,
                offset,
                size,
            },
        )
        .to_vec();
        (
            bytes,
            parsed_elf64 {
                kind: 2,
                machine: 62,
                entry: image_base,
                program_headers,
                section_headers,
            },
        )
    }

    #[test]
    fn native_loads_reject_foreign_personalities_permissions_and_extent_changes() {
        let (bytes, elf) = image_fixture();
        assert_eq!(inspect_image(&bytes, &elf).unwrap(), vec![0; 64]);
        for (index, flags) in [(0, 7), (0, 4), (1, 7), (1, 4)] {
            let mut changed = elf.clone();
            changed.program_headers[index].flags = flags;
            assert!(inspect_image(&bytes, &changed).is_err());
        }
        let mut changed = elf.clone();
        changed.kind = 3;
        assert!(inspect_image(&bytes, &changed).is_err());
        let mut changed = elf.clone();
        changed.machine = 3;
        assert!(inspect_image(&bytes, &changed).is_err());
        let mut changed = elf.clone();
        changed.entry += 1;
        assert!(inspect_image(&bytes, &changed).is_err());
        for (index, size) in [(0, image_limit + 1), (1, data_limit + 1), (1, 127)] {
            let mut changed = elf.clone();
            changed.program_headers[index].memory_size = size;
            assert!(inspect_image(&bytes, &changed).is_err());
        }
        let mut changed = elf.clone();
        changed.program_headers[1].offset = u64::MAX;
        assert!(inspect_image(&bytes, &changed).is_err());
        let mut changed = elf.clone();
        changed.program_headers[0].virtual_address += 4096;
        assert!(inspect_image(&bytes, &changed).is_err());
        let mut changed = elf.clone();
        changed.program_headers.push(changed.program_headers[0]);
        assert!(inspect_image(&bytes, &changed).is_err());
        assert!(inspect_image(&bytes[..bytes.len() - 1], &elf).is_err());
    }

    #[test]
    fn native_sections_require_zero_bootargs_and_exclude_tls_relocations_and_overlap() {
        let (bytes, elf) = image_fixture();
        for name in [
            ".got",
            ".plt",
            ".init_array",
            ".fini_array",
            ".eh_frame",
            ".unexpected",
        ] {
            let mut changed = elf.clone();
            changed.section_headers[1].name = name.into();
            assert!(inspect_image(&bytes, &changed).is_err());
        }
        for kind in [4, 6, 9, 11] {
            let mut changed = elf.clone();
            changed.section_headers[1].kind = kind;
            assert!(inspect_image(&bytes, &changed).is_err());
        }
        let mut changed = elf.clone();
        changed.section_headers[1].flags |= 0x400;
        assert!(inspect_image(&bytes, &changed).is_err());
        let mut changed = elf.clone();
        changed.section_headers[1].address -= 1;
        assert!(inspect_image(&bytes, &changed).is_err());
        let mut changed = elf.clone();
        changed.section_headers[3].address -= 1;
        assert!(inspect_image(&bytes, &changed).is_err());
        let mut changed = elf.clone();
        changed.section_headers[2].size = 127;
        assert!(inspect_image(&bytes, &changed).is_err());
        let mut changed = elf.clone();
        changed.section_headers[1].offset += 1;
        assert!(inspect_image(&bytes, &changed).is_err());
        for offset in [0x2000, 0x207f] {
            let mut changed = bytes.clone();
            changed[offset] = 1;
            assert!(inspect_image(&changed, &elf).is_err());
        }
    }

    #[test]
    fn elf_canary_profile_does_not_relax_native_zero_data() {
        let (mut bytes, mut elf) = image_fixture();
        bytes.resize(0x2088, 0);
        elf.program_headers[1].file_size = 136;
        elf.section_headers[2].size = 136;
        elf.section_headers[3].address += 8;
        elf.section_headers[3].offset += 8;
        elf.section_headers[3].size -= 8;
        let canary = 0x454c_465f_4441_5441_u64;
        bytes[0x2080..0x2088].copy_from_slice(&canary.to_le_bytes());
        inspect_image_with_canary(&bytes, &elf, canary).unwrap();
        assert!(inspect_image(&bytes, &elf).is_err());
        assert!(inspect_image_with_canary(&bytes, &elf, 0).is_err());
        for offset in [0x2000, 0x207f, 0x2080, 0x2087] {
            let mut changed = bytes.clone();
            changed[offset] ^= 1;
            assert!(inspect_image_with_canary(&changed, &elf, canary).is_err());
        }
        let mut changed = elf.clone();
        changed.section_headers[2].size = 128;
        assert!(inspect_image_with_canary(&bytes, &changed, canary).is_err());
        let (zero, original) = image_fixture();
        inspect_image(&zero, &original).unwrap();
        assert!(inspect_image_with_canary(&zero, &original, canary).is_err());
        let mut merged = elf.clone();
        merged.section_headers[1].flags = 0x12;
        inspect_image_with_canary(&bytes, &merged, canary).unwrap();
        for flags in [0x13, 0x16, 0x32, 0x200012] {
            merged.section_headers[1].flags = flags;
            assert!(inspect_image_with_canary(&bytes, &merged, canary).is_err());
        }
        let mut merged = original.clone();
        merged.section_headers[1].flags = 0x12;
        inspect_elf_loader_image(&zero, &merged).unwrap();
        assert!(inspect_image(&zero, &merged).is_err());
    }

    #[test]
    fn runtime_string_metadata_is_explicit_and_does_not_relax_legacy_profiles() {
        let (bytes, mut elf) = image_fixture();
        for flags in [2, 0x12, 0x32] {
            elf.section_headers[1].flags = flags;
            inspect_native_runtime_image(&bytes, &elf).unwrap();
        }
        assert!(inspect_image(&bytes, &elf).is_err());
        assert!(inspect_elf_loader_image(&bytes, &elf).is_err());
        // Metadata applies only to rodata, never any other protection domain.
        elf.section_headers[1].flags = 2;
        for index in [0, 2, 3] {
            let mut changed = elf.clone();
            changed.section_headers[index].flags = 0x32;
            assert!(inspect_native_runtime_image(&bytes, &changed).is_err());
        }
        for flags in [0x22, 0x33, 0x36, 0x432, 0x200032] {
            elf.section_headers[1].flags = flags;
            assert!(inspect_native_runtime_image(&bytes, &elf).is_err());
        }
    }

    #[test]
    fn manifest_extent_is_named_and_preserves_private_rw_rules() {
        let (mut bytes, mut elf) = image_fixture();
        // Independent five-page witness; generic/native runtime profiles
        // retain their existing eight-page limit.
        let rx_bytes = 20480;
        let rw_offset = 0x1000 + rx_bytes;
        bytes.resize(rw_offset as usize + 128, 0);
        elf.program_headers[0].file_size = rx_bytes;
        elf.program_headers[0].memory_size = rx_bytes;
        elf.program_headers[1].offset = rw_offset;
        elf.section_headers[1].size = rx_bytes - 32;
        elf.section_headers[2].offset = rw_offset;
        elf.section_headers[3].offset = rw_offset + 128;
        assert_eq!(
            inspect_native_manifest_image(&bytes, &elf).unwrap().len(),
            20480
        );
        assert!(inspect_image(&bytes, &elf).is_ok());
        assert!(inspect_elf_loader_image(&bytes, &elf).is_ok());
        assert!(inspect_native_runtime_image(&bytes, &elf).is_ok());
        for (index, size) in [(0, 20481), (1, 8193)] {
            let mut changed = elf.clone();
            changed.program_headers[index].memory_size = size;
            assert!(inspect_native_manifest_image(&bytes, &changed).is_err());
        }
        for index in [0, 1] {
            let mut changed = elf.clone();
            changed.program_headers[index].flags = 7;
            assert!(inspect_native_manifest_image(&bytes, &changed).is_err());
        }
        let mut legacy = elf.clone();
        let mut legacy_bytes = bytes.clone();
        legacy_bytes.resize(0x9080, 0);
        legacy.program_headers[0].file_size = 32768;
        legacy.program_headers[0].memory_size = 32768;
        legacy.program_headers[1].offset = 0x9000;
        legacy.section_headers[1].size = 32768 - 32;
        legacy.section_headers[2].offset = 0x9000;
        legacy.section_headers[3].offset = 0x9080;
        assert!(inspect_image(&legacy_bytes, &legacy).is_ok());
        assert!(inspect_native_runtime_image(&legacy_bytes, &legacy).is_ok());
        assert!(inspect_native_manifest_image(&legacy_bytes, &legacy).is_err());
        bytes[rw_offset as usize] = 1;
        assert!(inspect_native_manifest_image(&bytes, &elf).is_err());
    }

    fn instructions() -> (Vec<u8>, String) {
        let parts: &[(&[u8], &str)] = &[
            (&[0x31, 0xc0], "xor %eax,%eax"),
            (&[0xf3, 0x48, 0xab], "rep stos %rax,%es:(%rdi)"),
            (&[0xf3, 0x0f, 0x6f, 0x07], "movdqu (%rdi),%xmm0"),
            (&[0xcd, 0xf4], "int $0xf4"),
            (&[0x0f, 0x0b], "ud2"),
            (&[0xc3], "ret"),
        ];
        let mut bytes = Vec::new();
        let mut listing = String::new();
        for (raw, text) in parts {
            let hex = raw
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<Vec<_>>()
                .join(" ");
            listing.push_str(&format!(
                "{:x}:\t{hex}\t{text}\n",
                image_base + bytes.len() as u64
            ));
            bytes.extend_from_slice(raw);
        }
        (bytes, listing)
    }

    #[test]
    fn native_instruction_profile_rejects_tls_avx_system_and_wrong_gates() {
        let (bytes, listing) = instructions();
        let fault = image_base + 11;
        inspect_instructions(&listing, &bytes, bytes.len() as u64, fault).unwrap();
        for (old, new) in [
            ("movdqu", "vmovdqu"),
            ("movdqu", "xrstor"),
            ("movdqu", "pshufb"),
            ("ret", "syscall"),
            ("ret", "rdmsr"),
            ("ret", "cli"),
            ("%rdi),%xmm0", "%fs:%rdi),%xmm0"),
            ("%xmm0", "%ymm0"),
            ("int $0xf4", "int $0x80"),
            ("rep stos", "lock stos"),
            ("40000b:", "40000c:"),
            ("0f 0b", "0f 0c"),
        ] {
            assert!(
                inspect_instructions(
                    &listing.replace(old, new),
                    &bytes,
                    bytes.len() as u64,
                    fault
                )
                .is_err(),
                "accepted {new}"
            );
        }
        assert!(inspect_instructions(&listing, &bytes, bytes.len() as u64, fault + 1).is_err());
        assert!(
            inspect_instructions(
                &listing.lines().skip(1).collect::<Vec<_>>().join("\n"),
                &bytes,
                bytes.len() as u64,
                fault
            )
            .is_err()
        );
        assert!(inspect_instructions(&listing, &bytes, u64::MAX, fault).is_err());
    }

    #[test]
    fn manifest_text_extent_does_not_widen_legacy_cpu_or_instruction_profiles() {
        let (mut bytes, mut listing) = instructions();
        while bytes.len() < 20480 {
            listing.push_str(&format!("{:x}:\t90\tnop\n", 0x400000 + bytes.len()));
            bytes.push(0x90);
        }
        assert!(inspect_native_manifest_instructions(&listing, &bytes, 20480, 0x40000b).is_ok());
        assert!(inspect_instructions(&listing, &bytes, 20480, 0x40000b).is_ok());
        for (old, new) in [
            ("movdqu", "vmovdqu"),
            ("ret", "syscall"),
            ("int $0xf4", "int $0x80"),
        ] {
            assert!(
                inspect_native_manifest_instructions(
                    &listing.replace(old, new),
                    &bytes,
                    20480,
                    0x40000b
                )
                .is_err()
            );
        }
        listing.push_str("405000:\t90\tnop\n");
        bytes.push(0x90);
        assert!(inspect_native_manifest_instructions(&listing, &bytes, 20481, 0x40000b).is_err());
        assert!(inspect_instructions(&listing, &bytes, 20481, 0x40000b).is_ok());
    }

    #[test]
    fn linux_instruction_profile_requires_only_two_exact_syscall_markers() {
        let bytes = [0x31, 0xc0, 0x0f, 0x05, 0x90, 0x0f, 0x05, 0x0f, 0x0b, 0xc3];
        let listing = "400000:\t31 c0\txor %eax,%eax\n400002:\t0f 05\tsyscall\n400004:\t90\tnop\n400005:\t0f 05\tsyscall\n400007:\t0f 0b\tud2\n400009:\tc3\tret\n";
        let markers = [image_base + 2, image_base + 5];
        let fault = image_base + 7;
        inspect_linux_instructions(listing, &bytes, bytes.len() as u64, fault, &markers).unwrap();
        assert!(inspect_instructions(listing, &bytes, bytes.len() as u64, fault).is_err());
        for markers in [
            vec![],
            vec![image_base + 2],
            vec![image_base + 2; 2],
            vec![image_base + 2, image_base + 6],
        ] {
            assert!(
                inspect_linux_instructions(listing, &bytes, bytes.len() as u64, fault, &markers)
                    .is_err()
            );
        }
        let mut changed = bytes;
        changed[2..4].copy_from_slice(&[0xcd, 0xf4]);
        let changed_listing = listing.replacen("0f 05\tsyscall", "cd f4\tint $0xf4", 1);
        assert!(
            inspect_linux_instructions(
                &changed_listing,
                &changed,
                bytes.len() as u64,
                fault,
                &markers
            )
            .is_err()
        );
        assert!(
            inspect_linux_instructions(
                &listing.replace("syscall", "sysret"),
                &bytes,
                bytes.len() as u64,
                fault,
                &markers
            )
            .is_err()
        );
    }

    fn linux_tls_fixture() -> (Vec<u8>, String, String, u64, [u64; 2]) {
        let mut bytes = vec![0x90; 416];
        let text_size = 400;
        let symbols="anomalous_linux_client_tls_load T 400010 1f\nanomalous_linux_client_tls_check t 400040 80\nanomalous_linux_client_tls_canary T 4000d0 3e\nanomalous_linux_client_tls_spin T 400120 4e\n".to_owned();
        bytes[400..402].copy_from_slice(&[0x7f, 7]);
        bytes[402..406].copy_from_slice(&[0x80, 0x3f, 0, 0]);
        bytes[406..414].copy_from_slice(&[0, 0, 0, 0, 0, 0, 0xf0, 0x3f]);
        let mut instructions: Vec<(usize, Vec<u8>, String)> = vec![
            (0, vec![0x0f, 0x05], "syscall".into()),
            (2, vec![0x0f, 0x05], "syscall".into()),
            (4, vec![0x0f, 0x0b], "ud2".into()),
            (16, vec![0xdb, 0xe3], "fninit".into()),
            (
                208,
                vec![0x64, 0x48, 0x83, 0x3c, 0x25, 0, 0, 0, 0, 0],
                "cmpq $0x0,%fs:0x0".into(),
            ),
            (
                224,
                vec![0x64, 0x48, 0x83, 0x3c, 0x25, 8, 0, 0, 0, 0],
                "cmpq $0x0,%fs:0x8".into(),
            ),
            (
                236,
                vec![0x64, 0x48, 0x89, 0x3c, 0x25, 0, 0, 0, 0],
                "mov %rdi,%fs:0x0".into(),
            ),
            (
                245,
                vec![0x64, 0x48, 0xc7, 0x04, 0x25, 8, 0, 0, 0, 1, 0, 0, 0],
                "movq $0x1,%fs:0x8".into(),
            ),
            (
                258,
                vec![0x64, 0x48, 0x39, 0x3c, 0x25, 0, 0, 0, 0],
                "cmp %rdi,%fs:0x0".into(),
            ),
            (
                319,
                vec![0x64, 0x48, 0x39, 0x04, 0x25, 0, 0, 0, 0],
                "cmp %rax,%fs:0x0".into(),
            ),
            (
                330,
                vec![0x64, 0x48, 0xff, 0x04, 0x25, 8, 0, 0, 0],
                "incq %fs:0x8".into(),
            ),
        ];
        for (offset, mut raw, target, mnemonic) in [
            (18, vec![0xd9, 0x2d], image_base + 400, "fldcw"),
            (24, vec![0x0f, 0xae, 0x15], image_base + 402, "ldmxcsr"),
            (31, vec![0xdd, 0x05], image_base + 406, "fldl"),
            (64, vec![0xd9, 0x3d], 0x800090, "fnstcw"),
            (85, vec![0x0f, 0xae, 0x1d], 0x800090, "stmxcsr"),
            (108, vec![0xdd, 0x15], 0x800090, "fstl"),
        ] {
            let displacement =
                (target as i64 - (image_base + offset as u64 + raw.len() as u64 + 4) as i64) as i32;
            raw.extend(displacement.to_le_bytes());
            instructions.push((offset, raw, format!("{mnemonic} 0x{displacement:x}(%rip)")));
        }
        instructions.sort_by_key(|entry| entry.0);
        let mut listing = String::new();
        let mut cursor = 0;
        for (offset, raw, instruction) in instructions {
            while cursor < offset {
                listing.push_str(&format!("{:x}:\t90\tnop\n", image_base + cursor as u64));
                cursor += 1;
            }
            bytes[offset..offset + raw.len()].copy_from_slice(&raw);
            let hex = raw
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<Vec<_>>()
                .join(" ");
            listing.push_str(&format!(
                "{:x}:\t{hex}\t{instruction}\n",
                image_base + offset as u64
            ));
            cursor = offset + raw.len();
        }
        while cursor < text_size {
            listing.push_str(&format!("{:x}:\t90\tnop\n", image_base + cursor as u64));
            cursor += 1;
        }
        (
            bytes,
            listing,
            symbols,
            text_size as u64,
            [image_base, image_base + 2],
        )
    }

    #[test]
    fn tls_only_instruction_extension_requires_exact_helpers_and_keeps_legacy_bans() {
        let (bytes, listing, symbols, text_size, syscalls) = linux_tls_fixture();
        let profile = linux_tls_instruction_profile::from_symbols(
            &symbols, &bytes, text_size, 0x800090, 0x802000,
        )
        .unwrap();
        assert_eq!(profile.sites.len(), 14);
        inspect_linux_tls_instructions(
            &listing,
            &bytes,
            text_size,
            image_base + 4,
            &syscalls,
            &profile,
        )
        .unwrap();
        assert!(
            inspect_linux_instructions(&listing, &bytes, text_size, image_base + 4, &syscalls)
                .is_err()
        );
        assert!(inspect_instructions(&listing, &bytes, text_size, image_base + 4).is_err());
        for (old, new) in [
            ("fninit", "finit"),
            ("fldcw", "fldenv"),
            ("%fs:", "%gs:"),
            ("incq %fs:0x8", "wrfsbase %rax"),
            ("fninit", "vzeroall"),
            ("fninit", "rdmsr"),
            ("fninit", "cli"),
            ("fninit", "syscall"),
        ] {
            assert!(
                inspect_linux_tls_instructions(
                    &listing.replace(old, new),
                    &bytes,
                    text_size,
                    image_base + 4,
                    &syscalls,
                    &profile
                )
                .is_err(),
                "accepted {new}"
            );
        }
        assert!(
            inspect_linux_tls_instructions(
                &listing,
                &bytes,
                text_size,
                image_base + 4,
                &[image_base, image_base + 3],
                &profile
            )
            .is_err()
        );
        // Valid encodings at another address do not gain authority from opcode alone.
        let mut changed = bytes.clone();
        changed[6..8].copy_from_slice(&[0xdb, 0xe3]);
        let moved = listing.replace(
            "400006:\t90\tnop\n400007:\t90\tnop\n",
            "400006:\tdb e3\tfninit\n",
        );
        assert!(
            inspect_linux_tls_instructions(
                &moved,
                &changed,
                text_size,
                image_base + 4,
                &syscalls,
                &profile
            )
            .is_err()
        );
    }

    #[test]
    fn tls_instruction_metadata_encodings_targets_and_spans_are_fail_closed() {
        let (bytes, _, symbols, text_size, _) = linux_tls_fixture();
        for changed in [
            symbols.replace("400010 1f", "400010 20"),
            symbols.replace("400040 80", "400020 80"),
            symbols.replace("400120 4e", "fffffffffffffff0 4e"),
            symbols.replace(" T ", " R "),
            format!("{symbols}{symbols}"),
        ] {
            assert!(
                linux_tls_instruction_profile::from_symbols(
                    &changed, &bytes, text_size, 0x800090, 0x802000
                )
                .is_err()
            );
        }
        for offset in [
            16, 18, 24, 31, 64, 85, 108, 208, 224, 236, 245, 258, 319, 330, 400, 402, 406,
        ] {
            let mut changed = bytes.clone();
            changed[offset] ^= 1;
            assert!(
                linux_tls_instruction_profile::from_symbols(
                    &symbols, &changed, text_size, 0x800090, 0x802000
                )
                .is_err(),
                "accepted changed byte {offset}"
            );
        }
        // Operand displacement/width and selector changes cannot reuse a trusted site.
        for (offset, value) in [(20, 0), (66, 0), (208, 0x65), (229, 16)] {
            let mut changed = bytes.clone();
            changed[offset] = value;
            assert_ne!(changed, bytes);
            assert!(
                linux_tls_instruction_profile::from_symbols(
                    &symbols, &changed, text_size, 0x800090, 0x802000
                )
                .is_err()
            );
        }
        for (start, end) in [(0x800090, 0x800097), (0, 0x802000), (0x800090, u64::MAX)] {
            assert!(
                linux_tls_instruction_profile::from_symbols(
                    &symbols, &bytes, text_size, start, end
                )
                .is_err()
            );
        }
        assert!(
            linux_tls_instruction_profile::from_symbols(
                &symbols,
                &bytes,
                u64::MAX,
                0x800090,
                0x802000
            )
            .is_err()
        );
    }

    fn embedding() -> (Vec<u8>, parsed_elf64, String, artifact) {
        let image = vec![0x31, 0xc0, 0xcd, 0xf4, 0x0f, 0x0b, 0xc3];
        let mut bytes = vec![0; 256];
        bytes.extend_from_slice(&image);
        let start = 0x100100;
        let end = start + image.len() as u64;
        let elf = parsed_elf64 {
            kind: 2,
            machine: 62,
            entry: 0x100000,
            program_headers: vec![elf64_program_header {
                kind: 1,
                flags: 5,
                offset: 0,
                virtual_address: 0x100000,
                physical_address: 0x100000,
                file_size: bytes.len() as u64,
                memory_size: bytes.len() as u64,
                alignment: 4096,
            }],
            section_headers: vec![elf64_section_header {
                name: ".rodata".into(),
                kind: 1,
                flags: 2,
                address: start,
                offset: 256,
                size: image.len() as u64,
            }],
        };
        let fault = image_base + 4;
        let symbols = format!(
            "x86_64_native_application_payload_start R {start:x}\nx86_64_native_application_payload_end R {end:x}\n{fault_alias} A {fault:x}\nkernel_rodata_start R {start:x}\nkernel_rodata_end R {end:x}\n"
        );
        (
            bytes,
            elf,
            symbols,
            artifact {
                object: PathBuf::new(),
                image,
                fault,
            },
        )
    }

    #[test]
    fn native_embedding_rejects_partial_aliases_writable_backing_and_changed_bytes() {
        let (bytes, elf, symbols, artifact) = embedding();
        inspect_embedding(&bytes, &elf, &symbols, &artifact).unwrap();
        let mut changed = elf.clone();
        let mut alias = changed.program_headers[0];
        alias.virtual_address = 0x100103;
        alias.memory_size = 1;
        alias.file_size = 1;
        alias.flags = 6;
        changed.program_headers.push(alias);
        assert!(inspect_embedding(&bytes, &changed, &symbols, &artifact).is_err());
        let mut changed = elf.clone();
        let mut alias = changed.section_headers[0].clone();
        alias.address += 3;
        alias.size = 1;
        changed.section_headers.push(alias);
        assert!(inspect_embedding(&bytes, &changed, &symbols, &artifact).is_err());
        let mut changed = elf.clone();
        changed.section_headers[0].flags = 3;
        assert!(inspect_embedding(&bytes, &changed, &symbols, &artifact).is_err());
        let mut changed = bytes.clone();
        changed[256] ^= 1;
        assert!(inspect_embedding(&changed, &elf, &symbols, &artifact).is_err());
        let mut duplicated = bytes.clone();
        duplicated.extend_from_slice(&artifact.image);
        assert!(inspect_embedding(&duplicated, &elf, &symbols, &artifact).is_err());
        assert!(
            inspect_embedding(
                &bytes,
                &elf,
                &symbols.replace(
                    "kernel_rodata_start R 100100",
                    "kernel_rodata_start R 100101"
                ),
                &artifact
            )
            .is_err()
        );
        assert!(
            check_symbols("", &artifact, u64::MAX)
                .unwrap_err()
                .contains("overflow")
        );
        assert!(
            check_symbols(
                &symbols.replace(&format!("{fault_alias} A"), &format!("{fault_alias} R")),
                &artifact,
                0x100100
            )
            .is_err()
        );
    }

    #[test]
    fn normal_image_cannot_retain_native_client_markers() {
        verify_absent("kernel_entry T 100000", b"normal kernel").unwrap();
        assert!(verify_absent("x86_64_native_application_payload_start R 100100", b"").is_err());
        assert!(verify_absent("", b"anomalous_native_client_fault_instruction").is_err());
    }
}
