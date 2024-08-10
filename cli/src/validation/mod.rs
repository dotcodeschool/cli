use indicatif::ProgressBar;

use crate::parsing::{load_course, JsonCourseVersion, ParsingError};

use self::v2::{ValidatorStateV2, ValidatorV2};

pub mod v2;

pub enum ValidatorVersion {
    V2(ValidatorV2),
    Undefined,
}

pub trait Validator {
    fn run(self) -> Self;

    #[allow(clippy::new_ret_no_self)]
    fn new(path: &str) -> ValidatorVersion {
        match load_course(path) {
            Ok(course_version) => match course_version {
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

                    let progress = ProgressBar::new(slug_count as u64);

                    let validator = ValidatorV2::new(
                        progress,
                        ValidatorStateV2::Loaded,
                        course,
                    );

                    ValidatorVersion::V2(validator)
                }
            },
            Err(e) => {
                let msg = match e {
                    ParsingError::CourseFmtError(msg) => msg,
                    ParsingError::FileOpenError(msg) => msg,
                };
                log::error!("{msg}");

                ValidatorVersion::Undefined
            }
        }
    }

    fn is_finished(&self) -> bool;
}

impl Validator for ValidatorVersion {
    fn run(self) -> Self {
        match self {
            Self::V2(validator) => Self::V2(validator.run()),
            Self::Undefined => Self::Undefined,
        }
    }

    fn is_finished(&self) -> bool {
        match self {
            ValidatorVersion::V2(validator) => validator.is_finished(),
            ValidatorVersion::Undefined => true,
        }
    }
}
