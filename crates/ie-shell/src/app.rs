use std::path::PathBuf;
use std::sync::Arc;

use url::Url;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoopProxy};
use winit::keyboard::{Key, ModifiersState, NamedKey};
use winit::window::{Window, WindowId};

use crate::bookmarks::BookmarkStore;
use crate::keybindings::{self, Action};
use crate::navigation::{InProcessNavigator, NavigationError, NavigationResult, NavigationService};
use crate::overlay::{AddressBarState, OverlayState};
use crate::tab::{TabId, TabManager, TabState};

pub enum UserEvent {
    NavigationComplete(TabId, Result<NavigationResult, NavigationError>, u64),
}

pub struct Browser {
    // gpu_renderer MUST be before window — it holds a wgpu::Surface
    // referencing the window, so it must drop first
    gpu_renderer: Option<ie_render::GpuRenderer>,
    window: Option<Arc<Window>>,
    tab_manager: TabManager,
    bookmark_store: BookmarkStore,
    overlay: OverlayState,
    navigator: Arc<dyn NavigationService + Send + Sync>,
    tokio_runtime: tokio::runtime::Runtime,
    _network_child: Option<ie_sandbox::ChildHandle>,
    modifiers: ModifiersState,
    event_loop_proxy: EventLoopProxy<UserEvent>,
    status_log: Vec<String>,
    last_load_time_ms: Option<u64>,
}

impl Browser {
    pub fn new(
        url: Option<Url>,
        allow_http: bool,
        single_process: bool,
        proxy: EventLoopProxy<UserEvent>,
    ) -> Self {
        let tokio_runtime = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");

        let (navigator, network_child): (
            Arc<dyn NavigationService + Send + Sync>,
            Option<ie_sandbox::ChildHandle>,
        ) = if single_process {
            let nav = InProcessNavigator::new()
                .expect("failed to create navigator")
                .with_https_only(!allow_http);
            (Arc::new(nav), None)
        } else {
            let (nav, child) = tokio_runtime.block_on(async {
                let mut child = ie_sandbox::spawn_child(ie_sandbox::ProcessKind::Network)
                    .await
                    .expect("failed to spawn network process");
                let channel = child.take_channel();
                (
                    crate::ipc_navigator::IpcNavigator::new(channel, !allow_http),
                    child,
                )
            });
            (Arc::new(nav), Some(child))
        };

        let data_dir = dirs_data_dir().unwrap_or_else(|| PathBuf::from("."));
        let bookmark_store = BookmarkStore::new(&data_dir).unwrap_or_else(|e| {
            tracing::warn!("failed to load bookmarks: {e}");
            BookmarkStore::new(&std::env::temp_dir()).expect("failed to create bookmark store")
        });

        let mut browser = Self {
            window: None,
            gpu_renderer: None,
            tab_manager: TabManager::new(),
            bookmark_store,
            overlay: OverlayState::None,
            navigator,
            tokio_runtime,
            _network_child: network_child,
            modifiers: ModifiersState::empty(),
            event_loop_proxy: proxy,
            status_log: Vec::new(),
            last_load_time_ms: None,
        };

        if let Some(url) = url {
            browser.start_navigation(url);
        }

        browser
    }

    fn dispatch_action(&mut self, action: Action, event_loop: &ActiveEventLoop) {
        match action {
            Action::ShowAddressBar => {
                let current_url = self
                    .tab_manager
                    .active_tab()
                    .and_then(|t| t.url.as_ref())
                    .map(|u| u.as_str())
                    .unwrap_or("");
                self.overlay = OverlayState::AddressBar(AddressBarState::new(current_url));
                tracing::info!("overlay: AddressBar");
            }
            Action::NewTab => {
                let id = self.tab_manager.new_tab();
                tracing::info!("new tab: id={}", id.0);
            }
            Action::CloseTab => {
                if let Some(tab) = self.tab_manager.active_tab() {
                    let id = tab.id;
                    self.tab_manager.close_tab(id);
                    tracing::info!("close tab: id={}", id.0);
                    if self.tab_manager.tabs().is_empty() {
                        event_loop.exit();
                    }
                }
            }
            Action::NextTab => {
                self.tab_manager.next_tab();
                if let Some(tab) = self.tab_manager.active_tab() {
                    tracing::info!("switch tab: id={}", tab.id.0);
                }
            }
            Action::PrevTab => {
                self.tab_manager.prev_tab();
                if let Some(tab) = self.tab_manager.active_tab() {
                    tracing::info!("switch tab: id={}", tab.id.0);
                }
            }
            Action::ShowTabList => {
                self.overlay = OverlayState::TabList;
                tracing::info!("overlay: TabList");
            }
            Action::BookmarkCurrentPage => {
                if let Some(tab) = self.tab_manager.active_tab()
                    && let Some(url) = &tab.url
                {
                    let url_str = url.as_str().to_string();
                    let title = tab.title.clone();
                    if let Err(e) = self.bookmark_store.add(&url_str, &title) {
                        tracing::error!("failed to add bookmark: {e}");
                    } else {
                        tracing::info!("bookmark added: {}", url_str);
                    }
                }
            }
            Action::ShowBookmarks => {
                self.overlay = OverlayState::Bookmarks;
                tracing::info!("overlay: Bookmarks");
            }
            Action::ShowHelp => {
                self.overlay = OverlayState::Help;
                tracing::info!("overlay: Help");
            }
            Action::ShowStatusBar => {
                self.overlay = OverlayState::StatusBar;
                tracing::info!("overlay: StatusBar");
            }
            Action::Reload => {
                if let Some(tab) = self.tab_manager.active_tab()
                    && let Some(url) = tab.url.clone()
                {
                    tracing::info!("reload: tab={}, url={}", tab.id.0, url);
                    self.start_navigation(url);
                }
            }
            Action::GoBack => {
                if self.tab_manager.go_back()
                    && let Some(tab) = self.tab_manager.active_tab()
                {
                    tracing::info!("go_back: tab={}", tab.id.0);
                }
            }
            Action::GoForward => {
                if self.tab_manager.go_forward()
                    && let Some(tab) = self.tab_manager.active_tab()
                {
                    tracing::info!("go_forward: tab={}", tab.id.0);
                }
            }
            Action::DismissOverlay => {
                self.overlay = OverlayState::None;
                tracing::info!("overlay: None");
            }
            Action::Quit => {
                event_loop.exit();
            }
        }
        // Redraw after any action to update chrome overlays
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    fn handle_address_bar_input(&mut self, event: &winit::event::KeyEvent) {
        let OverlayState::AddressBar(ref mut bar) = self.overlay else {
            return;
        };

        match &event.logical_key {
            Key::Named(NamedKey::Backspace) => bar.delete_back(),
            Key::Named(NamedKey::Delete) => bar.delete_forward(),
            Key::Named(NamedKey::ArrowLeft) => bar.move_left(),
            Key::Named(NamedKey::ArrowRight) => bar.move_right(),
            Key::Named(NamedKey::Home) => bar.move_home(),
            Key::Named(NamedKey::End) => bar.move_end(),
            _ => {
                // Insert text characters (no ctrl/alt modifiers)
                if !self.modifiers.control_key()
                    && !self.modifiers.alt_key()
                    && let Some(text) = &event.text
                {
                    for c in text.chars() {
                        if !c.is_control() {
                            bar.insert_char(c);
                        }
                    }
                }
            }
        }
    }

    fn submit_address_bar(&mut self) {
        let input = match &self.overlay {
            OverlayState::AddressBar(bar) => bar.submit().to_string(),
            _ => return,
        };

        let url = match Url::parse(&input) {
            Ok(url) => url,
            Err(_) => match Url::parse(&format!("https://{input}")) {
                Ok(url) => url,
                Err(e) => {
                    tracing::error!("invalid URL: {input}: {e}");
                    return;
                }
            },
        };

        self.overlay = OverlayState::None;
        self.start_navigation(url);
    }

    fn start_navigation(&mut self, url: Url) {
        let tab_id = match self.tab_manager.active_tab() {
            Some(tab) => tab.id,
            None => return,
        };

        if let Some(tab) = self.tab_manager.active_tab_mut() {
            tab.state = TabState::Loading;
            tab.url = Some(url.clone());
        }

        tracing::info!("navigate: tab={}, url={}", tab_id.0, url);

        self.status_log
            .push(format!("navigating: tab={}, url={}", tab_id.0, url));

        let proxy = self.event_loop_proxy.clone();
        let navigator = Arc::clone(&self.navigator);
        self.tokio_runtime.spawn(async move {
            let start = std::time::Instant::now();
            let result = navigator.navigate(&url).await;
            let elapsed = start.elapsed().as_millis() as u64;
            let _ = proxy.send_event(UserEvent::NavigationComplete(tab_id, result, elapsed));
        });
    }

    fn handle_navigation_complete(
        &mut self,
        tab_id: TabId,
        result: Result<NavigationResult, NavigationError>,
    ) {
        match result {
            Ok(nav_result) => {
                let msg = format!(
                    "navigation complete: tab={}, status={}",
                    tab_id.0, nav_result.status
                );
                tracing::info!("{msg}");
                self.status_log.push(msg);
                let source = String::from_utf8(nav_result.body).ok();
                // Find the tab (it might have been closed while loading)
                if let Some(tab) = self
                    .tab_manager
                    .tabs_mut()
                    .iter_mut()
                    .find(|t| t.id == tab_id)
                {
                    tab.state = TabState::Loaded;
                    tab.title = nav_result
                        .final_url
                        .host_str()
                        .unwrap_or("Untitled")
                        .to_string();
                    tab.push_history(nav_result.final_url, source.clone());
                    tab.source = source;
                }
            }
            Err(e) => {
                let msg = format!("navigation error: tab={}, error={}", tab_id.0, e);
                tracing::error!("{msg}");
                self.status_log.push(msg);
                if let Some(tab) = self
                    .tab_manager
                    .tabs_mut()
                    .iter_mut()
                    .find(|t| t.id == tab_id)
                {
                    tab.state = TabState::Error(e.to_string());
                }
            }
        }
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    fn paint(&mut self) {
        let mut commands = self.render_page();

        // Append chrome overlay commands on top of page content
        let (vw, vh) = self
            .gpu_renderer
            .as_ref()
            .map(|r| r.size())
            .unwrap_or((800, 600));
        let chrome = self.build_chrome_overlay();
        commands.extend(ie_render::build_chrome_display_list(
            &chrome, vw as f32, vh as f32,
        ));

        if let Some(renderer) = &mut self.gpu_renderer
            && let Err(e) = renderer.render(&commands)
        {
            tracing::error!("render error: {e}");
        }
    }

    fn build_chrome_overlay(&self) -> ie_render::ChromeOverlay {
        match &self.overlay {
            OverlayState::None => ie_render::ChromeOverlay::none(),
            OverlayState::AddressBar(bar) => ie_render::ChromeOverlay {
                address_bar: Some(ie_render::AddressBarOverlay {
                    text: bar.input.clone(),
                    cursor: bar.cursor,
                }),
                tab_list: None,
                bookmarks: None,
                help: false,
                status_bar: None,
            },
            OverlayState::TabList => {
                let tabs = self
                    .tab_manager
                    .tabs()
                    .iter()
                    .map(|t| ie_render::TabEntry {
                        id: t.id.0,
                        title: t.title.clone(),
                        url: t
                            .url
                            .as_ref()
                            .map(|u| u.as_str().to_string())
                            .unwrap_or_default(),
                    })
                    .collect();
                let active_index = self
                    .tab_manager
                    .active_tab()
                    .and_then(|at| self.tab_manager.tabs().iter().position(|t| t.id == at.id))
                    .unwrap_or(0);
                ie_render::ChromeOverlay {
                    address_bar: None,
                    tab_list: Some(ie_render::TabListOverlay { tabs, active_index }),
                    bookmarks: None,
                    help: false,
                    status_bar: None,
                }
            }
            OverlayState::Bookmarks => {
                let bookmarks = self
                    .bookmark_store
                    .list()
                    .iter()
                    .map(|b| ie_render::BookmarkEntry {
                        title: b.title.clone(),
                        url: b.url.clone(),
                    })
                    .collect();
                ie_render::ChromeOverlay {
                    address_bar: None,
                    tab_list: None,
                    bookmarks: Some(ie_render::BookmarkListOverlay { bookmarks }),
                    help: false,
                    status_bar: None,
                }
            }
            OverlayState::Help => ie_render::ChromeOverlay {
                address_bar: None,
                tab_list: None,
                bookmarks: None,
                help: true,
                status_bar: None,
            },
            OverlayState::StatusBar => {
                let url = self
                    .tab_manager
                    .active_tab()
                    .and_then(|t| t.url.as_ref())
                    .map(|u| u.as_str().to_string())
                    .unwrap_or_default();
                let status = self
                    .tab_manager
                    .active_tab()
                    .map(|t| format!("{:?}", t.state))
                    .unwrap_or_else(|| "No tab".to_string());
                ie_render::ChromeOverlay {
                    address_bar: None,
                    tab_list: None,
                    bookmarks: None,
                    help: false,
                    status_bar: Some(ie_render::StatusBarOverlay {
                        url,
                        status,
                        load_time_ms: self.last_load_time_ms,
                        log_entries: self.status_log.clone(),
                    }),
                }
            }
        }
    }

    fn render_page(&self) -> Vec<ie_render::PaintCommand> {
        let source = self
            .tab_manager
            .active_tab()
            .and_then(|t| t.source.as_ref());
        let Some(html) = source else {
            return vec![];
        };

        let (width, height) = self
            .gpu_renderer
            .as_ref()
            .map(|r| r.size())
            .unwrap_or((800, 600));

        let parse_result = ie_html::parse(html);
        let ua = ie_css::ua_stylesheet();

        let mut sheets = vec![(ua, ie_css::cascade::Origin::UserAgent)];
        for style_css in &parse_result.style_elements {
            let sheet = ie_css::parse_stylesheet(style_css);
            sheets.push((sheet, ie_css::cascade::Origin::Author));
        }

        let styles = ie_css::resolve::resolve_styles(
            &parse_result.document,
            &sheets,
            &std::collections::HashMap::new(),
            ie_css::resolve::ViewportSize {
                width: width as f64,
                height: height as f64,
            },
        );

        let viewport = ie_layout::Rect {
            x: 0.0,
            y: 0.0,
            width: width as f32,
            height: height as f32,
        };
        let text_measure = ie_render::SoftwareTextMeasure;
        let layout_tree =
            ie_layout::layout(&parse_result.document, &styles, viewport, &text_measure);

        ie_render::build_display_list(&layout_tree, &styles)
    }
}

fn dirs_data_dir() -> Option<PathBuf> {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|h| {
                let mut p = PathBuf::from(h);
                p.push(".local/share");
                p
            })
        })
        .map(|mut p| {
            p.push("internet-exploder");
            p
        })
}

impl ApplicationHandler<UserEvent> for Browser {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            let attrs = Window::default_attributes()
                .with_title("Internet Exploder")
                .with_maximized(true);
            match event_loop.create_window(attrs) {
                Ok(window) => {
                    let window = Arc::new(window);
                    match self
                        .tokio_runtime
                        .block_on(ie_render::GpuRenderer::new(window.clone()))
                    {
                        Ok(renderer) => {
                            self.gpu_renderer = Some(renderer);
                        }
                        Err(e) => {
                            tracing::error!("failed to create GPU renderer: {e}");
                        }
                    }
                    self.window = Some(window);
                    self.window.as_ref().unwrap().request_redraw();
                }
                Err(e) => {
                    tracing::error!("failed to create window: {e}");
                    event_loop.exit();
                }
            }
        }
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: UserEvent) {
        match event {
            UserEvent::NavigationComplete(tab_id, result, elapsed_ms) => {
                self.last_load_time_ms = Some(elapsed_ms);
                self.handle_navigation_complete(tab_id, result);
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::ModifiersChanged(new_modifiers) => {
                self.modifiers = new_modifiers.state();
            }
            WindowEvent::KeyboardInput {
                event: ref key_event,
                ..
            } => {
                if key_event.state != ElementState::Pressed {
                    return;
                }

                if self.overlay.is_active() {
                    // Address bar special handling
                    if matches!(self.overlay, OverlayState::AddressBar(_)) {
                        match &key_event.logical_key {
                            Key::Named(NamedKey::Enter) => {
                                self.submit_address_bar();
                                return;
                            }
                            Key::Named(NamedKey::Escape) => {
                                self.overlay = OverlayState::None;
                                tracing::info!("overlay: None");
                                return;
                            }
                            _ => {}
                        }
                        // Ctrl combos still work in address bar
                        if (self.modifiers.control_key() || self.modifiers.alt_key())
                            && let Some(action) =
                                keybindings::resolve_keybinding(key_event, &self.modifiers)
                        {
                            self.dispatch_action(action, event_loop);
                            return;
                        }
                        self.handle_address_bar_input(key_event);
                        if let Some(window) = &self.window {
                            window.request_redraw();
                        }
                    } else {
                        // TabList / Bookmarks: Escape dismisses, other keys go through
                        if let Some(action) =
                            keybindings::resolve_keybinding(key_event, &self.modifiers)
                        {
                            self.dispatch_action(action, event_loop);
                        }
                    }
                } else if let Some(action) =
                    keybindings::resolve_keybinding(key_event, &self.modifiers)
                {
                    self.dispatch_action(action, event_loop);
                }
            }
            WindowEvent::RedrawRequested => {
                self.paint();
            }
            WindowEvent::Resized(size) => {
                if let Some(renderer) = &mut self.gpu_renderer {
                    renderer.resize(size.width, size.height);
                }
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn show_address_bar_sets_overlay() {
        let current_url = "https://example.com";
        let overlay = OverlayState::AddressBar(AddressBarState::new(current_url));
        assert!(overlay.is_active());
        if let OverlayState::AddressBar(bar) = &overlay {
            assert_eq!(bar.submit(), "https://example.com");
        } else {
            panic!("expected AddressBar overlay");
        }
    }

    #[test]
    fn new_tab_increments_count() {
        let mut tm = TabManager::new();
        assert_eq!(tm.tabs().len(), 1);
        tm.new_tab();
        assert_eq!(tm.tabs().len(), 2);
    }

    #[test]
    fn close_tab_decrements_count() {
        let mut tm = TabManager::new();
        let id = tm.new_tab();
        assert_eq!(tm.tabs().len(), 2);
        tm.close_tab(id);
        // close_tab always keeps at least 1 tab
        assert_eq!(tm.tabs().len(), 1);
    }

    #[test]
    fn next_prev_changes_active() {
        let mut tm = TabManager::new();
        let first = tm.active_tab().unwrap().id;
        let second = tm.new_tab();
        assert_eq!(tm.active_tab().unwrap().id, second);
        tm.prev_tab();
        assert_eq!(tm.active_tab().unwrap().id, first);
        tm.next_tab();
        assert_eq!(tm.active_tab().unwrap().id, second);
    }

    #[test]
    fn bookmark_current_page() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = BookmarkStore::new(dir.path()).unwrap();
        store.add("https://example.com", "Example").unwrap();
        assert_eq!(store.list().len(), 1);
    }

    #[test]
    fn go_back_forward() {
        let mut tm = TabManager::new();
        let tab = tm.active_tab_mut().unwrap();
        tab.push_history(Url::parse("https://a.com").unwrap(), Some("A".to_string()));
        tab.push_history(Url::parse("https://b.com").unwrap(), Some("B".to_string()));
        assert!(tm.go_back());
        assert!(tm.go_forward());
    }

    #[test]
    fn dismiss_overlay_resets() {
        let mut overlay = OverlayState::AddressBar(AddressBarState::new("test"));
        assert!(overlay.is_active());
        overlay = OverlayState::None;
        assert!(!overlay.is_active());
    }
}
