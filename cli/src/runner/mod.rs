use std::time::Duration;

use indicatif::{ProgressBar, ProgressStyle};

use regex::Regex;

use crate::monitor::StateMachine;

use self::{v1::RunnerV1, v2::RunnerV2};

pub mod v1;
pub mod v2;

pub enum RunnerVersion {
    V1(RunnerV1),
    V2(RunnerV2),
    Undefined,
}

impl StateMachine for RunnerVersion {
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
