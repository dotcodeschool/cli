//! A module for parsing `tests.json` files.
//!
//! This module is concerned with loading `test.json` files, parsing them and
//! executing providing an implementation for executing tests. The actual
//! execution is the responsibility of the test [runner].

use indexmap::IndexMap;
use parity_scale_codec::{Decode, Encode};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use crate::{db::TestState, parsing::v1::JsonCourseV1};

pub mod v1;

pub const V_1_0: &str = "1.0";

#[derive(Error, Debug)]
pub enum ParsingError {
    #[error("failed to open course file at {0}")]
    FileOpenError(String),
    #[error("invalid course format: {0}")]
    CourseFmtError(String),
}

#[derive(Error, Debug)]
pub enum MetadataError {
    #[error("failed to retrieve course metadata: {0}")]
    MetadataRetrievalError(String),
    #[error("Invalid course metadata format: {0}")]
    MetadataFmtError(String),
}

pub enum TestResult {
    Pass(String),
    Fail(String),
}

#[derive(Serialize, Deserialize, Encode, Decode, Debug, Default)]
pub struct CourseMetaData {
    pub logstream_url: String,
    pub logstream_id: String,
    pub ws_url: String,
    pub tester_url: String,
}

pub trait JsonCourse<'a> {
    fn name(&'a self) -> &'a str;
    fn author(&'a self) -> &'a str;
    fn list_tests(&self) -> IndexMap<String, TestState>;
    fn fetch_metatdata(&self) -> Result<CourseMetaData, MetadataError>;
}

pub enum JsonCourseVersion {
    V1(JsonCourseV1),
}

impl<'a> JsonCourse<'a> for JsonCourseVersion {
    fn name(&'a self) -> &'a str {
        match self {
            JsonCourseVersion::V1(course) => course.name(),
        }
    }

    fn author(&'a self) -> &'a str {
        match self {
            JsonCourseVersion::V1(course) => course.author(),
        }
    }

    fn list_tests(&self) -> IndexMap<String, TestState> {
        match self {
            JsonCourseVersion::V1(course) => course.list_tests(),
        }
    }

    fn fetch_metatdata(&self) -> Result<CourseMetaData, MetadataError> {
        match self {
            JsonCourseVersion::V1(course) => course.fetch_metatdata(),
        }
    }
}

pub fn load_course(path: &str) -> Result<JsonCourseVersion, ParsingError> {
    log::debug!("Loading course '{path}'");

    let file_contents = std::fs::read_to_string(path).map_err(|_| {
        ParsingError::FileOpenError(format!("failed to open file at {path}"))
    })?;
    let json_raw = serde_json::from_str::<serde_json::Value>(&file_contents)
        .map_err(|err| ParsingError::CourseFmtError(err.to_string()))?;
    let version = json_raw.get("version").ok_or(()).map_err(|_| {
        ParsingError::CourseFmtError(format!(
            "missing field 'version' in {path}"
        ))
    })?;

    match version {
        Value::String(version) => match version.as_ref() {
            V_1_0 => {
                let json_course_v1 =
                    serde_json::from_str::<JsonCourseV1>(&file_contents)
                        .map_err(|err| {
                            ParsingError::CourseFmtError(err.to_string())
                        })?;

                log::debug!("Course loaded successfully!");

                Ok(JsonCourseVersion::V1(json_course_v1))
            }
            _ => Err(ParsingError::CourseFmtError(format!(
                "invalid course version '{version}' in {path}"
            ))),
        },
        _ => Err(ParsingError::CourseFmtError(format!(
            "'version' must be a string in {path}"
        ))),
    }
}
