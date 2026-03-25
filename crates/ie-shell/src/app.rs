use std::num::NonZeroU32;
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
    NavigationComplete(TabId, Result<NavigationResult, NavigationError>),
}

// Dark background color (#1a1a2e)
const BG_COLOR: u32 = 0x001a1a2e;

pub struct Browser {
    window: Option<Arc<Window>>,
    surface: Option<softbuffer::Surface<Arc<Window>, Arc<Window>>>,
    tab_manager: TabManager,
    bookmark_store: BookmarkStore,
    overlay: OverlayState,
    navigator: Arc<dyn NavigationService + Send + Sync>,
    tokio_runtime: tokio::runtime::Runtime,
    modifiers: ModifiersState,
    event_loop_proxy: EventLoopProxy<UserEvent>,
}

impl Browser {
    pub fn new(url: Option<Url>, allow_http: bool, proxy: EventLoopProxy<UserEvent>) -> Self {
        let navigator = InProcessNavigator::new()
            .expect("failed to create navigator")
            .with_https_only(!allow_http);
        let data_dir = dirs_data_dir().unwrap_or_else(|| PathBuf::from("."));
        let bookmark_store = BookmarkStore::new(&data_dir).unwrap_or_else(|e| {
            tracing::warn!("failed to load bookmarks: {e}");
            BookmarkStore::new(&std::env::temp_dir()).expect("failed to create bookmark store")
        });
        let tokio_runtime = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");

        let mut browser = Self {
            window: None,
            surface: None,
            tab_manager: TabManager::new(),
            bookmark_store,
            overlay: OverlayState::None,
            navigator: Arc::new(navigator),
            tokio_runtime,
            modifiers: ModifiersState::empty(),
            event_loop_proxy: proxy,
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

        let proxy = self.event_loop_proxy.clone();
        let navigator = Arc::clone(&self.navigator);
        self.tokio_runtime.spawn(async move {
            let result = navigator.navigate(&url).await;
            let _ = proxy.send_event(UserEvent::NavigationComplete(tab_id, result));
        });
    }

    fn handle_navigation_complete(
        &mut self,
        tab_id: TabId,
        result: Result<NavigationResult, NavigationError>,
    ) {
        match result {
            Ok(nav_result) => {
                tracing::info!(
                    "navigation complete: tab={}, status={}",
                    tab_id.0,
                    nav_result.status
                );
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
                tracing::error!("navigation error: tab={}, error={}", tab_id.0, e);
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
        if let Some(surface) = self.surface.as_mut() {
            let size = self.window.as_ref().unwrap().inner_size();
            let Some(width) = NonZeroU32::new(size.width) else {
                return;
            };
            let Some(height) = NonZeroU32::new(size.height) else {
                return;
            };
            surface
                .resize(width, height)
                .expect("failed to resize surface");
            let mut buffer = surface.buffer_mut().expect("failed to get buffer");
            buffer.fill(BG_COLOR);
            buffer.present().expect("failed to present buffer");
        }
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
                    let context = softbuffer::Context::new(window.clone())
                        .expect("failed to create softbuffer context");
                    let surface = softbuffer::Surface::new(&context, window.clone())
                        .expect("failed to create softbuffer surface");
                    self.window = Some(window);
                    self.surface = Some(surface);
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
            UserEvent::NavigationComplete(tab_id, result) => {
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
            WindowEvent::Resized(_) => {
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
