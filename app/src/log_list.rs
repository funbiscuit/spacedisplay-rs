use log::Level;
use time::format_description::FormatItem;
use tui::buffer::Buffer;
use tui::layout::Rect;
use tui::style::{Color, Modifier, Style};
use tui::text::{Span, Spans};
use tui::widgets::{Block, StatefulWidget, Widget};

use crate::logger::LogEntry;

#[derive(Debug, Clone, Default)]
pub struct LogListState {
    offset: usize,
    follow: bool,
    move_pages: isize,
}

impl LogListState {
    pub fn move_down(&mut self) {
        self.offset += 1;
    }

    pub fn move_home(&mut self) {
        self.offset = 0;
        self.follow = false;
    }

    pub fn move_up(&mut self) {
        self.offset = self.offset.saturating_sub(1);
        self.follow = false;
    }

    pub fn page_down(&mut self) {
        self.move_pages += 1;
    }

    pub fn page_up(&mut self) {
        self.move_pages -= 1;
        self.follow = false;
    }

    pub fn set_follow(&mut self, follow: bool) {
        self.follow = follow;
    }
}

#[derive(Debug, Clone)]
pub struct LogList<'a> {
    block: Option<Block<'a>>,
    entries: &'a [LogEntry],
    time_formatter: &'static [FormatItem<'static>],
}

impl<'a> LogList<'a> {
    pub fn new(entries: &'a [LogEntry]) -> LogList<'a> {
        let time_formatter = time::macros::format_description!(
            "[year]-[month]-[day] [hour]:[minute]:[second].[subsecond digits:3]"
        );

        LogList {
            block: None,
            entries,
            time_formatter,
        }
    }

    pub fn block(mut self, block: Block<'a>) -> LogList<'a> {
        self.block = Some(block);
        self
    }

    fn get_items_bounds(
        &self,
        offset: usize,
        max_height: usize,
        move_pages: isize,
        follow: bool,
    ) -> (usize, usize) {
        let offset = if move_pages < 0 {
            offset.saturating_sub((-move_pages * max_height as isize) as usize)
        } else {
            offset + (move_pages as usize) * max_height
        };
        let offset = offset.min(self.entries.len().saturating_sub(1));
        let height = max_height.min(self.entries.len().saturating_sub(offset));
        let mut start = offset;
        let mut end = start + height;

        if follow && end < self.entries.len() {
            start += self.entries.len() - end;
            end = self.entries.len();
        }

        if (end - start) < max_height {
            start = start.saturating_sub(max_height - (end - start));
        }

        (start, end)
    }

    fn level_color(level: Level) -> Color {
        match level {
            Level::Error => Color::Red,
            Level::Warn => Color::Yellow,
            Level::Info => Color::Green,
            _ => Color::DarkGray,
        }
    }
}

impl<'a> StatefulWidget for LogList<'a> {
    type State = LogListState;

    fn render(mut self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        buf.set_style(area, Style::default());
        let list_area = match self.block.take() {
            Some(b) => {
                let inner_area = b.inner(area);
                b.render(area, buf);
                inner_area
            }
            None => area,
        };

        if list_area.width < 1 || list_area.height < 1 {
            return;
        }

        if self.entries.is_empty() {
            return;
        }
        let list_height = list_area.height as usize;

        let (start, end) =
            self.get_items_bounds(state.offset, list_height, state.move_pages, state.follow);
        state.offset = start;
        state.move_pages = 0;
        if end == self.entries.len() {
            state.follow = true;
        }

        let scroll_height = std::cmp::max(1, (end - start) * list_height / self.entries.len());
        let mut scroll_offset = (list_height - scroll_height) * start
            / (self.entries.len().saturating_sub(list_height + 1) + 1);
        if start > 0 {
            scroll_offset = std::cmp::max(1, scroll_offset);
        }
        if end < self.entries.len() {
            scroll_offset =
                std::cmp::min(list_height.saturating_sub(2 + scroll_height), scroll_offset);
        }

        for (i, item) in self
            .entries
            .iter()
            .skip(state.offset)
            .enumerate()
            .take(end - start)
        {
            let (x, y) = (list_area.left(), list_area.top() + i as u16);

            let time = item
                .timestamp
                .format(&self.time_formatter)
                .unwrap()
                .to_string();

            let mut spans = vec![
                Span::styled(time, Style::default().fg(Color::DarkGray)),
                Span::raw(" "),
                Span::styled(
                    item.level.as_str(),
                    Style::default()
                        .fg(LogList::level_color(item.level))
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" "),
            ];
            if let Some(module) = &item.module {
                spans.push(Span::styled(module, Style::default()));
                spans.push(Span::raw(" "));
            }
            spans.push(Span::styled(
                &item.text,
                Style::default().add_modifier(Modifier::BOLD),
            ));

            buf.set_spans(x, y, &Spans::from(spans), list_area.width - 1);

            if self.entries.len() > list_height
                && i >= scroll_offset
                && i <= scroll_offset + scroll_height
            {
                buf.set_string(
                    x + list_area.width - 1,
                    y,
                    tui::symbols::line::VERTICAL,
                    Style::default(),
                )
            }
        }
    }
}

impl<'a> Widget for LogList<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let mut state = LogListState::default();
        StatefulWidget::render(self, area, buf, &mut state);
    }
}
