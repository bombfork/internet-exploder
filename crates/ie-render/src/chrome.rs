use crate::paint::{Color, PaintCommand};

/// Browser chrome overlay data for rendering.
pub struct ChromeOverlay {
    pub address_bar: Option<AddressBarOverlay>,
    pub tab_list: Option<TabListOverlay>,
    pub bookmarks: Option<BookmarkListOverlay>,
    pub help: bool,
    pub status_bar: Option<StatusBarOverlay>,
}

pub struct AddressBarOverlay {
    pub text: String,
    pub cursor: usize,
}

pub struct TabListOverlay {
    pub tabs: Vec<TabEntry>,
    pub active_index: usize,
}

pub struct TabEntry {
    pub id: u64,
    pub title: String,
    pub url: String,
}

pub struct BookmarkListOverlay {
    pub bookmarks: Vec<BookmarkEntry>,
}

pub struct BookmarkEntry {
    pub title: String,
    pub url: String,
}

pub struct StatusBarOverlay {
    pub url: String,
    pub status: String,
    pub load_time_ms: Option<u64>,
    pub log_entries: Vec<String>,
}

impl ChromeOverlay {
    pub fn none() -> Self {
        Self {
            address_bar: None,
            tab_list: None,
            bookmarks: None,
            help: false,
            status_bar: None,
        }
    }

    #[allow(dead_code)]
    pub fn is_active(&self) -> bool {
        self.address_bar.is_some()
            || self.tab_list.is_some()
            || self.bookmarks.is_some()
            || self.help
            || self.status_bar.is_some()
    }
}

/// Build paint commands for chrome overlays (rendered on top of page content).
pub fn build_chrome_display_list(
    chrome: &ChromeOverlay,
    viewport_width: f32,
    viewport_height: f32,
) -> Vec<PaintCommand> {
    let mut commands = Vec::new();

    if let Some(bar) = &chrome.address_bar {
        render_address_bar(&mut commands, bar, viewport_width);
    }

    if let Some(tabs) = &chrome.tab_list {
        render_tab_list(&mut commands, tabs, viewport_width);
    }

    if let Some(bookmarks) = &chrome.bookmarks {
        render_bookmark_list(&mut commands, bookmarks, viewport_width);
    }

    if chrome.help {
        render_help(&mut commands, viewport_width, viewport_height);
    }

    if let Some(status) = &chrome.status_bar {
        render_status_bar(&mut commands, status, viewport_width, viewport_height);
    }

    commands
}

const BAR_HEIGHT: f32 = 40.0;
const BAR_BG: Color = Color {
    r: 30,
    g: 30,
    b: 50,
    a: 230,
};
const BAR_TEXT: Color = Color {
    r: 255,
    g: 255,
    b: 255,
    a: 255,
};
const BAR_CURSOR: Color = Color {
    r: 100,
    g: 180,
    b: 255,
    a: 255,
};
const PANEL_BG: Color = Color {
    r: 40,
    g: 40,
    b: 60,
    a: 240,
};
const ACTIVE_BG: Color = Color {
    r: 60,
    g: 80,
    b: 120,
    a: 240,
};
const ITEM_TEXT: Color = Color {
    r: 220,
    g: 220,
    b: 230,
    a: 255,
};
const URL_TEXT: Color = Color {
    r: 140,
    g: 160,
    b: 200,
    a: 255,
};

fn render_address_bar(commands: &mut Vec<PaintCommand>, bar: &AddressBarOverlay, width: f32) {
    // Background bar across top
    commands.push(PaintCommand::FillRect {
        x: 0.0,
        y: 0.0,
        width,
        height: BAR_HEIGHT,
        color: BAR_BG,
    });

    // URL text
    let text_x = 12.0;
    let text_y = 10.0;
    let font_size = 16.0;
    if !bar.text.is_empty() {
        commands.push(PaintCommand::Text {
            text: bar.text.clone(),
            x: text_x,
            y: text_y,
            font_size,
            color: BAR_TEXT,
        });
    }

    // Cursor
    let cursor_x = text_x + bar.cursor as f32 * font_size * 0.5;
    commands.push(PaintCommand::FillRect {
        x: cursor_x,
        y: text_y,
        width: 2.0,
        height: font_size,
        color: BAR_CURSOR,
    });
}

fn render_tab_list(commands: &mut Vec<PaintCommand>, tabs: &TabListOverlay, width: f32) {
    let panel_width = 300.0f32.min(width * 0.4);
    let item_height = 36.0;
    let panel_height = (tabs.tabs.len() as f32 * item_height).max(item_height);

    // Panel background
    commands.push(PaintCommand::FillRect {
        x: 0.0,
        y: 0.0,
        width: panel_width,
        height: panel_height,
        color: PANEL_BG,
    });

    for (i, tab) in tabs.tabs.iter().enumerate() {
        let y = i as f32 * item_height;

        // Active tab highlight
        if i == tabs.active_index {
            commands.push(PaintCommand::FillRect {
                x: 0.0,
                y,
                width: panel_width,
                height: item_height,
                color: ACTIVE_BG,
            });
        }

        // Tab title
        let title = if tab.title.is_empty() {
            "New Tab"
        } else {
            &tab.title
        };
        commands.push(PaintCommand::Text {
            text: title.to_string(),
            x: 12.0,
            y: y + 6.0,
            font_size: 14.0,
            color: ITEM_TEXT,
        });

        // Tab URL (smaller, below title)
        if !tab.url.is_empty() {
            let url_display = if tab.url.len() > 35 {
                format!("{}...", &tab.url[..35])
            } else {
                tab.url.clone()
            };
            commands.push(PaintCommand::Text {
                text: url_display,
                x: 12.0,
                y: y + 22.0,
                font_size: 10.0,
                color: URL_TEXT,
            });
        }
    }
}

fn render_bookmark_list(
    commands: &mut Vec<PaintCommand>,
    bookmarks: &BookmarkListOverlay,
    width: f32,
) {
    let panel_width = 350.0f32.min(width * 0.5);
    let item_height = 32.0;
    let panel_height = (bookmarks.bookmarks.len() as f32 * item_height)
        .max(item_height)
        .min(400.0);

    // Panel background
    commands.push(PaintCommand::FillRect {
        x: width - panel_width,
        y: 0.0,
        width: panel_width,
        height: panel_height,
        color: PANEL_BG,
    });

    for (i, bm) in bookmarks.bookmarks.iter().enumerate() {
        let y = i as f32 * item_height;
        if y + item_height > panel_height {
            break;
        }

        let x_offset = width - panel_width + 12.0;

        // Title
        commands.push(PaintCommand::Text {
            text: bm.title.clone(),
            x: x_offset,
            y: y + 6.0,
            font_size: 13.0,
            color: ITEM_TEXT,
        });

        // URL
        let url_display = if bm.url.len() > 40 {
            format!("{}...", &bm.url[..40])
        } else {
            bm.url.clone()
        };
        commands.push(PaintCommand::Text {
            text: url_display,
            x: x_offset,
            y: y + 20.0,
            font_size: 10.0,
            color: URL_TEXT,
        });
    }
}

const HELP_SHORTCUTS: &[(&str, &str)] = &[
    ("Ctrl+L", "Address bar"),
    ("Ctrl+T", "New tab"),
    ("Ctrl+W", "Close tab"),
    ("Ctrl+Tab", "Next tab"),
    ("Ctrl+Shift+Tab", "Previous tab"),
    ("Ctrl+Shift+T", "Tab list"),
    ("Ctrl+D", "Bookmark page"),
    ("Ctrl+Shift+B", "Bookmarks"),
    ("F5 / Ctrl+Shift+R", "Reload page"),
    ("Alt+Left", "Go back"),
    ("Alt+Right", "Go forward"),
    ("Ctrl+H / F1", "This help"),
    ("F12", "Status bar"),
    ("Ctrl+Q", "Quit"),
    ("Escape", "Dismiss overlay"),
];

fn render_help(commands: &mut Vec<PaintCommand>, viewport_width: f32, viewport_height: f32) {
    let panel_width = 360.0f32.min(viewport_width * 0.5);
    let line_height = 28.0;
    let panel_height = (HELP_SHORTCUTS.len() as f32 * line_height + 50.0).min(viewport_height);
    let panel_x = (viewport_width - panel_width) / 2.0;
    let panel_y = (viewport_height - panel_height) / 2.0;

    // Background
    commands.push(PaintCommand::FillRect {
        x: panel_x,
        y: panel_y,
        width: panel_width,
        height: panel_height,
        color: PANEL_BG,
    });

    // Title
    commands.push(PaintCommand::Text {
        text: "Keyboard Shortcuts".to_string(),
        x: panel_x + 16.0,
        y: panel_y + 12.0,
        font_size: 16.0,
        color: BAR_TEXT,
    });

    // Shortcuts
    for (i, (key, desc)) in HELP_SHORTCUTS.iter().enumerate() {
        let y = panel_y + 44.0 + i as f32 * line_height;

        // Key
        commands.push(PaintCommand::Text {
            text: key.to_string(),
            x: panel_x + 16.0,
            y,
            font_size: 13.0,
            color: BAR_CURSOR,
        });

        // Description
        commands.push(PaintCommand::Text {
            text: desc.to_string(),
            x: panel_x + 180.0,
            y,
            font_size: 13.0,
            color: ITEM_TEXT,
        });
    }
}

fn render_status_bar(
    commands: &mut Vec<PaintCommand>,
    status: &StatusBarOverlay,
    viewport_width: f32,
    viewport_height: f32,
) {
    let bar_height = 24.0 + status.log_entries.len().min(8) as f32 * 16.0;
    let bar_y = viewport_height - bar_height;

    // Background
    commands.push(PaintCommand::FillRect {
        x: 0.0,
        y: bar_y,
        width: viewport_width,
        height: bar_height,
        color: BAR_BG,
    });

    // Status line: URL | status | load time
    let mut status_text = String::new();
    if !status.url.is_empty() {
        status_text.push_str(&status.url);
        status_text.push_str("  |  ");
    }
    status_text.push_str(&status.status);
    if let Some(ms) = status.load_time_ms {
        status_text.push_str(&format!("  |  {}ms", ms));
    }

    commands.push(PaintCommand::Text {
        text: status_text,
        x: 8.0,
        y: bar_y + 4.0,
        font_size: 12.0,
        color: ITEM_TEXT,
    });

    // Log entries
    for (i, entry) in status.log_entries.iter().rev().take(8).enumerate() {
        let truncated = if entry.len() > 100 {
            format!("{}...", &entry[..100])
        } else {
            entry.clone()
        };
        commands.push(PaintCommand::Text {
            text: truncated,
            x: 8.0,
            y: bar_y + 22.0 + i as f32 * 16.0,
            font_size: 11.0,
            color: URL_TEXT,
        });
    }
}
