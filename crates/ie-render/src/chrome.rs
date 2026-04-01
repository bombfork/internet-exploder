use crate::paint::{Color, PaintCommand};

/// Browser chrome overlay data for rendering.
pub struct ChromeOverlay {
    pub address_bar: Option<AddressBarOverlay>,
    pub tab_list: Option<TabListOverlay>,
    pub bookmarks: Option<BookmarkListOverlay>,
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

impl ChromeOverlay {
    pub fn none() -> Self {
        Self {
            address_bar: None,
            tab_list: None,
            bookmarks: None,
        }
    }

    pub fn is_active(&self) -> bool {
        self.address_bar.is_some() || self.tab_list.is_some() || self.bookmarks.is_some()
    }
}

/// Build paint commands for chrome overlays (rendered on top of page content).
pub fn build_chrome_display_list(
    chrome: &ChromeOverlay,
    viewport_width: f32,
    _viewport_height: f32,
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
