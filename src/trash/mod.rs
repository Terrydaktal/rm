mod cleanup;
mod locking;
mod metadata;
mod move_item;
mod root;
mod run;

use std::ffi::OsString;

use chrono::Local;

use crate::cli::{Cli, InteractiveMode};
use crate::config::Config;
use crate::error::Result;
use crate::platform::MountTable;
use crate::process_chain::ProcessChain;
use crate::ui;

use metadata::Metadata;
use move_item::TrashSession;

pub fn execute(
    cli: Cli,
    config: Config,
    chain: ProcessChain,
    original: Vec<OsString>,
) -> Result<u8> {
    let mounts = MountTable::load()?;
    if cli.clean {
        if cli.operands.is_empty() {
            cleanup::clean_all(&config, &mounts, cli.force)?;
        } else {
            cleanup::clean_paths(&config, &mounts, cli.force, &cli.operands)?;
        }
        return Ok(0);
    }
    if !cli.clean_apps.is_empty() {
        cleanup::clean_apps(&config, &mounts, cli.force, &cli.clean_apps)?;
        return Ok(0);
    }

    if cli.interactive == InteractiveMode::Once
        && !cli.force
        && (cli.recursive || cli.operands.len() > 3)
        && !ui::confirm_once()?
    {
        return Ok(1);
    }

    let run_name = run_name(&cli, &chain);
    let metadata = Metadata::new(&original, &chain.names);
    let mut session = TrashSession::new(&config, &mounts, &cli, metadata, run_name);
    let mut had_error = false;
    for operand in &cli.operands {
        if let Err(error) = session.trash_one(operand) {
            eprintln!("{}", error.message);
            had_error = true;
        }
    }
    session.remove_empty_runs();
    Ok(u8::from(had_error))
}

fn run_name(cli: &Cli, chain: &ProcessChain) -> String {
    let parent = chain
        .preferred_app
        .as_deref()
        .or_else(|| chain.names.first().map(String::as_str))
        .unwrap_or("unknown");
    let parent = sanitize(parent.as_bytes(), 32, "unknown");
    let names = cli
        .operands
        .iter()
        .take(2)
        .map(|operand| {
            let bytes = operand.as_encoded_bytes();
            let mut bytes = bytes;
            while bytes.ends_with(b"/") {
                bytes = &bytes[..bytes.len() - 1];
            }
            let basename = bytes
                .rsplit(|byte| *byte == b'/')
                .next()
                .unwrap_or_default();
            sanitize(basename, 48, "item")
        })
        .collect::<Vec<_>>()
        .join("+");
    let prefix = if names.is_empty() {
        String::new()
    } else {
        format!("{names}-")
    };
    format!(
        "({parent}){prefix}{}-pid-{}",
        Local::now().format("%Y-%m-%d_%H-%M-%S"),
        std::process::id()
    )
}

fn sanitize(bytes: &[u8], limit: usize, fallback: &str) -> String {
    let mut value = bytes
        .iter()
        .take(limit)
        .map(|byte| {
            if byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-') {
                char::from(*byte)
            } else {
                '_'
            }
        })
        .collect::<String>();
    if value.is_empty() {
        value.push_str(fallback);
    }
    value
}
