use std::time::Duration;

use anyhow::Result;
use clap::Parser;

mod app;
mod dialog;
mod file_list;
mod log_list;
mod progressbar;
mod term;
mod ui;
mod utils;

#[derive(Debug, Parser)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// Path to directory to scan
    path: Option<String>,

    /// Use simple graphics instead of unicode
    #[arg(short, long)]
    simple_graphics: bool,

    /// Refresh rate of terminal UI
    #[arg(short, long, value_parser(parse_duration), default_value("200"))]
    tick_rate: Duration,
}

fn main() -> Result<()> {
    let args = Args::parse();

    term::run(args)?;

    Ok(())
}

fn parse_duration(arg: &str) -> Result<Duration, std::num::ParseIntError> {
    let seconds = arg.parse()?;
    Ok(Duration::from_millis(seconds))
}
