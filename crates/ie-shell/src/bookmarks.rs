use std::path::{Path, PathBuf};

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bookmark {
    pub url: String,
    pub title: String,
    pub created: DateTime<Utc>,
}

pub struct BookmarkStore {
    bookmarks: Vec<Bookmark>,
    path: PathBuf,
}

impl BookmarkStore {
    pub fn new(data_dir: &Path) -> Result<Self> {
        let path = data_dir.join("bookmarks.json");
        let bookmarks = if path.exists() {
            let data = std::fs::read_to_string(&path)?;
            serde_json::from_str(&data)?
        } else {
            Vec::new()
        };
        Ok(Self { bookmarks, path })
    }

    pub fn add(&mut self, url: &str, title: &str) -> Result<()> {
        self.bookmarks.push(Bookmark {
            url: url.to_string(),
            title: title.to_string(),
            created: Utc::now(),
        });
        self.save()
    }

    pub fn remove(&mut self, url: &str) -> Result<()> {
        self.bookmarks.retain(|b| b.url != url);
        self.save()
    }

    pub fn list(&self) -> &[Bookmark] {
        &self.bookmarks
    }

    fn save(&self) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(&self.bookmarks)?;
        std::fs::write(&self.path, json)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_with_empty_dir() {
        let dir = tempfile::tempdir().unwrap();
        let store = BookmarkStore::new(dir.path()).unwrap();
        assert!(store.list().is_empty());
    }

    #[test]
    fn add_then_list() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = BookmarkStore::new(dir.path()).unwrap();
        store.add("https://example.com", "Example").unwrap();
        assert_eq!(store.list().len(), 1);
        assert_eq!(store.list()[0].url, "https://example.com");
        assert_eq!(store.list()[0].title, "Example");
    }

    #[test]
    fn remove_bookmark() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = BookmarkStore::new(dir.path()).unwrap();
        store.add("https://a.com", "A").unwrap();
        store.add("https://b.com", "B").unwrap();
        store.remove("https://a.com").unwrap();
        assert_eq!(store.list().len(), 1);
        assert_eq!(store.list()[0].url, "https://b.com");
    }

    #[test]
    fn round_trip_persistence() {
        let dir = tempfile::tempdir().unwrap();
        {
            let mut store = BookmarkStore::new(dir.path()).unwrap();
            store.add("https://example.com", "Example").unwrap();
            store.add("https://rust-lang.org", "Rust").unwrap();
        }
        let store = BookmarkStore::new(dir.path()).unwrap();
        assert_eq!(store.list().len(), 2);
        assert_eq!(store.list()[0].url, "https://example.com");
        assert_eq!(store.list()[1].url, "https://rust-lang.org");
    }

    #[test]
    fn add_duplicate_url_keeps_both() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = BookmarkStore::new(dir.path()).unwrap();
        store.add("https://example.com", "First").unwrap();
        store.add("https://example.com", "Second").unwrap();
        assert_eq!(store.list().len(), 2);
    }
}
