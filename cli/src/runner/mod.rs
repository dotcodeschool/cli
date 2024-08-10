use std::time::Duration;

use indicatif::{ProgressBar, ProgressStyle};

use regex::Regex;

use crate::parsing::{load_course, JsonCourseVersion, ParsingError};

use self::{
    v1::{RunnerStateV1, RunnerV1},
    v2::{RunnerStateV2, RunnerV2},
};

pub mod v1;
pub mod v2;

pub enum RunnerVersion {
    V1(RunnerV1),
    V2(RunnerV2),
    Undefined,
}

pub trait Runner {
    /// Advances the [Runner]'s state machine.
    fn run(self) -> Self;

    /// Creates a new [Runner] instance depending on the version specified in
    /// `tests.json`.
    ///
    /// * `path`: path to `tests.json`.
    #[allow(clippy::new_ret_no_self)]
    fn new(path: &str) -> RunnerVersion {
        match load_course(path) {
            Ok(course_version) => match course_version {
                JsonCourseVersion::V1(course) => {
                    let test_count = course
                        .suites
                        .iter()
                        .fold(0, |acc, suite| acc + suite.tests.len());

                    let progress = ProgressBar::new(test_count as u64);

                    let runner = RunnerV1::new(
                        progress,
                        0,
                        RunnerStateV1::Loaded,
                        course,
                    );

                    RunnerVersion::V1(runner)
                }
                JsonCourseVersion::V2(course) => {
                    let test_count =
                        course.stages.iter().fold(0, |acc, stage| {
                            acc + stage.lessons.iter().fold(0, |acc, lesson| {
                                acc + match &lesson.suites {
                                    Some(suites) => {
                                        suites.iter().fold(0, |acc, suite| {
                                            acc + suite.tests.len()
                                        })
                                    }
                                    None => 0,
                                }
                            })
                        });

                    let progress = ProgressBar::new(test_count as u64);

                    let runner = RunnerV2::new(
                        progress,
                        0,
                        RunnerStateV2::Loaded,
                        course,
                    );

                    RunnerVersion::V2(runner)
                }
            },
            Err(e) => {
                let msg = match e {
                    ParsingError::CourseFmtError(msg) => msg,
                    ParsingError::FileOpenError(msg) => msg,
                };
                log::error!("{msg}");

                RunnerVersion::Undefined
            }
        }
    }

    fn is_finished(&self) -> bool;
}

impl Runner for RunnerVersion {
    fn run(self) -> Self {
        match self {
            RunnerVersion::V1(runner) => Self::V1(runner.run()),
            RunnerVersion::V2(runner) => Self::V2(runner.run()),
            RunnerVersion::Undefined => Self::Undefined,
        }
    }

    fn is_finished(&self) -> bool {
        match self {
            RunnerVersion::V1(runner) => runner.is_finished(),
            RunnerVersion::V2(runner) => runner.is_finished(),
            RunnerVersion::Undefined => true,
        }
    }
}

/// Formats tests `stderr` and `stdout` output.
///
/// Format is as follows:
///
/// ```bash
/// ╭─[ output ]
/// │ {output}
/// ╰─[ {msg} ]
/// ```
///
/// * `output`: test output.
/// * `msg`: custom message to display after the output.
fn format_output(output: &str, msg: &str) -> String {
    let output = output.replace("\n", "\n    │");
    format!("    ╭─[ output ]{output}\n    ╰─[ {msg} ]")
}

fn format_spinner(progress: &ProgressBar) {
    progress.set_style(
        ProgressStyle::with_template("\n{spinner} {msg} {elapsed_precise}")
            .unwrap(),
    );
    progress.enable_steady_tick(Duration::from_millis(50));
}

fn format_bar(progress: &ProgressBar) {
    progress.set_style(
        ProgressStyle::with_template("{wide_bar} {message} {elapsed_precise}")
            .unwrap(),
    );
}

fn submodule_name(stdout: &str) -> String {
    let regex = Regex::new(r"-[abcdef0123456789]* (?<submodule>\w*)").unwrap();
    let capture = regex.captures(stdout).map(|c| c["submodule"].to_string());

    // extracts the submodule name
    match capture {
        Some(submodule) => submodule,
        None => "".to_string(),
    }
}
