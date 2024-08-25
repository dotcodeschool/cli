use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct RedisTestResultV1 {
    output: String,
    passed: bool,
}

impl RedisTestResultV1 {
    pub fn pass(output: &str) -> Self {
        Self { output: output.to_string(), passed: true }
    }

    pub fn fail(output: &str) -> Self {
        Self { output: output.to_string(), passed: false }
    }
}
