use std::{env, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=native-relaunch-client.ld");
    let script =
        PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("native-relaunch-client.ld");
    println!(
        "cargo:rustc-link-arg-bin=anomalous-native-relaunch-client=-T{}",
        script.display()
    );
    println!("cargo:rustc-link-arg-bin=anomalous-native-relaunch-client=--build-id=none");
    println!("cargo:rustc-link-arg-bin=anomalous-native-relaunch-client=-z");
    println!("cargo:rustc-link-arg-bin=anomalous-native-relaunch-client=max-page-size=0x1000");
}
