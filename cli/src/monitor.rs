use indicatif::ProgressBar;

use colored::Colorize;
use parity_scale_codec::Decode;

use crate::{
    db::{db_open, db_should_update, db_update, DbError, TestState, KEY_TESTS},
    lister::{
        v2::{ListerStateV2, ListerV2},
        ListerVersion,
    },
    parsing::{
        load_course, v1::JsonCourseV1, JsonCourse, JsonCourseVersion,
        ParsingError,
    },
    runner::{
        v1::{RunnerStateV1, RunnerV1},
        v2::{RunnerStateV2, RunnerV2},
        RunnerVersion,
    },
    str_res::DOTCODESCHOOL,
    validator::{
        v2::{ValidatorStateV2, ValidatorV2},
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
    pub fn into_runner(self) -> RunnerVersion {
        self.greet();

        let Self { course, progress, .. } = self;

        match course {
            JsonCourseVersion::V1(course) => {
                let test_count = course
                    .suites
                    .iter()
                    .fold(0, |acc, suite| acc + suite.tests.len());

                progress.set_length(test_count as u64);

                let runner =
                    RunnerV1::new(progress, 0, RunnerStateV1::Loaded, course);

                RunnerVersion::V1(runner)
            }
            JsonCourseVersion::V2(course) => {
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

                let runner =
                    RunnerV2::new(progress, 0, RunnerStateV2::Loaded, course);

                RunnerVersion::V2(runner)
            }
        }
    }

    pub fn into_validator(self) -> ValidatorVersion {
        self.greet();

        let Self { course, progress, .. } = self;

        match course {
            JsonCourseVersion::V1(_) => ValidatorVersion::Undefined,
            JsonCourseVersion::V2(course) => {
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

                let validator = ValidatorV2::new(
                    progress,
                    ValidatorStateV2::Loaded,
                    course,
                );

                ValidatorVersion::V2(validator)
            }
        }
    }

    pub fn into_lister(self) -> Result<ListerVersion, DbError> {
        self.greet();

        let Self { course, progress, tree, .. } = self;

        match course {
            JsonCourseVersion::V1(_) => todo!(),
            JsonCourseVersion::V2(_) => {
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

                Ok(ListerVersion::V2(ListerV2 {
                    progress,
                    tests,
                    tree,
                    state: ListerStateV2::Loaded,
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
