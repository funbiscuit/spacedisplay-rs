use derivative::Derivative;

use spacedisplay_lib::{
    EntryPath, EntrySnapshot, ScanStats, Scanner, SnapshotConfig, TreeSnapshot,
};

use crate::dialog::{Dialog, NewScanDialog};
use crate::file_list::FileListState;
use crate::term::{InputHandler, InputProvider};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Screen {
    Help,
    Files,
}

#[derive(Derivative)]
#[derivative(Debug)]
pub struct App {
    pub scanner: Scanner,
    pub screen: Screen,
    pub file_list_state: FileListState,
    pub current_path: EntryPath,
    pub snapshot: Option<TreeSnapshot<EntrySnapshot>>,
    pub stats: ScanStats,
    #[derivative(Debug = "ignore")]
    pub dialog: Option<Box<dyn Dialog>>,
    pub should_quit: bool,
}

impl App {
    pub fn new(scanner: Scanner) -> Self {
        let mut state = FileListState::default();
        state.select(0);
        let current_path = scanner.get_scan_path().clone();
        let stats = scanner.stats();
        App {
            scanner,
            screen: Screen::Files,
            file_list_state: state,
            current_path,
            snapshot: None,
            stats,
            dialog: None,
            should_quit: false,
        }
    }

    pub fn check_input<H: InputProvider>(&mut self, provider: &H) {
        if let Some(mut dialog) = self.dialog.take() {
            let _ = provider.provide(&mut dialog);
            if let Err(dialog) = dialog.try_finish(self) {
                self.dialog = Some(dialog);
            }
        } else {
            let _ = provider.provide(self);
        }
    }

    pub fn on_tick(&mut self) {
        self.update_snapshot();
    }

    pub fn selected_tab(&self) -> usize {
        match self.screen {
            Screen::Files => 0,
            Screen::Help => 1,
        }
    }

    pub fn start_scan(&mut self, path: String) {
        self.scanner = Scanner::new(path);
        self.file_list_state = FileListState::default();
        self.current_path = self.scanner.get_scan_path().clone();
        self.stats = self.scanner.stats();
        self.screen = Screen::Files;
        self.snapshot = None;
    }

    pub fn tab_titles(&self) -> Vec<String> {
        let suffix = if self.scanner.is_scanning() {
            " (scanning)"
        } else {
            ""
        };
        let files = format!(
            "Files at {}{}",
            self.scanner.get_scan_path().get_name(),
            suffix
        );
        vec![files, "Help".into(), "Quit".into()]
    }

    fn select_entry(&mut self, name: &str) {
        if let Some(snapshot) = self.snapshot.as_ref() {
            self.file_list_state.select(
                snapshot
                    .get_root()
                    .iter()
                    .position(|e| e.get_name() == name)
                    .unwrap_or(0),
            );
        }
    }

    fn update_snapshot(&mut self) {
        let selected = if let Some(snapshot) = self.snapshot.as_ref() {
            snapshot
                .get_root()
                .get_nth_child(self.file_list_state.selected())
                .map(|e| e.get_name().to_string())
        } else {
            None
        };

        self.stats = self.scanner.stats();
        self.snapshot = self.scanner.get_tree(
            &self.current_path,
            SnapshotConfig {
                max_depth: 1,
                min_size: 0,
            },
        );
        if self.current_path.is_root() {
            if let Some(snapshot) = self.snapshot.as_ref() {
                // when root is opened manually set used size in stats
                self.stats.used_size = snapshot.get_root().get_size()
            }
        }

        if let Some(name) = selected {
            self.select_entry(&name);
        }
    }
}

impl InputHandler for App {
    fn on_backspace(&mut self) {
        if !self.current_path.is_root() && self.screen == Screen::Files {
            let name = self.current_path.get_name().to_string();
            self.current_path.go_up();
            self.update_snapshot();
            self.select_entry(&name);
        }
    }

    fn on_down(&mut self) {
        if self.screen == Screen::Files {
            self.file_list_state
                .select(self.file_list_state.selected() + 1);
        }
    }

    fn on_enter(&mut self) {
        if self.screen == Screen::Files {
            if let Some(snapshot) = self.snapshot.as_ref() {
                let files: Vec<_> = snapshot.get_root().iter().collect();
                if let Some(entry) = files.get(self.file_list_state.selected()) {
                    if entry.is_dir() {
                        self.current_path.join(entry.get_name().to_string());
                        self.file_list_state.select(0);
                    }
                }
            }
        }
    }

    fn on_esc(&mut self) {
        self.on_backspace();
    }

    fn on_fn(&mut self, n: u8) {
        if n == 1 {
            self.screen = Screen::Help;
        }
    }

    fn on_key(&mut self, c: char) {
        match c {
            'h' => self.screen = Screen::Help,
            'f' => self.screen = Screen::Files,
            'q' => self.should_quit = true,
            'n' => {
                self.dialog = Some(Box::new(NewScanDialog::new(
                    spacedisplay_lib::get_available_mounts(),
                )))
            }
            _ => {}
        }
    }

    fn on_left(&mut self) {
        self.on_backspace();
    }

    fn on_right(&mut self) {
        self.on_enter();
    }

    fn on_up(&mut self) {
        let selected = self.file_list_state.selected();
        if self.screen == Screen::Files && selected > 0 {
            self.file_list_state.select(selected - 1);
        }
    }
}
