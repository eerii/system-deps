pub fn main() {
    #[cfg(feature = "build_meta")]
    meta();

    #[cfg(feature = "binary")]
    binary();
}

#[cfg(feature = "build_meta")]
/// Makes metadata variables accessible to `system-deps`
/// This allows to set env variable configurations via Cargo.toml
pub fn meta() {
    let metadata = system_deps_meta::metadata();
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
            // TODO: Set env! instead
            //println!("cargo:{}_{}={}", name, key, value);
        }
    }
}

#[cfg(feature = "binary")]
pub fn binary() {
    // Add pkg-config paths to the overrides
    // TODO: This should probably follow some deterministic ordering to avoid issues
    let paths = system_deps_binary::build();
    if !paths.is_empty() {
        let path = std::env::join_paths(&paths)
            .expect("The binary directories contain invalid characters")
            .into_string()
            .unwrap();
        println!("cargo:rustc-env=BINARY_PKG_CONFIG_PATH={}", path);
    }
}
