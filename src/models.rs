use std::collections::HashMap;

use bson::oid::ObjectId;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use strum_macros::Display;

use crate::{
    db::{PathLink, TestState, ValidationState},
    parsing::v1::{no_empty_vec, JsonAuthorV1, JsonSectionV1},
};

/// The type of document. This is used to identify the type of document in the
/// relationships between documents.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DocumentType {
    Repository,
    User,
    Course,
}

/// Expected activity frequency for a repository. This is used to determine how
/// often the user wants to practice.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Display)]
#[serde(rename_all = "snake_case")]
pub enum ExpectedPracticeFrequency {
    EveryDay,
    OnceAWeek,
    OnceAMonth,
}

/// A repository document. This is used to store information about the owner of
/// the repository, the template used to create the repository, and the
/// relationships between the repository and other documents.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Repository {
    pub repo_name: String,
    pub repo_template: String,
    pub tester_url: String,
    pub relationships: HashMap<String, Relationship>,
    pub expected_practice_frequency: ExpectedPracticeFrequency,
    pub is_reminder_enabled: bool,
}

/// A user document. This is used to store information about the user, the
/// repositories they own, and the relationships between the user and other
/// documents.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct User {
    pub name: String,
    pub repositories: Vec<Relationship>,
    pub relationships: Vec<Relationship>,
}

/// A course document. This is used to store information about the course, the
/// users enrolled in the course, and the relationships between the course and
/// other documents.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Course {
    pub version: String,
    #[serde(rename = "_id")]
    pub id: ObjectId,
    pub slug: String,
    pub name: String,
    pub title: String,
    pub author: JsonAuthorV1,
    pub tester_url: String,
    #[serde(default)]
    pub relationships: Vec<Relationship>,
}

/// A relationship between documents. This is used to store the ID of the
/// document and the type of document in the relationship.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Relationship {
    pub id: ObjectId,
    pub r#type: DocumentType,
}

/// Tester definition structure
#[derive(Serialize, Deserialize, Debug, Default)]
pub struct TesterDefinition {
    #[serde(deserialize_with = "no_empty_vec")]
    pub sections: Vec<JsonSectionV1>,
    pub course_name: String,
}

impl TesterDefinition {
    // TODO: remove copy
    pub fn list_tests(&self) -> IndexMap<String, TestState> {
        let Self { sections, course_name, .. } = self;
        log::debug!("Listing tests...");

        sections.iter().fold(IndexMap::new(), |acc, section| {
            section.lessons.iter().fold(acc, |acc, lesson| match &lesson.suites {
                Some(suites) => suites.iter().fold(acc, |acc, suite| {
                    suite.tests.iter().fold(acc, |mut acc, test| {
                        let key = [
                            test.name.to_lowercase(),
                            suite.name.to_lowercase(),
                            lesson.name.to_lowercase(),
                            section.name.to_lowercase(),
                            course_name.to_lowercase(),
                        ]
                        .concat();

                        let cmd = test
                            .cmd
                            .split_whitespace()
                            .map(|arg| arg.to_string())
                            .collect::<Vec<_>>();

                        let path = vec![
                            PathLink::Link(section.name.clone()),
                            PathLink::Link(lesson.name.clone()),
                            if suite.optional {
                                PathLink::LinkOptional(suite.name.clone())
                            } else {
                                PathLink::Link(suite.name.clone())
                            },
                            if !suite.optional && test.optional {
                                PathLink::LinkOptional(test.name.clone())
                            } else {
                                PathLink::Link(test.name.clone())
                            },
                        ];

                        let test = TestState {
                            name: test.name.clone(),
                            slug: test.slug.clone(),
                            message_on_success: test.message_on_success.clone(),
                            message_on_fail: test.message_on_fail.clone(),
                            cmd,
                            path,
                            passed: ValidationState::Unkown,
                            optional: suite.optional || test.optional,
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
