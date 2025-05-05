use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct RedisTestResultV1 {
    slug: String,
    output: String,
    pub state: RedisTestState,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum RedisTestState {
    Passed,
    Failed { optional: bool },
}

impl RedisTestResultV1 {
    pub fn pass(slug: &str, output: &str) -> Self {
        Self {
            slug: slug.to_string(),
            output: output.to_string(),
            state: RedisTestState::Passed,
        }
    }

    pub fn fail(slug: &str, output: &str, optional: bool) -> Self {
        Self {
            slug: slug.to_string(),
            output: output.to_string(),
            state: RedisTestState::Failed { optional },
        }
    }
}
