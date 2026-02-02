//! Storage utilities for Slow Computer apps
//! 
//! Handles file dialogs, recent files, and preferences.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum StorageError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("File not found: {0}")]
    NotFound(PathBuf),
}

pub type Result<T> = std::result::Result<T, StorageError>;

/// Recent files tracking
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RecentFiles {
    pub files: Vec<PathBuf>,
    pub max_entries: usize,
}

impl RecentFiles {
    pub fn new(max_entries: usize) -> Self {
        Self {
            files: Vec::new(),
            max_entries,
        }
    }
    
    pub fn add(&mut self, path: PathBuf) {
        // Remove if already exists
        self.files.retain(|p| p != &path);
        
        // Add to front
        self.files.insert(0, path);
        
        // Trim to max
        self.files.truncate(self.max_entries);
    }
    
    pub fn load(config_path: &Path) -> Result<Self> {
        let contents = std::fs::read_to_string(config_path)?;
        Ok(serde_json::from_str(&contents)?)
    }
    
    pub fn save(&self, config_path: &Path) -> Result<()> {
        let contents = serde_json::to_string_pretty(self)?;
        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(config_path, contents)?;
        Ok(())
    }
}

/// Simple file browser state
#[derive(Debug, Clone)]
pub struct FileBrowser {
    pub current_dir: PathBuf,
    pub entries: Vec<FileEntry>,
    pub selected_index: Option<usize>,
    pub filter_extensions: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct FileEntry {
    pub name: String,
    pub path: PathBuf,
    pub is_directory: bool,
}

impl FileBrowser {
    pub fn new(start_dir: PathBuf) -> Self {
        let mut browser = Self {
            current_dir: start_dir,
            entries: Vec::new(),
            selected_index: None,
            filter_extensions: Vec::new(),
        };
        browser.refresh();
        browser
    }
    
    pub fn with_filter(mut self, extensions: Vec<String>) -> Self {
        self.filter_extensions = extensions;
        self.refresh();
        self
    }
    
    pub fn refresh(&mut self) {
        self.entries.clear();
        self.selected_index = None;
        
        // Add parent directory entry
        if let Some(parent) = self.current_dir.parent() {
            self.entries.push(FileEntry {
                name: "..".to_string(),
                path: parent.to_path_buf(),
                is_directory: true,
            });
        }
        
        // Read directory
        if let Ok(read_dir) = std::fs::read_dir(&self.current_dir) {
            let mut dirs = Vec::new();
            let mut files = Vec::new();
            
            for entry in read_dir.flatten() {
                let path = entry.path();
                let name = entry.file_name().to_string_lossy().to_string();
                
                // Skip hidden files
                if name.starts_with('.') {
                    continue;
                }
                
                let is_directory = path.is_dir();
                
                // Apply extension filter for files
                if !is_directory && !self.filter_extensions.is_empty() {
                    let ext = path
                        .extension()
                        .map(|e| e.to_string_lossy().to_lowercase())
                        .unwrap_or_default();
                    if !self.filter_extensions.iter().any(|f| f.to_lowercase() == ext) {
                        continue;
                    }
                }
                
                let entry = FileEntry {
                    name,
                    path,
                    is_directory,
                };
                
                if is_directory {
                    dirs.push(entry);
                } else {
                    files.push(entry);
                }
            }
            
            // Sort alphabetically
            dirs.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
            files.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
            
            // Directories first, then files
            self.entries.extend(dirs);
            self.entries.extend(files);
        }
    }
    
    pub fn navigate_to(&mut self, path: PathBuf) {
        if path.is_dir() {
            self.current_dir = path;
            self.refresh();
        }
    }
    
    pub fn selected_entry(&self) -> Option<&FileEntry> {
        self.selected_index.and_then(|i| self.entries.get(i))
    }
    
    pub fn select_by_name(&mut self, name: &str) {
        self.selected_index = self.entries.iter().position(|e| e.name == name);
    }
}

/// Get the config directory for Slow Computer apps
pub fn config_dir(app_name: &str) -> PathBuf {
    directories::ProjectDirs::from("co", "slowcomputer", app_name)
        .map(|dirs| dirs.config_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."))
}

/// Get the documents directory
pub fn documents_dir() -> PathBuf {
    directories::UserDirs::new()
        .and_then(|dirs| dirs.document_dir().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."))
}
