use chrono::Local;
use clap::Parser;
use env_logger::Builder;
use runner::{Runner, RunnerVersion};
use std::io::Write;

mod db;
mod parsing;
mod runner;
mod validation;

const TEST_DEFAULT: &str = "./tests.json";

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Cli {
    #[arg(short, long)]
    tests: Option<String>,
    #[arg(long)]
    check: bool,
}

fn main() {
    let args = Cli::parse();

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
        None => TEST_DEFAULT.to_string(),
    };

    let mut runner = RunnerVersion::new(&path);

    if args.check {
        todo!()
    } else {
        while !runner.is_finished() {
            runner = runner.run();
        }
    }
}
