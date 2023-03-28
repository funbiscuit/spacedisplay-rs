use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Spans;
use ratatui::widgets::{
    Block, BorderType, Borders, Clear, List, ListItem, ListState, StatefulWidget, Widget,
};
use unicode_width::UnicodeWidthStr;

use crate::app::App;
use crate::dialog::{Dialog, DialogWidget};
use crate::term::InputHandler;

pub struct NewScanDialog {
    mounts: Vec<String>,
    selected: usize,
    chosen: Option<usize>,
    should_close: bool,
}

impl NewScanDialog {
    const TITLE: &'static str = "New Scan ";

    pub fn new(mounts: Vec<String>) -> Self {
        Self {
            mounts,
            selected: 0,
            chosen: None,
            should_close: false,
        }
    }
}

impl InputHandler for NewScanDialog {
    fn on_down(&mut self) {
        if self.selected + 1 < self.mounts.len() {
            self.selected += 1;
        }
    }

    fn on_enter(&mut self) {
        self.chosen = Some(self.selected);
    }

    fn on_esc(&mut self) {
        self.should_close = true;
    }

    fn on_key(&mut self, c: char) {
        self.should_close = c == 'q' || c == 'n';
    }

    fn on_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }
}

impl Dialog for NewScanDialog {
    fn get_widget<'a>(&'a self, app: &'a App) -> DialogWidget<'_> {
        DialogWidget(self, app)
    }

    fn render(&self, _: &App, area: Rect, buf: &mut Buffer) {
        Clear.render(area, buf);
        buf.set_style(area, Style::default().bg(Color::Black));

        let items: Vec<_> = self
            .mounts
            .iter()
            .map(|file| ListItem::new(Spans::from(file.as_str())))
            .collect();

        let list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .style(Style::default().fg(Color::White))
                    .title(Self::TITLE)
                    .border_type(BorderType::Plain),
            )
            .highlight_symbol(" > ")
            .highlight_style(Style::default().add_modifier(Modifier::BOLD));
        let mut state = ListState::default();
        state.select(Some(self.selected));
        StatefulWidget::render(list, area, buf, &mut state);
    }

    fn size(&self, _: &App) -> (u16, u16) {
        let max_width = std::iter::once(Self::TITLE.width())
            .chain(self.mounts.iter().map(|m| m.width() + 4))
            .max()
            .unwrap_or(0);
        (2 + max_width as u16, 2 + self.mounts.len() as u16)
    }

    fn try_finish(mut self: Box<Self>, app: &mut App) -> Result<(), Box<dyn Dialog>> {
        if let Some(mount) = self.chosen {
            app.start_scan(self.mounts.swap_remove(mount));
            return Ok(());
        }

        if self.should_close {
            Ok(())
        } else {
            Err(self)
        }
    }
}
