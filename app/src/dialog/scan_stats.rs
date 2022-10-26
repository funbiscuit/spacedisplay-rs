use num_format::{CustomFormat, Grouping};
use tui::buffer::Buffer;
use tui::layout::Rect;
use tui::style::{Color, Style};
use tui::widgets::{Block, BorderType, Borders, Clear, Widget};
use unicode_width::UnicodeWidthStr;

use diskscan::ScanStats;

use crate::app::App;
use crate::dialog::{Dialog, DialogWidget};
use crate::term::InputHandler;
use crate::utils;

pub struct ScanStatsDialog {
    should_close: bool,
}

impl ScanStatsDialog {
    pub fn new() -> Self {
        Self {
            should_close: false,
        }
    }
}

impl ScanStatsDialog {
    const TITLE: &'static str = "Scan Stats ";

    fn lines(stats: &ScanStats) -> Vec<String> {
        let format = CustomFormat::builder()
            .grouping(Grouping::Standard)
            .separator(" ")
            .build()
            .unwrap();

        let files = {
            let mut buf = num_format::Buffer::new();
            buf.write_formatted(&(stats.files), &format);
            format!("Files: {}", buf.as_str())
        };
        let dirs = {
            let mut buf = num_format::Buffer::new();
            buf.write_formatted(&(stats.dirs), &format);
            format!("Dirs: {}", buf.as_str())
        };

        let mut lines = vec![];
        lines.push(format!(
            "Used size: {}",
            utils::byte_to_str(stats.used_size, 0)
        ));

        if let Some(available) = stats.available_size {
            lines.push(format!(
                "Available size: {}",
                utils::byte_to_str(available, 0)
            ));
        }
        if let Some(total) = stats.total_size {
            lines.push(format!("Total size: {}", utils::byte_to_str(total, 0)));
        }

        lines.push(files);
        lines.push(dirs);
        lines.push(format!("Scan took: {:?}", stats.scan_duration));
        if let Some(memory) = stats.used_memory {
            lines.push(format!("Memory usage: {}", utils::byte_to_str(memory, 0)));
        }

        lines
    }
}

impl InputHandler for ScanStatsDialog {
    fn on_esc(&mut self) {
        self.should_close = true;
    }

    fn on_key(&mut self, c: char) {
        self.should_close = c == 'q' || c == 's';
    }
}

impl Dialog for ScanStatsDialog {
    fn get_widget<'a>(&'a self, app: &'a App) -> DialogWidget<'_> {
        DialogWidget(self, app)
    }

    fn render(&self, app: &App, area: Rect, buf: &mut Buffer) {
        Clear.render(area, buf);
        buf.set_style(area, Style::default().bg(Color::Black));

        let block = Block::default()
            .borders(Borders::ALL)
            .style(Style::default().fg(Color::White))
            .title(Self::TITLE)
            .border_type(BorderType::Plain);
        Widget::render(block, area, buf);

        let stats = &app.files.as_ref().unwrap().stats;
        let lines = ScanStatsDialog::lines(stats);
        for (i, line) in lines.iter().enumerate() {
            buf.set_string(area.x + 2, area.y + 1 + i as u16, line, Style::default());
        }
    }

    fn size(&self, app: &App) -> (u16, u16) {
        let stats = &app.files.as_ref().unwrap().stats;
        let lines = ScanStatsDialog::lines(stats);
        let max_width = std::iter::once(Self::TITLE.width())
            .chain(lines.iter().map(|m| m.width()))
            .max()
            .unwrap();
        (4 + max_width as u16, 2 + lines.len() as u16)
    }

    fn try_finish(self: Box<Self>, _: &mut App) -> Result<(), Box<dyn Dialog>> {
        if self.should_close {
            Ok(())
        } else {
            Err(self)
        }
    }
}
