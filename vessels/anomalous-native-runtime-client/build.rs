use std::{env, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=native-runtime-client.ld");
    let script =
        PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("native-runtime-client.ld");
    println!(
        "cargo:rustc-link-arg-bin=anomalous-native-runtime-client=-T{}",
        script.display()
    );
    println!("cargo:rustc-link-arg-bin=anomalous-native-runtime-client=--build-id=none");
    println!("cargo:rustc-link-arg-bin=anomalous-native-runtime-client=-z");
    println!("cargo:rustc-link-arg-bin=anomalous-native-runtime-client=max-page-size=0x1000");
}
