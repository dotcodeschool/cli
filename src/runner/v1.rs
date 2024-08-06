use std::{ops::Deref, thread, time::Duration};

use indicatif::ProgressBar;

use crate::parsing::{v1::JsonCourseV1, Test, TestResult};

use super::{
    format_bar, format_output, format_spinner, submodule_name, Runner,
    TestRunnerState, DOTCODESCHOOL, OPTIONAL,
};

use colored::Colorize;
use derive_more::Constructor;

pub const TEST_DIR: &str = "./tests";

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
///
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
pub struct TestRunnerV1 {
    progress: ProgressBar,
    success: u32,
    pub state: TestRunnerState,
    course: JsonCourseV1,
}

impl Runner for TestRunnerV1 {
    fn run(self) -> Self {
        let Self { progress, mut success, state, course } = self;

        match state {
            // Genesis state, displays information about the course and the
            // number of exercises left.
            TestRunnerState::Loaded => {
                progress.println(DOTCODESCHOOL.clone());

                progress.println(format!(
                    "\n🎓 {} by {}",
                    course.name.to_uppercase().white().bold(),
                    course.instructor.white().bold()
                ));

                let exercise_count = course
                    .suites
                    .iter()
                    .fold(0, |acc, suite| acc + suite.tests.len());
                progress.println(format!(
                    "\n📒 You have {} exercises left",
                    exercise_count.to_string().bold()
                ));
                Self {
                    progress,
                    success,
                    state: TestRunnerState::Update,
                    course,
                }
            }
            // Initializes all submodules and checks for tests updates. This
            // happens if the `TEST_DIR` submodule is out of date,
            // in which case it will be pulled. A new commit is then
            // created which contains the submodule update.
            TestRunnerState::Update => {
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
                                "Downloading tests"
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
                    progress.println("⚠ Failed to check for updates");
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
                            "Updating tests".italic().dimmed().to_string(),
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
                            .arg("🧪 Updated tests")
                            .output();

                        progress.println("\n📝 Updated tests");
                    }
                } else {
                    progress.println("⚠ Failed to check for updates");
                }

                format_bar(&progress);
                Self {
                    progress,
                    success,
                    state: TestRunnerState::NewSuite(0),
                    course,
                }
            }
            // Displays the name of the current suite
            TestRunnerState::NewSuite(index_suite) => {
                let suite = &course.suites[index_suite];
                let suite_name =
                    suite.name.deref().to_uppercase().bold().green();

                progress.println(format!(
                    "\n{suite_name} {}",
                    if suite.optional { &OPTIONAL } else { "" },
                ));

                Self {
                    progress,
                    success,
                    state: TestRunnerState::NewTest(index_suite, 0),
                    course,
                }
            }
            // Runs the current test. This state is responsible for exiting
            // into a Failed state in case a mandatory test
            // does not pass.
            TestRunnerState::NewTest(index_suite, index_test) => {
                let suite = &course.suites[index_suite];
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
                                state: TestRunnerState::Failed(format!(
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
                    index_suite + 1 < course.suites.len(),
                    index_test + 1 < suite.tests.len(),
                ) {
                    (_, true) => Self {
                        progress,
                        success,
                        state: TestRunnerState::NewTest(
                            index_suite,
                            index_test + 1,
                        ),
                        course,
                    },
                    (true, false) => Self {
                        progress,
                        success,
                        state: TestRunnerState::NewSuite(index_suite + 1),
                        course,
                    },
                    (false, false) => Self {
                        progress,
                        success,
                        state: TestRunnerState::Passed,
                        course,
                    },
                }
            }
            // A mandatory test failed. Displays a custom error message as
            // defined in the `message_on_fail` field of a
            // Test JSON object. This state can also be used for general
            // error logging.
            TestRunnerState::Failed(msg) => {
                progress.finish_and_clear();
                progress.println(format!("\n⚠ Error: {}", msg.red().bold()));

                Self {
                    progress,
                    success,
                    state: TestRunnerState::Finish,
                    course,
                }
            }
            // ALL mandatory tests passed. Displays the success rate across
            // all tests. It is not important how low that
            // rate is, as long as all mandatory tests pass,
            // and simply serves as an indication of progress for the
            // student.
            TestRunnerState::Passed => {
                progress.finish_and_clear();
                let exercise_count = course
                    .suites
                    .iter()
                    .fold(0, |acc, suite| acc + suite.tests.len());
                let score = format!(
                    "{:.2}",
                    success as f64 / exercise_count as f64 * 100f64
                );

                progress.println(format!(
                    "\n🏁 final score: {}%",
                    score.green().bold()
                ));

                Self {
                    progress,
                    success,
                    state: TestRunnerState::Finish,
                    course,
                }
            }
            // Exit state, does nothing when called.
            TestRunnerState::Finish => Self {
                progress,
                success,
                state: TestRunnerState::Finish,
                course,
            },
        }
    }

    fn state(&self) -> TestRunnerState {
        self.state.clone()
    }
}
