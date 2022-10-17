use derivative::Derivative;

use spacedisplay_lib::{
    EntryPath, EntrySnapshot, ScanStats, Scanner, SnapshotConfig, TreeSnapshot,
};

use crate::dialog::{Dialog, NewScanDialog, ScanStatsDialog};
use crate::file_list::FileListState;
use crate::term::{InputHandler, InputProvider};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Screen {
    Help,
    Files,
}

#[derive(Debug)]
pub struct FilesApp {
    pub scanner: Scanner,
    pub file_list_state: FileListState,
    pub current_path: EntryPath,
    pub snapshot: Option<TreeSnapshot<EntrySnapshot>>,
    pub stats: ScanStats,
}

impl FilesApp {
    pub fn new_scan(path: String) -> Self {
        let scanner = Scanner::new(path);
        let file_list_state = FileListState::default();
        let current_path = scanner.get_scan_path().clone();
        let stats = scanner.stats();
        FilesApp {
            scanner,
            file_list_state,
            current_path,
            snapshot: None,
            stats,
        }
    }

    pub fn go_up(&mut self) {
        if !self.current_path.is_root() {
            let name = self.current_path.get_name().to_string();
            self.current_path.go_up();
            self.update_snapshot();
            self.select_entry(&name);
        }
    }

    pub fn open_selected(&mut self) {
        if let Some(snapshot) = self.snapshot.as_ref() {
            let files: Vec<_> = snapshot.get_root().iter().collect();
            if let Some(entry) = files.get(self.file_list_state.selected()) {
                if entry.is_dir() {
                    self.current_path.join(entry.get_name().to_string());
                    self.file_list_state.select(0);
                    self.snapshot = None;
                    self.update_snapshot();
                }
            }
        }
    }

    pub fn rescan(&mut self) {
        self.scanner.rescan_path(self.current_path.clone());
    }

    pub fn select_down(&mut self) {
        self.file_list_state
            .select(self.file_list_state.selected() + 1);
    }

    pub fn select_entry(&mut self, name: &str) {
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

    pub fn select_up(&mut self) {
        self.file_list_state
            .select(self.file_list_state.selected().saturating_sub(1));
    }

    pub fn tab_title(&self) -> String {
        let suffix = if self.scanner.is_scanning() {
            " (scanning)"
        } else {
            ""
        };
        format!(
            "Files at {}{}",
            self.scanner.get_scan_path().get_name(),
            suffix
        )
    }

    pub fn update_snapshot(&mut self) {
        let selected = self.snapshot.as_ref().and_then(|snapshot| {
            snapshot
                .get_root()
                .get_nth_child(self.file_list_state.selected())
                .map(|e| e.get_name().to_string())
        });

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

#[derive(Derivative)]
#[derivative(Debug)]
pub struct App {
    pub files: Option<FilesApp>,
    pub screen: Screen,
    #[derivative(Debug = "ignore")]
    pub dialog: Option<Box<dyn Dialog>>,
    pub should_quit: bool,
}

impl App {
    pub fn new() -> Self {
        App {
            files: None,
            screen: Screen::Help,
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
        self.files.as_mut().map(FilesApp::update_snapshot);
    }

    pub fn selected_tab(&self) -> usize {
        match self.screen {
            Screen::Files => 0,
            Screen::Help if self.files.is_none() => 0,
            Screen::Help => 1,
        }
    }

    pub fn start_scan(&mut self, path: String) {
        self.files = Some(FilesApp::new_scan(path));
        self.screen = Screen::Files;
    }

    pub fn tab_titles(&self) -> Vec<String> {
        let mut titles = if let Some(files) = &self.files {
            vec![files.tab_title()]
        } else {
            vec![]
        };
        titles.append(&mut vec!["Help".into(), "New scan".into()]);
        if self.screen == Screen::Files {
            titles.push("Scan stats".into());
        }
        titles.push("Quit".into());
        titles
    }
}

impl InputHandler for App {
    fn on_backspace(&mut self) {
        if self.screen == Screen::Files {
            self.files.as_mut().map(FilesApp::go_up);
        }
    }

    fn on_down(&mut self) {
        if self.screen == Screen::Files {
            self.files.as_mut().map(FilesApp::select_down);
        }
    }

    fn on_enter(&mut self) {
        if self.screen == Screen::Files {
            self.files.as_mut().map(FilesApp::open_selected);
        }
    }

    fn on_esc(&mut self) {
        self.on_backspace();
    }

    fn on_fn(&mut self, n: u8) {
        match n {
            1 => self.screen = Screen::Help,
            5 if self.screen == Screen::Files => self.files.as_mut().unwrap().rescan(),
            _ => {}
        }
    }

    fn on_key(&mut self, c: char) {
        match c {
            'f' if self.files.is_some() => self.screen = Screen::Files,
            'h' => self.screen = Screen::Help,
            'n' => {
                self.dialog = Some(Box::new(NewScanDialog::new(
                    spacedisplay_lib::get_available_mounts(),
                )))
            }
            'r' if self.screen == Screen::Files => self.files.as_mut().unwrap().rescan(),
            'q' => self.should_quit = true,
            's' if self.screen == Screen::Files => {
                self.dialog = Some(Box::new(ScanStatsDialog::new()))
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
        if self.screen == Screen::Files {
            self.files.as_mut().map(FilesApp::select_up);
        }
    }
}
