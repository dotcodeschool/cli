use colored::Colorize;
use std::{fmt::Display, os::unix::fs::MetadataExt};

use blake2::{
    digest::{Update, VariableOutput},
    Blake2bVar,
};
use indexmap::IndexMap;
use parity_scale_codec::{Decode, Encode};
use thiserror::Error;

use crate::{
    parsing::{CourseMetaData, MetadataError, TestResult},
    str_res::OPTIONAL,
};

pub const PATH_DB: &str = "./db";
pub const KEY_TIME: &[u8] = b"time_last_modified";
pub const KEY_TESTS: &[u8] = b"tests";
pub const KEY_STAGGERED: &[u8] = b"staggered";
pub const KEY_METADATA: &[u8] = b"metadata";
const HASH_SIZE: usize = 2;

#[derive(Error, Debug)]
pub enum DbError {
    #[error("failed to open database at '{0}': {1}")]
    DbOpen(String, String),
    #[error("failed to open tree at '{0}': {1}")]
    DbOpenTree(String, String),
    #[error(
        "failed to check for database update, could not open file at {0}: {1}"
    )]
    DbUpdateCheck(String, String),
    #[error("failed to retrieve value at key '{0}': {1}")]
    DbGet(String, String),
    #[error("failed to insert value at key '{0}': {1}")]
    DbInsert(String, String),
    #[error("failed to decode data stored at key '{0}': {1}")]
    DecodeError(String, String),
    #[error("failed to retrieve course metadata")]
    MetadataError(#[from] MetadataError),
}

#[derive(Encode, Decode, Debug, Clone)]
pub enum ValidationState {
    Unkown,
    Pass,
    Fail,
}

#[derive(Encode, Decode, Debug, Clone)]
pub enum PathLink {
    Link(String),
    LinkOptional(String),
}

#[derive(Encode, Decode, Debug, Clone)]
pub struct TestState {
    pub name: String,
    pub slug: String,
    pub message_on_success: String,
    pub message_on_fail: String,
    pub cmd: Vec<String>,
    pub path: Vec<PathLink>,
    pub passed: ValidationState,
    pub optional: bool,
}

impl TestState {
    pub fn run(&self, target: &str) -> TestResult {
        log::debug!("Running test: '{:?}", self.cmd);
        log::debug!("Test location: '{:?}", target);

        let output = std::process::Command::new(&self.cmd[0])
            .args(self.cmd[1..].iter())
            .current_dir(target)
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

    pub fn path_to(&self) -> String {
        let [section_link, lesson_link, suite_link, _] = &self.path[..] else {
            return String::default();
        };

        match (section_link, lesson_link, suite_link) {
            (
                PathLink::Link(section_name),
                PathLink::Link(lesson_name),
                PathLink::Link(suite_name),
            ) => {
                format!("{section_name}/{lesson_name}/{suite_name}")
            }
            (
                PathLink::Link(section_name),
                PathLink::Link(lesson_name),
                PathLink::LinkOptional(suite_name),
            ) => {
                format!("{section_name}/{lesson_name}/{suite_name}")
            }
            _ => unreachable!(),
        }
    }
}

impl Display for TestState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let [section_link, lesson_link, suite_link, test_link] = &self.path[..]
        else {
            return write!(f, "");
        };

        match (section_link, lesson_link, suite_link, test_link) {
            (
                PathLink::Link(section_name),
                PathLink::Link(lesson_name),
                PathLink::Link(suite_name),
                PathLink::Link(test_name),
            ) => {
                write!(
                    f,
                    "\n{}\n╰─{}\n  ╰─{}\n\n   🧪 Running test {test_name}",
                    section_name.green(),
                    lesson_name.green(),
                    suite_name.green()
                )
            }
            (
                PathLink::Link(section_name),
                PathLink::Link(lesson_name),
                PathLink::LinkOptional(suite_name),
                PathLink::Link(test_name),
            ) => {
                write!(
                    f,
                    "\n{}\n╰─{}\n  ╰─{} {}\n\n   🧪 Running test {test_name}",
                    section_name.green(),
                    lesson_name.green(),
                    suite_name.green(),
                    *OPTIONAL
                )
            }
            (
                PathLink::Link(section_name),
                PathLink::Link(lesson_name),
                PathLink::Link(suite_name),
                PathLink::LinkOptional(test_name),
            ) => {
                write!(
                    f,
                    "\n{}\n╰─{}\n  ╰─{}\n\n   🧪 Running test {test_name} {}",
                    section_name.green(),
                    lesson_name.green(),
                    suite_name.green(),
                    *OPTIONAL
                )
            }
            _ => unreachable!(),
        }
    }
}

pub fn hash(words: &[&str]) -> String {
    let phrase = words.join("");

    let mut hasher = Blake2bVar::new(HASH_SIZE).unwrap();
    let mut hash = [0; HASH_SIZE];

    hasher.update(phrase.as_bytes());
    hasher.finalize_variable(&mut hash).unwrap();

    hex::encode(hash)
}

pub fn db_open(
    path_db: &str,
    path_course: &str,
) -> Result<(sled::Db, sled::Tree), DbError> {
    let db = sled::open(path_db)
        .map_err(|err| DbError::DbOpen(path_db.to_string(), err.to_string()))?;

    let tree = db.open_tree(path_course).map_err(|err| {
        DbError::DbOpenTree(path_course.to_string(), err.to_string())
    })?;

    Ok((db, tree))
}

pub fn db_should_update(
    tree: &sled::Tree,
    path: &str,
) -> Result<bool, DbError> {
    let metadata = std::fs::metadata(path).map_err(|err| {
        DbError::DbUpdateCheck(path.to_string(), err.to_string())
    })?;

    let time_last_modified = metadata.mtime();
    let time_store = tree
        .get(KEY_TIME)
        .map_err(|err| DbError::DbGet(hex::encode(KEY_TIME), err.to_string()))?
        .map(|bytes| i64::decode(&mut &bytes[..]).unwrap());

    // TODO: replace this with `fetch_and_update`
    tree.insert(KEY_TIME, time_last_modified.encode()).map_err(|err| {
        DbError::DbInsert(hex::encode(KEY_TIME), err.to_string())
    })?;

    let should_update = match time_store {
        Some(time_store) => time_last_modified > time_store,
        None => true,
    };

    Ok(should_update)
}

pub fn db_update(
    tree: &sled::Tree,
    tests: &IndexMap<String, TestState>,
    metadata: CourseMetaData,
) -> Result<(), DbError> {
    tree.insert(KEY_METADATA, CourseMetaData::encode(&metadata)).map_err(
        |err| DbError::DbInsert(hex::encode(KEY_METADATA), err.to_string()),
    )?;

    // Inserts all new tests. This could be optimized so that only test that
    // have changed are updated -and this was the case initially. However, the
    // maintenance cost of deciding when a test in db is invalid proved to be
    // too much for something as simple (and most likely infrequent) as this
    for (key, test) in tests.iter() {
        tree.insert(key, test.encode()).map_err(|err| {
            DbError::DbInsert(hex::encode(key), err.to_string())
        })?;
    }

    // Updates the list of available tests
    let test_keys_new =
        tests.into_iter().map(|(key, _)| key).collect::<Vec<_>>();
    tree.insert(KEY_TESTS, test_keys_new.encode()).map_err(|err| {
        DbError::DbInsert(hex::encode(KEY_TESTS), err.to_string())
    })?;

    // Resets staggered test count: this is the number of tests to have
    // successfully be run sequentially
    tree.insert(KEY_STAGGERED, 1u32.encode()).map_err(|err| {
        DbError::DbInsert(hex::encode(KEY_STAGGERED), err.to_string())
    })?;

    Ok(())
}
