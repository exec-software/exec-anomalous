use std::{env, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=linux-client.ld");
    let script = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("linux-client.ld");
    println!(
        "cargo:rustc-link-arg-bin=anomalous-linux-client=-T{}",
        script.display()
    );
    println!("cargo:rustc-link-arg-bin=anomalous-linux-client=--build-id=none");
    println!("cargo:rustc-link-arg-bin=anomalous-linux-client=-z");
    println!("cargo:rustc-link-arg-bin=anomalous-linux-client=max-page-size=0x1000");
}
