use std::sync::Mutex;

use chrono::{DateTime, Utc};

#[derive(Clone, Debug)]
pub struct LogEntry {
    pub timestamp: DateTime<Utc>,
    pub text: String,
}

impl LogEntry {
    fn new(msg: String) -> Self {
        Self {
            timestamp: Utc::now(),
            text: msg,
        }
    }
}

#[derive(Debug)]
pub struct Logger {
    entries: Mutex<Vec<LogEntry>>,
}

impl Logger {
    /// Return all new entries from last call
    ///
    /// Entries are sorted from oldest (first) to newest (last)
    pub fn read_entries(&self) -> Vec<LogEntry> {
        let mut entries = vec![];
        std::mem::swap(self.entries.lock().unwrap().as_mut(), &mut entries);
        entries
    }

    /// Add given message to log
    pub fn log(&self, msg: String) {
        self.entries.lock().unwrap().push(LogEntry::new(msg));
    }
}

impl Default for Logger {
    fn default() -> Self {
        Self {
            entries: Mutex::new(vec![]),
        }
    }
}
