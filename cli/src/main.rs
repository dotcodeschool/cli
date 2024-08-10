use chrono::Local;
use clap::{Parser, Subcommand};
use db::{db_open, db_should_update, db_update, DbError, PATH_DB};
use env_logger::Builder;
use monitor::Monitor;
use runner::{Runner, RunnerVersion};
use std::io::Write;
use validator::{Validator, ValidatorVersion};

mod db;
mod monitor;
mod parsing;
mod runner;
mod str_res;
mod validator;

const PATH_COURSE: &str = "./tests.json";

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Command,
    #[arg(short, long)]
    tests: Option<String>,
}

#[derive(Subcommand, Debug)]
enum Command {
    #[command(name = "test")]
    Test,
    #[command(name = "check")]
    Check,
    #[command(name = "list")]
    List,
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

    let monitor = Monitor::new(path_course);
    let tests_new = monitor.list_tests();
    let (db, tree) = db_open(path_db, path_course)?;

    if db_should_update(&tree, path_course)? {
        db_update(&tree, &tests_new)?;
    }

    match args.command {
        Command::Test => {
            let mut runner = monitor.into_runner();

            while !runner.is_finished() {
                runner = runner.run();
            }
        }
        Command::Check => {
            let mut validator = monitor.into_validator();

            while !validator.is_finished() {
                validator = validator.run();
            }
        }
        Command::List => todo!(),
    }

    let _ = db.flush();
    Ok(())
}
