use std::net::TcpStream;

use indicatif::ProgressBar;
use parity_scale_codec::{Decode, Encode};
use tungstenite::{stream::MaybeTlsStream, Message, WebSocket};

use crate::{
    db::{TestState, ValidationState},
    monitor::StateMachine,
    parsing::{
        v1::redis::{RedisCourseResultV1, RedisTestResultV1},
        TestResult,
    },
};

use super::{format_bar, format_output, format_spinner};

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
    redis_results: RedisCourseResultV1,
    success: u32,
    state: RunnerStateV1,
    on_pass: Box<dyn Fn()>,
    on_fail: Box<dyn Fn()>,
    on_finish: Box<dyn Fn()>,
}

#[derive(Eq, PartialEq, Clone)]
pub enum RunnerStateV1 {
    Loaded,
    NewTest { index_test: usize },
    Fail(String),
    Pass,
    Redis,
    Finish,
}

impl StateMachine for RunnerV1 {
    fn run(self) -> Self {
        let Self {
            progress,
            tree,
            target,
            mut client,
            tests,
            mut redis_results,
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
                        target,
                        client,
                        tests,
                        redis_results,
                        success,
                        state: RunnerStateV1::Fail(
                            "🚫 no tests found".to_string(),
                        ),
                        on_pass,
                        on_fail,
                        on_finish,
                    }
                } else {
                    Self {
                        progress,
                        tree,
                        target,
                        client,
                        tests,
                        redis_results,
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
                            let state = RunnerStateV1::Fail(format!(
                                "failed to update test {}",
                                tests[index_test].1.name
                            ));

                            return Self {
                                progress,
                                tree,
                                target,
                                client,
                                tests,
                                redis_results,
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

                        redis_results.log_test(RedisTestResultV1::pass(
                            &tests[index_test].1.slug,
                            &output,
                        ));

                        progress.println(output);

                        1
                    }
                    TestResult::Fail(stderr) => {
                        let query = tree
                            .update_and_fetch(&tests[index_test].0, test_fail);

                        if query.is_err() || matches!(query, Ok(None)) {
                            let state = RunnerStateV1::Fail(format!(
                                "failed to update test {}",
                                tests[index_test].1.name
                            ));

                            return Self {
                                progress,
                                tree,
                                target,
                                client,
                                tests,
                                redis_results,
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

                        redis_results.log_test(RedisTestResultV1::fail(
                            &tests[index_test].1.slug,
                            &output,
                            tests[index_test].1.optional,
                        ));

                        progress.println(output);

                        if !tests[index_test].1.optional {
                            let state = RunnerStateV1::Fail(format!(
                                "Test {}:{} failed",
                                index_test, &tests[index_test].1.name
                            ));

                            return Self {
                                progress,
                                tree,
                                target,
                                client,
                                tests,
                                redis_results,
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
                        target,
                        client,
                        tests,
                        redis_results,
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
                        target,
                        client,
                        tests,
                        redis_results,
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
            RunnerStateV1::Fail(msg) => {
                progress.finish_and_clear();
                progress.println(format!("\n⚠ Error: {}", msg.red().bold()));

                on_fail();
                on_finish();

                Self {
                    progress,
                    tree,
                    target,
                    client,
                    tests,
                    redis_results,
                    success,
                    state: RunnerStateV1::Redis,
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

                redis_results.pass();

                on_pass();
                on_finish();

                Self {
                    progress,
                    tree,
                    target,
                    client,
                    tests,
                    redis_results,
                    success,
                    state: RunnerStateV1::Redis,
                    on_pass,
                    on_fail,
                    on_finish,
                }
            }
            RunnerStateV1::Redis => {
                log::debug!("Streaming results back to DotCodeSchool");

                format_spinner(&progress);

                progress.set_message(
                    "⏳ Streaming results back to DotCodeSchool"
                        .white()
                        .dimmed()
                        .italic()
                        .to_string(),
                );

                #[cfg(debug_assertions)]
                let Ok(results) = serde_json::to_string_pretty(&redis_results) else {
                    progress.println(
                        "🚫 Failed to convert tests results to JSON"
                            .red()
                            .to_string(),
                    );
                    progress.finish_and_clear();

                    return Self {
                        progress,
                        tree,
                        target,
                        client,
                        tests,
                        redis_results,
                        success,
                        state: RunnerStateV1::Finish,
                        on_pass,
                        on_fail,
                        on_finish,
                    };
                };

                #[cfg(not(debug_assertions))]
                let Ok(results) = serde_json::to_string(&redis_results) else {
                    progress.println(
                        "🚫 Failed to convert tests results to JSON"
                            .red()
                            .to_string(),
                    );
                    progress.finish_and_clear();

                    return Self {
                        progress,
                        tree,
                        target,
                        client,
                        tests,
                        redis_results,
                        success,
                        state: RunnerStateV1::Finish,
                        on_pass,
                        on_fail,
                        on_finish,
                    };
                };

                log::debug!("Test results: {results}");

                let query = client.send(Message::Text(format!(
                    concat!(
                        "{{",
                        "\"event_type:\"",
                        "\"log\",",
                        "\"bytes:\"",
                        "\"{},\"",
                        "}}"
                    ),
                    results
                )));

                if let Err(_) = query {
                    progress.println(
                        "🚫 Failed to stream results back to DotCodeSchool"
                            .red()
                            .to_string(),
                    );
                    progress.finish_and_clear();

                    return Self {
                        progress,
                        tree,
                        target,
                        client,
                        tests,
                        redis_results,
                        success,
                        state: RunnerStateV1::Finish,
                        on_pass,
                        on_fail,
                        on_finish,
                    };
                }

                log::debug!("Closing websocket connection");

                let query = client.send(Message::Text(
                    concat!("{{", "\"event_type:\"", "\"disconnect\"", "}}")
                        .to_string(),
                ));

                if let Err(_) = query {
                    progress.println(
                        "🚫 Failed to close websocket".red().to_string(),
                    );
                    progress.finish_and_clear();

                    return Self {
                        progress,
                        tree,
                        target,
                        client,
                        tests,
                        redis_results,
                        success,
                        state: RunnerStateV1::Finish,
                        on_pass,
                        on_fail,
                        on_finish,
                    };
                }

                log::debug!("Test results streamed back successfully");

                progress.println(
                    "DotCodeSchool updated successfully"
                        .white()
                        .dimmed()
                        .italic()
                        .to_string(),
                );
                progress.finish_and_clear();

                Self {
                    progress,
                    tree,
                    target,
                    client,
                    tests,
                    redis_results,
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
                target,
                client,
                tests,
                redis_results,
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

pub struct RunnerV1Builder<A, B, C, D, E, F> {
    progress: A,
    target: B,
    tree: C,
    client: D,
    tests: E,
    redis_results: F,
    success: u32,
    state: RunnerStateV1,
    on_pass: Box<dyn Fn()>,
    on_fail: Box<dyn Fn()>,
    on_finish: Box<dyn Fn()>,
}

impl RunnerV1Builder<(), (), (), (), (), ()> {
    pub fn new() -> Self {
        RunnerV1Builder {
            progress: (),
            target: (),
            tree: (),
            client: (),
            tests: (),
            redis_results: (),
            success: 0,
            state: RunnerStateV1::Loaded,
            on_pass: Box::new(|| {}),
            on_fail: Box::new(|| {}),
            on_finish: Box::new(|| {}),
        }
    }
}

#[allow(dead_code)]
impl<A, B, C, D, E, F> RunnerV1Builder<A, B, C, D, E, F> {
    pub fn progress(
        self,
        progress: ProgressBar,
    ) -> RunnerV1Builder<ProgressBar, B, C, D, E, F> {
        RunnerV1Builder {
            progress,
            target: self.target,
            tree: self.tree,
            client: self.client,
            tests: self.tests,
            redis_results: self.redis_results,
            success: self.success,
            state: self.state,
            on_pass: self.on_pass,
            on_fail: self.on_fail,
            on_finish: self.on_finish,
        }
    }

    pub fn target(
        self,
        target: String,
    ) -> RunnerV1Builder<A, String, C, D, E, F> {
        RunnerV1Builder {
            progress: self.progress,
            target,
            tree: self.tree,
            client: self.client,
            tests: self.tests,
            redis_results: self.redis_results,
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
    ) -> RunnerV1Builder<A, B, sled::Tree, D, E, F> {
        RunnerV1Builder {
            progress: self.progress,
            target: self.target,
            tree,
            client: self.client,
            tests: self.tests,
            redis_results: self.redis_results,
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
    ) -> RunnerV1Builder<A, B, C, WebSocket<MaybeTlsStream<TcpStream>>, E, F>
    {
        RunnerV1Builder {
            progress: self.progress,
            target: self.target,
            tree: self.tree,
            client,
            tests: self.tests,
            redis_results: self.redis_results,
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
    ) -> RunnerV1Builder<
        A,
        B,
        C,
        D,
        Vec<(sled::IVec, TestState)>,
        RedisCourseResultV1,
    > {
        RunnerV1Builder {
            progress: self.progress,
            target: self.target,
            tree: self.tree,
            client: self.client,
            redis_results: RedisCourseResultV1::new(tests.len()),
            tests,
            success: self.success,
            state: self.state,
            on_pass: self.on_pass,
            on_fail: self.on_fail,
            on_finish: self.on_finish,
        }
    }

    pub fn on_pass<F1>(mut self, f: F1) -> RunnerV1Builder<A, B, C, D, E, F>
    where
        F1: Fn() + 'static,
    {
        self.on_pass = Box::new(f);
        self
    }

    pub fn on_fail<F2>(mut self, f: F2) -> RunnerV1Builder<A, B, C, D, E, F>
    where
        F2: Fn() + 'static,
    {
        self.on_fail = Box::new(f);
        self
    }

    pub fn on_finish<F3>(mut self, f: F3) -> RunnerV1Builder<A, B, C, D, E, F>
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
        RedisCourseResultV1,
    >
{
    pub fn build(self) -> RunnerV1 {
        RunnerV1 {
            progress: self.progress,
            target: self.target,
            tree: self.tree,
            client: self.client,
            tests: self.tests,
            redis_results: self.redis_results,
            success: self.success,
            state: self.state,
            on_pass: self.on_pass,
            on_fail: self.on_fail,
            on_finish: self.on_finish,
        }
    }
}
