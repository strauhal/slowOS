//! Document model for SlowWrite
//! 
//! Uses ropey for efficient handling of large text documents.

use ropey::Rope;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// A text document with metadata
#[derive(Clone)]
pub struct Document {
    /// The text content using a rope data structure
    pub content: Rope,
    /// File path if saved
    pub path: Option<PathBuf>,
    /// Whether the document has unsaved changes
    pub modified: bool,
    /// Document metadata
    pub meta: DocumentMeta,
    /// Undo history
    undo_stack: Vec<UndoState>,
    /// Redo history  
    redo_stack: Vec<UndoState>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct DocumentMeta {
    pub title: String,
    pub word_count: usize,
    pub char_count: usize,
}

#[derive(Clone)]
struct UndoState {
    content: Rope,
    cursor_pos: usize,
}

impl Default for Document {
    fn default() -> Self {
        Self::new()
    }
}

impl Document {
    pub fn new() -> Self {
        Self {
            content: Rope::new(),
            path: None,
            modified: false,
            meta: DocumentMeta::default(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }
    
    pub fn from_string(text: &str) -> Self {
        let mut doc = Self {
            content: Rope::from_str(text),
            path: None,
            modified: false,
            meta: DocumentMeta::default(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        };
        doc.update_stats();
        doc
    }
    
    pub fn open(path: PathBuf) -> Result<Self, std::io::Error> {
        let text = std::fs::read_to_string(&path)?;
        let mut doc = Self::from_string(&text);
        doc.path = Some(path.clone());
        doc.meta.title = path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "untitled".to_string());
        Ok(doc)
    }
    
    pub fn save(&mut self) -> Result<(), std::io::Error> {
        if let Some(ref path) = self.path {
            std::fs::write(path, self.content.to_string())?;
            self.modified = false;
        }
        Ok(())
    }
    
    pub fn save_as(&mut self, path: PathBuf) -> Result<(), std::io::Error> {
        std::fs::write(&path, self.content.to_string())?;
        self.meta.title = path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "untitled".to_string());
        self.path = Some(path);
        self.modified = false;
        Ok(())
    }
    
    /// Get the title for display
    pub fn display_title(&self) -> String {
        let title = if self.meta.title.is_empty() {
            "untitled"
        } else {
            &self.meta.title
        };
        
        if self.modified {
            format!("{}*", title)
        } else {
            title.to_string()
        }
    }
    
    /// Save current state for undo
    pub fn save_undo_state(&mut self, cursor_pos: usize) {
        self.undo_stack.push(UndoState {
            content: self.content.clone(),
            cursor_pos,
        });
        // Clear redo stack on new edit
        self.redo_stack.clear();
        
        // Limit undo history
        if self.undo_stack.len() > 100 {
            self.undo_stack.remove(0);
        }
    }
    
    /// Undo last change
    pub fn undo(&mut self) -> Option<usize> {
        if let Some(state) = self.undo_stack.pop() {
            // Save current state for redo
            self.redo_stack.push(UndoState {
                content: self.content.clone(),
                cursor_pos: state.cursor_pos,
            });
            
            self.content = state.content;
            self.update_stats();
            self.modified = true;
            Some(state.cursor_pos)
        } else {
            None
        }
    }
    
    /// Redo last undone change
    pub fn redo(&mut self) -> Option<usize> {
        if let Some(state) = self.redo_stack.pop() {
            self.undo_stack.push(UndoState {
                content: self.content.clone(),
                cursor_pos: state.cursor_pos,
            });
            
            self.content = state.content;
            self.update_stats();
            self.modified = true;
            Some(state.cursor_pos)
        } else {
            None
        }
    }
    
    /// Insert text at position
    pub fn insert(&mut self, pos: usize, text: &str) {
        let pos = pos.min(self.content.len_chars());
        self.content.insert(pos, text);
        self.modified = true;
        self.update_stats();
    }
    
    /// Delete character at position
    pub fn delete(&mut self, pos: usize) {
        if pos < self.content.len_chars() {
            self.content.remove(pos..pos + 1);
            self.modified = true;
            self.update_stats();
        }
    }
    
    /// Delete range
    pub fn delete_range(&mut self, start: usize, end: usize) {
        let start = start.min(self.content.len_chars());
        let end = end.min(self.content.len_chars());
        if start < end {
            self.content.remove(start..end);
            self.modified = true;
            self.update_stats();
        }
    }
    
    /// Get text in range
    pub fn get_range(&self, start: usize, end: usize) -> String {
        let start = start.min(self.content.len_chars());
        let end = end.min(self.content.len_chars());
        self.content.slice(start..end).to_string()
    }
    
    /// Update word and character counts
    fn update_stats(&mut self) {
        let text = self.content.to_string();
        self.meta.char_count = text.chars().count();
        self.meta.word_count = text.split_whitespace().count();
    }
    
    /// Get line count
    pub fn line_count(&self) -> usize {
        self.content.len_lines()
    }
    
    /// Get character count
    pub fn char_count(&self) -> usize {
        self.content.len_chars()
    }
    
    /// Convert char index to line and column
    pub fn char_to_line_col(&self, char_idx: usize) -> (usize, usize) {
        let char_idx = char_idx.min(self.content.len_chars());
        let line = self.content.char_to_line(char_idx);
        let line_start = self.content.line_to_char(line);
        let col = char_idx - line_start;
        (line, col)
    }
    
    /// Convert line and column to char index
    pub fn line_col_to_char(&self, line: usize, col: usize) -> usize {
        let line = line.min(self.content.len_lines().saturating_sub(1));
        let line_start = self.content.line_to_char(line);
        let line_len = self.content.line(line).len_chars();
        let col = col.min(line_len.saturating_sub(1));
        line_start + col
    }
    
    /// Get content of a specific line
    pub fn line(&self, line_idx: usize) -> Option<String> {
        if line_idx < self.content.len_lines() {
            Some(self.content.line(line_idx).to_string())
        } else {
            None
        }
    }
    
    /// Find text in document
    pub fn find(&self, query: &str, start_pos: usize) -> Option<(usize, usize)> {
        let text = self.content.to_string();
        let search_text = &text[start_pos.min(text.len())..];
        search_text.find(query).map(|pos| {
            let absolute_start = start_pos + pos;
            (absolute_start, absolute_start + query.len())
        })
    }
    
    /// Find all occurrences
    pub fn find_all(&self, query: &str) -> Vec<(usize, usize)> {
        let text = self.content.to_string();
        let mut results = Vec::new();
        let mut start = 0;
        
        while let Some(pos) = text[start..].find(query) {
            let absolute_start = start + pos;
            results.push((absolute_start, absolute_start + query.len()));
            start = absolute_start + 1;
        }
        
        results
    }
    
    /// Replace text
    pub fn replace(&mut self, start: usize, end: usize, replacement: &str) {
        self.delete_range(start, end);
        self.insert(start, replacement);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_new_document() {
        let doc = Document::new();
        assert_eq!(doc.char_count(), 0);
        assert!(!doc.modified);
    }
    
    #[test]
    fn test_insert_and_delete() {
        let mut doc = Document::new();
        doc.insert(0, "Hello");
        assert_eq!(doc.content.to_string(), "Hello");
        assert!(doc.modified);
        
        doc.insert(5, " World");
        assert_eq!(doc.content.to_string(), "Hello World");
        
        doc.delete(5);
        assert_eq!(doc.content.to_string(), "HelloWorld");
    }
    
    #[test]
    fn test_undo_redo() {
        let mut doc = Document::new();
        doc.insert(0, "Hello");
        doc.save_undo_state(5);
        doc.insert(5, " World");
        
        let pos = doc.undo();
        assert!(pos.is_some());
        assert_eq!(doc.content.to_string(), "Hello");
        
        let pos = doc.redo();
        assert!(pos.is_some());
        assert_eq!(doc.content.to_string(), "Hello World");
    }
}
