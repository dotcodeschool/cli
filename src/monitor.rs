use std::fs;
use std::net::TcpStream;
use std::path::{Path, PathBuf};

use indicatif::ProgressBar;

use colored::Colorize;
use ignore::Walk;
use itertools::{FoldWhile, Itertools};
use parity_scale_codec::{Decode, Encode};
use rand::Rng;
use reqwest::blocking::Client;
use sled::IVec;
use thiserror::Error;
use tungstenite::{stream::MaybeTlsStream, Message, WebSocket};

use crate::{
    db::{
        db_open, db_should_update, db_update, DbError, TestState, KEY_METADATA,
        KEY_STAGGERED, KEY_TESTS,
    },
    lister::{v1::ListerV1, ListerVersion},
    models::TesterDefinition,
    parsing::{
        load_course, load_repo, load_tester, CourseMetaData, JsonCourse,
        JsonCourseVersion, MetadataError, ParsingError,
    },
    runner::{v1::RunnerV1Builder, RunnerVersion},
    str_res::{DOTCODESCHOOL, STAGGERED},
    validator::{
        v1::{ValidatorStateV1, ValidatorV1},
        ValidatorVersion,
    },
};

pub trait StateMachine {
    fn run(self) -> Self;
    fn is_finished(&self) -> bool;
}

#[derive(Error, Debug)]
pub enum MonitorError {
    #[error("{0}")]
    DbError(#[from] DbError),
    #[error("{0}")]
    WSError(#[from] tungstenite::Error),
    #[error("{0}")]
    IOError(#[from] std::io::Error),
    #[error("{0}")]
    ParsingError(#[from] ParsingError),
    #[error("{0}")]
    MetadataError(#[from] MetadataError),
}

pub struct Monitor {
    course: JsonCourseVersion,
    tester: TesterDefinition,
    progress: ProgressBar,
    tree: sled::Tree,
}

impl Monitor {
    pub fn new(path_db: &str) -> Result<Self, MonitorError> {
        log::debug!("Creating new Monitor instance");
        let client = Client::new();
        let course = load_course(&client)?;
        let tester = load_tester(&client, &course)?;
        let repo = load_repo()?;
        let tests_new = tester.list_tests();

        let (_, tree) = db_open(path_db, ".")?;

        if db_should_update(&tree, ".")? {
            let metadata = repo.fetch_metadata()?;
            db_update(&tree, &tests_new, metadata)?;
        }

        log::debug!("Monitor instance created successfully");
        Ok(Self { course, progress: ProgressBar::new(0), tree, tester })
    }

    pub fn into_runner(
        self,
        test_name: Option<String>,
        keep: bool,
    ) -> Result<RunnerVersion, MonitorError> {
        self.greet();

        let Self { course, progress, tree, .. } = self;

        let tests = match test_name {
            Some(test_name) => {
                let mut path_to = test_name.split("/").collect::<Vec<_>>();
                path_to.reverse();
                let key = path_to.join("");

                log::debug!("looking for tests which match path '{key}'");

                Self::tests_accumulate_matching(key, &tree)
            }
            None => Self::tests_accumulate_all(&tree),
        }
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;

        let metadata = match tree.get(KEY_METADATA) {
            Ok(Some(bytes)) => CourseMetaData::decode(&mut &bytes[..])
                .map_err(|e| {
                    DbError::DecodeError(
                        hex::encode(KEY_METADATA),
                        e.to_string(),
                    )
                })?,
            _ => {
                return Err(DbError::DbGet(
                    hex::encode(KEY_METADATA),
                    String::default(),
                )
                .into());
            }
        };

        log::debug!("initiating redis websocket stream");

        let client =
            Self::ws_stream_init(&metadata.ws_url, &metadata.logstream_id)?;

        match course {
            JsonCourseVersion::V1(_) => {
                progress.set_length(tests.len() as u64);

                let repo_name = Self::tester_repo_init(&metadata.tester_url)?;
                let repo_name_1 = repo_name.clone();
                let tree1 = tree.clone();
                let staggered = tests.len() as u32;

                let runner = RunnerV1Builder::new()
                    .progress(progress)
                    .target(repo_name)
                    .tree(tree.clone())
                    .client(client)
                    .tests(tests)
                    .on_pass(move || {
                        let _ = tree.insert(KEY_STAGGERED, staggered.encode());
                    })
                    .on_fail(move |index_test| {
                        // TODO: is this good UX?
                        let staggered = (index_test + 1) as u32;
                        let _ = tree1.insert(KEY_STAGGERED, staggered.encode());
                    })
                    .on_finish(move || {
                        if keep {
                            log::debug!(
                                "keeping tester repo '{}'",
                                &repo_name_1
                            );
                        } else {
                            let _ = Self::tester_repo_destroy(&repo_name_1);
                        }
                    })
                    .build();

                Ok(RunnerVersion::V1(runner))
            }
        }
    }

    pub fn into_runner_staggered(
        self,
        keep: bool,
    ) -> Result<RunnerVersion, MonitorError> {
        self.greet();

        let Self { course, progress, tree, tester, .. } = self;

        progress.println(format!("\n{}", STAGGERED.clone()));

        let query = tree.get(KEY_STAGGERED).map_err(|err| {
            DbError::DbGet(hex::encode(KEY_STAGGERED), err.to_string())
        })?;

        let staggered = match query {
            Some(bytes) => u32::decode(&mut &bytes[..]).map_err(|err| {
                DbError::DecodeError(
                    hex::encode(KEY_STAGGERED),
                    err.to_string(),
                )
            })?,
            None => 1,
        };

        let tests = Self::tests_accumulate_some(&tree, staggered as usize)
            .into_iter()
            .collect::<Result<Vec<_>, _>>()?;

        let metadata = match tree.get(KEY_METADATA) {
            Ok(Some(bytes)) => CourseMetaData::decode(&mut &bytes[..])
                .map_err(|e| {
                    DbError::DecodeError(
                        hex::encode(KEY_METADATA),
                        e.to_string(),
                    )
                })?,
            _ => {
                return Err(DbError::DbGet(
                    hex::encode(KEY_METADATA),
                    String::default(),
                )
                .into());
            }
        };

        log::debug!("initiating redis websocket stream");

        let client =
            Self::ws_stream_init(&metadata.ws_url, &metadata.logstream_id)?;

        match course {
            JsonCourseVersion::V1(_) => {
                let test_count =
                    tester.sections.iter().fold(0, |acc, section| {
                        acc + section.lessons.iter().fold(0, |acc, lesson| {
                            acc + match &lesson.tests {
                                Some(tests) => tests.len(),
                                None => 0,
                            }
                        })
                    });

                progress.set_length(test_count as u64);

                let repo_name = Self::tester_repo_init(&metadata.tester_url)?;
                let repo_name_1 = repo_name.clone();
                let tree1 = tree.clone();

                let runner = RunnerV1Builder::new()
                    .progress(progress)
                    .target(repo_name)
                    .tree(tree.clone())
                    .client(client)
                    .tests(tests)
                    .on_pass(move || {
                        let staggered = staggered + 1;
                        let _ = tree.insert(KEY_STAGGERED, staggered.encode());
                    })
                    .on_fail(move |index_test| {
                        // TODO: is this good UX?
                        let staggered = (index_test + 1) as u32;
                        let _ = tree1.insert(KEY_STAGGERED, staggered.encode());
                    })
                    .on_finish(move || {
                        if keep {
                            log::debug!("keeping tester repo '{repo_name_1}'");
                        } else {
                            let _ = Self::tester_repo_destroy(&repo_name_1);
                        }
                    })
                    .build();

                Ok(RunnerVersion::V1(runner))
            }
        }
    }

    #[cfg(not(debug_assertions))]
    pub fn into_validator(self) -> ValidatorVersion {
        self.greet();

        let Self { course, progress, tester, .. } = self;

        match course {
            JsonCourseVersion::V1(course) => {
                let slug_count =
                    1 + tester.sections.iter().fold(0, |acc, section| {
                        acc + 1
                            + section.lessons.iter().fold(0, |acc, lesson| {
                                acc + 1
                                    + if let Some(tests) = &lesson.tests {
                                        tests.len()
                                    } else {
                                        0
                                    }
                            })
                    });

                progress.set_length(slug_count as u64);

                let validator = ValidatorV1::new(
                    progress,
                    ValidatorStateV1::Loaded,
                    course,
                    tester,
                );

                ValidatorVersion::V1(validator)
            }
        }
    }

    pub fn into_lister(self) -> Result<ListerVersion, DbError> {
        let Self { course, progress, tree, .. } = self;

        match course {
            JsonCourseVersion::V1(_) => {
                let bytes = tree
                    .get(KEY_TESTS)
                    .map_err(|err| {
                        DbError::DbGet(hex::encode(KEY_TESTS), err.to_string())
                    })?
                    .unwrap();

                let tests =
                    <Vec<String>>::decode(&mut &bytes[..]).map_err(|err| {
                        DbError::DecodeError(
                            hex::encode(KEY_TESTS),
                            err.to_string(),
                        )
                    })?;

                Ok(ListerVersion::V1(ListerV1::new(progress, tests, tree)))
            }
        }
    }

    fn greet(&self) {
        let Self { course, progress, .. } = self;

        progress.println(DOTCODESCHOOL.clone());

        progress.println(format!(
            "\n🎓 {} by {}",
            course.name().to_uppercase().white().bold(),
            course.author().white().bold()
        ));
    }

    fn tests_accumulate_matching(
        test_name: String,
        tree: &sled::Tree,
    ) -> Vec<Result<(IVec, TestState), DbError>> {
        tree.scan_prefix(test_name)
            .fold_while(vec![], |mut acc, query| match query {
                Ok((key, bytes)) => {
                    acc.push(match TestState::decode(&mut &bytes[..]) {
                        Ok(test_state) => Ok((key, test_state)),
                        Err(e) => Err(DbError::DecodeError(
                            hex::encode(key),
                            e.to_string(),
                        )),
                    });
                    FoldWhile::Continue(acc)
                }
                Err(_) => FoldWhile::Done(vec![]),
            })
            .into_inner()
    }

    fn tests_accumulate_all(
        tree: &sled::Tree,
    ) -> Vec<Result<(IVec, TestState), DbError>> {
        let test_names = match tree.get(KEY_TESTS) {
            Ok(Some(bytes)) => <Vec<Vec<u8>>>::decode(&mut &bytes[..]).unwrap(),
            _ => vec![],
        };

        let tests = test_names
            .into_iter()
            .map(|key| {
                let query = tree.get(&key);
                (IVec::from(key), query)
            })
            .fold_while(vec![], |mut acc, (key, query)| match query {
                Ok(Some(bytes)) => {
                    acc.push(match TestState::decode(&mut &bytes[..]) {
                        Ok(test_state) => Ok((key, test_state)),
                        Err(e) => Err(DbError::DecodeError(
                            hex::encode(key),
                            e.to_string(),
                        )),
                    });
                    FoldWhile::Continue(acc)
                }
                _ => FoldWhile::Done(vec![]),
            });

        tests.into_inner()
    }

    fn tests_accumulate_some(
        tree: &sled::Tree,
        n: usize,
    ) -> Vec<Result<(IVec, TestState), DbError>> {
        let test_names = match tree.get(KEY_TESTS) {
            Ok(Some(bytes)) => <Vec<Vec<u8>>>::decode(&mut &bytes[..]).unwrap(),
            _ => vec![],
        };

        let tests = test_names
            .into_iter()
            .take(n)
            .map(|key| {
                let query = tree.get(&key);
                (IVec::from(key), query)
            })
            .fold_while(vec![], |mut acc, (key, query)| match query {
                Ok(Some(bytes)) => {
                    acc.push(match TestState::decode(&mut &bytes[..]) {
                        Ok(test_state) => Ok((key, test_state)),
                        Err(e) => Err(DbError::DecodeError(
                            hex::encode(key),
                            e.to_string(),
                        )),
                    });
                    FoldWhile::Continue(acc)
                }
                _ => FoldWhile::Done(vec![]),
            });

        tests.into_inner()
    }

    fn copy_user_code_to_tester(
        source: &str,
        destination: &str,
    ) -> Result<(), std::io::Error> {
        let source_path = Path::new(source);
        let destination_path = Path::new(destination);

        // Get the name of the destination directory
        let dest_dir_name =
            destination_path.file_name().unwrap().to_str().unwrap();

        for entry in Walk::new(source_path) {
            let entry = entry.map_err(|e| {
                std::io::Error::new(std::io::ErrorKind::Other, e)
            })?;
            let path = entry.path();

            // Skip the root directory itself and the destination directory
            if path == source_path
                || path.file_name().map_or(false, |name| name == dest_dir_name)
            {
                continue;
            }

            let relative_path = path.strip_prefix(source_path).unwrap();
            let dest_path = destination_path.join(relative_path);

            if path.is_dir() {
                fs::create_dir_all(&dest_path)?;
            } else {
                if let Some(parent) = dest_path.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::copy(path, &dest_path)?;
            }
        }

        Ok(())
    }

    fn ws_stream_init(
        ws_url: &str,
        logstream_id: &str,
    ) -> Result<WebSocket<MaybeTlsStream<TcpStream>>, MonitorError> {
        // TODO: use https://docs.rs/zeroize/latest/zeroize/ to handle ws address
        // + should be received from initial curl response
        let (mut client, _) = tungstenite::client::connect(ws_url)?;
        client.send(Message::Text(format!(
            "{{\"event_type\":\"init\",\"stream_id\":\"{}\"}}",
            logstream_id
        )))?;

        Ok(client)
    }

    fn tester_repo_init(repo_url: &str) -> Result<String, MonitorError> {
        // Extract the repo name from the git config
        let repo_name = crate::parsing::extract_repo_name()?;

        std::process::Command::new("git")
            .arg("clone")
            .arg(repo_url)
            .arg(&repo_name)
            .output()?;

        // Copy user's code to the tester directory
        let current_dir = std::env::current_dir()?;
        Self::copy_user_code_to_tester(
            current_dir.to_str().unwrap(),
            &repo_name,
        )?;

        Ok(repo_name)
    }

    fn tester_repo_destroy(repo_name: &str) -> Result<(), MonitorError> {
        let path = format!("./{repo_name}");
        std::fs::remove_dir_all(path)?;

        Ok(())
    }
}
