use std::env;

pub fn get(var: &str) -> Option<String> {
    env::var(var).ok()
}
