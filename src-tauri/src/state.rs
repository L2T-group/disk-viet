use crate::scanner::Tree;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex, RwLock};

pub struct AppState {
    pub tree: RwLock<Option<Tree>>,
    /// Cancel flag of the current scan; replaced (after cancelling the old one) on each `scan_start`.
    pub cancel: Mutex<Arc<AtomicBool>>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            tree: RwLock::new(None),
            cancel: Mutex::new(Arc::new(AtomicBool::new(false))),
        }
    }
}
