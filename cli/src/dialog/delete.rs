use byte_unit::Byte;
use tui::buffer::Buffer;
use tui::layout::{Alignment, Rect};
use tui::style::{Color, Modifier, Style};
use tui::text::{Span, Spans};
use tui::widgets::{Block, BorderType, Borders, Clear, Paragraph, Widget};
use unicode_width::UnicodeWidthStr;

use spacedisplay_lib::EntryPath;

use crate::app::App;
use crate::dialog::{Dialog, DialogWidget};
use crate::term::InputHandler;
use crate::utils;

pub struct DeleteDialog {
    path: EntryPath,
    size: Byte,
    selected_yes: bool,
    chosen: Option<bool>,
    should_close: bool,
}

impl DeleteDialog {
    const TITLE: &'static str = "Confirm delete ";

    pub fn new(path: EntryPath, size: Byte) -> Self {
        Self {
            path,
            size,
            selected_yes: false,
            chosen: None,
            should_close: false,
        }
    }

    fn lines(&self) -> Vec<String> {
        let mut lines = vec![];
        lines.push("Are you sure you want to delete:".into());
        lines.push(self.path.to_string());
        lines.push(format!("Size: {}", utils::byte_to_str(self.size, 0)));
        lines.push("This cannot be undone!".into());

        lines
    }
}

impl InputHandler for DeleteDialog {
    fn on_enter(&mut self) {
        self.chosen = Some(self.selected_yes);
        self.should_close = true;
    }

    fn on_esc(&mut self) {
        self.should_close = true;
    }

    fn on_key(&mut self, c: char) {
        if c == 'y' {
            self.chosen = Some(true);
        }
        self.should_close = c == 'q' || c == 'd' || c == 'n';
    }

    fn on_left(&mut self) {
        self.selected_yes = true;
    }

    fn on_right(&mut self) {
        self.selected_yes = false;
    }
}

impl Dialog for DeleteDialog {
    fn get_widget<'a>(&'a self, app: &'a App) -> DialogWidget<'_> {
        DialogWidget(self, app)
    }

    fn render(&self, _: &App, area: Rect, buf: &mut Buffer) {
        Clear.render(area, buf);
        buf.set_style(area, Style::default().bg(Color::Black));

        let block = Block::default()
            .borders(Borders::ALL)
            .style(Style::default().fg(Color::White))
            .title(Self::TITLE)
            .border_type(BorderType::Plain);
        Widget::render(block, area, buf);

        let lines = self.lines();
        for (i, line) in lines.iter().enumerate() {
            buf.set_string(area.x + 2, area.y + 1 + i as u16, line, Style::default());
        }

        let y = area.y + area.height.saturating_sub(2);
        let (yes_fg, yes_bg, no_fg, no_bg) = if self.selected_yes {
            (Color::Black, Color::Gray, Color::White, Color::Black)
        } else {
            (Color::White, Color::Black, Color::Black, Color::Gray)
        };

        let text = vec![Spans::from(vec![
            Span::styled(
                "Y",
                Style::default()
                    .fg(yes_fg)
                    .bg(yes_bg)
                    .add_modifier(Modifier::UNDERLINED),
            ),
            Span::styled("es", Style::default().fg(yes_fg).bg(yes_bg)),
            Span::raw("   "),
            Span::styled(
                "N",
                Style::default()
                    .fg(no_fg)
                    .bg(no_bg)
                    .add_modifier(Modifier::UNDERLINED),
            ),
            Span::styled("o", Style::default().fg(no_fg).bg(no_bg)),
        ])];
        let p = Paragraph::new(text).alignment(Alignment::Center);
        let area = Rect {
            x: area.x,
            y,
            width: area.width,
            height: 1,
        };
        p.render(area, buf);
    }

    fn size(&self, _: &App) -> (u16, u16) {
        let lines = self.lines();
        let max_width = std::iter::once(Self::TITLE.width())
            .chain(lines.iter().map(|m| m.width()))
            .max()
            .unwrap();
        (4 + max_width as u16, 4 + lines.len() as u16)
    }

    fn try_finish(self: Box<Self>, _: &mut App) -> Result<(), Box<dyn Dialog>> {
        if self.chosen.unwrap_or(false) {
            let path = self.path.get_path();
            spacedisplay_lib::delete_path(&path);
            Ok(())
        } else if self.should_close {
            Ok(())
        } else {
            Err(self)
        }
    }
}
