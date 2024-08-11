use chrono::Local;
use clap::{Parser, Subcommand};
use db::{DbError, PATH_DB};
use env_logger::Builder;
use monitor::{Monitor, StateMachine};
use std::io::Write;

mod db;
mod lister;
mod monitor;
mod parsing;
mod runner;
mod str_res;
mod validator;

const PATH_COURSE: &str = "./course.json";

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Command,
    #[arg(short, long)]
    course: Option<String>,
}

#[derive(Subcommand, Debug)]
enum Command {
    #[command(name = "test")]
    Test { name: Option<String> },
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

    let path_course = match args.course {
        Some(path) => &path.clone(),
        None => PATH_COURSE,
    };

    // TODO: add '--db' flag
    let path_db = PATH_DB;

    let monitor = Monitor::new(path_db, path_course)?;

    match args.command {
        Command::Test { name } => {
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
        Command::List => {
            let mut lister = monitor.into_lister()?;

            while !lister.is_finished() {
                lister = lister.run();
            }
        }
    }

    Ok(())
}
