use std::cmp;

use byte_unit::Byte;
use tui::buffer::Buffer;
use tui::layout::Rect;
use tui::style::{Color, Style};
use tui::widgets::{Block, StatefulWidget, Widget};
use unicode_width::UnicodeWidthStr;

use crate::utils;

#[derive(Debug, Clone, Default)]
pub struct FileListState {
    offset: usize,
    selected: usize,
    busy_item: Option<usize>,
    spinner_state: usize,
}

impl FileListState {
    pub fn selected(&self) -> usize {
        self.selected
    }

    pub fn select(&mut self, index: usize) {
        self.selected = index;
    }

    pub fn set_busy_item(&mut self, busy_item: Option<usize>) {
        self.busy_item = busy_item;
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileListItem {
    name: String,
    size: Byte,
    style: Style,
}

impl FileListItem {
    pub fn new(name: String, size: Byte) -> FileListItem {
        FileListItem {
            name,
            size,
            style: Style::default(),
        }
    }

    pub fn style(mut self, style: Style) -> FileListItem {
        self.style = style;
        self
    }
}

#[derive(Debug, Clone)]
pub struct FileList<'a> {
    block: Option<Block<'a>>,
    items: Vec<FileListItem>,
    highlight_style: Style,
    simple_graphics: bool,
}

const SPINNER: [char; 10] = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
const SPINNER_SIMPLE: [char; 4] = ['/', '-', '\\', '|'];

impl<'a> FileList<'a> {
    pub fn new<T>(items: T) -> FileList<'a>
    where
        T: Into<Vec<FileListItem>>,
    {
        FileList {
            block: None,
            items: items.into(),
            highlight_style: Style::default(),
            simple_graphics: false,
        }
    }

    pub fn block(mut self, block: Block<'a>) -> FileList<'a> {
        self.block = Some(block);
        self
    }

    pub fn highlight_style(mut self, style: Style) -> FileList<'a> {
        self.highlight_style = style;
        self
    }

    pub fn simple_graphics(mut self, simple_graphics: bool) -> FileList<'a> {
        self.simple_graphics = simple_graphics;
        self
    }

    fn get_items_bounds(
        &self,
        selected: usize,
        offset: usize,
        max_height: usize,
    ) -> (usize, usize) {
        let offset = offset.min(self.items.len().saturating_sub(1));
        let mut height = max_height.min(self.items.len().saturating_sub(offset));
        let mut start = offset;
        let mut end = offset + height;

        let selected = selected.min(self.items.len() - 1);
        // if selection is not in bounds, adjust bounds
        if selected >= end {
            height += selected + 1 - end;
            end = selected + 1;
            if height > max_height {
                start += height - max_height;
            }
        } else if selected < start {
            height += start - selected;
            start = selected;
            if height > max_height {
                end -= height - max_height;
            }
        }
        (start, end)
    }
}

impl<'a> StatefulWidget for FileList<'a> {
    type State = FileListState;

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

        if self.items.is_empty() {
            return;
        }
        let list_height = list_area.height as usize;

        let (start, end) = self.get_items_bounds(state.selected, state.offset, list_height);
        state.offset = start;

        let highlight_symbol = " > ";
        let spinner = if self.simple_graphics {
            &SPINNER_SIMPLE[..]
        } else {
            &SPINNER[..]
        };
        let busy_symbol = spinner[state.spinner_state].to_string();
        let blank_symbol = " ".repeat(3);
        // space between elements
        let spaces = 5;

        let total_size: u64 = self.items.iter().map(|f| f.size.get_bytes()).sum();

        for (i, item) in self
            .items
            .iter_mut()
            .enumerate()
            .skip(state.offset)
            .take(end - start)
        {
            let (x, y) = (
                list_area.left(),
                list_area.top() + (i - state.offset) as u16,
            );
            let area = Rect {
                x,
                y,
                width: list_area.width,
                height: 1,
            };
            let item_style = item.style;
            buf.set_style(area, item_style);

            let is_selected = state.selected == i;
            let symbol = if is_selected {
                highlight_symbol
            } else {
                &blank_symbol
            };
            let max_name_width = cmp::min(30, list_area.width);
            let (elem_x, max_name_width) = {
                let (elem_x, _) =
                    buf.set_stringn(x, y, symbol, max_name_width as usize, item_style);
                (elem_x, (max_name_width - (elem_x - x)) as u16)
            };
            let line = &item.name;
            buf.set_stringn(elem_x, y as u16, line, max_name_width as usize, item.style);

            if is_selected {
                buf.set_style(area, self.highlight_style);
            }
            if state.busy_item == Some(i) {
                buf.set_string(
                    elem_x + max_name_width + 1,
                    y,
                    busy_symbol.clone(),
                    Style::default().fg(Color::LightYellow),
                );
            }

            let size_str = utils::byte_to_str(item.size, 0);
            let size_width = list_area.width.saturating_sub(
                max_name_width + (size_str.width() + highlight_symbol.width()) as u16 + spaces,
            );

            let size = (item.size.get_bytes() as f64 * size_width as f64) / total_size as f64;
            let size_full = size as u64;
            let size_frac = size - size_full as f64;

            if self.simple_graphics {
                buf.set_string(
                    elem_x + max_name_width + 3,
                    y,
                    " ".repeat(size_full as usize),
                    Style::default().bg(Color::LightYellow),
                );
            } else {
                let mut str = utils::get_unicode_block(1.0).repeat(size_full as usize);
                str.push_str(utils::get_unicode_block(size_frac));

                buf.set_string(
                    elem_x + max_name_width + 3,
                    y,
                    str,
                    Style::default().fg(Color::LightYellow),
                );
            }

            buf.set_string(
                elem_x + max_name_width + 5 + size_full as u16,
                y,
                &size_str,
                Style::default().fg(Color::LightYellow),
            );
        }

        state.spinner_state = (state.spinner_state + 1) % spinner.len();
    }
}

impl<'a> Widget for FileList<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let mut state = FileListState::default();
        StatefulWidget::render(self, area, buf, &mut state);
    }
}
