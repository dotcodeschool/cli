use std::os::unix::fs::MetadataExt;

use blake2::{
    digest::{Update, VariableOutput},
    Blake2bVar,
};
use indexmap::{IndexMap, IndexSet};
use parity_scale_codec::{Decode, Encode};
use thiserror::Error;

pub const PATH_DB: &str = "./db";
pub const KEY_TIME: &[u8] = b"time_last_modified";
pub const KEY_TESTS: &[u8] = b"tests";
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
    #[error("failed to remove value at key '{0}': {1}")]
    DbRemove(String, String),
    #[error("failed to decode data stored at key '{0}': {1}")]
    DecodeError(String, String),
}

#[derive(Encode, Decode, Debug)]
pub enum ValidationState {
    Unkown,
    Pass,
    Fail,
}

#[derive(Encode, Decode, Debug)]
pub struct TestState {
    pub path: Vec<String>,
    pub passed: ValidationState,
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

// TODO: support tests with the same name but different paths
pub fn db_update(
    tree: &sled::Tree,
    tests_new: &IndexMap<String, TestState>,
) -> Result<(), DbError> {
    let tests_old = tree
        .get(KEY_TESTS)
        .map_err(|err| DbError::DbGet(hex::encode(KEY_TESTS), err.to_string()))?
        .map(|bytes| <Vec<String>>::decode(&mut &bytes[..]).unwrap());

    // Db already contains tests for this course file.
    if let Some(tests_old) = tests_old {
        let tests_keys_old = tests_old.into_iter().collect::<IndexSet<_>>();
        let tests_keys_new = tests_new
            .iter()
            .map(|(key, _)| key.clone())
            .collect::<IndexSet<_>>();

        let test_keys_deprecated =
            tests_keys_old.difference(&tests_keys_new).collect::<Vec<_>>();

        // Removes all tests which are no longer in course file
        for key_str in test_keys_deprecated {
            let key = key_str.encode();
            tree.remove(&key).map_err(|err| {
                DbError::DbRemove(hex::encode(&key), err.to_string())
            })?;
        }

        let tests_keys_unkown =
            tests_keys_new.difference(&tests_keys_old).collect::<Vec<_>>();

        // Inserts tests which were not already in the course file
        for key_str in tests_keys_unkown {
            let key = key_str.encode();
            let test = tests_new.get(key_str).unwrap();

            tree.insert(&key, test.encode()).map_err(|err| {
                DbError::DbInsert(hex::encode(&key), err.to_string())
            })?;
        }

        // Updates the list of available tests
        let test_keys_new = tests_keys_new.iter().collect::<Vec<_>>();
        tree.insert(KEY_TESTS, test_keys_new.encode()).map_err(|err| {
            DbError::DbInsert(hex::encode(KEY_TESTS), err.to_string())
        })?;
    // Db does not already contain tests for the current course file
    } else {
        // Inserts all new tests
        for (key_str, test) in tests_new.iter() {
            let key = key_str.encode();

            tree.insert(&key, test.encode()).map_err(|err| {
                DbError::DbInsert(hex::encode(&key), err.to_string())
            })?;
        }

        // Updates the list of available tests
        let test_keys_new =
            tests_new.into_iter().map(|(key, _)| key).collect::<Vec<_>>();
        tree.insert(KEY_TESTS, test_keys_new.encode()).map_err(|err| {
            DbError::DbInsert(hex::encode(KEY_TESTS), err.to_string())
        })?;
    }

    Ok(())
}
