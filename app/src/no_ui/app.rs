use std::io::stdout;
use std::thread;
use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::{cursor, terminal, ExecutableCommand, QueueableCommand};

use diskscan::{ScanStats, ScannerBuilder, SnapshotConfig};

use crate::{utils, Args};

pub fn run(args: Args) -> Result<()> {
    if let Some(path) = args.path {
        let scanner = ScannerBuilder::default().scan(path);
        let start = Instant::now();
        while scanner.is_scanning() {
            print_stats(scanner.stats())?;
            thread::sleep(Duration::from_millis(10));
        }
        stdout().execute(terminal::Clear(terminal::ClearType::FromCursorDown))?;
        let stats = scanner.stats();
        println!("Scanned {} files, {} dirs", stats.files, stats.dirs);
        if let Some(available) = stats.available_size {
            println!("Available space: {}", utils::byte_to_str(available, 0));
        }
        println!("Scan took {:?}", start.elapsed());
        let tree = scanner
            .get_tree(
                scanner.get_scan_path(),
                SnapshotConfig {
                    max_depth: 1,
                    ..SnapshotConfig::default()
                },
            )
            .unwrap();
        tree.print(&|size| utils::byte_to_str(size, 0), 1);
    }

    Ok(())
}

fn print_stats(stats: ScanStats) -> Result<()> {
    let mut stdout = stdout();
    stdout.queue(terminal::Clear(terminal::ClearType::FromCursorDown))?;
    let mut rows = 2;
    println!("\t{} files, {} dirs", stats.files, stats.dirs);
    print!("\tScanned {}", utils::byte_to_str(stats.used_size, 0));
    if let Some(total) = stats.total_size {
        println!("/{}", utils::byte_to_str(total, 0));
    } else {
        println!()
    }
    if let Some(available) = stats.available_size {
        println!("\tAvailable: {}", utils::byte_to_str(available, 0));
        rows += 1;
    }
    stdout.execute(cursor::MoveUp(rows))?;
    Ok(())
}
