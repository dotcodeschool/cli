use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct RedisCourseResultV1 {
    tests: Vec<RedisTestResultV1>,
    passed: bool,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct RedisTestResultV1 {
    slug: String,
    output: String,
    state: RedisTestState,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum RedisTestState {
    Passed,
    Failed { optional: bool },
}

impl RedisCourseResultV1 {
    pub fn new(test_count: usize) -> RedisCourseResultV1 {
        Self { tests: Vec::with_capacity(test_count), passed: false }
    }

    pub fn log_test(&mut self, test: RedisTestResultV1) {
        self.tests.push(test);
    }

    pub fn pass(&mut self) {
        self.passed = true;
    }
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
