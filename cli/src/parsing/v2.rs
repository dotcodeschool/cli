use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct JsonTestV2 {
    name: String,
    slug: String,
    optional: bool,
    cmd: String,
    message_on_fail: String,
    message_on_success: String,
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct JsonTestSuiteV2 {
    name: String,
    slug: String,
    optional: bool,
    tests: Vec<JsonTestV2>,
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct JsonPositionV2 {
    x: u32,
    y: u32,
}

#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(tag = "type")]
pub enum JsonContentV2 {
    Markdown {
        file: String,
        position: JsonPositionV2,
    },
    #[default]
    Invalid,
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct JsonLessonV2 {
    name: String,
    slug: String,
    description: String,
    duration: u32,
    content: Vec<JsonContentV2>,
    suites: Option<JsonTestSuiteV2>,
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct JsonStageV2 {
    name: String,
    description: String,
    lessons: Vec<JsonLessonV2>,
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct JsonAuthorV2 {
    name: String,
    url: String,
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct JsonRequisiteV2 {
    name: String,
    url: String,
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub enum JsonLevelV2 {
    Beginner,
    #[default]
    Invalid,
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub enum JsonLanguageV2 {
    Rust,
    Go,
    #[default]
    Invalid,
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct JsonCourseV2 {
    version: String,
    name: String,
    title: String,
    slug: String,
    description: String,
    tagline: String,
    author: JsonAuthorV2,
    requisites: Vec<JsonRequisiteV2>,
    outcomes: Vec<String>,
    level: JsonLevelV2,
    languages: Vec<JsonLanguageV2>,
    tags: Vec<String>,
    stages: Vec<JsonStageV2>,
}
