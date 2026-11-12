use std::{env, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=native-manifest-loader.ld");
    let script =
        PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("native-manifest-loader.ld");
    println!(
        "cargo:rustc-link-arg-bin=anomalous-native-manifest-loader=-T{}",
        script.display()
    );
    // The freestanding target invokes rust-lld directly, without a cc driver.
    println!("cargo:rustc-link-arg-bin=anomalous-native-manifest-loader=--build-id=none");
    println!("cargo:rustc-link-arg-bin=anomalous-native-manifest-loader=-z");
    println!("cargo:rustc-link-arg-bin=anomalous-native-manifest-loader=max-page-size=0x1000");
}
