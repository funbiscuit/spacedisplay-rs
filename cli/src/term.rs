use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::event::{DisableMouseCapture, EnableMouseCapture, Event as CEvent, KeyCode};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::{event, execute};
use tui::backend::{Backend, CrosstermBackend};
use tui::Terminal;

use crate::app::App;
use crate::{ui, Args};

pub trait InputHandler {
    fn on_backspace(&mut self) {}
    fn on_down(&mut self) {}
    fn on_enter(&mut self) {}
    fn on_esc(&mut self) {}
    fn on_fn(&mut self, _n: u8) {}
    fn on_key(&mut self, _c: char) {}
    fn on_left(&mut self) {}
    fn on_right(&mut self) {}
    fn on_up(&mut self) {}
}

pub trait InputProvider {
    fn provide<T: InputHandler>(&self, t: &mut T) -> Result<()>;
}

struct AppRunner<'a, B: Backend> {
    terminal: &'a mut Terminal<B>,
    tick_rate: Duration,
    simple_graphics: bool,
    last_tick: Instant,
}

impl<'a, B: Backend> AppRunner<'a, B> {
    fn new(terminal: &'a mut Terminal<B>, tick_rate: Duration, simple_graphics: bool) -> Self {
        Self {
            terminal,
            tick_rate,
            simple_graphics,
            last_tick: Instant::now(),
        }
    }

    fn run(mut self, mut app: App) -> Result<()> {
        loop {
            self.terminal
                .draw(|f| ui::draw(f, &mut app, self.simple_graphics))?;

            app.check_input(&self);
            if self.last_tick.elapsed() >= self.tick_rate {
                app.on_tick();
                self.last_tick = Instant::now();
            }
            if app.should_quit {
                return Ok(());
            }
        }
    }
}

impl<'a, B: Backend> InputProvider for AppRunner<'a, B> {
    fn provide<T: InputHandler>(&self, handler: &mut T) -> Result<()> {
        let timeout = self
            .tick_rate
            .checked_sub(self.last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));
        if event::poll(timeout)? {
            if let CEvent::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char(c) => handler.on_key(c),
                    KeyCode::Up => handler.on_up(),
                    KeyCode::Down => handler.on_down(),
                    KeyCode::Left => handler.on_left(),
                    KeyCode::Right => handler.on_right(),
                    KeyCode::Enter => handler.on_enter(),
                    KeyCode::Esc => handler.on_esc(),
                    KeyCode::Backspace => handler.on_esc(),
                    KeyCode::F(n) => handler.on_fn(n),
                    _ => {}
                }
            }
        }

        Ok(())
    }
}

pub fn run(args: Args) -> Result<()> {
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic| {
        reset_terminal().unwrap();
        original_hook(panic);
    }));

    let mut terminal = init_terminal()?;
    let runner = AppRunner::new(&mut terminal, args.tick_rate, args.simple_graphics);
    let mut app = App::new();
    if let Some(path) = args.path {
        app.start_scan(path);
    }
    let res = runner.run(app);

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
