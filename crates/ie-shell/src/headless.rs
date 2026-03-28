use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use url::Url;

use crate::bookmarks::BookmarkStore;
use crate::cli::HeadlessAction;
use crate::ipc_navigator::IpcNavigator;
use crate::navigation::{InProcessNavigator, NavigationService};
use crate::tab::{TabId, TabManager, TabState};

pub fn run_headless(
    url: Option<Url>,
    action: HeadlessAction,
    allow_http: bool,
    data_dir: Option<String>,
    single_process: bool,
) -> Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        match action {
            HeadlessAction::DumpSource => {
                run_dump_source(url.unwrap(), allow_http, single_process).await
            }
            HeadlessAction::DumpStatus => {
                run_dump_status(url.unwrap(), allow_http, single_process).await
            }
            HeadlessAction::Interactive => {
                run_interactive(allow_http, data_dir, single_process).await
            }
        }
    })
}

struct NavigatorHandle {
    navigator: Arc<dyn NavigationService + Send + Sync>,
    _child: Option<ie_sandbox::ChildHandle>,
}

async fn create_navigator(allow_http: bool, single_process: bool) -> Result<NavigatorHandle> {
    if single_process {
        let nav = InProcessNavigator::new()?.with_https_only(!allow_http);
        Ok(NavigatorHandle {
            navigator: Arc::new(nav),
            _child: None,
        })
    } else {
        let mut child = ie_sandbox::spawn_child(ie_sandbox::ProcessKind::Network).await?;
        let channel = child.take_channel();
        let nav = IpcNavigator::new(channel, !allow_http);
        Ok(NavigatorHandle {
            navigator: Arc::new(nav),
            _child: Some(child),
        })
    }
}

async fn run_dump_source(url: Url, allow_http: bool, single_process: bool) -> Result<()> {
    let handle = create_navigator(allow_http, single_process).await?;
    match handle.navigator.navigate(&url).await {
        Ok(result) => {
            let text = String::from_utf8_lossy(&result.body);
            print!("{text}");
            Ok(())
        }
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    }
}

async fn run_dump_status(url: Url, allow_http: bool, single_process: bool) -> Result<()> {
    let handle = create_navigator(allow_http, single_process).await?;
    match handle.navigator.navigate(&url).await {
        Ok(result) => {
            println!("{}", result.status);
            Ok(())
        }
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    }
}

// --- Interactive headless mode ---

#[derive(Deserialize)]
#[serde(tag = "cmd")]
enum Command {
    #[serde(rename = "navigate")]
    Navigate { url: String },
    #[serde(rename = "get_source")]
    GetSource,
    #[serde(rename = "get_tabs")]
    GetTabs,
    #[serde(rename = "new_tab")]
    NewTab,
    #[serde(rename = "close_tab")]
    CloseTab { id: u64 },
    #[serde(rename = "switch_tab")]
    SwitchTab { id: u64 },
    #[serde(rename = "go_back")]
    GoBack,
    #[serde(rename = "go_forward")]
    GoForward,
    #[serde(rename = "bookmark_add")]
    BookmarkAdd { url: String, title: String },
    #[serde(rename = "bookmark_list")]
    BookmarkList,
    #[serde(rename = "quit")]
    Quit,
}

#[derive(Serialize)]
struct Response {
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

impl Response {
    fn ok_empty() -> Self {
        Self {
            ok: true,
            data: None,
            error: None,
        }
    }

    fn ok_data(data: serde_json::Value) -> Self {
        Self {
            ok: true,
            data: Some(data),
            error: None,
        }
    }

    fn err(msg: String) -> Self {
        Self {
            ok: false,
            data: None,
            error: Some(msg),
        }
    }
}

struct HeadlessSession {
    tab_manager: TabManager,
    bookmark_store: BookmarkStore,
    navigator: Arc<dyn NavigationService + Send + Sync>,
    _child: Option<ie_sandbox::ChildHandle>,
}

impl HeadlessSession {
    async fn new(allow_http: bool, data_dir: Option<String>, single_process: bool) -> Result<Self> {
        let handle = create_navigator(allow_http, single_process).await?;
        let bookmark_path = data_dir
            .map(PathBuf::from)
            .unwrap_or_else(|| std::env::temp_dir().join("ie-headless"));
        let bookmark_store = BookmarkStore::new(&bookmark_path)?;
        Ok(Self {
            tab_manager: TabManager::new(),
            bookmark_store,
            navigator: handle.navigator,
            _child: handle._child,
        })
    }

    async fn handle_command(&mut self, cmd: Command) -> Response {
        match cmd {
            Command::Navigate { url } => self.handle_navigate(url).await,
            Command::GetSource => self.handle_get_source(),
            Command::GetTabs => self.handle_get_tabs(),
            Command::NewTab => self.handle_new_tab(),
            Command::CloseTab { id } => self.handle_close_tab(id),
            Command::SwitchTab { id } => self.handle_switch_tab(id),
            Command::GoBack => self.handle_go_back(),
            Command::GoForward => self.handle_go_forward(),
            Command::BookmarkAdd { url, title } => self.handle_bookmark_add(url, title),
            Command::BookmarkList => self.handle_bookmark_list(),
            Command::Quit => Response::ok_empty(),
        }
    }

    async fn handle_navigate(&mut self, input: String) -> Response {
        let url = match Url::parse(&input) {
            Ok(url) => url,
            Err(_) => match Url::parse(&format!("https://{input}")) {
                Ok(url) => url,
                Err(e) => return Response::err(format!("invalid URL: {e}")),
            },
        };

        if let Some(tab) = self.tab_manager.active_tab_mut() {
            tab.state = TabState::Loading;
            tab.url = Some(url.clone());
        }

        match self.navigator.navigate(&url).await {
            Ok(result) => {
                let source = String::from_utf8(result.body).ok();
                if let Some(tab) = self.tab_manager.active_tab_mut() {
                    tab.state = TabState::Loaded;
                    tab.title = result
                        .final_url
                        .host_str()
                        .unwrap_or("Untitled")
                        .to_string();
                    tab.push_history(result.final_url.clone(), source.clone());
                    tab.source = source;
                }
                Response::ok_data(serde_json::json!({
                    "status": result.status,
                    "url": result.final_url.as_str(),
                }))
            }
            Err(e) => {
                if let Some(tab) = self.tab_manager.active_tab_mut() {
                    tab.state = TabState::Error(e.to_string());
                }
                Response::err(e.to_string())
            }
        }
    }

    fn handle_get_source(&self) -> Response {
        match self.tab_manager.active_tab() {
            Some(tab) => match &tab.source {
                Some(source) => Response::ok_data(serde_json::Value::String(source.clone())),
                None => Response::err("no source available".to_string()),
            },
            None => Response::err("no active tab".to_string()),
        }
    }

    fn handle_get_tabs(&self) -> Response {
        let tabs: Vec<serde_json::Value> = self
            .tab_manager
            .tabs()
            .iter()
            .map(|t| {
                let state = match &t.state {
                    TabState::Blank => "blank",
                    TabState::Loading => "loading",
                    TabState::Loaded => "loaded",
                    TabState::Error(_) => "error",
                };
                serde_json::json!({
                    "id": t.id.0,
                    "url": t.url.as_ref().map(|u| u.as_str()),
                    "title": t.title,
                    "state": state,
                })
            })
            .collect();
        Response::ok_data(serde_json::Value::Array(tabs))
    }

    fn handle_new_tab(&mut self) -> Response {
        let id = self.tab_manager.new_tab();
        Response::ok_data(serde_json::json!({"id": id.0}))
    }

    fn handle_close_tab(&mut self, id: u64) -> Response {
        if self.tab_manager.close_tab(TabId(id)) {
            Response::ok_empty()
        } else {
            Response::err("tab not found".to_string())
        }
    }

    fn handle_switch_tab(&mut self, id: u64) -> Response {
        if self.tab_manager.switch_to(TabId(id)) {
            Response::ok_empty()
        } else {
            Response::err("tab not found".to_string())
        }
    }

    fn handle_go_back(&mut self) -> Response {
        if self.tab_manager.go_back() {
            let url = self
                .tab_manager
                .active_tab()
                .and_then(|t| t.url.as_ref())
                .map(|u| u.as_str().to_string())
                .unwrap_or_default();
            Response::ok_data(serde_json::json!({"url": url}))
        } else {
            Response::err("no back history".to_string())
        }
    }

    fn handle_go_forward(&mut self) -> Response {
        if self.tab_manager.go_forward() {
            let url = self
                .tab_manager
                .active_tab()
                .and_then(|t| t.url.as_ref())
                .map(|u| u.as_str().to_string())
                .unwrap_or_default();
            Response::ok_data(serde_json::json!({"url": url}))
        } else {
            Response::err("no forward history".to_string())
        }
    }

    fn handle_bookmark_add(&mut self, url: String, title: String) -> Response {
        match self.bookmark_store.add(&url, &title) {
            Ok(()) => Response::ok_empty(),
            Err(e) => Response::err(e.to_string()),
        }
    }

    fn handle_bookmark_list(&self) -> Response {
        let bookmarks: Vec<serde_json::Value> = self
            .bookmark_store
            .list()
            .iter()
            .map(|b| {
                serde_json::json!({
                    "url": b.url,
                    "title": b.title,
                    "created": b.created.to_rfc3339(),
                })
            })
            .collect();
        Response::ok_data(serde_json::Value::Array(bookmarks))
    }
}

async fn run_interactive(
    allow_http: bool,
    data_dir: Option<String>,
    single_process: bool,
) -> Result<()> {
    let stdin = BufReader::new(tokio::io::stdin());
    let mut stdout = tokio::io::stdout();
    let mut session = HeadlessSession::new(allow_http, data_dir, single_process).await?;

    let mut lines = stdin.lines();
    while let Ok(Some(line)) = lines.next_line().await {
        let is_quit;
        let response = match serde_json::from_str::<Command>(&line) {
            Ok(cmd) => {
                is_quit = matches!(cmd, Command::Quit);
                session.handle_command(cmd).await
            }
            Err(e) => {
                is_quit = false;
                Response::err(format!("invalid command: {e}"))
            }
        };
        let json = serde_json::to_string(&response)?;
        stdout.write_all(json.as_bytes()).await?;
        stdout.write_all(b"\n").await?;
        stdout.flush().await?;
        if is_quit {
            break;
        }
    }
    Ok(())
}
