use indicatif::ProgressBar;

use colored::Colorize;
use parity_scale_codec::Decode;

use crate::{
    db::{TestState, ValidationState},
    monitor::StateMachine,
};

#[derive(PartialEq, Eq)]
pub enum ListerStateV1 {
    Loaded,
    List { index_test: usize },
    Error { reason: String },
    Finished,
}

pub struct ListerV1 {
    pub progress: ProgressBar,
    pub tests: Vec<String>,
    pub tree: sled::Tree,
    pub state: ListerStateV1,
}

impl ListerV1 {
    pub fn new(
        progress: ProgressBar,
        tests: Vec<String>,
        tree: sled::Tree,
    ) -> Self {
        Self { progress, tests, tree, state: ListerStateV1::Loaded }
    }
}

impl StateMachine for ListerV1 {
    fn run(self) -> Self {
        let Self { progress, tests, tree: db, state } = self;

        match state {
            ListerStateV1::Loaded => {
                let test_count = tests.len();
                progress.println(format!(
                    "{} tests available\n ",
                    test_count.to_string().bold()
                ));

                if test_count == 0 {
                    Self {
                        progress,
                        tests,
                        tree: db,
                        state: ListerStateV1::Error {
                            reason: "🚫 no tests found".to_string(),
                        },
                    }
                } else {
                    Self {
                        progress,
                        tests,
                        tree: db,
                        state: ListerStateV1::List { index_test: 0 },
                    }
                }
            }
            ListerStateV1::List { index_test } => {
                let query = db.get(&tests[index_test]);

                match query {
                    Ok(Some(bytes)) => {
                        let test = TestState::decode(&mut &bytes[..]).unwrap();
                        let path_to = test.path_to().to_lowercase();
                        let test_name = test.name.to_lowercase();

                        match test.passed {
                            ValidationState::Unkown => {
                                progress.println(format!(
                                    "• {} {}/{}",
                                    "[   ..   ]".white().dimmed(),
                                    path_to.white().dimmed().italic(),
                                    test_name.white().bold()
                                ))
                            }
                            ValidationState::Pass => progress.println(format!(
                                "• {} {}/{}",
                                "[ Passed ]".green().bold(),
                                path_to.white().dimmed().italic(),
                                test_name.white().bold(),
                            )),
                            ValidationState::Fail => progress.println(format!(
                                "• {} {}/{}",
                                "[ Failed ]".red().bold(),
                                path_to.white().dimmed().italic(),
                                test_name.white().bold(),
                            )),
                        }

                        if index_test + 1 < tests.len() {
                            Self {
                                progress,
                                tests,
                                tree: db,
                                state: ListerStateV1::List {
                                    index_test: index_test + 1,
                                },
                            }
                        } else {
                            Self {
                                progress,
                                tests,
                                tree: db,
                                state: ListerStateV1::Finished,
                            }
                        }
                    }
                    Ok(None) => {
                        let state = ListerStateV1::Error {
                            reason: format!(
                                "failed to read test, no data at key 0x{}",
                                hex::encode(&tests[index_test])
                            ),
                        };
                        Self { progress, tests, tree: db, state }
                    }
                    Err(err) => {
                        let state = ListerStateV1::Error {
                            reason: format!(
                                "failed to read test at key 0x{}, {}",
                                hex::encode(&tests[index_test]),
                                err
                            ),
                        };
                        Self { progress, tests, tree: db, state }
                    }
                }
            }
            ListerStateV1::Error { reason } => {
                progress.finish_and_clear();
                progress.println(format!("\n⚠ Error: {}", reason.red().bold()));

                Self {
                    progress,
                    tests,
                    tree: db,
                    state: ListerStateV1::Finished,
                }
            }
            ListerStateV1::Finished => Self {
                progress,
                tests,
                tree: db,
                state: ListerStateV1::Finished,
            },
        }
    }

    fn is_finished(&self) -> bool {
        self.state == ListerStateV1::Finished
    }
}
