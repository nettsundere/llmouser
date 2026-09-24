//! Launches the app under test and talks to its automation socket.

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::process::{Child, Command, Stdio};
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};

use serde_json::Value;

pub use llmouser_browser::automation::{
    AboutInfo, Command as Cmd, MenuItemInfo, Response, SettingsInfo, StateInfo, TabInfo,
};
use llmouser_browser::env;

/// One app at a time: the shells are real GUI processes competing for focus.
static SERIAL: Mutex<()> = Mutex::new(());

pub const WAIT: Duration = Duration::from_secs(15);

pub struct App {
    _serial: MutexGuard<'static, ()>,
    child: Child,
    writer: TcpStream,
    reader: BufReader<TcpStream>,
    data_dir: Option<tempfile::TempDir>,
    _out_dir: tempfile::TempDir,
}

pub struct Launch {
    pub mock: bool,
    pub save_path: Option<String>,
    pub data_dir: Option<tempfile::TempDir>,
}

impl Default for Launch {
    fn default() -> Self {
        Launch {
            mock: true,
            save_path: None,
            data_dir: None,
        }
    }
}

pub fn is_macos() -> bool {
    cfg!(target_os = "macos")
}

/// Platforms whose tabs are windows (closing the last one closes the window).
pub fn tabs_are_windows() -> bool {
    is_macos()
}

pub fn modifier() -> &'static str {
    if is_macos() {
        "cmd"
    } else {
        "ctrl"
    }
}

impl App {
    pub fn launch() -> App {
        App::launch_with(Launch::default())
    }

    pub fn launch_with(options: Launch) -> App {
        let serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
        let data_dir = options
            .data_dir
            .unwrap_or_else(|| tempfile::tempdir().expect("temp dir"));
        let out_dir = tempfile::tempdir().expect("temp dir");
        let port = {
            let listener = TcpListener::bind("127.0.0.1:0").expect("free port");
            listener.local_addr().unwrap().port()
        };
        let mut command = Command::new(env!("CARGO_BIN_EXE_llmouser"));
        command
            .env(env::DATA_DIR, data_dir.path())
            .env(env::E2E_PORT, port.to_string())
            .env_remove(env::SAVE_PATH)
            .stdout(Stdio::null())
            .stderr(Stdio::inherit());
        if options.mock {
            command.env(env::MOCK, "1");
        } else {
            command.env_remove(env::MOCK);
        }
        if let Some(path) = &options.save_path {
            command.env(env::SAVE_PATH, path);
        }
        let child = command.spawn().expect("launch app");

        let deadline = Instant::now() + Duration::from_secs(30);
        let stream = loop {
            match TcpStream::connect(("127.0.0.1", port)) {
                Ok(s) => break s,
                Err(_) if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(100))
                }
                Err(e) => panic!("app did not open its automation port: {e}"),
            }
        };
        stream
            .set_read_timeout(Some(Duration::from_secs(60)))
            .unwrap();
        let reader = BufReader::new(stream.try_clone().unwrap());
        let mut app = App {
            _serial: serial,
            child,
            writer: stream,
            reader,
            data_dir: Some(data_dir),
            _out_dir: out_dir,
        };
        // Wait for the first window and its start page to be up.
        app.wait_for("first window", |a| !a.state().tabs.is_empty());
        app.wait_for("start page", |a| a.on_start_page());
        app.wait_committed();
        app
    }

    /// Relaunch on the same settings directory (persistence tests).
    pub fn relaunch(mut self) -> App {
        let data_dir = self.data_dir.take();
        drop(self);
        App::launch_with(Launch {
            data_dir,
            ..Launch::default()
        })
    }

    pub fn data_path(&self) -> std::path::PathBuf {
        self.data_dir
            .as_ref()
            .expect("data dir")
            .path()
            .to_path_buf()
    }

    pub fn send(&mut self, command: Cmd) -> Response {
        let mut line = serde_json::to_string(&command).unwrap();
        line.push('\n');
        self.writer
            .write_all(line.as_bytes())
            .expect("write command");
        let mut reply = String::new();
        self.reader.read_line(&mut reply).expect("read response");
        serde_json::from_str(&reply).unwrap_or_else(|e| panic!("bad response {reply:?}: {e}"))
    }

    pub fn ok(&mut self, command: Cmd) -> Value {
        let response = self.send(command.clone());
        assert!(
            response.ok,
            "{command:?} failed: {}",
            response.error.unwrap_or_default()
        );
        response.value.unwrap_or(Value::Null)
    }

    pub fn state(&mut self) -> StateInfo {
        serde_json::from_value(self.ok(Cmd::State)).expect("state")
    }

    pub fn eval(&mut self, js: &str) -> Value {
        self.ok(Cmd::Eval { js: js.to_string() })
    }

    /// Like [`eval`](Self::eval) but returns `None` when the webview is not yet
    /// ready (its creation is asynchronous on Windows). Used by the launch-time
    /// polling helpers so they retry instead of panicking.
    pub fn try_eval(&mut self, js: &str) -> Option<String> {
        let response = self.send(Cmd::Eval { js: js.to_string() });
        if !response.ok {
            return None;
        }
        match response.value? {
            Value::String(s) => Some(s),
            Value::Null => Some(String::new()),
            other => Some(other.to_string()),
        }
    }

    pub fn eval_str(&mut self, js: &str) -> String {
        match self.eval(js) {
            Value::String(s) => s,
            Value::Null => String::new(),
            other => other.to_string(),
        }
    }

    pub fn text(&mut self, id: &str) -> String {
        self.eval_str(&format!(
            "(document.getElementById({id:?}) || {{textContent: ''}}).textContent"
        ))
    }

    /// Whether the blank new-tab page is what the webview shows.
    pub fn on_start_page(&mut self) -> bool {
        self.try_eval("document.body && document.body.dataset.llmouser || ''")
            .map(|v| v == "start")
            .unwrap_or(false)
    }

    pub fn page_click(&mut self, id: &str) {
        self.eval(&format!("(function(){{ var e = document.getElementById({id:?}); if (!e) return 'missing'; e.click(); return 'ok' }})()"));
    }

    pub fn click(&mut self, target: &str) {
        self.ok(Cmd::Click {
            target: target.to_string(),
        });
    }

    pub fn set_address(&mut self, value: &str) {
        self.ok(Cmd::SetAddress {
            value: value.to_string(),
        });
    }

    pub fn submit(&mut self) {
        self.ok(Cmd::SubmitAddress);
    }

    /// Type an address, submit it, and wait until the page has settled.
    pub fn go(&mut self, address: &str) -> StateInfo {
        self.set_address(address);
        self.submit();
        self.wait_settled()
    }

    /// Wait until nothing is loading and the status is no longer "Loading".
    pub fn wait_settled(&mut self) -> StateInfo {
        self.wait_for("page to settle", |a| {
            let s = a.state();
            !s.loading
                && !s.status.starts_with("Loading")
                && !s.status.starts_with("Загрузка")
                && !s.status.starts_with("正在加载")
        });
        self.wait_committed()
    }

    pub fn wait_status(&mut self, expected: &str) -> StateInfo {
        let expected = expected.to_string();
        self.wait_for(&format!("status {expected:?}"), |a| {
            a.state().status == expected
        });
        self.wait_committed()
    }

    pub fn wait_status_contains(&mut self, part: &str) -> StateInfo {
        let part = part.to_string();
        self.wait_for(&format!("status containing {part:?}"), |a| {
            a.state().status.contains(&part)
        });
        self.wait_committed()
    }

    /// Sequence number of the document the webview currently shows.
    pub fn shown_seq(&mut self) -> Option<u64> {
        self.try_eval("(document.querySelector('meta[name=llmouser-content]') || {}).content || ''")
            .and_then(|v| v.parse().ok())
    }

    /// Wait until the webview shows the document the model says is current.
    pub fn wait_committed(&mut self) -> StateInfo {
        self.wait_for("document to commit", |a| {
            let expected = a.state().content_seq;
            a.shown_seq() == Some(expected)
        });
        self.state()
    }

    pub fn wait_for(&mut self, what: &str, mut condition: impl FnMut(&mut App) -> bool) {
        let deadline = Instant::now() + WAIT;
        loop {
            if condition(self) {
                return;
            }
            if Instant::now() > deadline {
                let state = self.state();
                panic!("timed out waiting for {what}; last state: {state:?}");
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }

    pub fn menu(&mut self) -> Vec<MenuItemInfo> {
        serde_json::from_value(self.ok(Cmd::Menu)).expect("menu")
    }

    pub fn menu_click(&mut self, id: &str) {
        self.ok(Cmd::MenuClick { id: id.to_string() });
    }

    pub fn context_menu(&mut self) -> Vec<MenuItemInfo> {
        serde_json::from_value(self.ok(Cmd::ContextMenu { x: 120.0, y: 120.0 }))
            .expect("context menu")
    }

    pub fn settings(&mut self) -> SettingsInfo {
        serde_json::from_value(self.ok(Cmd::Settings)).expect("settings")
    }

    pub fn open_settings(&mut self) -> SettingsInfo {
        self.click("settings");
        self.wait_for("settings window", |a| a.settings().open);
        self.settings()
    }

    pub fn settings_set(&mut self, field: &str, value: &str) {
        self.ok(Cmd::SettingsSet {
            field: field.to_string(),
            value: value.to_string(),
        });
    }

    pub fn settings_tab(&mut self, tab: &str) {
        self.ok(Cmd::SettingsTab {
            tab: tab.to_string(),
        });
    }

    pub fn settings_save(&mut self) {
        self.ok(Cmd::SettingsSave);
    }

    pub fn about(&mut self) -> AboutInfo {
        serde_json::from_value(self.ok(Cmd::About)).expect("about")
    }

    pub fn key(&mut self, combo: &str) {
        self.ok(Cmd::Key {
            combo: combo.to_string(),
        });
    }

    pub fn tabs(&mut self) -> Vec<TabInfo> {
        self.state().tabs
    }

    pub fn active_title(&mut self) -> String {
        let s = self.state();
        s.tabs
            .get(s.active_tab)
            .map(|t| t.title.clone())
            .unwrap_or_default()
    }

    pub fn new_tab(&mut self) {
        let before = self.tabs().len();
        self.click("new_tab");
        self.wait_for("new tab", |a| {
            a.tabs().len() == before + 1 && a.state().address.is_empty()
        });
        self.wait_for("new tab start page", |a| a.on_start_page());
        self.wait_committed();
    }

    pub fn activate_tab(&mut self, index: usize) {
        self.ok(Cmd::ActivateTab { index });
        self.wait_for("tab activation", |a| a.state().active_tab == index);
        self.wait_committed();
    }

    pub fn close_tab(&mut self, index: usize) {
        let before = self.tabs();
        self.ok(Cmd::CloseTab { index });
        // Either a tab disappeared, or (last tab, non-window platforms) it was replaced by a fresh one.
        self.wait_for("tab close", |a| {
            let now = a.tabs();
            now.len() != before.len() || (before.len() == 1 && now[0].title != before[0].title)
        });
    }
}

impl Drop for App {
    fn drop(&mut self) {
        let _ = self.send(Cmd::Quit);
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if let Ok(Some(_)) = self.child.try_wait() {
                return;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Find a menu item anywhere in the tree.
pub fn find_menu<'a>(items: &'a [MenuItemInfo], id: &str) -> Option<&'a MenuItemInfo> {
    for item in items {
        if item.id == id {
            return Some(item);
        }
        if let Some(found) = find_menu(&item.children, id) {
            return Some(found);
        }
    }
    None
}
