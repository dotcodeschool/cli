use indicatif::ProgressBar;

use colored::Colorize;

use crate::{
    db::TestState,
    parsing::{
        load_course, v1::JsonCourseV1, v2::JsonCourseV2, JsonCourse,
        JsonCourseVersion, ParsingError,
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

pub struct Monitor {
    course: JsonCourseVersion,
    progress: ProgressBar,
}

impl Monitor {
    pub fn new(path: &str) -> Self {
        match load_course(path) {
            Ok(course) => Self { course, progress: ProgressBar::new(0) },
            Err(e) => {
                let msg = match e {
                    ParsingError::CourseFmtError(msg) => msg,
                    ParsingError::FileOpenError(msg) => msg,
                };
                log::error!("{msg}");

                Self {
                    course: JsonCourseVersion::V1(JsonCourseV1::default()),
                    progress: ProgressBar::new(0),
                }
            }
        }
    }

    /// Creates a new [Runner] instance depending on the version specified in
    /// the course json config file.
    pub fn into_runner(self) -> RunnerVersion {
        self.greet();

        let Self { course, progress } = self;

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

        let Self { course, progress } = self;

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

    pub fn list_tests(&self) -> Vec<TestState> {
        self.course.list_tests()
    }

    fn greet(&self) {
        let Self { course, progress } = self;

        progress.println(DOTCODESCHOOL.clone());

        progress.println(format!(
            "\n🎓 {} by {}",
            course.name().to_uppercase().white().bold(),
            course.author().white().bold()
        ));
    }
}
