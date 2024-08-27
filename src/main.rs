use chrono::Local;
use clap::{Args, Parser, Subcommand};
use db::PATH_DB;
use env_logger::{Builder, Target};
use monitor::{Monitor, MonitorError, StateMachine};
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
    Test(TestArgs),
    #[command(name = "check")]
    Check,
}

#[derive(Args, Debug)]
#[group(required = false, multiple = false)]
struct TestArgs {
    #[arg(group = "exclusive")]
    name: Option<String>,
    #[command(flatten)]
    options: TestOptions,
}

#[derive(Args, Debug)]
#[group(required = false, multiple = false)]
struct TestOptions {
    #[arg(long, group = "exclusive")]
    list: bool,
    #[arg(long, group = "exclusive")]
    staggered: bool,
}

fn main() -> Result<(), MonitorError> {
    let args = Cli::parse();

    init_log()?;

    let path_course = match args.course {
        Some(path) => &path.clone(),
        None => PATH_COURSE,
    };

    // TODO: add '--db' flag
    let path_db = PATH_DB;

    let monitor = Monitor::new(path_db, path_course)?;

    match args.command {
        Command::Test(TestArgs { name, options }) => {
            if options.list {
                let mut lister = monitor.into_lister()?;

                while !lister.is_finished() {
                    lister = lister.run();
                }
            } else if options.staggered {
                let mut runner = monitor.into_runner_staggered()?;

                while !runner.is_finished() {
                    runner = runner.run();
                }
            } else {
                let mut runner = monitor.into_runner(name)?;

                while !runner.is_finished() {
                    runner = runner.run();
                }
            }
        }
        Command::Check => {
            let mut validator = monitor.into_validator();

            while !validator.is_finished() {
                validator = validator.run();
            }
        }
    }

    Ok(())
}

fn init_log() -> Result<(), MonitorError> {
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open(str_res::LOG)?;

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
        .target(Target::Pipe(Box::new(file)))
        .init();

    Ok(())
}
