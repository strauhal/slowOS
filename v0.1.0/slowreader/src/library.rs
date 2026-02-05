//! Library - book collection and reading progress tracking

use crate::book::BookMetadata;
use serde::{Deserialize, Serialize};
use slowcore::storage::config_dir;
use std::path::PathBuf;

/// A book entry in the library
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LibraryEntry {
    pub path: PathBuf,
    pub metadata: BookMetadata,
    pub last_chapter: usize,
    pub last_scroll: f32,
    pub added_date: u64,
    pub last_read: u64,
    /// Total number of chapters in the book (for progress calculation)
    #[serde(default)]
    pub total_chapters: usize,
}

/// The user's book library
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Library {
    pub books: Vec<LibraryEntry>,
}

impl Library {
    /// Load library from disk
    pub fn load() -> Self {
        let path = config_dir("slowreader").join("library.json");
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }
    
    /// Save library to disk
    pub fn save(&self) {
        let path = config_dir("slowreader").join("library.json");
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(&path, json);
        }
    }
    
    /// Add or update a book
    pub fn add_book(&mut self, path: PathBuf, metadata: BookMetadata, total_chapters: usize) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        // Check if already exists
        if let Some(entry) = self.books.iter_mut().find(|b| b.path == path) {
            entry.metadata = metadata;
            entry.last_read = now;
            entry.total_chapters = total_chapters;
        } else {
            self.books.push(LibraryEntry {
                path,
                metadata,
                last_chapter: 0,
                last_scroll: 0.0,
                added_date: now,
                last_read: now,
                total_chapters,
            });
        }

        self.save();
    }
    
    /// Update reading position
    pub fn update_position(&mut self, path: &PathBuf, chapter: usize, scroll: f32) {
        if let Some(entry) = self.books.iter_mut().find(|b| &b.path == path) {
            entry.last_chapter = chapter;
            entry.last_scroll = scroll;
            entry.last_read = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            self.save();
        }
    }
    
    /// Get reading position for a book
    pub fn get_position(&self, path: &PathBuf) -> Option<(usize, f32)> {
        self.books
            .iter()
            .find(|b| &b.path == path)
            .map(|b| (b.last_chapter, b.last_scroll))
    }
    
    /// Remove a book
    pub fn remove_book(&mut self, path: &PathBuf) {
        self.books.retain(|b| &b.path != path);
        self.save();
    }
    
    /// Get recently read books
    pub fn recent_books(&self) -> Vec<&LibraryEntry> {
        let mut sorted: Vec<_> = self.books.iter().collect();
        sorted.sort_by(|a, b| b.last_read.cmp(&a.last_read));
        sorted.truncate(10);
        sorted
    }
}
