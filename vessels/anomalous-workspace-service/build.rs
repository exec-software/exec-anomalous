use std::{env, fs, path::PathBuf};
fn main() {
    println!("cargo:rerun-if-changed=workspace.ld");
    println!("cargo:rerun-if-changed=owned-workspace.ld");
    println!("cargo:rerun-if-changed=src/entry.S");
    println!("cargo:rerun-if-changed=src/cpu_probe.S");
    println!("cargo:rerun-if-changed=owned-workspace-cpu.ld");
    println!("cargo:rerun-if-changed=native-remote.ld");
    println!("cargo:rerun-if-changed=src/native_remote_entry.S");
    let manifest = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let owned = env::var_os("CARGO_FEATURE_OWNED_ADMISSION").is_some();
    let cpu = env::var_os("CARGO_FEATURE_OWNED_CPU_PROBE").is_some();
    let remote = env::var_os("CARGO_FEATURE_NATIVE_REMOTE").is_some();
    assert!(!cpu || owned);
    assert!(!remote || (owned && !cpu));
    let script = manifest.join(if remote {
        "native-remote.ld"
    } else if cpu {
        "owned-workspace-cpu.ld"
    } else if owned {
        "owned-workspace.ld"
    } else {
        "workspace.ld"
    });
    let mailbox = if owned {
        "0x0000000000800000"
    } else {
        "0x0000600000080000"
    };
    // Generate only the selected address. The legacy assembly and Rust source
    // locations stay unchanged; opt-in admission does not perturb its payload.
    let output = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let mut entry = fs::read_to_string(manifest.join(if remote {
        "src/native_remote_entry.S"
    } else {
        "src/entry.S"
    }))
    .unwrap();
    if cpu {
        // Generate only the opt-in probe path; feature-off assembly is byte for
        // byte the original source expansion. All FP lives in the named range.
        for original in [
            "    cld\n",
            "    pushq %r15\n    xorq %rax",
            "    int $workspace_gate\n    popq %r15",
        ] {
            assert_eq!(
                entry.matches(original).count(),
                1,
                "CPU probe insertion moved"
            );
        }
        entry = entry.replacen("    cld\n", "    cld\n    pushq %rdi\n    callq anomalous_workspace_cpu_initialize\n    popq %rdi\n", 1);
        entry = entry.replacen(
            "    pushq %r15\n    xorq %rax",
            "    pushq %r15\n    callq anomalous_workspace_cpu_check_mmx\n    xorq %rax",
            1,
        );
        entry = entry.replacen(
            "    int $workspace_gate\n    popq %r15",
            "    int $workspace_gate\n    callq anomalous_workspace_cpu_check_mmx\n    popq %r15",
            1,
        );
        entry.push_str(&fs::read_to_string(manifest.join("src/cpu_probe.S")).unwrap());
    }
    assert_eq!(entry.matches("@workspace_mailbox@").count(), 1);
    let entry_path = output.join("workspace-entry.S");
    fs::write(
        &entry_path,
        entry
            .replace("@workspace_mailbox@", mailbox)
            .replace("@owned_admission@", if owned { "1" } else { "0" }),
    )
    .unwrap();
    fs::write(output.join("mailbox.rs"), mailbox).unwrap();
    println!("cargo:rustc-env=WORKSPACE_ENTRY={}", entry_path.display());
    println!(
        "cargo:rustc-link-arg-bin=anomalous-workspace-service=-T{}",
        script.display()
    );
    println!("cargo:rustc-link-arg-bin=anomalous-workspace-service=--build-id=none");
    println!("cargo:rustc-link-arg-bin=anomalous-workspace-service=-z");
    println!("cargo:rustc-link-arg-bin=anomalous-workspace-service=max-page-size=0x1000");
}
