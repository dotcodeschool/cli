use serde::{Deserialize, Deserializer, Serialize};

use crate::{db::TestState, parsing::TestResult};

use super::{JsonCourse, JsonTest};

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct JsonTestV2 {
    pub name: String,
    pub slug: String,
    pub optional: bool,
    pub cmd: String,
    pub message_on_fail: String,
    pub message_on_success: String,
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct JsonTestSuiteV2 {
    pub name: String,
    pub slug: String,
    pub optional: bool,
    #[serde(deserialize_with = "no_empty_vec")]
    pub tests: Vec<JsonTestV2>,
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct JsonPositionV2 {
    pub x: u32,
    pub y: u32,
}

#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(tag = "type")]
pub enum JsonContentV2 {
    #[serde(rename = "markdown")]
    Markdown { file: String, position: JsonPositionV2 },
    #[default]
    #[serde(skip)]
    Invalid,
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct JsonLessonV2 {
    pub name: String,
    pub slug: String,
    pub description: String,
    pub duration: u32,
    #[serde(deserialize_with = "no_empty_vec")]
    pub content: Vec<JsonContentV2>,
    pub suites: Option<Vec<JsonTestSuiteV2>>,
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct JsonStageV2 {
    pub name: String,
    pub slug: String,
    pub description: String,
    #[serde(deserialize_with = "no_empty_vec")]
    pub lessons: Vec<JsonLessonV2>,
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct JsonAuthorV2 {
    pub name: String,
    pub url: String,
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct JsonRequisiteV2 {
    pub name: String,
    pub url: String,
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub enum JsonLevelV2 {
    #[serde(rename = "beginner")]
    Beginner,
    #[default]
    #[serde(skip)]
    Invalid,
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub enum JsonLanguageV2 {
    #[serde(rename = "rust")]
    Rust,
    #[serde(rename = "go")]
    Go,
    #[default]
    #[serde(skip)]
    Invalid,
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct JsonCourseV2 {
    pub version: String,
    pub name: String,
    pub title: String,
    pub slug: String,
    pub description: String,
    pub tagline: String,
    pub author: JsonAuthorV2,
    pub requisites: Vec<JsonRequisiteV2>,
    #[serde(deserialize_with = "no_empty_vec")]
    pub outcomes: Vec<String>,
    pub level: JsonLevelV2,
    #[serde(deserialize_with = "no_empty_vec")]
    pub languages: Vec<JsonLanguageV2>,
    #[serde(deserialize_with = "no_empty_vec")]
    pub tags: Vec<String>,
    #[serde(deserialize_with = "no_empty_vec")]
    pub stages: Vec<JsonStageV2>,
}

fn no_empty_vec<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    use serde::de::Error;
    let v: Vec<T> = Deserialize::deserialize(deserializer)?;
    if v.is_empty() {
        Err(Error::custom("empty arrays are not allowed"))
    } else {
        Ok(v)
    }
}

impl JsonTest for JsonTestV2 {
    fn run(&self) -> super::TestResult {
        log::debug!("Running test: '{}", self.cmd);

        let command: Vec<&str> = self.cmd.split_whitespace().collect();
        let output = std::process::Command::new(command[0])
            .args(command[1..].iter())
            .output();
        let output = match output {
            Ok(output) => output,
            Err(_) => {
                return TestResult::Fail("could not execute test".to_string());
            }
        };

        log::debug!("Test executed successfully!");

        match output.status.success() {
            true => TestResult::Pass(String::from_utf8(output.stdout).unwrap()),
            false => {
                TestResult::Fail(String::from_utf8(output.stderr).unwrap())
            }
        }
    }
}

impl<'a> JsonCourse<'a> for JsonCourseV2 {
    fn name(&'a self) -> &'a str {
        &self.name
    }

    fn author(&'a self) -> &'a str {
        &self.author.name
    }

    fn list_tests(&self) -> Vec<crate::db::TestState> {
        let Self { stages, name, .. } = self;

        stages.iter().fold(vec![], |acc, stage| {
            stage.lessons.iter().fold(acc, |acc, lesson| match &lesson.suites {
                Some(suites) => suites.iter().fold(acc, |acc, suite| {
                    suite.tests.iter().fold(acc, |mut acc, test| {
                        acc.push(TestState {
                            path: vec![
                                name.clone(),
                                stage.name.clone(),
                                lesson.name.clone(),
                                suite.name.clone(),
                                test.name.clone(),
                            ],
                            passed: false,
                        });
                        acc
                    })
                }),
                None => acc,
            })
        })
    }
}
