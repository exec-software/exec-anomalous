use std::{env, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=linux-veil.ld");
    let script = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("linux-veil.ld");
    println!(
        "cargo:rustc-link-arg-bin=anomalous-linux-veil=-T{}",
        script.display()
    );
    println!("cargo:rustc-link-arg-bin=anomalous-linux-veil=--build-id=none");
    println!("cargo:rustc-link-arg-bin=anomalous-linux-veil=-z");
    println!("cargo:rustc-link-arg-bin=anomalous-linux-veil=max-page-size=0x1000");
    println!(
        "cargo:rustc-link-arg-bin=anomalous-linux-tls-veil=-T{}",
        script.display()
    );
    println!("cargo:rustc-link-arg-bin=anomalous-linux-tls-veil=--build-id=none");
    println!("cargo:rustc-link-arg-bin=anomalous-linux-tls-veil=-z");
    println!("cargo:rustc-link-arg-bin=anomalous-linux-tls-veil=max-page-size=0x1000");
    println!(
        "cargo:rustc-link-arg-bin=anomalous-linux-loader-entry-veil=-T{}",
        script.display()
    );
    println!("cargo:rustc-link-arg-bin=anomalous-linux-loader-entry-veil=--build-id=none");
    println!("cargo:rustc-link-arg-bin=anomalous-linux-loader-entry-veil=-z");
    println!("cargo:rustc-link-arg-bin=anomalous-linux-loader-entry-veil=max-page-size=0x1000");
}
