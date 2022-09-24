#![warn(rust_2018_idioms, missing_debug_implementations)]

pub use scanner::{ScanStats, Scanner};
pub use tree::entry_snapshot::{EntrySnapshot, EntrySnapshotRef};
pub use tree::path::EntryPath;
pub use tree::tree_snapshot::{SnapshotConfig, TreeSnapshot};

mod scanner;
mod tree;
mod utils;
