//! Minimal asar reader/writer supporting the subset of features used by the
//! game this patcher targets: packed files, SHA256 block-integrity metadata,
//! and the `.unpacked` sidecar tree driven by a basename glob.

mod error;
mod extract;
mod header;
mod integrity;
mod pack;
mod progress;

#[cfg(test)]
mod tests;

pub use error::AsarError;
pub use extract::extract;
pub use pack::{PackOptions, pack};
pub use progress::{Progress, ProgressEvent};

pub type Result<T> = std::result::Result<T, AsarError>;

pub const BLOCK_SIZE: usize = 4 * 1024 * 1024;
