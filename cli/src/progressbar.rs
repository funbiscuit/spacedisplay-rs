use num_format::{CustomFormat, Grouping};
use tui::buffer::Buffer;
use tui::layout::Rect;
use tui::style::{Color, Style};
use tui::text::Span;
use tui::widgets::Widget;
use unicode_width::UnicodeWidthStr;

#[derive(Debug, Clone)]
pub struct BarItem {
    pub label: String,
    pub weight: f64,
    pub bg: Color,
    pub fg: Color,
    pub min_ratio: Option<f64>,
}

#[derive(Clone, Debug, Default)]
pub struct ProgressBar {
    parts: Vec<BarItem>,
    files: u32,
}

impl ProgressBar {
    pub fn files(mut self, files: u32) -> ProgressBar {
        self.files = files;
        self
    }
    pub fn parts(mut self, parts: Vec<BarItem>) -> ProgressBar {
        self.parts = parts;
        self
    }
}

impl Widget for ProgressBar {
    fn render(self, area: Rect, buf: &mut Buffer) {
        buf.set_style(area, Style::default());

        let files = {
            let format = CustomFormat::builder()
                .grouping(Grouping::Standard)
                .separator(" ")
                .build()
                .unwrap();

            let mut buf = num_format::Buffer::new();
            buf.write_formatted(&(self.files), &format);

            format!("{} files ", buf.as_str())
        };
        let mut gauge_area = area;
        if gauge_area.height < 1
            || gauge_area.width < 3 + files.width() as u16
            || self.parts.is_empty()
        {
            return;
        }
        gauge_area.x += 1 + files.width() as u16;
        gauge_area.width -= 2 + files.width() as u16;

        let parts = make_layout(&self.parts, gauge_area.width as usize);

        let mut x = gauge_area.x;
        for (item, width) in parts {
            let label = Span::from(item.label.as_ref());
            let offset = (width - label.width()) as u16 / 2;

            buf.set_string(
                x,
                gauge_area.y,
                " ".repeat(width),
                Style::default().bg(item.bg).fg(item.fg),
            );
            buf.set_span(x + offset, gauge_area.top(), &label, width as u16);
            //todo add fractions

            x += width as u16;
        }
        buf.set_string(1, gauge_area.y, files, Style::default());
    }
}

fn make_layout(items: &[BarItem], width: usize) -> Vec<(BarItem, usize)> {
    // remove items that have too small ratio
    let total_weight: f64 = items.iter().map(|item| item.weight).sum();
    let items: Vec<_> = items
        .iter()
        .filter(|item| {
            item.min_ratio
                .map(|ratio| item.weight > total_weight * ratio)
                .unwrap_or(true)
        })
        .cloned()
        .collect();

    // calc total width for labels and total weight of weights
    let (str_width, total_weight) = items
        .iter()
        .map(|item| (item.label.width(), item.weight))
        .reduce(|a, b| (a.0 + b.0, a.1 + b.1))
        .unwrap();

    if width <= str_width {
        // we don't have enough space, so just use min sizes
        items
            .into_iter()
            .map(|i| {
                let width = i.label.width();
                (i, width)
            })
            .collect()
    } else {
        let mut widths = Vec::with_capacity(items.len());
        let mut width_available = 0.0;
        let mut total_width = 0 as f64;
        for item in &items {
            let item_width = ((width as f64) * item.weight / total_weight).round();
            let min_width = item.label.width() as f64;
            let item_width = f64::max(min_width, item_width);
            widths.push(item_width);
            if item_width > min_width {
                width_available += item_width - min_width;
            }
            total_width += item_width;
        }
        let mut overdraw = total_width - width as f64;

        // remove some space from items that have it to compensate
        // for overdraw
        let items: Vec<_> = items
            .into_iter()
            .zip(widths.into_iter())
            .map(|(item, mut width)| {
                let available = width - item.label.width() as f64;
                if available > 0.0 {
                    let sub = f64::min(
                        ((available / width_available) * overdraw).round(),
                        available,
                    );
                    width_available -= available;
                    overdraw -= sub;
                    width -= sub;
                }
                (item, width.round() as usize)
            })
            .collect();

        items
    }
}
