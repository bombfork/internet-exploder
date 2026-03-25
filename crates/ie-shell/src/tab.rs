use url::Url;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TabId(pub u64);

#[derive(Debug, Clone)]
pub enum TabState {
    Blank,
    Loading,
    Loaded,
    Error(String),
}

#[derive(Debug, Clone)]
pub struct HistoryEntry {
    pub url: Url,
    pub source: Option<String>,
}

pub struct Tab {
    pub id: TabId,
    pub title: String,
    pub url: Option<Url>,
    pub state: TabState,
    pub source: Option<String>,
    pub history: Vec<HistoryEntry>,
    pub history_index: usize,
}

impl Tab {
    pub fn new(id: TabId) -> Self {
        Self {
            id,
            title: "New Tab".to_string(),
            url: None,
            state: TabState::Blank,
            source: None,
            history: Vec::new(),
            history_index: 0,
        }
    }

    /// Record a completed navigation in history.
    /// Truncates forward history if navigating after go_back.
    pub fn push_history(&mut self, url: Url, source: Option<String>) {
        if !self.history.is_empty() && self.history_index < self.history.len() - 1 {
            self.history.truncate(self.history_index + 1);
        }
        self.history.push(HistoryEntry {
            url: url.clone(),
            source: source.clone(),
        });
        self.history_index = self.history.len() - 1;
        self.url = Some(url);
        self.source = source;
    }
}

pub struct TabManager {
    tabs: Vec<Tab>,
    active: usize,
    next_id: u64,
}

impl TabManager {
    pub fn new() -> Self {
        let first = Tab::new(TabId(0));
        Self {
            tabs: vec![first],
            active: 0,
            next_id: 1,
        }
    }

    pub fn new_tab(&mut self) -> TabId {
        let id = TabId(self.next_id);
        self.next_id += 1;
        self.tabs.push(Tab::new(id));
        self.active = self.tabs.len() - 1;
        id
    }

    pub fn close_tab(&mut self, id: TabId) -> bool {
        let Some(pos) = self.tabs.iter().position(|t| t.id == id) else {
            return false;
        };
        self.tabs.remove(pos);
        if self.tabs.is_empty() {
            // Always keep at least one tab
            let new_tab = Tab::new(TabId(self.next_id));
            self.next_id += 1;
            self.tabs.push(new_tab);
            self.active = 0;
        } else if self.active >= self.tabs.len() {
            self.active = self.tabs.len() - 1;
        } else if self.active > pos {
            self.active -= 1;
        }
        true
    }

    pub fn active_tab(&self) -> Option<&Tab> {
        self.tabs.get(self.active)
    }

    pub fn active_tab_mut(&mut self) -> Option<&mut Tab> {
        self.tabs.get_mut(self.active)
    }

    pub fn tabs(&self) -> &[Tab] {
        &self.tabs
    }

    pub fn next_tab(&mut self) {
        if !self.tabs.is_empty() {
            self.active = (self.active + 1) % self.tabs.len();
        }
    }

    pub fn prev_tab(&mut self) {
        if !self.tabs.is_empty() {
            self.active = if self.active == 0 {
                self.tabs.len() - 1
            } else {
                self.active - 1
            };
        }
    }

    pub fn switch_to(&mut self, id: TabId) -> bool {
        if let Some(pos) = self.tabs.iter().position(|t| t.id == id) {
            self.active = pos;
            true
        } else {
            false
        }
    }

    pub fn go_back(&mut self) -> bool {
        let Some(tab) = self.active_tab_mut() else {
            return false;
        };
        if tab.history_index == 0 {
            return false;
        }
        tab.history_index -= 1;
        let entry = tab.history[tab.history_index].clone();
        tab.url = Some(entry.url);
        tab.source = entry.source;
        true
    }

    pub fn go_forward(&mut self) -> bool {
        let Some(tab) = self.active_tab_mut() else {
            return false;
        };
        if tab.history.is_empty() || tab.history_index >= tab.history.len() - 1 {
            return false;
        }
        tab.history_index += 1;
        let entry = tab.history[tab.history_index].clone();
        tab.url = Some(entry.url);
        tab.source = entry.source;
        true
    }

    pub fn can_go_back(&self) -> bool {
        self.active_tab().is_some_and(|t| t.history_index > 0)
    }

    pub fn can_go_forward(&self) -> bool {
        self.active_tab()
            .is_some_and(|t| !t.history.is_empty() && t.history_index < t.history.len() - 1)
    }
}

impl Default for TabManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url(s: &str) -> Url {
        Url::parse(s).unwrap()
    }

    // --- Tab model tests ---

    #[test]
    fn new_starts_with_one_tab() {
        let tm = TabManager::new();
        assert_eq!(tm.tabs().len(), 1);
        assert!(tm.active_tab().is_some());
    }

    #[test]
    fn new_tab_increases_count_and_is_active() {
        let mut tm = TabManager::new();
        let id = tm.new_tab();
        assert_eq!(tm.tabs().len(), 2);
        assert_eq!(tm.active_tab().unwrap().id, id);
    }

    #[test]
    fn close_tab_removes_and_adjusts_active() {
        let mut tm = TabManager::new();
        let first_id = tm.active_tab().unwrap().id;
        let second_id = tm.new_tab();
        // Active is second tab
        assert_eq!(tm.active_tab().unwrap().id, second_id);
        // Close second tab
        assert!(tm.close_tab(second_id));
        assert_eq!(tm.tabs().len(), 1);
        assert_eq!(tm.active_tab().unwrap().id, first_id);
    }

    #[test]
    fn close_tab_nonexistent_returns_false() {
        let mut tm = TabManager::new();
        assert!(!tm.close_tab(TabId(999)));
    }

    #[test]
    fn next_prev_tab_cycle() {
        let mut tm = TabManager::new();
        let id0 = tm.active_tab().unwrap().id;
        let id1 = tm.new_tab();
        let id2 = tm.new_tab();
        // Active is id2 (index 2)
        assert_eq!(tm.active_tab().unwrap().id, id2);

        tm.next_tab(); // wraps to 0
        assert_eq!(tm.active_tab().unwrap().id, id0);

        tm.next_tab(); // 1
        assert_eq!(tm.active_tab().unwrap().id, id1);

        tm.prev_tab(); // back to 0
        assert_eq!(tm.active_tab().unwrap().id, id0);

        tm.prev_tab(); // wraps to 2
        assert_eq!(tm.active_tab().unwrap().id, id2);
    }

    #[test]
    fn switch_to_changes_active() {
        let mut tm = TabManager::new();
        let first_id = tm.active_tab().unwrap().id;
        tm.new_tab();
        assert!(tm.switch_to(first_id));
        assert_eq!(tm.active_tab().unwrap().id, first_id);
    }

    #[test]
    fn switch_to_nonexistent_returns_false() {
        let mut tm = TabManager::new();
        assert!(!tm.switch_to(TabId(999)));
    }

    #[test]
    fn tab_starts_blank() {
        let tm = TabManager::new();
        assert!(matches!(tm.active_tab().unwrap().state, TabState::Blank));
    }

    // --- Navigation history tests ---

    #[test]
    fn navigate_two_pages() {
        let mut tm = TabManager::new();
        let tab = tm.active_tab_mut().unwrap();
        tab.push_history(url("https://a.com"), Some("A".to_string()));
        tab.push_history(url("https://b.com"), Some("B".to_string()));
        assert_eq!(tab.history.len(), 2);
        assert_eq!(tab.history_index, 1);
    }

    #[test]
    fn go_back_restores_previous() {
        let mut tm = TabManager::new();
        let tab = tm.active_tab_mut().unwrap();
        tab.push_history(url("https://a.com"), Some("A".to_string()));
        tab.push_history(url("https://b.com"), Some("B".to_string()));

        assert!(tm.go_back());
        let tab = tm.active_tab().unwrap();
        assert_eq!(tab.url.as_ref().unwrap().as_str(), "https://a.com/");
        assert_eq!(tab.source.as_deref(), Some("A"));
        assert_eq!(tab.history_index, 0);
    }

    #[test]
    fn go_forward_after_go_back() {
        let mut tm = TabManager::new();
        let tab = tm.active_tab_mut().unwrap();
        tab.push_history(url("https://a.com"), Some("A".to_string()));
        tab.push_history(url("https://b.com"), Some("B".to_string()));

        tm.go_back();
        assert!(tm.go_forward());
        let tab = tm.active_tab().unwrap();
        assert_eq!(tab.url.as_ref().unwrap().as_str(), "https://b.com/");
        assert_eq!(tab.history_index, 1);
    }

    #[test]
    fn can_go_back_and_forward() {
        let mut tm = TabManager::new();
        assert!(!tm.can_go_back());
        assert!(!tm.can_go_forward());

        let tab = tm.active_tab_mut().unwrap();
        tab.push_history(url("https://a.com"), None);
        tab.push_history(url("https://b.com"), None);

        assert!(tm.can_go_back());
        assert!(!tm.can_go_forward());

        tm.go_back();
        assert!(!tm.can_go_back());
        assert!(tm.can_go_forward());
    }

    #[test]
    fn navigate_after_go_back_truncates_forward() {
        let mut tm = TabManager::new();
        let tab = tm.active_tab_mut().unwrap();
        tab.push_history(url("https://a.com"), Some("A".to_string()));
        tab.push_history(url("https://b.com"), Some("B".to_string()));

        tm.go_back();
        let tab = tm.active_tab_mut().unwrap();
        tab.push_history(url("https://c.com"), Some("C".to_string()));

        let tab = tm.active_tab().unwrap();
        assert_eq!(tab.history.len(), 2);
        assert_eq!(tab.history[0].url.as_str(), "https://a.com/");
        assert_eq!(tab.history[1].url.as_str(), "https://c.com/");
    }

    #[test]
    fn go_back_at_beginning_returns_false() {
        let mut tm = TabManager::new();
        let tab = tm.active_tab_mut().unwrap();
        tab.push_history(url("https://a.com"), None);
        assert!(!tm.go_back());
    }

    #[test]
    fn go_forward_at_end_returns_false() {
        let mut tm = TabManager::new();
        let tab = tm.active_tab_mut().unwrap();
        tab.push_history(url("https://a.com"), None);
        assert!(!tm.go_forward());
    }
}
