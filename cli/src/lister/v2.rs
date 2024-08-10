use indicatif::ProgressBar;

use colored::Colorize;
use parity_scale_codec::{Decode, Encode};

use crate::{
    db::{TestState, ValidationState},
    monitor::StateMachine,
};

#[derive(PartialEq, Eq)]
pub enum ListerStateV2 {
    Loaded,
    List { index_test: usize },
    Error { reason: String },
    Finished,
}

pub struct ListerV2 {
    pub progress: ProgressBar,
    pub tests: Vec<String>,
    pub tree: sled::Tree,
    pub state: ListerStateV2,
}

impl StateMachine for ListerV2 {
    fn run(self) -> Self {
        let Self { progress, tests, tree: db, state } = self;

        match state {
            ListerStateV2::Loaded => {
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
                        state: ListerStateV2::Error {
                            reason: "🚫 no tests found".to_string(),
                        },
                    }
                } else {
                    Self {
                        progress,
                        tests,
                        tree: db,
                        state: ListerStateV2::List { index_test: 0 },
                    }
                }
            }
            ListerStateV2::List { index_test } => {
                let test_name = &tests[index_test];

                let key = test_name.encode();
                let test = db.get(&key);

                match test {
                    Ok(Some(bytes)) => {
                        let TestState { path, passed } =
                            TestState::decode(&mut &bytes[..]).unwrap();

                        let path_to = path[..(path.len() - 1)].join("/");
                        match passed {
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
                                state: ListerStateV2::List {
                                    index_test: index_test + 1,
                                },
                            }
                        } else {
                            Self {
                                progress,
                                tests,
                                tree: db,
                                state: ListerStateV2::Finished,
                            }
                        }
                    }
                    Ok(None) => Self {
                        progress,
                        tests,
                        tree: db,
                        state: ListerStateV2::Error {
                            reason: format!(
                                "failed to read test, no data at key 0x{}",
                                hex::encode(key)
                            )
                            .to_string(),
                        },
                    },
                    Err(err) => Self {
                        progress,
                        tests,
                        tree: db,
                        state: ListerStateV2::Error {
                            reason: format!(
                                "failed to read test at key 0x{}, {}",
                                hex::encode(key),
                                err
                            ),
                        },
                    },
                }
            }
            ListerStateV2::Error { reason } => {
                progress.finish_and_clear();
                progress.println(format!("\n⚠ Error: {}", reason.red().bold()));

                Self {
                    progress,
                    tests,
                    tree: db,
                    state: ListerStateV2::Finished,
                }
            }
            ListerStateV2::Finished => Self {
                progress,
                tests,
                tree: db,
                state: ListerStateV2::Finished,
            },
        }
    }

    fn is_finished(&self) -> bool {
        self.state == ListerStateV2::Finished
    }
}
