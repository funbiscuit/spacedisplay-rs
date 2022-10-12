use tui::buffer::Buffer;
use tui::layout::Rect;
use tui::widgets::Widget;

pub use new_scan::NewScanDialog;

use crate::app::App;
use crate::term::InputHandler;

mod new_scan;

pub trait Dialog: InputHandler {
    fn get_widget(&self) -> DialogWidget;

    fn render(&self, area: Rect, buf: &mut Buffer);

    /// Returns size of dialog
    fn size(&self) -> (u16, u16);

    /// Attempt to finish dialog
    ///
    /// Dialog will finish if user satisfied its condition for finish
    /// In this case app might be modified and Ok is returned
    /// Otherwise Err is returned that contains this dialog
    /// so it can be checked later
    fn try_finish(self: Box<Self>, app: &mut App) -> Result<(), Box<dyn Dialog>>;
}

impl InputHandler for Box<dyn Dialog> {
    fn on_backspace(&mut self) {
        self.as_mut().on_backspace();
    }

    fn on_down(&mut self) {
        self.as_mut().on_down();
    }

    fn on_enter(&mut self) {
        self.as_mut().on_enter();
    }

    fn on_esc(&mut self) {
        self.as_mut().on_esc();
    }

    fn on_fn(&mut self, n: u8) {
        self.as_mut().on_fn(n);
    }

    fn on_key(&mut self, c: char) {
        self.as_mut().on_key(c);
    }

    fn on_left(&mut self) {
        self.as_mut().on_left();
    }

    fn on_right(&mut self) {
        self.as_mut().on_right();
    }

    fn on_up(&mut self) {
        self.as_mut().on_up();
    }
}

pub struct DialogWidget<'a>(&'a dyn Dialog);

impl<'a> Widget for DialogWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        self.0.render(area, buf);
    }
}
