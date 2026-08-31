#[cfg(any(target_os = "linux", target_os = "android"))]
mod linux;

#[cfg(any(target_os = "linux", target_os = "android"))]
pub use linux::{MountTable, canonical_location, rename_noreplace};

#[cfg(not(any(target_os = "linux", target_os = "android")))]
compile_error!("trash supports Linux and Android only");
