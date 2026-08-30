use chrono::Local;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

/// Minimal debug logger writing to `~/Library/Application Support/HermitGPUI/Hermit.log`.
/// Mirrors the SwiftUI HermitLogger: disabled by default, opt-in from Settings.
pub struct HermitLogger {
    enabled: AtomicBool,
    path: PathBuf,
    handle: Mutex<Option<File>>,
}

impl HermitLogger {
    fn log_dir() -> PathBuf {
        let base = dirs::app_support();
        base.join("HermitGPUI")
    }

    pub fn new() -> Self {
        let dir = Self::log_dir();
        let _ = std::fs::create_dir_all(&dir);
        Self {
            enabled: AtomicBool::new(false),
            path: dir.join("Hermit.log"),
            handle: Mutex::new(None),
        }
    }

    pub fn path(&self) -> &std::path::Path {
        &self.path
    }

    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Relaxed);
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    pub fn log(&self, category: &str, message: impl AsRef<str>) {
        if !self.is_enabled() {
            return;
        }
        let line = format!(
            "{} [{category}] {}\n",
            Local::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            message.as_ref()
        );
        let mut guard = self.handle.lock().unwrap_or_else(|e| e.into_inner());
        if guard.is_none() {
            let file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&self.path)
                .ok();
            *guard = file;
        }
        if let Some(file) = guard.as_mut() {
            let _ = file.write_all(line.as_bytes());
        }
    }

    pub fn clear(&self) {
        let mut guard = self.handle.lock().unwrap_or_else(|e| e.into_inner());
        *guard = None;
        let _ = std::fs::write(&self.path, b"");
    }
}

pub fn global_logger() -> &'static HermitLogger {
    static LOGGER: std::sync::OnceLock<HermitLogger> = std::sync::OnceLock::new();
    LOGGER.get_or_init(HermitLogger::new)
}

#[macro_export]
macro_rules! log_debug {
    ($category:expr, $($arg:tt)*) => {
        if $crate::logger::global_logger().is_enabled() {
            $crate::logger::global_logger().log($category, format!($($arg)*));
        }
    };
}

/// Small helper namespace so the rest of the app can locate support dirs.
pub mod dirs {
    use std::path::PathBuf;

    pub fn home() -> PathBuf {
        std::env::var("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("/"))
    }

    pub fn app_support() -> PathBuf {
        home().join("Library/Application Support")
    }
}
