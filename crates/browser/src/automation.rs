//! Test-mode automation channel: newline-delimited JSON over a local TCP port.
//!
//! The E2E suite launches the real app with `LLMOUSER_E2E_PORT`, connects, and
//! sends [`Command`]s. Each shell answers them by operating its real native
//! widgets (clicking buttons, typing into fields, activating menu items), so the
//! tests exercise the same paths a user does. This module only carries the
//! protocol and the socket loop; the handler runs on the shell's main thread.

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc;
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum Command {
    /// Press a toolbar button: `back`, `forward`, `go`, `stop`, `settings`, `new_tab`.
    Click {
        target: String,
    },
    /// Type into the address bar (replacing its text) without submitting.
    SetAddress {
        value: String,
    },
    /// Submit the address bar (Enter).
    SubmitAddress,
    /// Read the browser state as the widgets show it.
    State,
    /// Activate the tab at `index` by clicking it.
    ActivateTab {
        index: usize,
    },
    /// Close the tab at `index` through its close control.
    CloseTab {
        index: usize,
    },
    /// Evaluate JavaScript in the active tab's page; the expression's JSON value comes back.
    Eval {
        js: String,
    },
    /// The application menu as the user sees it.
    Menu,
    /// Activate a menu item by its id (see `MenuItemInfo::id`).
    MenuClick {
        id: String,
    },
    /// Open the page context menu (right-click at the given page point).
    ContextMenu {
        x: f64,
        y: f64,
    },
    /// Activate an item of the currently open context menu.
    ContextMenuClick {
        id: String,
    },
    /// Read the settings dialog.
    Settings,
    /// Set a settings field (`provider`, `endpoint`, `model`, `api_key`, `max_tokens`,
    /// `universe`, `language`, `search_url`), like a user editing the control.
    SettingsSet {
        field: String,
        value: String,
    },
    /// Switch the settings tab: `llm` or `ux`.
    SettingsTab {
        tab: String,
    },
    SettingsSave,
    SettingsClose,
    /// Read the About window.
    About,
    AboutClose,
    /// Press a key combination in the main window, e.g. `cmd+[`, `ctrl+t`, `escape`.
    Key {
        combo: String,
    },
    Quit,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Response {
    pub ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl Response {
    pub fn ok(value: impl Serialize) -> Response {
        Response {
            ok: true,
            value: Some(serde_json::to_value(value).unwrap_or(Value::Null)),
            error: None,
        }
    }

    pub fn unit() -> Response {
        Response {
            ok: true,
            value: None,
            error: None,
        }
    }

    pub fn err(message: impl Into<String>) -> Response {
        Response {
            ok: false,
            value: None,
            error: Some(message.into()),
        }
    }
}

/// Snapshot of the visible browser state (answer to [`Command::State`]).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct StateInfo {
    pub address: String,
    pub status: String,
    pub status_error: bool,
    pub can_back: bool,
    pub can_forward: bool,
    pub loading: bool,
    pub tabs: Vec<TabInfo>,
    pub active_tab: usize,
    pub settings_open: bool,
    pub about_open: bool,
    pub window_title: String,
    /// Placeholder of the address bar (proves retranslation).
    pub address_placeholder: String,
    /// Tooltips of the toolbar buttons, in order: back, forward, go/stop, settings, new tab.
    pub tooltips: Vec<String>,
    /// Content sequence of the active tab; the document shown carries the same
    /// number in its `llmouser-content` meta tag once it is committed.
    #[serde(default)]
    pub content_seq: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct TabInfo {
    pub title: String,
    pub active: bool,
    /// Kind of icon the tab shows: `favicon`, `letter`, `blank`, or `none` (platforms without tab icons).
    pub icon: String,
    /// Whether the tab shows a loading indicator.
    pub loading: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct MenuItemInfo {
    pub id: String,
    pub label: String,
    pub enabled: bool,
    #[serde(default)]
    pub accelerator: String,
    #[serde(default)]
    pub children: Vec<MenuItemInfo>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct SettingsInfo {
    pub open: bool,
    pub title: String,
    /// Active tab: `llm` or `ux`.
    pub tab: String,
    pub tab_labels: Vec<String>,
    /// Field labels in display order.
    pub labels: Vec<String>,
    pub provider: String,
    pub provider_options: Vec<String>,
    pub endpoint: String,
    pub model: String,
    pub api_key: String,
    pub api_key_placeholder: String,
    pub max_tokens: String,
    pub universe: String,
    pub universe_placeholder: String,
    pub language: String,
    pub language_options: Vec<String>,
    pub search_url: String,
    pub save_label: String,
    pub close_label: String,
    /// Validation message currently shown, if any.
    pub validation: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct AboutInfo {
    pub open: bool,
    pub title: String,
    pub name: String,
    pub version_line: String,
    pub copyright: String,
    pub icon_loaded: bool,
    /// Horizontal offset of the icon's center from the window's center, in points.
    pub icon_off_center: f64,
}

/// A handler the shell registers: receives a command and a way to answer it.
/// It is invoked from the socket thread and must dispatch to the main thread.
pub type Dispatch = Arc<dyn Fn(Command, mpsc::Sender<Response>) + Send + Sync>;

/// How long a single command may take to be answered by the UI thread.
pub const COMMAND_TIMEOUT: Duration = Duration::from_secs(30);

/// Start serving on `127.0.0.1:port` in a background thread.
pub fn serve(port: u16, dispatch: Dispatch) -> std::io::Result<()> {
    let listener = TcpListener::bind(("127.0.0.1", port))?;
    std::thread::Builder::new()
        .name("llmouser-automation".into())
        .spawn(move || {
            for stream in listener.incoming().flatten() {
                let dispatch = dispatch.clone();
                std::thread::spawn(move || handle_client(stream, dispatch));
            }
        })?;
    Ok(())
}

fn handle_client(stream: TcpStream, dispatch: Dispatch) {
    let mut writer = match stream.try_clone() {
        Ok(w) => w,
        Err(_) => return,
    };
    let reader = BufReader::new(stream);
    for line in reader.lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let response = match serde_json::from_str::<Command>(&line) {
            Ok(command) => {
                let (tx, rx) = mpsc::channel();
                dispatch(command, tx);
                rx.recv_timeout(COMMAND_TIMEOUT)
                    .unwrap_or_else(|_| Response::err("UI thread did not answer in time"))
            }
            Err(e) => Response::err(format!("bad command: {e}")),
        };
        let mut text = serde_json::to_string(&response).unwrap_or_else(|_| "{\"ok\":false}".into());
        text.push('\n');
        if writer.write_all(text.as_bytes()).is_err() {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    #[test]
    fn commands_round_trip_as_json() {
        let json = serde_json::to_string(&Command::SettingsSet {
            field: "model".into(),
            value: "m".into(),
        })
        .unwrap();
        assert_eq!(
            json,
            "{\"cmd\":\"settings_set\",\"field\":\"model\",\"value\":\"m\"}"
        );
        assert_eq!(
            serde_json::from_str::<Command>("{\"cmd\":\"state\"}").unwrap(),
            Command::State
        );
        let r = Response::ok(StateInfo::default());
        assert!(serde_json::to_string(&r).unwrap().contains("\"ok\":true"));
        assert_eq!(
            serde_json::to_string(&Response::unit()).unwrap(),
            "{\"ok\":true}"
        );
    }

    #[test]
    fn serves_commands_over_tcp() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);
        serve(
            port,
            Arc::new(|command, reply| {
                let response = match command {
                    Command::State => Response::ok(StateInfo {
                        address: "typed".into(),
                        ..StateInfo::default()
                    }),
                    Command::Quit => Response::unit(),
                    other => Response::err(format!("unsupported {other:?}")),
                };
                reply.send(response).unwrap();
            }),
        )
        .unwrap();

        let mut client = TcpStream::connect(("127.0.0.1", port)).unwrap();
        client.write_all(b"{\"cmd\":\"state\"}\n\n{\"cmd\":\"quit\"}\nnot json\n{\"cmd\":\"eval\",\"js\":\"1\"}\n").unwrap();
        client.shutdown(std::net::Shutdown::Write).unwrap();
        let mut text = String::new();
        client.read_to_string(&mut text).unwrap();
        let lines: Vec<Response> = text
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        assert_eq!(lines.len(), 4);
        assert_eq!(lines[0].value.as_ref().unwrap()["address"], "typed");
        assert_eq!(lines[1], Response::unit());
        assert!(lines[2].error.as_ref().unwrap().starts_with("bad command"));
        assert!(lines[3].error.as_ref().unwrap().starts_with("unsupported"));
    }
}
