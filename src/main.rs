use clap::{Args, Parser, Subcommand};
use constants::LOG;
use db::PATH_DB;
use monitor::{Monitor, MonitorError, StateMachine};

mod constants;
mod db;
mod lister;
mod models;
mod monitor;
mod parsing;
mod runner;
mod str_res;
mod validator;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Command,
    #[arg(long)]
    db: Option<String>,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Run tests (uses staggered mode by default)
    #[command(name = "test")]
    Test(TestArgs),
    /// Submit the current commit to DotCodeSchool, use --empty to create an
    /// empty commit and submit it
    #[command(name = "submit")]
    Submit(SubmitArgs),
    #[cfg(not(debug_assertions))]
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
    /// List all available tests for the course
    #[arg(long, group = "exclusive")]
    list: bool,
    /// Run all tests at once
    #[arg(long)]
    all: bool,
    /// Do not destroy the test environment after running the tests
    #[arg(long)]
    keep: bool,
}

#[derive(Args, Debug)]
struct SubmitArgs {
    /// Create an empty commit and submit it
    #[arg(long)]
    empty: bool,
}

fn main() -> Result<(), MonitorError> {
    let args = Cli::parse();

    let file = std::fs::OpenOptions::new()
        .read(true)
        .create(true)
        .append(true)
        .open(LOG)?;

    let _ = simplelog::WriteLogger::init(
        simplelog::LevelFilter::Debug,
        simplelog::ConfigBuilder::default()
            .add_filter_allow_str("dotcodeschool_cli")
            .build(),
        file,
    );

    let path_db = match args.db {
        Some(path) => path,
        None => PATH_DB.to_string(),
    };

    let monitor = Monitor::new(&path_db)?;

    match args.command {
        Command::Test(TestArgs { name, options }) => {
            if options.list {
                let mut lister = monitor.into_lister()?;

                while !lister.is_finished() {
                    lister = lister.run();
                }
            } else if options.all || name.is_some() {
                let mut runner = monitor.into_runner(name, options.keep)?;

                while !runner.is_finished() {
                    runner = runner.run();
                }
            } else {
                let mut runner = monitor.into_runner(name, options.keep)?;
                // TODO: replace with into_runner_staggered
                // let mut runner =
                // monitor.into_runner_staggered(options.keep)?;

                while !runner.is_finished() {
                    runner = runner.run();
                }
            }
        }
        Command::Submit(SubmitArgs { empty }) => {
            handle_submit(empty)?;
        }
        #[cfg(not(debug_assertions))]
        Command::Check => {
            let mut validator = monitor.into_validator();

            while !validator.is_finished() {
                validator = validator.run();
            }
        }
    }

    Ok(())
}

fn handle_submit(empty: bool) -> Result<(), MonitorError> {
    if empty {
        // Create an empty commit
        let commit_output = std::process::Command::new("git")
            .args(&[
                "commit",
                "--allow-empty",
                "-m",
                "Empty commit for submission",
            ])
            .output()?;

        if !commit_output.status.success() {
            return Err(MonitorError::IOError(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Failed to create an empty commit",
            )));
        }

        log::debug!("Successfully created empty commit.");
    }
    // Push the commit to the remote repository
    let push_output =
        std::process::Command::new("git").args(&["push"]).output()?;

    if !push_output.status.success() {
        return Err(MonitorError::IOError(std::io::Error::new(
            std::io::ErrorKind::Other,
            "Failed to push the commit to the remote repository",
        )));
    }

    log::debug!("Successfully pushed the commit to the remote repository.");

    Ok(())
}
