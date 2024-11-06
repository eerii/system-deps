pub fn main() {
    #[cfg(feature = "binary")]
    {
        // Add pkg-config paths to the overrides
        // TODO: This should probably follow some deterministic ordering to avoid issues
        // TODO: Dependency specific paths
        let paths = system_deps_binary::build();
        if !paths.is_empty() {
            let path = std::env::join_paths(&paths)
                .expect("The binary directories contain invalid characters")
                .into_string()
                .unwrap();
            println!("cargo:rustc-env=BINARY_PKG_CONFIG_PATH={}", path);
        }
    }
}
