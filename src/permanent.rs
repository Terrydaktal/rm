use std::path::PathBuf;
use std::process::Command;

pub const DELETE_BATCH_SIZE: usize = 4_096;

pub fn remove_paths(paths: &[PathBuf]) -> bool {
    remove_paths_with(paths, |batch| {
        Command::new("/bin/rm")
            .args(["--one-file-system", "-rf", "--"])
            .args(batch)
            .status()
            .is_ok_and(|status| status.success())
    })
}

fn remove_paths_with(paths: &[PathBuf], mut remove_batch: impl FnMut(&[PathBuf]) -> bool) -> bool {
    let mut succeeded = true;
    let byte_budget = argument_byte_budget();
    let mut start = 0;
    while start < paths.len() {
        let mut end = start;
        let mut bytes = 128;
        while end < paths.len() && end - start < DELETE_BATCH_SIZE {
            let path_bytes = paths[end].as_os_str().as_encoded_bytes().len() + 1;
            if end > start && bytes + path_bytes > byte_budget {
                break;
            }
            bytes += path_bytes;
            end += 1;
        }
        let batch = &paths[start..end];
        succeeded &= remove_batch(batch);
        start = end;
    }
    succeeded
}

fn argument_byte_budget() -> usize {
    // Reserve at least half of ARG_MAX for the inherited environment and command
    // overhead, and cap batches to keep memory and deletion latency predictable.
    // SAFETY: sysconf has no pointer arguments and _SC_ARG_MAX is a valid selector.
    let arg_max = unsafe { libc::sysconf(libc::_SC_ARG_MAX) };
    if arg_max > 0 {
        (arg_max as usize / 2).clamp(32 * 1024, 1024 * 1024)
    } else {
        64 * 1024
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{DELETE_BATCH_SIZE, argument_byte_budget, remove_paths_with};

    #[test]
    fn deletion_budget_leaves_argument_headroom() {
        assert!(argument_byte_budget() >= 32 * 1024);
        assert!(argument_byte_budget() <= 1024 * 1024);
    }

    #[test]
    fn partial_batch_failures_are_reported_after_all_batches_run() {
        let paths = (0..DELETE_BATCH_SIZE + 1)
            .map(|index| PathBuf::from(format!("item-{index}")))
            .collect::<Vec<_>>();
        let mut batch_sizes = Vec::new();
        let succeeded = remove_paths_with(&paths, |batch| {
            batch_sizes.push(batch.len());
            batch_sizes.len() != 1
        });
        assert!(!succeeded);
        assert_eq!(batch_sizes, [DELETE_BATCH_SIZE, 1]);
    }

    #[test]
    fn argument_bytes_split_batches_before_path_count_limit() {
        let long_component = "x".repeat(200_000);
        let paths = (0..8)
            .map(|index| PathBuf::from(format!("{long_component}-{index}")))
            .collect::<Vec<_>>();
        let mut batches = 0;
        assert!(remove_paths_with(&paths, |_| {
            batches += 1;
            true
        }));
        assert!(batches > 1);
    }
}
