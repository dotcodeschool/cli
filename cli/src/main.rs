use chrono::Local;
use clap::Parser;
use db::{db_open, DbError, PATH_DB};
use env_logger::Builder;
use runner::{Runner, RunnerVersion};
use std::io::Write;
use thiserror::Error;
use validation::{Validator, ValidatorVersion};

mod db;
mod parsing;
mod runner;
mod str_res;
mod validation;

const PATH_COURSE: &str = "./tests.json";

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Cli {
    #[arg(short, long)]
    tests: Option<String>,
    #[arg(long)]
    check: bool,
}

fn main() -> Result<(), DbError> {
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

    let path_course = match args.tests {
        Some(path) => &path.clone(),
        None => PATH_COURSE,
    };

    // TODO: add '--db' flag
    let path_db = PATH_DB;

    let (db, tree) = db_open(path_db, path_course)?;

    if args.check {
        let mut validator = ValidatorVersion::new(path_course);

        while !validator.is_finished() {
            validator = validator.run();
        }
    } else {
        let mut runner = RunnerVersion::new(path_course);

        while !runner.is_finished() {
            runner = runner.run();
        }
    }

    let _ = db.flush();
    Ok(())
}
