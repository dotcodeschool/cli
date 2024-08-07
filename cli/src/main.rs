use chrono::Local;
use clap::Parser;
use env_logger::Builder;
use runner::{Runner, RunnerVersion, TestRunnerState};
use std::io::Write;

mod parsing;
mod runner;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    tests: Option<String>,
}

fn main() {
    let args = Args::parse();

    Builder::from_default_env()
        .format(|buf, record| {
            writeln!(
                buf,
                "{} [{}] - {}",
                Local::now().format("%Y-%m-%dT%H:%M:%S"),
                record.level(),
                record.args()
            )
        })
        .init();

    let path = match args.tests {
        Some(path) => path,
        None => "./tests.json".to_string(),
    };

    let mut runner = RunnerVersion::new(&path);
    while runner.state() != TestRunnerState::Finish {
        runner = runner.run();
    }
}
