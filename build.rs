pub fn main() {
    #[cfg(feature = "build_meta")]
    let (_v, _t) = meta();

    #[cfg(feature = "binary")]
    binary(_v, _t);
}

#[cfg(feature = "build_meta")]
/// Makes metadata variables accessible to `system-deps`
/// This allows to set env variable configurations via Cargo.toml
pub fn meta() -> (system_deps_meta::Values, std::path::PathBuf) {
    let root = system_deps_meta::root();
    let metadata = system_deps_meta::metadata(&root);
    let target = match std::env::var("CARGO_TARGET_DIRECTORY") {
        Ok(t) => root.join(t),
        Err(_) => metadata.target_directory.clone().into(),
    };

    println!("cargo:rustc-env=BUILD_MANIFEST={}", root.display());
    println!("cargo:rustc-env=BUILD_TARGET={}", target.display());

    let values = system_deps_meta::read(&metadata, "system-deps");
    for (name, map) in &values {
        let Some(map) = map.as_object() else {
            continue;
        };
        for (key, value) in map {
            if key == "name" || key == "version" {
                continue;
            }
            // TODO: Support cfg and versions
            let Some(value) = value.as_str() else {
                continue;
            };
            println!(
                "cargo:rustc-env={}_{}={}",
                name.to_uppercase(),
                key.to_uppercase(),
                value
            );
        }
    }

    (values, target)
}

#[cfg(feature = "binary")]
pub fn binary(values: system_deps_meta::Values, target: std::path::PathBuf) {
    // Add pkg-config paths to the overrides
    // TODO: This should probably follow some deterministic ordering to avoid issues
    let paths = system_deps_binary::build(values, target);
    if !paths.is_empty() {
        let path = std::env::join_paths(&paths)
            .expect("The binary directories contain invalid characters")
            .into_string()
            .unwrap();
        println!("cargo:rustc-env=BINARY_PKG_CONFIG_PATH={}", path);
    }
}
