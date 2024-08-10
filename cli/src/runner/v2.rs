use std::ops::Deref;

use indicatif::ProgressBar;
use itertools::{FoldWhile, Itertools};

use crate::{
    parsing::{v2::JsonCourseV2, Test, TestResult},
    str_res::{DOTCODESCHOOL, OPTIONAL},
};

use super::{
    format_bar, format_output, format_spinner, submodule_name, Runner,
};

use colored::Colorize;
use derive_more::Constructor;

pub const TEST_DIR: &str = "./course";

/// Runs all the tests specified in a `tests.json` file.
///
/// Tests are run sequentially in their order of definition. Running tests
/// occurs in 3 steps:
///
/// 1. Loading the `tests.json` file.
/// 2. Executing tests one by one, displaying `stderr` and `stdout` as
///    appropriate.
/// 3. Test stop running once all test have been run or a mandatory test fails.
/// 4. A summary of the run is displayed at the end of the process.
///
/// # `tests.json` file format
///
/// ## Version 1.0
///
/// Capabilities are divided into 3 parts:
///
/// - Course definition.
/// - Test suite definition
/// - Test definition
///
/// ### Course definition
///
/// ```json
/// {
///     "version": "1.0",
///     "course": "Course name",
///     "instructor": "Instructor name",
///     "course_id": 123,
///     "suites": [
///         ...
///     ]
/// }
/// ```
//
/// Course Id will be checked against the DotCodeScool servers to make sure that
/// the tests are being run in the correct git repository.
///
/// ### Suite definition
///
/// ```json
/// {
///     "name": "Suite name",
///     "optional": false,
///     "tests": [
///         ...
///     ]
/// }
/// ```
///
/// Test suites marked as optional do not need to be passed for the course to be
/// validated. They will however still count towards the overall success of the
/// course, so if a student passes 9 mandatory test suites but fails 1 optional
/// test suite, their overall score will still be 90%.
///
/// ### Test definition
// ```json
/// {
///     "name": "Test name",
///     "optional": false,
///     "cmd": "cargo test test_name",
///     "message_on_fail": "This test failed, back to the drawing board.",
///     "message_on_success": "This test passed, congrats!"
/// }
/// ```
/// 
/// `cmd` defines which command to run for the test to execute. Like test
/// suites, tests can be marked as `optional`. `optional` tests will still count
/// towards the overall success of the course but do not need to be validated as
/// part of a test suite.
///
/// * `progress`: number of tests left to run.
/// * `course`: deserialized course information.
#[derive(Constructor)]
pub struct RunnerV2 {
    progress: ProgressBar,
    success: u32,
    pub state: RunnerStateV2,
    course: JsonCourseV2,
}

#[derive(Eq, PartialEq, Clone)]
pub enum RunnerStateV2 {
    Loaded,
    Update,
    NewSuite {
        index_stage: usize,
        index_lesson: usize,
        index_suite: usize,
    },
    NewTest {
        index_stage: usize,
        index_lesson: usize,
        index_suite: usize,
        index_test: usize,
    },
    Fail(String),
    Pass,
    Finish,
}

impl Runner for RunnerV2 {
    fn run(self) -> Self {
        let Self { progress, mut success, state, course } = self;

        match state {
            // Genesis state, displays information about the course and the
            // number of exercises left.
            RunnerStateV2::Loaded => {
                progress.println(DOTCODESCHOOL.clone());

                progress.println(format!(
                    "\n🎓 {} by {}",
                    course.name.to_uppercase().white().bold(),
                    course.author.name.white().bold()
                ));

                let exercise_count =
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
                progress.println(format!(
                    "\n📒 You have {} exercises left",
                    exercise_count.to_string().bold()
                ));
                Self { progress, success, state: RunnerStateV2::Update, course }
            }
            // Initializes all submodules and checks for tests updates. This
            // happens if the `TEST_DIR` submodule is out of date,
            // in which case it will be pulled. A new commit is then
            // created which contains the submodule update.
            RunnerStateV2::Update => {
                format_spinner(&progress);

                let output = std::process::Command::new("git")
                    .arg("submodule")
                    .arg("status")
                    .output();

                // Auto-initializes submodules
                if let Ok(output) = output {
                    let stdout = String::from_utf8(output.stdout).unwrap();
                    let lines = stdout.split("\n");

                    for line in lines {
                        let submodule = submodule_name(&stdout);

                        if line.starts_with("-") {
                            progress.set_message(
                                "Downloading course"
                                    .italic()
                                    .dimmed()
                                    .to_string(),
                            );

                            let _ = std::process::Command::new("git")
                                .arg("submodule")
                                .arg("update")
                                .arg("--init")
                                .arg(&submodule)
                                .output();
                        }
                    }
                } else {
                    progress.println(
                        "\n🔄 Failed to check for updates"
                            .white()
                            .dimmed()
                            .italic()
                            .to_string(),
                    );
                }

                // Checks for updates
                progress.set_message(
                    "Checking for updates".italic().dimmed().to_string(),
                );

                let _ = std::process::Command::new("git")
                    .arg("fetch")
                    .current_dir(TEST_DIR)
                    .output();
                let output = std::process::Command::new("git")
                    .arg("status")
                    .current_dir(TEST_DIR)
                    .output();

                // Applies updates
                if let Ok(output) = output {
                    let stdout = String::from_utf8(output.stdout).unwrap();

                    if stdout.contains("Your branch is behind") {
                        progress.set_message(
                            "Updating course".italic().dimmed().to_string(),
                        );

                        let _ = std::process::Command::new("git")
                            .arg("pull")
                            .current_dir(TEST_DIR)
                            .output();

                        let _ = std::process::Command::new("git")
                            .arg("add")
                            .arg(TEST_DIR)
                            .output();

                        let _ = std::process::Command::new("git")
                            .arg("commit")
                            .arg("-m")
                            .arg("🧪 Updated course")
                            .output();

                        progress.println("\n📝 Updated course");
                    }
                } else {
                    progress.println(
                        "\n🔄 Failed to check for updates"
                            .white()
                            .dimmed()
                            .italic()
                            .to_string(),
                    );
                }

                format_bar(&progress);

                match follow_path(&course, [0, 0]) {
                    Some([index_stage, index_lesson]) => Self {
                        progress,
                        success,
                        state: RunnerStateV2::NewSuite {
                            index_stage,
                            index_lesson,
                            index_suite: 0,
                        },
                        course,
                    },
                    None => Self {
                        progress,
                        success,
                        state: RunnerStateV2::Pass,
                        course,
                    },
                }
            }
            // Displays the name of the current suite
            RunnerStateV2::NewSuite {
                index_stage,
                index_lesson,
                index_suite,
            } => {
                let stage = &course.stages[index_stage];
                let lesson = &stage.lessons[index_lesson];
                let suite =
                    &lesson.suites.as_ref().expect(
                        "Runner should have detected a non-optioned suite",
                    )[index_suite];

                let stage_name =
                    stage.name.deref().to_uppercase().bold().green();
                let lesson_name =
                    lesson.name.deref().to_uppercase().bold().green();
                let suite_name =
                    suite.name.deref().to_uppercase().bold().green();

                progress.println(format!(
                    "\n{stage_name}\n╰─{lesson_name}\n  ╰─{suite_name} {}",
                    if suite.optional { &OPTIONAL } else { "" },
                ));

                Self {
                    progress,
                    success,
                    state: RunnerStateV2::NewTest {
                        index_stage,
                        index_lesson,
                        index_suite,
                        index_test: 0,
                    },
                    course,
                }
            }
            // Runs the current test. This state is responsible for exiting
            // into a Failed state in case a mandatory test
            // does not pass.
            RunnerStateV2::NewTest {
                index_stage,
                index_lesson,
                index_suite,
                index_test,
            } => {
                let stage = &course.stages[index_stage];
                let lesson = &stage.lessons[index_lesson];
                let suites = lesson
                    .suites
                    .as_ref()
                    .expect("Runner should have detected a non-optioned suite");
                let suite = &suites[index_suite];
                let test = &suite.tests[index_test];
                let test_name = test.name.to_lowercase().bold();

                progress.println(format!(
                    "\n  🧪 Running test {test_name} {}",
                    if test.optional { &OPTIONAL } else { "" },
                ));

                progress.inc(1);

                // Testing happens HERE
                match test.run() {
                    TestResult::Pass(stdout) => {
                        progress.println(format_output(
                            &stdout,
                            &format!("✅ {}", &test.message_on_success),
                        ));

                        success += 1;
                    }
                    TestResult::Fail(stderr) => {
                        progress.println(
                            format_output(
                                &stderr,
                                &format!("❌ {}", &test.message_on_fail),
                            )
                            .red()
                            .dimmed()
                            .to_string(),
                        );

                        if !test.optional && !suite.optional {
                            return Self {
                                progress,
                                success,
                                state: RunnerStateV2::Fail(format!(
                                    "Failed test {test_name}"
                                )),
                                course,
                            };
                        }
                    }
                };

                // Moves on to the next text, the next suite, or marks the
                // tests as Passed
                match (
                    index_stage + 1 < course.stages.len(),
                    index_suite + 1 < suites.len(),
                    index_test + 1 < suite.tests.len(),
                ) {
                    (_, _, true) => Self {
                        progress,
                        success,
                        state: RunnerStateV2::NewTest {
                            index_stage,
                            index_lesson,
                            index_suite,
                            index_test: index_test + 1,
                        },
                        course,
                    },
                    (_, true, false) => Self {
                        progress,
                        success,
                        state: RunnerStateV2::NewSuite {
                            index_stage,
                            index_lesson,
                            index_suite: index_suite + 1,
                        },
                        course,
                    },
                    (true, false, false) => {
                        match follow_path(
                            &course,
                            [index_stage, index_lesson + 1],
                        ) {
                            Some([index_stage, index_lesson]) => Self {
                                progress,
                                success,
                                state: RunnerStateV2::NewSuite {
                                    index_stage,
                                    index_lesson,
                                    index_suite: 0,
                                },
                                course,
                            },
                            None => Self {
                                progress,
                                success,
                                state: RunnerStateV2::Pass,
                                course,
                            },
                        }
                    }
                    (false, false, false) => Self {
                        progress,
                        success,
                        state: RunnerStateV2::Pass,
                        course,
                    },
                }
            }
            // A mandatory test failed. Displays a custom error message as
            // defined in the `message_on_fail` field of a
            // Test JSON object. This state can also be used for general
            // error logging.
            RunnerStateV2::Fail(msg) => {
                progress.finish_and_clear();
                progress.println(format!("\n⚠ Error: {}", msg.red().bold()));

                Self { progress, success, state: RunnerStateV2::Finish, course }
            }
            // ALL mandatory tests passed. Displays the success rate across
            // all tests. It is not important how low that
            // rate is, as long as all mandatory tests pass,
            // and simply serves as an indication of progress for the
            // student.
            RunnerStateV2::Pass => {
                progress.finish_and_clear();
                let exercise_count =
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
                let score = format!(
                    "{:.2}",
                    success as f64 / exercise_count as f64 * 100f64
                );

                progress.println(format!(
                    "\n🏁 final score: {}%",
                    score.green().bold()
                ));

                Self { progress, success, state: RunnerStateV2::Finish, course }
            }
            // Exit state, does nothing when called.
            RunnerStateV2::Finish => {
                Self { progress, success, state: RunnerStateV2::Finish, course }
            }
        }
    }

    fn is_finished(&self) -> bool {
        self.state == RunnerStateV2::Finish
    }
}

fn follow_path(course: &JsonCourseV2, path: [usize; 2]) -> Option<[usize; 2]> {
    let [index_stage, index_lesson] = course
        .stages
        .iter()
        .skip(path[0])
        .enumerate()
        .fold_while([0, 0], |mut acc, (i_stage, stage)| {
            acc[0] = i_stage;
            stage.lessons.iter().skip(path[1]).enumerate().fold_while(
                acc,
                |mut acc, (i_lesson, lesson)| {
                    acc[1] = i_lesson;
                    match &lesson.suites {
                        Some(_) => FoldWhile::Done(acc),
                        None => FoldWhile::Continue(acc),
                    }
                },
            )
        })
        .into_inner();

    let stage = &course.stages[index_stage];
    let lesson = &stage.lessons[index_lesson];

    lesson.suites.as_ref().map(|_| [index_stage, index_lesson])
}
