mod cli;
mod config;
mod error;
mod permanent;
mod platform;
mod process_chain;
mod trash;
mod ui;

use std::env;
use std::os::unix::process::CommandExt;
use std::process::Command;

use cli::Cli;
use config::Config;
use error::AppError;
use process_chain::ProcessChain;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn run() -> u8 {
    match run_inner() {
        Ok(code) => code,
        Err(error) => {
            eprintln!("{}", error.message);
            error.code
        }
    }
}

fn run_inner() -> Result<u8, AppError> {
    let argv: Vec<_> = env::args_os().collect();
    let program = argv
        .first()
        .and_then(|value| std::path::Path::new(value).file_name())
        .and_then(|value| value.to_str())
        .unwrap_or("trash")
        .to_owned();
    let original = argv.into_iter().skip(1).collect::<Vec<_>>();

    let config = Config::from_env()?;
    let chain = ProcessChain::inspect(&config.exceptions);
    if chain.must_bypass {
        return exec_real_rm(&original);
    }

    let cli = Cli::parse(original.clone())?;
    if cli.show_help {
        print!("{}", cli::help(&program));
        return Ok(0);
    }
    if cli.show_version {
        println!("{program} {VERSION}");
        return Ok(0);
    }
    if !cli.is_cleanup() && cli.operands.is_empty() {
        return exec_real_rm(&original);
    }

    trash::execute(cli, config, chain, original)
}

fn exec_real_rm(arguments: &[std::ffi::OsString]) -> Result<u8, AppError> {
    let error = Command::new("/bin/rm").args(arguments).exec();
    Err(AppError::operation(format!(
        "rm: cannot execute /bin/rm: {error}"
    )))
}
