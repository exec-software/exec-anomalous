//! Append opt-in TLS caller, loader and userspace veil after the exact legacy
//! native/Linux/veil artifact prefix. TLS policy never enters the kernel image.
use super::*;
use anomalous_elf_adapter::{encoded_plan_size, load_plan, parse};
pub(super) use elf_application_artifact::artifact;
use elf_application_artifact::{
    blob, build_vessel, compact_elf, inspect_linux_tls_vessel, inspect_vessel, retain_vessel,
    write_wrapper,
};

pub(super) fn package_image(root: &Path, build: &Path) -> Result<artifact, String> {
    let baseline = veil_application_artifact::package_image(root, build)?;
    let prefix = (0..12)
        .map(|index| {
            fs::read(
                build
                    .join("veil-application")
                    .join(format!("blob_{index}.bin")),
            )
            .map_err(|e| e.to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    let output = build.join("linux-tls");
    fs::create_dir_all(&output).map_err(|e| format!("create Linux artifact directory: {e}"))?;
    let output = fs::canonicalize(output).map_err(|e| e.to_string())?;
    for name in [
        "qualification.txt",
        "client.elf",
        "loader.elf",
        "deployment.elf",
        "veil.elf",
        "payload.o",
    ] {
        remove_optional_file(&output.join(name))?;
    }
    build_vessel(root, "anomalous-linux-client", Some("tls"), None)?;
    let client_path = retain_vessel(root, &output, "anomalous-linux-client", "client.elf")?;
    let client = inspect_linux_tls_vessel(root, &client_path)?;
    let symbols = native_application_artifact::listing(root, &client_path)?;
    let deployment = compact_elf(&client)?;
    let plan = parse(&deployment).map_err(|e| format!("Linux ELF manifest: {e:?}"))?;
    let encoded = plan.encode();
    if load_plan::decode(&encoded).map_err(|e| format!("Linux manifest roundtrip: {e:?}"))? != plan
        || encoded.len() != encoded_plan_size
    {
        return Err("Linux manifest is not canonical".into());
    }
    let deployment_path = output.join("deployment.elf");
    fs::write(&deployment_path, &deployment).map_err(|e| e.to_string())?;
    build_vessel(root, "anomalous-elf-loader", None, Some(&deployment_path))?;
    let loader_path = retain_vessel(root, &output, "anomalous-elf-loader", "loader.elf")?;
    let loader = inspect_vessel(root, &loader_path, "anomalous_elf_loader", false)?;
    if count_subslice(&loader.rx, &deployment) != 1 {
        return Err("Linux loader must contain exactly one complete Linux deployment".into());
    }
    build_vessel(root, "anomalous-linux-veil", Some("tls"), None)?;
    let veil_path = retain_vessel(root, &output, "anomalous-linux-tls-veil", "veil.elf")?;
    let veil = inspect_vessel(root, &veil_path, "anomalous_linux_veil", true)?;
    let mut aliases = vec![
        ("x86_64_linux_tls_veil_user_fault_instruction", veil.fault),
        (
            "x86_64_linux_tls_application_user_fault_instruction",
            client.fault,
        ),
        (
            "x86_64_linux_tls_application_loader_fault_instruction",
            loader.fault,
        ),
    ];
    for (wrapper, source) in [
        (
            "x86_64_linux_tls_application_syscall_instruction",
            "anomalous_linux_client_syscall_instruction",
        ),
        (
            "x86_64_linux_tls_application_syscall_return",
            "anomalous_linux_client_syscall_return",
        ),
        (
            "x86_64_linux_tls_application_probe_syscall_instruction",
            "anomalous_linux_client_probe_syscall_instruction",
        ),
        (
            "x86_64_linux_tls_application_probe_syscall_return",
            "anomalous_linux_client_probe_syscall_return",
        ),
    ] {
        if symbol_kind_from_listing(&symbols, source)? != 'T' {
            return Err("Linux syscall marker is not executable text".into());
        }
        aliases.push((wrapper, symbol_u64_from_listing(&symbols, source)?));
    }
    let mut receipt = format!(
        "profile=supervised_linux_tls_eager_cpu_v1\nentry=0x400000\nclient_rx_bytes={}\nclient_rw_file_bytes={}\nclient_rw_memory_bytes={}\nelf_bytes={}\nmanifest_bytes={encoded_plan_size}\nloader_rx_bytes={}\nloader_rw_memory_bytes={}\n",
        client.rx.len(),
        client.rw.len(),
        client.headers[1].memory_size,
        deployment.len(),
        loader.rx.len(),
        loader.headers[1].memory_size,
    );
    for (name, value) in &aliases {
        receipt.push_str(&format!("{name}=0x{value:x}\n"));
    }
    let artifact = baseline.extend(
        output.join("payload.o"),
        vec![
            blob {
                name: "x86_64_linux_tls_application_payload",
                bytes: client.rx,
            },
            blob {
                name: "x86_64_linux_tls_application_data",
                bytes: client.rw,
            },
            blob {
                name: "x86_64_linux_tls_application_elf",
                bytes: deployment,
            },
            blob {
                name: "x86_64_linux_tls_application_plan",
                bytes: encoded.to_vec(),
            },
            blob {
                name: "x86_64_linux_tls_application_loader",
                bytes: loader.rx,
            },
            blob {
                name: "x86_64_linux_tls_veil_payload",
                bytes: veil.rx,
            },
            blob {
                name: "x86_64_linux_tls_veil_data",
                bytes: veil.rw,
            },
        ],
        aliases,
    );
    write_wrapper(root, &output, &artifact)?;
    for (index, expected) in prefix.iter().enumerate() {
        let actual =
            fs::read(output.join(format!("blob_{index}.bin"))).map_err(|e| e.to_string())?;
        verify_preserved_blob(expected, &actual)?;
    }
    receipt.push_str("retained_prefix_blobs=12\n");
    for name in [
        "client.elf",
        "loader.elf",
        "deployment.elf",
        "veil.elf",
        "payload.o",
    ]
    .into_iter()
    .map(str::to_owned)
    .chain((0..19).map(|index| format!("blob_{index}.bin")))
    {
        let digest = run_output_checked(root, "sha256sum", [output.join(&name).into_os_string()])?;
        let digest = String::from_utf8(digest).map_err(|_| "Linux artifact hash is not UTF-8")?;
        let value = digest
            .split_whitespace()
            .next()
            .ok_or("Linux artifact hash missing")?;
        if value.len() != 64 || !value.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err("Linux artifact hash malformed".into());
        }
        receipt.push_str(&format!("{name}_sha256={value}\n"));
    }
    fs::write(output.join("qualification.txt"), receipt).map_err(|e| e.to_string())?;
    Ok(artifact)
}

pub(super) fn inspect_linked(
    root: &Path,
    kernel: &Path,
    artifact: &artifact,
) -> Result<(), String> {
    // The combined inspector binds every named blob and absolute marker, all
    // nested copies, the one readonly wrapper, and the unchanged native prefix.
    veil_application_artifact::inspect_linked(root, kernel, artifact)
}

fn verify_preserved_blob(expected: &[u8], actual: &[u8]) -> Result<(), String> {
    if expected.is_empty() || expected != actual {
        return Err("TLS packaging changed or omitted a qualified baseline blob".into());
    }
    Ok(())
}

pub(super) fn verify_absent(symbols: &str, bytes: &[u8]) -> Result<(), String> {
    for marker in [
        "linux_tls",
        "anomalous_linux_tls",
        "caller_fs_stage",
        "caller_fs_query",
    ] {
        if symbols.contains(marker) || bytes.windows(marker.len()).any(|w| w == marker.as_bytes()) {
            return Err("normal image retains Linux TLS fixture".into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tls_packaging_preserves_every_legacy_prefix_byte() {
        let prefix = b"native/Linux/veil\0legacy";
        verify_preserved_blob(prefix, prefix).unwrap();
        for index in 0..prefix.len() {
            let mut changed = prefix.to_vec();
            changed[index] ^= 1;
            assert!(verify_preserved_blob(prefix, &changed).is_err());
        }
        assert!(verify_preserved_blob(prefix, &prefix[..prefix.len() - 1]).is_err());
        assert!(verify_preserved_blob(&[], &[]).is_err());
    }
    #[test]
    fn normal_images_exclude_tls_payload_and_policy_markers() {
        verify_absent("kernel_entry T 100000", b"normal").unwrap();
        for marker in [
            "linux_tls",
            "anomalous_linux_tls",
            "caller_fs_stage",
            "caller_fs_query",
        ] {
            assert!(verify_absent(marker, b"").is_err());
            assert!(verify_absent("", marker.as_bytes()).is_err());
        }
    }
}
