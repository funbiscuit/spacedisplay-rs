#![warn(rust_2018_idioms, missing_debug_implementations)]

pub use entry_snapshot::{EntrySnapshot, EntrySnapshotRef};
pub use path::EntryPath;
pub use platform::get_available_mounts;
pub use scanner::{ScanStats, Scanner};
pub use tree_snapshot::{SnapshotConfig, TreeSnapshot};

mod arena;
mod entry;
mod entry_snapshot;
mod path;
mod platform;
mod scanner;
mod tree;
mod tree_snapshot;
mod watcher;
