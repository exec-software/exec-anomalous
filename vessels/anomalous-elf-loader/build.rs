use std::{env, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=elf-loader.ld");
    println!("cargo:rerun-if-env-changed=ANOMALOUS_ELF_DEPLOYMENT");
    let deployment = PathBuf::from(
        env::var_os("ANOMALOUS_ELF_DEPLOYMENT")
            .expect("ANOMALOUS_ELF_DEPLOYMENT must name the inspected deployment ELF"),
    )
    .canonicalize()
    .expect("deployment ELF must exist");
    assert!(deployment.is_file(), "deployment ELF must be a file");
    println!("cargo:rerun-if-changed={}", deployment.display());
    println!(
        "cargo:rustc-env=ANOMALOUS_ELF_DEPLOYMENT={}",
        deployment.display()
    );
    let script = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("elf-loader.ld");
    println!(
        "cargo:rustc-link-arg-bin=anomalous-elf-loader=-T{}",
        script.display()
    );
    println!("cargo:rustc-link-arg-bin=anomalous-elf-loader=--build-id=none");
    println!("cargo:rustc-link-arg-bin=anomalous-elf-loader=-z");
    println!("cargo:rustc-link-arg-bin=anomalous-elf-loader=max-page-size=0x1000");
}
