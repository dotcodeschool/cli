use indexmap::IndexMap;
use parity_scale_codec::Encode;
use serde::{Deserialize, Deserializer, Serialize};

use crate::{
    db::{PathLink, TestState, ValidationState},
    parsing::TestResult,
};

use super::{JsonCourse, JsonTest};

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct JsonTestV1 {
    pub name: String,
    pub slug: String,
    pub optional: bool,
    pub cmd: String,
    pub message_on_fail: String,
    pub message_on_success: String,
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct JsonTestSuiteV1 {
    pub name: String,
    pub slug: String,
    pub optional: bool,
    #[serde(deserialize_with = "no_empty_vec")]
    pub tests: Vec<JsonTestV1>,
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct JsonPositionV1 {
    pub x: u32,
    pub y: u32,
}

#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(tag = "type")]
pub enum JsonContentV1 {
    #[serde(rename = "markdown")]
    Markdown { file: String, position: JsonPositionV1 },
    #[default]
    #[serde(skip)]
    Invalid,
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct JsonLessonV1 {
    pub name: String,
    pub slug: String,
    pub description: String,
    pub duration: u32,
    #[serde(deserialize_with = "no_empty_vec")]
    pub content: Vec<JsonContentV1>,
    pub suites: Option<Vec<JsonTestSuiteV1>>,
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct JsonStageV1 {
    pub name: String,
    pub slug: String,
    pub description: String,
    #[serde(deserialize_with = "no_empty_vec")]
    pub lessons: Vec<JsonLessonV1>,
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct JsonAuthorV1 {
    pub name: String,
    pub url: String,
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct JsonRequisiteV1 {
    pub name: String,
    pub url: String,
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub enum JsonLevelV1 {
    #[serde(rename = "beginner")]
    Beginner,
    #[default]
    #[serde(skip)]
    Invalid,
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub enum JsonLanguageV1 {
    #[serde(rename = "rust")]
    Rust,
    #[serde(rename = "go")]
    Go,
    #[default]
    #[serde(skip)]
    Invalid,
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct JsonCourseV1 {
    pub version: String,
    pub name: String,
    pub title: String,
    pub slug: String,
    pub description: String,
    pub tagline: String,
    pub author: JsonAuthorV1,
    pub requisites: Vec<JsonRequisiteV1>,
    #[serde(deserialize_with = "no_empty_vec")]
    pub outcomes: Vec<String>,
    pub level: JsonLevelV1,
    #[serde(deserialize_with = "no_empty_vec")]
    pub languages: Vec<JsonLanguageV1>,
    #[serde(deserialize_with = "no_empty_vec")]
    pub tags: Vec<String>,
    #[serde(deserialize_with = "no_empty_vec")]
    pub stages: Vec<JsonStageV1>,
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

impl JsonTest for JsonTestV1 {
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

impl<'a> JsonCourse<'a> for JsonCourseV1 {
    fn name(&'a self) -> &'a str {
        &self.name
    }

    fn author(&'a self) -> &'a str {
        &self.author.name
    }

    // TODO: remove copy
    fn list_tests(&self) -> IndexMap<Vec<u8>, TestState> {
        let Self { stages, name, .. } = self;

        stages.iter().fold(IndexMap::new(), |acc, stage| {
            stage.lessons.iter().fold(acc, |acc, lesson| match &lesson.suites {
                Some(suites) => suites.iter().fold(acc, |acc, suite| {
                    suite.tests.iter().fold(acc, |mut acc, test| {
                        let key = format!(
                            "{}{}{}{}{}",
                            test.name,
                            suite.name,
                            lesson.name,
                            stage.name,
                            name
                        )
                        .encode();

                        let cmd = test
                            .cmd
                            .split_whitespace()
                            .map(|arg| arg.to_string())
                            .collect::<Vec<_>>();

                        let path = vec![
                            PathLink::Link(stage.name.clone()),
                            PathLink::Link(lesson.name.clone()),
                            if suite.optional {
                                PathLink::LinkOptional(suite.name.clone())
                            } else {
                                PathLink::Link(suite.name.clone())
                            },
                            PathLink::Link(test.name.clone()),
                        ];

                        let test = TestState {
                            name: test.name.clone(),
                            cmd,
                            path,
                            passed: ValidationState::Unkown,
                        };

                        acc.insert(key, test);
                        acc
                    })
                }),
                None => acc,
            })
        })
    }
}
