use std::sync::Mutex;

use lazy_static::lazy_static;
use log::{Level, Log, Metadata, Record};
use time::OffsetDateTime;

#[derive(Clone, Debug)]
pub struct LogEntry {
    pub timestamp: OffsetDateTime,
    pub level: Level,
    pub module: Option<String>,
    pub text: String,
}

lazy_static! {
    static ref LOGGER: Logger = Logger::default();
}

#[derive(Debug)]
pub struct Logger {
    entries: Mutex<Vec<LogEntry>>,
}

impl Logger {
    pub fn global() -> &'static Logger {
        &LOGGER
    }

    /// Return all new entries from last call
    ///
    /// Entries are sorted from oldest (first) to newest (last)
    pub fn read_entries(&self) -> Vec<LogEntry> {
        std::mem::take(self.entries.lock().unwrap().as_mut())
    }
}

impl Default for Logger {
    fn default() -> Self {
        Self {
            entries: Mutex::new(vec![]),
        }
    }
}

impl Log for Logger {
    fn enabled(&self, _: &Metadata) -> bool {
        true
    }

    fn log(&self, record: &Record) {
        //todo use channel instead of locking
        self.entries.lock().unwrap().push(LogEntry {
            timestamp: OffsetDateTime::now_utc(),
            level: record.level(),
            module: record.module_path().map(|s| s.to_string()),
            text: record.args().to_string(),
        });
    }

    fn flush(&self) {
        // everything in memory
    }
}
