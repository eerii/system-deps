pub fn main() {
    // Convert env variables into metadata
    let vars = ["BINARY_CONFIG", "BINARY_DIR", "BINARY_URL"];
    for var in vars {
        println!("cargo:rerun-if-env-changed={}", var);
        if let Ok(value) = std::env::var(format!("SYSTEM_DEPS_{}", var)) {
            println!("cargo:{}={}", var, value);
        }
    }
}
