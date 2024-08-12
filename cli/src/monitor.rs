use hex::decode;
use indicatif::ProgressBar;

use colored::Colorize;
use itertools::{FoldWhile, Itertools};
use parity_scale_codec::{Decode, Encode};

use crate::{
    db::{db_open, db_should_update, db_update, DbError, TestState, KEY_TESTS},
    lister::{
        v1::{ListerStateV1, ListerV1},
        ListerVersion,
    },
    parsing::{load_course, JsonCourse, JsonCourseVersion, ParsingError},
    runner::{v1::RunnerV1, RunnerVersion},
    str_res::DOTCODESCHOOL,
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
                    db_update(&tree, &tests_new)?;
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
                let tests = tree.scan_prefix(test_name.encode()).fold_while(
                    vec![],
                    |mut acc, query| match query {
                        Ok((key, bytes)) => {
                            acc.push(
                                TestState::decode(&mut &bytes[..]).map_err(
                                    |err| {
                                        DbError::DecodeError(
                                            hex::encode(key),
                                            err.to_string(),
                                        )
                                    },
                                ),
                            );
                            FoldWhile::Continue(acc)
                        }
                        Err(_) => FoldWhile::Done(vec![]),
                    },
                );

                tests.into_inner()
            }
            None => {
                let test_names = match tree.get(KEY_TESTS) {
                    Ok(Some(bytes)) => {
                        <Vec<String>>::decode(&mut &bytes[..]).unwrap()
                    }
                    _ => vec![],
                };

                let tests = test_names
                    .into_iter()
                    .map(|key| (key.clone(), tree.get(key.encode())))
                    .fold_while(vec![], |mut acc, (key, query)| match query {
                        Ok(Some(bytes)) => {
                            acc.push(
                                TestState::decode(&mut &bytes[..]).map_err(
                                    |err| {
                                        DbError::DecodeError(
                                            hex::encode(key),
                                            err.to_string(),
                                        )
                                    },
                                ),
                            );
                            FoldWhile::Continue(acc)
                        }
                        _ => FoldWhile::Done(vec![]),
                    });

                tests.into_inner()
            }
        }
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

                let runner = RunnerV1::new(course, progress, tree);

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
                    Vec::<String>::decode(&mut &bytes[..]).map_err(|err| {
                        DbError::DecodeError(
                            hex::encode(KEY_TESTS),
                            err.to_string(),
                        )
                    })?;

                Ok(ListerVersion::V1(ListerV1 {
                    progress,
                    tests,
                    tree,
                    state: ListerStateV1::Loaded,
                }))
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
}
