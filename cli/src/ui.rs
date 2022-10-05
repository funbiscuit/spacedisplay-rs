use byte_unit::Byte;
use tui::backend::Backend;
use tui::layout::{Alignment, Constraint, Direction, Layout};
use tui::style::{Color, Modifier, Style};
use tui::text::{Span, Spans};
use tui::widgets::{Block, BorderType, Borders, Paragraph};
use tui::Frame;

use spacedisplay_lib::SnapshotConfig;

use crate::app::{App, Screen};
use crate::file_list::{FileList, FileListItem};
use crate::progressbar::{BarItem, ProgressBar};
use crate::utils;

pub fn draw(frame: &mut Frame<impl Backend>, app: &mut App) {
    match app.screen {
        Screen::Help => render_controls(frame),
        Screen::Files => render_files(frame, app),
    }
}

fn render_controls(frame: &mut Frame<impl Backend>) {
    let lines = vec![
        Spans::from(vec![Span::raw("Welcome to")]),
        Spans::from(vec![Span::styled(
            "spacedisplay-cli",
            Style::default().fg(Color::LightYellow),
        )]),
        Spans::from(vec![Span::raw("")]),
        Spans::from(vec![Span::raw("Press:")]),
        Spans::from(vec![Span::raw("'H' or 'F1' to return to this screen")]),
        Spans::from(vec![Span::raw("'F' to open files list")]),
        Spans::from(vec![Span::raw("'Up' and 'Down' to move inside list")]),
        Spans::from(vec![Span::raw(
            "'Enter' or 'Right' to open selected directory",
        )]),
        Spans::from(vec![Span::raw(
            "'Esc', 'Backspace' or 'Left' to navigate up",
        )]),
        Spans::from(vec![Span::raw("'Q' to quit")]),
    ];

    let text_height = lines.len() as u16;

    let home = Paragraph::new(lines).alignment(Alignment::Center);

    frame.render_widget(
        Block::default()
            .borders(Borders::ALL)
            .style(Style::default().fg(Color::White))
            .border_type(BorderType::Plain),
        frame.size(),
    );
    let mut rect = frame.size();
    if rect.height >= text_height {
        rect.y = (rect.height - text_height) / 2;
        rect.height = text_height;
    }
    frame.render_widget(home, rect);
}

fn render_files(frame: &mut Frame<impl Backend>, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(10), Constraint::Length(1)].as_ref())
        .split(frame.size());

    let list = create_files_list(app);
    let progressbar = create_progressbar(app);

    frame.render_stateful_widget(list, chunks[0], &mut app.file_list_state);
    frame.render_widget(progressbar, chunks[1]);
}

fn create_files_list(app: &mut App) -> FileList<'static> {
    let tree = app
        .scanner
        .get_tree(
            &app.current_path,
            SnapshotConfig {
                max_depth: 1,
                min_size: 0,
            },
        )
        .unwrap();
    let files: Vec<_> = tree.get_root().iter().collect();
    if app.file_list_state.selected() >= files.len() && !files.is_empty() {
        app.file_list_state.select(files.len() - 1);
    }

    let items: Vec<_> = files
        .into_iter()
        .map(|file| {
            FileListItem::new(file.get_name().to_string(), file.get_size()).style(
                if file.is_dir() {
                    Style::default().fg(Color::LightYellow)
                } else {
                    Style::default().fg(Color::LightBlue)
                },
            )
        })
        .collect();

    let list = FileList::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .style(Style::default().fg(Color::White))
                .title(app.current_path.to_string())
                .border_type(BorderType::Plain),
        )
        .highlight_style(Style::default().add_modifier(Modifier::BOLD));

    list
}

fn create_progressbar(app: &App) -> ProgressBar {
    let mut items = vec![];
    let stats = &app.stats;
    let used = stats.used_size.get_bytes();
    if let Some(snapshot) = app.snapshot.as_ref() {
        let current = snapshot.get_root().get_size();
        let invisible = Byte::from_bytes(used.saturating_sub(current.get_bytes()));
        items.push(BarItem {
            label: utils::byte_to_str(current, 1),
            weight: current.get_bytes() as f64,
            bg: Color::LightBlue,
            fg: Color::White,
            min_ratio: None,
        });
        if invisible.get_bytes() > 0 {
            items.push(BarItem {
                label: utils::byte_to_str(invisible, 1),
                weight: invisible.get_bytes() as f64,
                bg: Color::Blue,
                fg: Color::White,
                min_ratio: None,
            });
        }
    }
    if let Some(available) = stats.available_size {
        if let Some(total) = stats.total_size {
            let unknown = total
                .get_bytes()
                .saturating_sub(available.get_bytes() + used);
            if unknown > 0 {
                items.push(BarItem {
                    label: utils::byte_to_str(Byte::from_bytes(unknown), 1),
                    weight: if stats.is_mount_point {
                        unknown as f64
                    } else {
                        0.0
                    },
                    bg: Color::Gray,
                    fg: Color::Black,
                    min_ratio: None,
                });
            }
        }
        items.push(BarItem {
            label: utils::byte_to_str(available, 1),
            weight: if stats.is_mount_point {
                available.get_bytes() as f64
            } else {
                0.0
            },
            bg: Color::Green,
            fg: Color::White,
            min_ratio: None,
        });
    }

    ProgressBar::default().parts(items)
}
