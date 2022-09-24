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

#[derive(Debug, Clone)]
pub struct ProgressBar {
    parts: Vec<BarItem>,
}

impl Default for ProgressBar {
    fn default() -> ProgressBar {
        ProgressBar { parts: vec![] }
    }
}

impl ProgressBar {
    pub fn parts(mut self, parts: Vec<BarItem>) -> ProgressBar {
        self.parts = parts;
        self
    }
}

impl Widget for ProgressBar {
    fn render(self, area: Rect, buf: &mut Buffer) {
        buf.set_style(area, Style::default());
        let mut gauge_area = area;
        if gauge_area.height < 1 || gauge_area.width < 3 || self.parts.is_empty() {
            return;
        }
        gauge_area.x += 1;
        gauge_area.width -= 2;

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
    }
}

fn make_layout(items: &[BarItem], width: usize) -> Vec<(BarItem, usize)> {
    let total_weight: f64 = items.iter().map(|item| item.weight).sum();
    let items: Vec<_> = items
        .iter()
        .filter(|item| {
            item.min_ratio
                .map(|ratio| item.weight > total_weight * ratio)
                .unwrap_or(true)
        })
        .collect();

    let (str_width, mut total_weight) = items
        .iter()
        .map(|item| (item.label.width(), item.weight))
        .reduce(|a, b| (a.0 + b.0, a.1 + b.1))
        .unwrap();
    let mut total_spacing = width.saturating_sub(str_width);

    let mut widths = vec![];
    for item in &items {
        let spacing = ((total_spacing as f64) * item.weight / total_weight).round() as usize;
        widths.push(item.label.width() + spacing);
        total_spacing -= spacing;
        total_weight -= item.weight;
    }

    items.into_iter().cloned().zip(widths.into_iter()).collect()
}
