use indicatif::ProgressBar;

use colored::Colorize;
use itertools::{FoldWhile, Itertools};
use parity_scale_codec::{Decode, Encode};
use sled::IVec;

use crate::{
    db::{
        db_open, db_should_update, db_update, DbError, TestState,
        KEY_STAGGERED, KEY_TESTS,
    },
    lister::{v1::ListerV1, ListerVersion},
    parsing::{load_course, JsonCourse, JsonCourseVersion, ParsingError},
    runner::{v1::RunnerV1, RunnerVersion},
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

pub struct Monitor {
    course: JsonCourseVersion,
    progress: ProgressBar,
    #[allow(dead_code)]
    db: sled::Db,
    tree: sled::Tree,
}

impl Monitor {
    pub fn new(path_db: &str, path_course: &str) -> Result<Self, DbError> {
        match load_course(path_course) {
            Ok(course) => {
                let tests_new = course.list_tests();
                let (db, tree) = db_open(path_db, path_course)?;

                if db_should_update(&tree, path_course)? {
                    let metadata = course.fetch_metatdata()?;
                    db_update(&tree, &tests_new, metadata)?;
                }

                Ok(Self { course, progress: ProgressBar::new(0), db, tree })
            }
            Err(e) => {
                let msg = match e {
                    ParsingError::CourseFmtError(msg) => msg,
                    ParsingError::FileOpenError(msg) => msg,
                };
                log::error!("{msg}");

                Err(DbError::DbOpen(
                    path_db.to_string(),
                    "could not deserialize course".to_string(),
                ))
            }
        }
    }

    /// Creates a new [Runner] instance depending on the version specified in
    /// the course json config file.
    pub fn into_runner(
        self,
        test_name: Option<String>,
    ) -> Result<RunnerVersion, DbError> {
        self.greet();

        let Self { course, progress, tree, .. } = self;

        let tests = match test_name {
            Some(test_name) => {
                Self::tests_accumulate_matching(test_name, &tree)
            }
            None => Self::tests_accumulate_all(&tree),
        }
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;

        match course {
            JsonCourseVersion::V1(_) => {
                progress.set_length(tests.len() as u64);

                let runner = RunnerV1::new(progress, tree, tests);

                Ok(RunnerVersion::V1(runner))
            }
        }
    }

    pub fn into_runner_staggered(self) -> Result<RunnerVersion, DbError> {
        self.greet();

        let Self { course, progress, tree, .. } = self;

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

        match course {
            JsonCourseVersion::V1(course) => {
                let test_count = course.stages.iter().fold(0, |acc, stage| {
                    acc + stage.lessons.iter().fold(0, |acc, lesson| {
                        acc + match &lesson.suites {
                            Some(suites) => suites
                                .iter()
                                .fold(0, |acc, suite| acc + suite.tests.len()),
                            None => 0,
                        }
                    })
                });

                progress.set_length(test_count as u64);

                let runner = RunnerV1::new_with_hooks(
                    progress,
                    tree.clone(),
                    tests,
                    move || {
                        let staggered = staggered + 1;
                        tree.insert(KEY_STAGGERED, staggered.encode()).unwrap();
                    },
                    || {},
                );

                Ok(RunnerVersion::V1(runner))
            }
        }
    }

    pub fn into_validator(self) -> ValidatorVersion {
        self.greet();

        let Self { course, progress, .. } = self;

        match course {
            JsonCourseVersion::V1(course) => {
                let slug_count =
                    1 + course.stages.iter().fold(0, |acc, stage| {
                        acc + 1
                            + stage.lessons.iter().fold(0, |acc, lesson| {
                                acc + 1
                                    + match &lesson.suites {
                                        Some(suites) => suites.iter().fold(
                                            0,
                                            |acc, suite| {
                                                acc + 1 + suite.tests.len()
                                            },
                                        ),
                                        None => 0,
                                    }
                            })
                    });

                progress.set_length(slug_count as u64);

                let validator = ValidatorV1::new(
                    progress,
                    ValidatorStateV1::Loaded,
                    course,
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
}
