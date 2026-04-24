use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum AsarError {
    #[error("I/O error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("asar header is malformed: {0}")]
    MalformedHeader(String),

    #[error("asar header JSON is invalid: {0}")]
    InvalidJson(#[from] serde_json::Error),

    #[error("asar entry '{path}' is invalid: {reason}")]
    InvalidEntry { path: String, reason: String },

    #[error("invalid unpack glob pattern: {0}")]
    BadGlob(#[from] globset::Error),

    #[error("walkdir error: {0}")]
    Walk(#[from] walkdir::Error),

    #[error("operation cancelled")]
    Cancelled,
}

impl AsarError {
    pub(crate) fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }
}
