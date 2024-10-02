//! A module for parsing course data.
//!
//! This module is concerned with loading course data from the backend API,
//! parsing it, and providing an implementation for executing tests. The actual
//! execution is the responsibility of the test [runner].

use git2::Repository;
use parity_scale_codec::{Decode, Encode};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use std::path::Path;
use thiserror::Error;
use v1::JsonRepoV1;

use crate::{
    constants::BACKEND_URL,
    models::{
        Course, Relationship, Repository as RepositoryModel, TesterDefinition,
    },
    parsing::v1::JsonCourseV1,
};

pub mod v1;

pub const V_1_0: &str = "1.0";

#[derive(Error, Debug)]
pub enum ParsingError {
    #[error("invalid course format: {0}")]
    CourseFmtError(String),
    #[error("failed to fetch course data: {0}")]
    CourseFetchError(String),
    #[error("invalid repository format: {0}")]
    RepositoryFmtError(String),
    #[error("failed to extract repo name: {0}")]
    RepoNameExtractionError(String),
    #[error("failed to fetch repository data: {0}")]
    RepositoryFetchError(String),
    #[error("HTTP request error: {0}")]
    HttpError(#[from] reqwest::Error),
    #[error("Git error: {0}")]
    GitError(#[from] git2::Error),
    #[error("YAML parsing error: {0}")]
    YamlError(#[from] serde_yaml::Error),
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
}

fn extract_repo_name() -> Result<String, ParsingError> {
    log::debug!("Extracting repo name from .git/config");
    let repo = Repository::open(".")?;
    let remote = repo.find_remote("origin")?;
    let url = remote.url().ok_or_else(|| {
        ParsingError::RepoNameExtractionError("No remote URL found".to_string())
    })?;

    log::debug!("Found remote URL: {}", url);

    let repo_name = Path::new(url)
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
        ParsingError::RepoNameExtractionError(
            "Failed to extract repo name from URL".to_string(),
        )
    })?;

    log::debug!("Extracted repo name: {}", repo_name);
    Ok(repo_name.to_string())
}

fn fetch_course(
    client: &Client,
    course_id: &str,
) -> Result<Course, ParsingError> {
    log::debug!("Fetching course with id `{}`", course_id);
    let response =
        client.get(format!("{}/course/{}", BACKEND_URL, course_id)).send()?;

    if !response.status().is_success() {
        log::error!(
            "Failed to fetch course data. HTTP status: {}",
            response.status()
        );
        return Err(ParsingError::CourseFetchError(format!(
            "HTTP error: {}",
            response.status()
        )));
    }

    log::debug!("{:#?}", response);

    let response_text = response
        .json()
        .map_err(|e| ParsingError::CourseFetchError(e.to_string()));

    log::debug!("Successfully fetched course data:\n{:#?}", response_text);

    response_text
}

fn fetch_repository(
    client: &Client,
    repo_name: &str,
) -> Result<RepositoryModel, ParsingError> {
    log::debug!("Fetching repository details for `{}`", repo_name);
    let response = client
        .get(format!("{}/repository/{}", BACKEND_URL, repo_name))
        .send()?;

    if !response.status().is_success() {
        log::error!(
            "Failed to fetch repository data. HTTP status: {}",
            response.status()
        );
        return Err(ParsingError::RepositoryFetchError(format!(
            "HTTP error: {}",
            response.status()
        )));
    }

    let response_text = response
        .json()
        .map_err(|e| ParsingError::RepositoryFetchError(e.to_string()));
    log::debug!("Successfully fetched repository data:\n{:#?}", response_text);

    response_text
}

pub fn load_course(client: &Client) -> Result<JsonCourseVersion, ParsingError> {
    log::debug!("Starting to load course");

    let repo_name = extract_repo_name()?;
    let repo_data = fetch_repository(client, &repo_name)?;

    let course_relation: &Relationship =
        repo_data.relationships.get("course").ok_or(()).map_err(|_| {
            ParsingError::RepositoryFmtError(
                "missing field 'relationships.course' in repository data"
                    .to_string(),
            )
        })?;

    let course_data: Course =
        fetch_course(client, &course_relation.id.to_string())?;

    log::debug!("Parsing course data");

    let version = &course_data.version;

    log::debug!("Course version: {:?}", version);

    let Course { version, slug, name, title, tester_url, author, .. } =
        course_data.clone();

    match version.as_ref() {
        V_1_0 => {
            log::debug!("Parsing course data as version 1.0");
            let json_course_v1 =
                JsonCourseV1 { version, slug, author, name, title, tester_url };

            log::debug!("Course loaded successfully!");

            Ok(JsonCourseVersion::V1(json_course_v1))
        }
        _ => {
            log::error!("Invalid course version: {}", version);
            Err(ParsingError::CourseFmtError(format!(
                "invalid course version '{version}' in course data"
            )))
        }
    }
}

pub fn load_tester(
    client: &Client,
    course: &JsonCourseVersion,
) -> Result<TesterDefinition, ParsingError> {
    log::debug!("Starting to load tester definition");

    let tester_url = match course {
        JsonCourseVersion::V1(course) => &course.tester_url,
    };

    // Construct the URL for the tester-definition.yml file
    let tester_definition_url =
        format!("{}/raw/refs/heads/master/tester-definition.yml", tester_url);
    log::debug!("Fetching tester definition from: {}", tester_definition_url);

    // Fetch the tester-definition.yml file
    let response = client.get(&tester_definition_url).send()?;

    if !response.status().is_success() {
        log::error!(
            "Failed to fetch tester definition. HTTP status: {}",
            response.status()
        );
        return Err(ParsingError::RepositoryFetchError(format!(
            "HTTP error: {}",
            response.status()
        )));
    }

    // Get the content of the response as text
    let yaml_content = response.text()?;
    log::debug!("Successfully fetched tester definition YAML");

    // Parse the YAML content into TesterDefinition
    let tester_definition: TesterDefinition =
        serde_yaml::from_str(&yaml_content)?;
    log::debug!("Successfully parsed tester definition");

    Ok(tester_definition)
}

pub fn load_repo() -> Result<JsonRepoV1, ParsingError> {
    Ok(JsonRepoV1 { name: extract_repo_name()?, commit_sha: "".to_string() })
}
