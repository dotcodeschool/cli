use serde::{Deserialize, Deserializer, Serialize};

use crate::constants::BACKEND_URL;

use super::{CourseMetaData, JsonCourse, MetadataError};

pub mod redis;

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
pub struct JsonRepoV1 {
    pub name: String,
    pub commit_sha: String,
}

impl JsonRepoV1 {
    pub fn fetch_metadata(&self) -> Result<CourseMetaData, MetadataError> {
        let Self { name, commit_sha } = self;

        let request = format!(
            concat!(
                "{{",
                "\"repo_name\":",
                "\"{}\",",
                "\"commit_sha\":",
                "\"{}\"",
                "}}"
            ),
            name, commit_sha
        );

        log::debug!("fetching metadata: {request}");

        // TODO: use reqwest for fetching data
        let output = std::process::Command::new("curl")
            .arg("-fsSL")
            .arg("-H")
            .arg("Content-Type: application/json")
            .arg("-d")
            .arg(request)
            .arg(format!("{}/submission", BACKEND_URL))
            .output()
            .map(|output| (output.status.success(), output));

        match output {
            Ok((true, output)) => {
                log::debug!("extracting course metadata from JSON");

                let metadata =
                    serde_json::from_slice::<CourseMetaData>(&output.stdout)
                        .map_err(|e| {
                            MetadataError::MetadataFmtError(e.to_string())
                        })?;

                Ok(metadata)
            }
            Ok((false, output)) => {
                let stderr = String::from_utf8(output.stderr).unwrap();

                log::debug!("course metadata retrieval failed: {stderr}");

                Err(MetadataError::MetadataRetrievalError(stderr))
            }
            Err(e) => Err(MetadataError::MetadataRetrievalError(e.to_string())),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Default, PartialEq, Eq, Clone)]
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
    pub slug: String,
    pub name: String,
    pub author: JsonAuthorV1,
    pub title: String,
    pub tester_url: String,
}

pub fn no_empty_vec<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
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

impl<'a> JsonCourse<'a> for JsonCourseV1 {
    fn name(&'a self) -> &'a str {
        &self.name
    }

    fn author(&'a self) -> &'a str {
        &self.author.name
    }
}
