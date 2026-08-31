use std::io::{self, IsTerminal, Write};

use crate::error::{AppError, Result};

pub fn confirm_cleanup(force: bool, prompt: &str, expected: &str) -> Result<()> {
    if force {
        return Ok(());
    }
    if !io::stdin().is_terminal() || !io::stderr().is_terminal() {
        return Err(AppError::operation(
            "rm: refusing permanent cleanup without a terminal; use --force explicitly",
        ));
    }
    eprint!("{prompt}");
    io::stderr().flush().map_err(|error| {
        AppError::operation(format!("rm: failed to write cleanup confirmation: {error}"))
    })?;
    let response = read_line("rm: failed to read cleanup confirmation")?;
    if response != expected {
        return Err(AppError::operation("rm: cleanup confirmation failed"));
    }
    Ok(())
}

pub fn confirm_target(path: &str) -> Result<bool> {
    eprint!("rm: move '{path}' to trash? [y/N] ");
    io::stderr().flush().map_err(|error| {
        AppError::operation(format!(
            "rm: failed to write interactive confirmation: {error}"
        ))
    })?;
    let response = read_line("rm: failed to read interactive confirmation")?;
    Ok(matches!(
        response.as_str(),
        "y" | "Y" | "yes" | "YES" | "Yes"
    ))
}

pub fn confirm_once() -> Result<bool> {
    eprint!("rm: move these targets to trash? [y/N] ");
    io::stderr().flush().map_err(|error| {
        AppError::operation(format!(
            "rm: failed to write interactive confirmation: {error}"
        ))
    })?;
    let response = read_line("rm: failed to read interactive confirmation")?;
    Ok(matches!(
        response.as_str(),
        "y" | "Y" | "yes" | "YES" | "Yes"
    ))
}

fn read_line(message: &str) -> Result<String> {
    let mut response = String::new();
    let bytes = io::stdin()
        .read_line(&mut response)
        .map_err(|_| AppError::operation(message))?;
    if bytes == 0 {
        return Err(AppError::operation(message));
    }
    while matches!(response.as_bytes().last(), Some(b'\n' | b'\r')) {
        response.pop();
    }
    Ok(response)
}
