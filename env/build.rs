fn main() {
    // Ensure that all of the binaries from different crates are sourced from the same folder
    let out_dir = std::env::var("OUT_DIR").unwrap();
    println!("cargo:rustc-env=SYSTEM_DEPS_BINARY_DIR={}/binary", out_dir);
}
