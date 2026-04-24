use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum ProgressEvent {
    Started { total_bytes: u64, total_files: u64 },
    FileStarted { path: String, bytes: u64 },
    Bytes { delta: u64 },
    FileFinished,
    Finished,
}

pub struct Progress<'a> {
    callback: Option<Box<dyn FnMut(ProgressEvent) + Send + 'a>>,
    cancel: Option<Arc<AtomicBool>>,
}

impl<'a> Progress<'a> {
    pub fn new() -> Self {
        Self {
            callback: None,
            cancel: None,
        }
    }

    pub fn with_callback<F: FnMut(ProgressEvent) + Send + 'a>(mut self, f: F) -> Self {
        self.callback = Some(Box::new(f));
        self
    }

    pub fn with_cancel(mut self, flag: Arc<AtomicBool>) -> Self {
        self.cancel = Some(flag);
        self
    }

    pub(crate) fn emit(&mut self, event: ProgressEvent) {
        if let Some(cb) = &mut self.callback {
            cb(event);
        }
    }

    pub(crate) fn check_cancel(&self) -> super::Result<()> {
        if let Some(flag) = &self.cancel
            && flag.load(Ordering::Relaxed)
        {
            return Err(super::AsarError::Cancelled);
        }
        Ok(())
    }
}

impl Default for Progress<'_> {
    fn default() -> Self {
        Self::new()
    }
}
