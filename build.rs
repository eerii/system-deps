pub fn main() {
    #[cfg(feature = "build_meta")]
    let (_v, _d) = meta();

    #[cfg(feature = "binary")]
    binary(_v, _d);
}

#[cfg(feature = "build_meta")]
/// Makes metadata variables accessible to `system-deps`
/// This allows to set env variable configurations via Cargo.toml
pub fn meta() -> (system_deps_meta::Values, std::path::PathBuf) {
    use system_deps_meta::*;
    let root = root();
    let metadata = metadata(&root);
    println!("cargo:rustc-env=BUILD_MANIFEST={}", root.display());

    let binary_dir = std::env::var("SYSTEM_DEPS_BINARY_DIR")
        .map(|dir| std::path::PathBuf::from(dir))
        .unwrap_or(metadata.target_directory.clone().into());
    println!("cargo:rustc-env=BUILD_BINARY_DIR={}", binary_dir.display());

    let values = read(&metadata, "system-deps");
    export_metadata(&values);
    (values, binary_dir)
}

#[cfg(feature = "binary")]
pub fn binary(values: system_deps_meta::Values, dir: std::path::PathBuf) {
    // Add pkg-config paths to the overrides
    // TODO: This should probably follow some deterministic ordering to avoid issues
    let paths = system_deps_binary::build(values, dir);
    if !paths.is_empty() {
        let path = std::env::join_paths(&paths)
            .expect("The binary directories contain invalid characters")
            .into_string()
            .unwrap();
        println!("cargo:rustc-env=BINARY_PKG_CONFIG_PATH={}", path);
    }
}
