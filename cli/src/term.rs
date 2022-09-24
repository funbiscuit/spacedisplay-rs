use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::event::{DisableMouseCapture, EnableMouseCapture, Event as CEvent, KeyCode};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::{event, execute};
use tui::backend::{Backend, CrosstermBackend};
use tui::Terminal;

use spacedisplay_lib::Scanner;

use crate::app::App;
use crate::{ui, Args};

pub fn run(args: Args) -> Result<()> {
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic| {
        reset_terminal().unwrap();
        original_hook(panic);
    }));

    let mut terminal = init_terminal()?;

    let app = App::new(Scanner::new(args.path));
    let res = run_app(&mut terminal, app, args.tick_rate);

    reset_terminal()?;

    res
}

fn init_terminal() -> Result<Terminal<CrosstermBackend<std::io::Stdout>>> {
    execute!(std::io::stdout(), EnterAlternateScreen, EnableMouseCapture)?;
    enable_raw_mode()?;

    let backend = CrosstermBackend::new(std::io::stdout());
    let mut terminal = Terminal::new(backend)?;
    terminal.hide_cursor()?;

    Ok(terminal)
}

fn reset_terminal() -> Result<()> {
    disable_raw_mode()?;
    execute!(std::io::stdout(), LeaveAlternateScreen, DisableMouseCapture)?;

    Ok(())
}

fn run_app<B: Backend>(
    terminal: &mut Terminal<B>,
    mut app: App,
    tick_rate: Duration,
) -> Result<()> {
    let mut last_tick = Instant::now();
    loop {
        terminal.draw(|f| ui::draw(f, &mut app))?;

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));
        if event::poll(timeout)? {
            if let CEvent::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char(c) => app.on_key(c),
                    KeyCode::Up => app.on_up(),
                    KeyCode::Down => app.on_down(),
                    KeyCode::Left => app.on_left(),
                    KeyCode::Right => app.on_right(),
                    KeyCode::Enter => app.on_enter(),
                    KeyCode::Esc => app.on_esc(),
                    KeyCode::Backspace => app.on_esc(),
                    KeyCode::F(n) => app.on_fn(n),
                    _ => {}
                }
            }
        }
        if last_tick.elapsed() >= tick_rate {
            app.on_tick();
            last_tick = Instant::now();
        }
        if app.should_quit {
            return Ok(());
        }
    }
}
