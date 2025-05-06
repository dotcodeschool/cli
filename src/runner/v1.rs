use std::net::TcpStream;

use indicatif::ProgressBar;
use parity_scale_codec::{Decode, Encode};
use reqwest::{blocking::Client, StatusCode};
use thiserror::Error;
use tungstenite::{stream::MaybeTlsStream, Message, WebSocket};

use crate::{
    db::{PathLink, TestState, ValidationState},
    models::TestLogEntry,
    monitor::StateMachine,
    parsing::{
        v1::redis::{RedisTestResultV1, RedisTestState},
        TestResult,
    },
};

use super::{format_bar, format_output};

use colored::Colorize;

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
// {
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
// part of a test suite.
/// * `progress`: number of tests left to run.
/// * `course`: deserialized course information.
pub struct RunnerV1 {
    progress: ProgressBar,
    target: String,
    tree: sled::Tree,
    client: WebSocket<MaybeTlsStream<TcpStream>>,
    tests: Vec<(sled::IVec, TestState)>,
    success: u32,
    state: RunnerStateV1,
    on_pass: Box<dyn Fn()>,
    on_fail: Box<dyn Fn(usize)>,
    on_finish: Box<dyn Fn()>,
}

#[derive(Eq, PartialEq, Clone)]
pub enum RunnerStateV1 {
    Loaded,
    NewTest { index_test: usize },
    Fail { index_test: usize, err: String },
    Pass,
    Finish,
}

impl StateMachine for RunnerV1 {
    fn run(self) -> Self {
        let Self {
            progress,
            tree,
            ref target,
            mut client,
            tests,
            success,
            state,
            on_pass,
            on_fail,
            on_finish,
        } = self;

        match state {
            // Genesis state, displays information about the course and the
            // number of exercises left.
            RunnerStateV1::Loaded => {
                progress.println(format!(
                    "\n📒 You have {} exercises left",
                    tests.len().to_string().bold()
                ));

                format_bar(&progress);

                if tests.is_empty() {
                    Self {
                        progress,
                        tree,
                        target: target.to_string(),
                        client,
                        tests,
                        success,
                        state: RunnerStateV1::Fail {
                            index_test: 0,
                            err: "🚫 no tests found".to_string(),
                        },
                        on_pass,
                        on_fail,
                        on_finish,
                    }
                } else {
                    Self {
                        progress,
                        tree,
                        target: target.to_string(),
                        client,
                        tests,
                        success,
                        state: RunnerStateV1::NewTest { index_test: 0 },
                        on_pass,
                        on_fail,
                        on_finish,
                    }
                }
            }
            // Runs the current test. This state is responsible for exiting
            // into a Failed state in case a mandatory test
            // does not pass.
            RunnerStateV1::NewTest { index_test } => {
                progress.println(format!("{}", &tests[index_test].1));

                progress.inc(1);

                // Testing happens HERE
                let success_inc = match &tests[index_test].1.run(&target) {
                    TestResult::Pass(stdout) => {
                        let query = tree
                            .update_and_fetch(&tests[index_test].0, test_pass);

                        if query.is_err() || matches!(query, Ok(None)) {
                            let state = RunnerStateV1::Fail {
                                index_test,
                                err: format!(
                                    "failed to update test {}",
                                    tests[index_test].1.name
                                ),
                            };

                            return Self {
                                progress,
                                tree,
                                target: target.to_string(),
                                client,
                                tests,
                                success,
                                state,
                                on_pass,
                                on_fail,
                                on_finish,
                            };
                        }

                        let output = format_output(
                            stdout,
                            &format!(
                                "✅ {}",
                                tests[index_test].1.message_on_success
                            ),
                        );

                        let test_result = RedisTestResultV1::pass(
                            &tests[index_test].1.slug,
                            &output,
                        );

                        if let Err(e) = json_report_test(
                            test_result,
                            &mut client,
                            &tests[index_test].1,
                            &self.target,
                        ) {
                            return Self {
                                progress,
                                tree,
                                target: target.to_string(),
                                client,
                                tests,
                                success,
                                state: RunnerStateV1::Fail {
                                    index_test,
                                    err: e.to_string(),
                                },
                                on_pass,
                                on_fail,
                                on_finish,
                            };
                        }

                        progress.println(output);

                        1
                    }
                    TestResult::Fail(stderr) => {
                        let query = tree
                            .update_and_fetch(&tests[index_test].0, test_fail);

                        if query.is_err() || matches!(query, Ok(None)) {
                            let state = RunnerStateV1::Fail {
                                index_test,
                                err: format!(
                                    "failed to update test {}",
                                    tests[index_test].1.name
                                ),
                            };

                            return Self {
                                progress,
                                tree,
                                target: target.to_string(),
                                client,
                                tests,
                                success,
                                state,
                                on_pass,
                                on_fail,
                                on_finish,
                            };
                        }

                        let output = format_output(
                            stderr,
                            &format!(
                                "❌ {}",
                                tests[index_test].1.message_on_fail
                            ),
                        )
                        .red()
                        .dimmed()
                        .to_string();

                        let test_result = RedisTestResultV1::fail(
                            &tests[index_test].1.slug,
                            &output,
                            tests[index_test].1.optional,
                        );

                        if let Err(e) = json_report_test(
                            test_result,
                            &mut client,
                            &tests[index_test].1,
                            &self.target,
                        ) {
                            return Self {
                                progress,
                                tree,
                                target: target.to_string(),
                                client,
                                tests,
                                success,
                                state: RunnerStateV1::Fail {
                                    index_test,
                                    err: e.to_string(),
                                },
                                on_pass,
                                on_fail,
                                on_finish,
                            };
                        }

                        progress.println(output);

                        if !tests[index_test].1.optional {
                            let state = RunnerStateV1::Fail {
                                index_test,
                                err: format!(
                                    "Test {}:{} failed",
                                    index_test, &tests[index_test].1.name
                                ),
                            };

                            return Self {
                                progress,
                                tree,
                                target: target.to_string(),
                                client,
                                tests,
                                success,
                                state,
                                on_pass,
                                on_fail,
                                on_finish,
                            };
                        }

                        0
                    }
                };

                // Moves on to the next test or marks the tests as Passed
                if index_test + 1 < tests.len() {
                    Self {
                        progress,
                        tree,
                        target: target.to_string(),
                        client,
                        tests,
                        success: success + success_inc,
                        state: RunnerStateV1::NewTest {
                            index_test: index_test + 1,
                        },
                        on_pass,
                        on_fail,
                        on_finish,
                    }
                } else {
                    Self {
                        progress,
                        tree,
                        target: target.to_string(),
                        client,
                        tests,
                        success: success + success_inc,
                        state: RunnerStateV1::Pass,
                        on_pass,
                        on_fail,
                        on_finish,
                    }
                }
            }
            // A mandatory test failed. Displays a custom error message as
            // defined in the `message_on_fail` field of a
            // Test JSON object. This state can also be used for general
            // error logging.
            RunnerStateV1::Fail { index_test, err } => {
                progress.finish_and_clear();
                progress.println(format!("\n⚠ Error: {}", err.red().bold()));

                on_fail(index_test);
                on_finish();

                if json_report_are_tests_passing(false, &mut client).is_err() {
                    progress.println(
                        "🚫 Failed to send test results to DotCodeSchool"
                            .red()
                            .bold()
                            .to_string(),
                    );
                }

                if json_report_close(&mut client).is_err() {
                    progress.println(
                        "🚫 Failed to close Websocket connection to DotCodeSchool".red().bold().to_string()
                    );
                }

                Self {
                    progress,
                    tree,
                    target: target.to_string(),
                    client,
                    tests,
                    success,
                    state: RunnerStateV1::Finish,
                    on_pass,
                    on_fail,
                    on_finish,
                }
            }
            // ALL mandatory tests passed. Displays the success rate across
            // all tests. It is not important how low that
            // rate is, as long as all mandatory tests pass,
            // and simply serves as an indication of progress for the
            // student.
            RunnerStateV1::Pass => {
                progress.finish_and_clear();
                let score = format!(
                    "{:.2}",
                    success as f64 / tests.len() as f64 * 100f64
                );

                progress.println(format!(
                    "\n🏁 final score: {}%",
                    score.green().bold()
                ));

                on_pass();
                on_finish();

                if json_report_are_tests_passing(true, &mut client).is_err() {
                    progress.println(
                        "🚫 Failed to send test results to DotCodeSchool"
                            .red()
                            .bold()
                            .to_string(),
                    );
                }

                if json_report_close(&mut client).is_err() {
                    progress.println(
                        "🚫 Failed to close Websocket connection to DotCodeSchool".red().bold().to_string()
                    );
                }

                Self {
                    progress,
                    tree,
                    target: target.to_string(),
                    client,
                    tests,
                    success,
                    state: RunnerStateV1::Finish,
                    on_pass,
                    on_fail,
                    on_finish,
                }
            }
            // Exit state, does nothing when called.
            RunnerStateV1::Finish => Self {
                progress,
                tree,
                target: target.to_string(),
                client,
                tests,
                success,
                state: RunnerStateV1::Finish,
                on_pass,
                on_fail,
                on_finish,
            },
        }
    }

    fn is_finished(&self) -> bool {
        self.state == RunnerStateV1::Finish
    }
}

fn test_pass(old: Option<&[u8]>) -> Option<Vec<u8>> {
    let bytes = old?;
    let mut test = TestState::decode(&mut &bytes[..]).ok()?;

    test.passed = ValidationState::Pass;

    Some(test.encode())
}

fn test_fail(old: Option<&[u8]>) -> Option<Vec<u8>> {
    let bytes = old?;
    let mut test = TestState::decode(&mut &bytes[..]).ok()?;

    test.passed = ValidationState::Fail;

    Some(test.encode())
}

#[derive(Error, Debug)]
enum RedisReportError {
    #[error("failed to convert test result to JSON: {0}")]
    JsonError(String),
    #[error("failed to send report via websocket: {0}")]
    WsError(String),
}

fn json_report_test(
    result: RedisTestResultV1,
    client: &mut WebSocket<MaybeTlsStream<TcpStream>>,
    test: &TestState,
    repo_name: &String,
) -> Result<(), RedisReportError> {
    #[cfg(debug_assertions)]
    let json = serde_json::to_string_pretty(&result)
        .map_err(|err| RedisReportError::JsonError(err.to_string()))?;

    #[cfg(not(debug_assertions))]
    let json = serde_json::to_string(&result)
        .map_err(|err| RedisReportError::JsonError(err.to_string()))?;

    log::debug!("Test result: {json}");

    #[cfg(debug_assertions)]
    let message = format!(
        concat!(
            "{{\n",
            "  \"event_type\":",
            "  \"log\",\n",
            "  \"bytes\":",
            "  \"{:?}\"\n",
            "}}"
        ),
        json.as_bytes()
    );

    #[cfg(not(debug_assertions))]
    let message = format!(
        concat!(
            "{{",
            "\"event_type\":",
            "\"log\",",
            "\"bytes\":",
            "\"{:?}\"",
            "}}"
        ),
        json.as_bytes()
    );

    log::debug!("Sending message to redis: {message}");

    client
        .send(Message::Text(message))
        .map_err(|err| RedisReportError::WsError(err.to_string()))?;

    log::debug!("Message sent successfully");

    // Get path info from test state
    let [section_link, lesson_link, _, _] = &test.path[..] else {
        return Ok(());
    };

    // Extract section and lesson names
    let section_name = match section_link {
        PathLink::Link(name) | PathLink::LinkOptional(name) => name.clone(),
    };

    let lesson_name = match lesson_link {
        PathLink::Link(name) | PathLink::LinkOptional(name) => name.clone(),
    };

    // Use the lesson slug directly from the test state
    // This is the slug defined in the tester-definition.yml file
    let lesson_slug = test.lesson_slug.clone();

    let test_log = TestLogEntry {
        test_slug: test.slug.clone(),
        passed: matches!(result.state, RedisTestState::Passed),
        timestamp: chrono::Utc::now(),
        section_name,
        lesson_name,
        lesson_slug,
        test_name: test.name.clone(),
        repo_name: repo_name.clone(),
    };

    // TODO: Send log entry to MongoDB using the backend endpoint
    // BACKEND_URL/test-log
    let url = format!("{}/test-log", crate::constants::BACKEND_URL);
    match Client::new().post(&url).json(&test_log).send() {
        Ok(response) => {
            if response.status() == StatusCode::OK {
                log::info!("Test log entry sent successfully");
            } else {
                log::error!(
                    "Failed to send test log entry: {}",
                    response.status()
                );
            }
        }
        Err(err) => {
            log::error!("Failed to send test log entry: {}", err);
        }
    }

    Ok(())
}

fn json_report_are_tests_passing(
    status: bool,
    client: &mut WebSocket<MaybeTlsStream<TcpStream>>,
) -> Result<(), RedisReportError> {
    #[cfg(debug_assertions)]
    let message = format!(
        concat!(
            "{{\n",
            "  \"event_type\":\n",
            "  \"status\",\n",
            "  \"success\": {}\n",
            "}}"
        ),
        status
    );

    #[cfg(not(debug_assertions))]
    let message = format!(
        concat!(
            "{{",
            "\"event_type\":",
            "\"status\",",
            "\"success\": {}",
            "}}"
        ),
        status
    );

    log::debug!("Sending message to redis: {message}");

    client
        .send(Message::Text(message))
        .map_err(|err| RedisReportError::WsError(err.to_string()))?;

    log::debug!("Message sent successfully");

    Ok(())
}

fn json_report_close(
    client: &mut WebSocket<MaybeTlsStream<TcpStream>>,
) -> Result<(), RedisReportError> {
    log::debug!("Closing websocket connection");

    #[cfg(debug_assertions)]
    let message =
        concat!("{\n", "  \"event_type\":", "  \"disconnect\"\n", "}")
            .to_string();

    #[cfg(not(debug_assertions))]
    let message =
        concat!("{", "\"event_type\":", "\"disconnect\"", "}").to_string();

    log::debug!("Sending message to redis: {message}");

    client
        .send(Message::Text(message))
        .map_err(|err| RedisReportError::WsError(err.to_string()))?;

    log::debug!("Websocket connection closed successfully");

    Ok(())
}

pub struct RunnerV1Builder<A, B, C, D, E> {
    progress: A,
    target: B,
    tree: C,
    client: D,
    tests: E,
    success: u32,
    state: RunnerStateV1,
    on_pass: Box<dyn Fn()>,
    on_fail: Box<dyn Fn(usize)>,
    on_finish: Box<dyn Fn()>,
}

impl RunnerV1Builder<(), (), (), (), ()> {
    pub fn new() -> Self {
        RunnerV1Builder {
            progress: (),
            target: (),
            tree: (),
            client: (),
            tests: (),
            success: 0,
            state: RunnerStateV1::Loaded,
            on_pass: Box::new(|| {}),
            on_fail: Box::new(|_| {}),
            on_finish: Box::new(|| {}),
        }
    }
}

#[allow(dead_code)]
impl<A, B, C, D, E> RunnerV1Builder<A, B, C, D, E> {
    pub fn progress(
        self,
        progress: ProgressBar,
    ) -> RunnerV1Builder<ProgressBar, B, C, D, E> {
        RunnerV1Builder {
            progress,
            target: self.target,
            tree: self.tree,
            client: self.client,
            tests: self.tests,
            success: self.success,
            state: self.state,
            on_pass: self.on_pass,
            on_fail: self.on_fail,
            on_finish: self.on_finish,
        }
    }

    pub fn target(self, target: String) -> RunnerV1Builder<A, String, C, D, E> {
        RunnerV1Builder {
            progress: self.progress,
            target,
            tree: self.tree,
            client: self.client,
            tests: self.tests,
            success: self.success,
            state: self.state,
            on_pass: self.on_pass,
            on_fail: self.on_fail,
            on_finish: self.on_finish,
        }
    }

    pub fn tree(
        self,
        tree: sled::Tree,
    ) -> RunnerV1Builder<A, B, sled::Tree, D, E> {
        RunnerV1Builder {
            progress: self.progress,
            target: self.target,
            tree,
            client: self.client,
            tests: self.tests,
            success: self.success,
            state: self.state,
            on_pass: self.on_pass,
            on_fail: self.on_fail,
            on_finish: self.on_finish,
        }
    }

    pub fn client(
        self,
        client: WebSocket<MaybeTlsStream<TcpStream>>,
    ) -> RunnerV1Builder<A, B, C, WebSocket<MaybeTlsStream<TcpStream>>, E> {
        RunnerV1Builder {
            progress: self.progress,
            target: self.target,
            tree: self.tree,
            client,
            tests: self.tests,
            success: self.success,
            state: self.state,
            on_pass: self.on_pass,
            on_fail: self.on_fail,
            on_finish: self.on_finish,
        }
    }

    pub fn tests(
        self,
        tests: Vec<(sled::IVec, TestState)>,
    ) -> RunnerV1Builder<A, B, C, D, Vec<(sled::IVec, TestState)>> {
        RunnerV1Builder {
            progress: self.progress,
            target: self.target,
            tree: self.tree,
            client: self.client,
            tests,
            success: self.success,
            state: self.state,
            on_pass: self.on_pass,
            on_fail: self.on_fail,
            on_finish: self.on_finish,
        }
    }

    pub fn on_pass<F1>(mut self, f: F1) -> RunnerV1Builder<A, B, C, D, E>
    where
        F1: Fn() + 'static,
    {
        self.on_pass = Box::new(f);
        self
    }

    pub fn on_fail<F2>(mut self, f: F2) -> RunnerV1Builder<A, B, C, D, E>
    where
        F2: Fn(usize) + 'static,
    {
        self.on_fail = Box::new(f);
        self
    }

    pub fn on_finish<F3>(mut self, f: F3) -> RunnerV1Builder<A, B, C, D, E>
    where
        F3: Fn() + 'static,
    {
        self.on_finish = Box::new(f);
        self
    }
}

impl
    RunnerV1Builder<
        ProgressBar,
        String,
        sled::Tree,
        WebSocket<MaybeTlsStream<TcpStream>>,
        Vec<(sled::IVec, TestState)>,
    >
{
    pub fn build(self) -> RunnerV1 {
        RunnerV1 {
            progress: self.progress,
            target: self.target,
            tree: self.tree,
            client: self.client,
            tests: self.tests,
            success: self.success,
            state: self.state,
            on_pass: self.on_pass,
            on_fail: self.on_fail,
            on_finish: self.on_finish,
        }
    }
}
