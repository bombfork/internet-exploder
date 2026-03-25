#[derive(Debug, Clone)]
pub enum OverlayState {
    None,
    AddressBar(AddressBarState),
    TabList,
    Bookmarks,
}

impl OverlayState {
    pub fn is_active(&self) -> bool {
        !matches!(self, OverlayState::None)
    }
}

#[derive(Debug, Clone)]
pub struct AddressBarState {
    pub input: String,
    pub cursor: usize,
}

impl AddressBarState {
    pub fn new(initial: &str) -> Self {
        Self {
            input: initial.to_string(),
            cursor: initial.len(),
        }
    }

    pub fn insert_char(&mut self, c: char) {
        self.input.insert(self.cursor, c);
        self.cursor += c.len_utf8();
    }

    pub fn delete_back(&mut self) {
        if self.cursor == 0 {
            return;
        }
        let prev = self.input[..self.cursor]
            .char_indices()
            .last()
            .map(|(i, _)| i)
            .unwrap();
        self.input.remove(prev);
        self.cursor = prev;
    }

    pub fn delete_forward(&mut self) {
        if self.cursor >= self.input.len() {
            return;
        }
        self.input.remove(self.cursor);
    }

    pub fn move_left(&mut self) {
        if self.cursor > 0 {
            self.cursor = self.input[..self.cursor]
                .char_indices()
                .last()
                .map(|(i, _)| i)
                .unwrap_or(0);
        }
    }

    pub fn move_right(&mut self) {
        if self.cursor < self.input.len() {
            self.cursor = self.input[self.cursor..]
                .char_indices()
                .nth(1)
                .map(|(i, _)| self.cursor + i)
                .unwrap_or(self.input.len());
        }
    }

    pub fn move_home(&mut self) {
        self.cursor = 0;
    }

    pub fn move_end(&mut self) {
        self.cursor = self.input.len();
    }

    pub fn submit(&self) -> &str {
        &self.input
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn none_is_not_active() {
        assert!(!OverlayState::None.is_active());
    }

    #[test]
    fn address_bar_is_active() {
        assert!(OverlayState::AddressBar(AddressBarState::new("")).is_active());
    }

    #[test]
    fn tab_list_is_active() {
        assert!(OverlayState::TabList.is_active());
    }

    #[test]
    fn new_with_url() {
        let bar = AddressBarState::new("https://example.com");
        assert_eq!(bar.input, "https://example.com");
        assert_eq!(bar.cursor, 19);
    }

    #[test]
    fn new_empty() {
        let bar = AddressBarState::new("");
        assert_eq!(bar.input, "");
        assert_eq!(bar.cursor, 0);
    }

    #[test]
    fn insert_char_empty() {
        let mut bar = AddressBarState::new("");
        bar.insert_char('a');
        assert_eq!(bar.input, "a");
        assert_eq!(bar.cursor, 1);
    }

    #[test]
    fn insert_char_middle() {
        let mut bar = AddressBarState::new("abc");
        bar.cursor = 1;
        bar.insert_char('x');
        assert_eq!(bar.input, "axbc");
        assert_eq!(bar.cursor, 2);
    }

    #[test]
    fn delete_back() {
        let mut bar = AddressBarState::new("abc");
        bar.cursor = 2;
        bar.delete_back();
        assert_eq!(bar.input, "ac");
        assert_eq!(bar.cursor, 1);
    }

    #[test]
    fn delete_back_at_zero() {
        let mut bar = AddressBarState::new("abc");
        bar.cursor = 0;
        bar.delete_back();
        assert_eq!(bar.input, "abc");
        assert_eq!(bar.cursor, 0);
    }

    #[test]
    fn delete_forward() {
        let mut bar = AddressBarState::new("abc");
        bar.cursor = 1;
        bar.delete_forward();
        assert_eq!(bar.input, "ac");
        assert_eq!(bar.cursor, 1);
    }

    #[test]
    fn delete_forward_at_end() {
        let mut bar = AddressBarState::new("abc");
        bar.delete_forward();
        assert_eq!(bar.input, "abc");
    }

    #[test]
    fn move_left_and_right() {
        let mut bar = AddressBarState::new("abc");
        bar.move_left();
        assert_eq!(bar.cursor, 2);
        bar.move_left();
        assert_eq!(bar.cursor, 1);
        bar.move_left();
        assert_eq!(bar.cursor, 0);
        bar.move_left(); // clamp
        assert_eq!(bar.cursor, 0);
        bar.move_right();
        assert_eq!(bar.cursor, 1);
    }

    #[test]
    fn move_right_clamps() {
        let mut bar = AddressBarState::new("ab");
        bar.move_right(); // already at end
        assert_eq!(bar.cursor, 2);
    }

    #[test]
    fn move_home_end() {
        let mut bar = AddressBarState::new("hello");
        bar.move_home();
        assert_eq!(bar.cursor, 0);
        bar.move_end();
        assert_eq!(bar.cursor, 5);
    }

    #[test]
    fn submit_returns_input() {
        let bar = AddressBarState::new("https://example.com");
        assert_eq!(bar.submit(), "https://example.com");
    }

    #[test]
    fn unicode_char() {
        let mut bar = AddressBarState::new("");
        bar.insert_char('é');
        assert_eq!(bar.input, "é");
        assert_eq!(bar.cursor, 2); // é is 2 bytes in UTF-8
        bar.insert_char('a');
        assert_eq!(bar.input, "éa");
        assert_eq!(bar.cursor, 3);
        bar.move_left();
        assert_eq!(bar.cursor, 2);
        bar.move_left();
        assert_eq!(bar.cursor, 0);
    }
}
