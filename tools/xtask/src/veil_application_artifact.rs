//! Package an independent native veil beside the exact qualified Linux/native
//! artifact prefix. ELF framing does not grant caller or channel authority.
use super::*;
pub(super) use elf_application_artifact::artifact;
use elf_application_artifact::{blob, build_vessel, inspect_vessel, retain_vessel, write_wrapper};

pub(super) fn package_image(root: &Path, build: &Path) -> Result<artifact, String> {
    let baseline = linux_application_artifact::package_image(root, build)?;
    let baseline_output = build.join("linux-application");
    let prefix = (0..10)
        .map(|index| {
            fs::read(baseline_output.join(format!("blob_{index}.bin")))
                .map_err(|e| format!("read Linux baseline blob: {e}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let output = build.join("veil-application");
    fs::create_dir_all(&output).map_err(|e| format!("create veil artifact directory: {e}"))?;
    let output = fs::canonicalize(output).map_err(|e| e.to_string())?;
    for name in ["qualification.txt", "veil.elf", "payload.o"] {
        remove_optional_file(&output.join(name))?;
    }
    build_vessel(root, "anomalous-linux-veil", None, None)?;
    let path = retain_vessel(root, &output, "anomalous-linux-veil", "veil.elf")?;
    let veil = inspect_vessel(root, &path, "anomalous_linux_veil", true)?;
    let mut receipt = format!(
        "profile=supervised_linux_veil_native_comparison_v1\nentry=0x400000\nveil_rx_bytes={}\nveil_rw_file_bytes={}\nveil_rw_memory_bytes={}\nveil_fault=0x{:x}\nretained_prefix_blobs=10\n",
        veil.rx.len(),
        veil.rw.len(),
        veil.headers[1].memory_size,
        veil.fault,
    );
    let artifact = baseline.extend(
        output.join("payload.o"),
        vec![
            blob {
                name: "x86_64_veil_application_payload",
                bytes: veil.rx,
            },
            blob {
                name: "x86_64_veil_application_data",
                bytes: veil.rw,
            },
        ],
        vec![("x86_64_veil_application_user_fault_instruction", veil.fault)],
    );
    write_wrapper(root, &output, &artifact)?;
    for (index, expected) in prefix.iter().enumerate() {
        let actual = fs::read(output.join(format!("blob_{index}.bin")))
            .map_err(|e| format!("read retained wrapper blob: {e}"))?;
        verify_preserved_blob(expected, &actual)?;
    }
    for name in core::iter::once(String::from("veil.elf"))
        .chain((0..12).map(|index| format!("blob_{index}.bin")))
        .chain(core::iter::once(String::from("payload.o")))
    {
        let digest = run_output_checked(root, "sha256sum", [output.join(&name).into_os_string()])?;
        let digest = String::from_utf8(digest).map_err(|_| "veil artifact hash is not UTF-8")?;
        let value = digest
            .split_whitespace()
            .next()
            .ok_or("veil artifact hash missing")?;
        if value.len() != 64 || !value.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err("veil artifact hash malformed".into());
        }
        receipt.push_str(&format!("{name}_sha256={value}\n"));
    }
    fs::write(output.join("qualification.txt"), receipt).map_err(|e| e.to_string())?;
    Ok(artifact)
}

fn verify_preserved_blob(expected: &[u8], actual: &[u8]) -> Result<(), String> {
    if expected.is_empty() || expected != actual {
        return Err("veil packaging changed or omitted a qualified baseline blob".into());
    }
    Ok(())
}

pub(super) fn inspect_linked(
    root: &Path,
    kernel: &Path,
    artifact: &artifact,
) -> Result<(), String> {
    // Reuse exact aliases/extents/readonly checks and complete nested-copy
    // accounting; the additional native veil does not relax the Linux prefix.
    elf_application_artifact::inspect_linked(root, kernel, artifact)?;
    verify_kernel_policy_absent(&native_application_artifact::listing(root, kernel)?)
}

fn verify_kernel_policy_absent(symbols: &str) -> Result<(), String> {
    // Inspect kernel symbols, not embedded vessel bytes: the independent veil
    // intentionally contains the foreign policy in its own immutable payload.
    for marker in ["linux_calls", "linux_io", "linux_policy"] {
        if symbols.contains(marker) {
            return Err("veil image retains the old kernel Linux policy adapter".into());
        }
    }
    Ok(())
}

pub(super) fn verify_absent(symbols: &str, bytes: &[u8]) -> Result<(), String> {
    for marker in [
        "veil_application",
        "shared_veil",
        "anomalous_linux_veil",
        "anomalous_veil_protocol",
    ] {
        if symbols.contains(marker) || bytes.windows(marker.len()).any(|w| w == marker.as_bytes()) {
            return Err("normal image retains userspace veil fixture".into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn veil_packaging_rejects_changed_truncated_extended_or_empty_baseline_bytes() {
        let baseline = b"qualified\0client\xffservice";
        verify_preserved_blob(baseline, baseline).unwrap();
        for index in 0..baseline.len() {
            let mut changed = baseline.to_vec();
            changed[index] ^= 1;
            assert!(verify_preserved_blob(baseline, &changed).is_err());
        }
        assert!(verify_preserved_blob(baseline, &baseline[..baseline.len() - 1]).is_err());
        let mut extended = baseline.to_vec();
        extended.push(0);
        assert!(verify_preserved_blob(baseline, &extended).is_err());
        assert!(verify_preserved_blob(&[], &[]).is_err());
    }

    #[test]
    fn normal_images_reject_veil_payload_policy_and_protocol_markers() {
        verify_absent("kernel_entry T 100000", b"normal").unwrap();
        for marker in [
            "x86_64_veil_application_payload_start",
            "anomalous_linux_veil_entry",
            "anomalous_veil_protocol",
        ] {
            assert!(verify_absent(marker, b"").is_err());
            assert!(verify_absent("", marker.as_bytes()).is_err());
        }
    }

    #[test]
    fn veil_kernel_symbols_exclude_the_previous_foreign_policy_adapter() {
        verify_kernel_policy_absent("native_application_runtime T 100000\nveil_profile T 110000")
            .unwrap();
        for marker in ["linux_calls", "linux_io", "linux_policy"] {
            assert!(verify_kernel_policy_absent(&format!("kernel::{marker} T 100000")).is_err());
        }
    }
}
