use winit::event::{ElementState, KeyEvent};
use winit::keyboard::{Key, ModifiersState, NamedKey};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    ShowAddressBar,
    NewTab,
    CloseTab,
    NextTab,
    PrevTab,
    ShowTabList,
    BookmarkCurrentPage,
    ShowBookmarks,
    ShowHelp,
    ShowStatusBar,
    Reload,
    GoBack,
    GoForward,
    DismissOverlay,
    Quit,
}

pub fn resolve_keybinding(event: &KeyEvent, modifiers: &ModifiersState) -> Option<Action> {
    if event.state != ElementState::Pressed {
        return None;
    }
    resolve_key(&event.logical_key, modifiers)
}

fn resolve_key(key: &Key, modifiers: &ModifiersState) -> Option<Action> {
    let ctrl = modifiers.control_key();
    let shift = modifiers.shift_key();
    let alt = modifiers.alt_key();

    match key {
        Key::Named(NamedKey::Escape) => Some(Action::DismissOverlay),
        Key::Named(NamedKey::F1) => Some(Action::ShowHelp),
        Key::Named(NamedKey::F5) => Some(Action::Reload),
        Key::Named(NamedKey::F12) => Some(Action::ShowStatusBar),
        Key::Named(NamedKey::Tab) if ctrl && shift => Some(Action::PrevTab),
        Key::Named(NamedKey::Tab) if ctrl => Some(Action::NextTab),
        Key::Named(NamedKey::ArrowLeft) if alt => Some(Action::GoBack),
        Key::Named(NamedKey::ArrowRight) if alt => Some(Action::GoForward),
        Key::Character(c) if ctrl => match c.as_str() {
            "h" | "H" => Some(Action::ShowHelp),
            "R" => Some(Action::Reload),
            "r" if shift => Some(Action::Reload),
            "l" | "L" => Some(Action::ShowAddressBar),
            "T" => Some(Action::ShowTabList),
            "t" if shift => Some(Action::ShowTabList),
            "t" => Some(Action::NewTab),
            "W" | "w" => Some(Action::CloseTab),
            "D" | "d" => Some(Action::BookmarkCurrentPage),
            "B" => Some(Action::ShowBookmarks),
            "b" if shift => Some(Action::ShowBookmarks),
            "Q" | "q" => Some(Action::Quit),
            _ => None,
        },
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use winit::keyboard::{Key, ModifiersState, NamedKey, SmolStr};

    use super::*;

    fn ctrl() -> ModifiersState {
        ModifiersState::CONTROL
    }

    fn ctrl_shift() -> ModifiersState {
        ModifiersState::CONTROL | ModifiersState::SHIFT
    }

    fn alt() -> ModifiersState {
        ModifiersState::ALT
    }

    fn no_mods() -> ModifiersState {
        ModifiersState::empty()
    }

    #[test]
    fn ctrl_l_show_address_bar() {
        assert_eq!(
            resolve_key(&Key::Character(SmolStr::new("l")), &ctrl()),
            Some(Action::ShowAddressBar)
        );
    }

    #[test]
    fn ctrl_t_new_tab() {
        assert_eq!(
            resolve_key(&Key::Character(SmolStr::new("t")), &ctrl()),
            Some(Action::NewTab)
        );
    }

    #[test]
    fn ctrl_shift_t_show_tab_list() {
        assert_eq!(
            resolve_key(&Key::Character(SmolStr::new("t")), &ctrl_shift()),
            Some(Action::ShowTabList)
        );
    }

    #[test]
    fn ctrl_w_close_tab() {
        assert_eq!(
            resolve_key(&Key::Character(SmolStr::new("w")), &ctrl()),
            Some(Action::CloseTab)
        );
    }

    #[test]
    fn ctrl_tab_next_tab() {
        assert_eq!(
            resolve_key(&Key::Named(NamedKey::Tab), &ctrl()),
            Some(Action::NextTab)
        );
    }

    #[test]
    fn ctrl_shift_tab_prev_tab() {
        assert_eq!(
            resolve_key(&Key::Named(NamedKey::Tab), &ctrl_shift()),
            Some(Action::PrevTab)
        );
    }

    #[test]
    fn ctrl_d_bookmark() {
        assert_eq!(
            resolve_key(&Key::Character(SmolStr::new("d")), &ctrl()),
            Some(Action::BookmarkCurrentPage)
        );
    }

    #[test]
    fn ctrl_shift_b_show_bookmarks() {
        assert_eq!(
            resolve_key(&Key::Character(SmolStr::new("b")), &ctrl_shift()),
            Some(Action::ShowBookmarks)
        );
    }

    #[test]
    fn alt_left_go_back() {
        assert_eq!(
            resolve_key(&Key::Named(NamedKey::ArrowLeft), &alt()),
            Some(Action::GoBack)
        );
    }

    #[test]
    fn alt_right_go_forward() {
        assert_eq!(
            resolve_key(&Key::Named(NamedKey::ArrowRight), &alt()),
            Some(Action::GoForward)
        );
    }

    #[test]
    fn escape_dismiss() {
        assert_eq!(
            resolve_key(&Key::Named(NamedKey::Escape), &no_mods()),
            Some(Action::DismissOverlay)
        );
    }

    #[test]
    fn ctrl_q_quit() {
        assert_eq!(
            resolve_key(&Key::Character(SmolStr::new("q")), &ctrl()),
            Some(Action::Quit)
        );
    }

    #[test]
    fn plain_letter_none() {
        assert_eq!(
            resolve_key(&Key::Character(SmolStr::new("a")), &no_mods()),
            None
        );
    }

    #[test]
    fn unmapped_ctrl_combo_none() {
        assert_eq!(
            resolve_key(&Key::Character(SmolStr::new("x")), &ctrl()),
            None
        );
    }
}
