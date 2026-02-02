//! Book representation and EPUB parsing

use html5ever::parse_document;
use html5ever::tendril::TendrilSink;
use markup5ever_rcdom::{Handle, NodeData, RcDom};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// A loaded book with chapters
#[derive(Clone)]
pub struct Book {
    pub path: PathBuf,
    pub metadata: BookMetadata,
    pub chapters: Vec<Chapter>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct BookMetadata {
    pub title: String,
    pub author: String,
    pub language: String,
    pub description: String,
}

#[derive(Clone)]
pub struct Chapter {
    pub title: String,
    pub content: Vec<ContentBlock>,
}

/// Content blocks for rendering
#[derive(Clone, Debug)]
pub enum ContentBlock {
    Heading { level: u8, text: String },
    Paragraph(String),
    Quote(String),
    Code(String),
    ListItem(String),
    HorizontalRule,
    Image { alt: String, data: Option<Vec<u8>> },
}

impl Book {
    /// Load an EPUB file
    pub fn open_epub(path: PathBuf) -> Result<Self, BookError> {
        let doc = epub::doc::EpubDoc::new(&path).map_err(|_| BookError::ParseError)?;
        
        // Extract metadata - mdata returns Option<String> in older versions
        // but Option<&MetadataItem> in newer versions
        let title = get_metadata_string(&doc, "title").unwrap_or_else(|| "unknown".to_string());
        let author = get_metadata_string(&doc, "creator").unwrap_or_else(|| "unknown".to_string());
        let language = get_metadata_string(&doc, "language").unwrap_or_else(|| "en".to_string());
        let description = get_metadata_string(&doc, "description").unwrap_or_default();
        
        let metadata = BookMetadata {
            title,
            author,
            language,
            description,
        };
        
        let mut book = Book {
            path,
            metadata,
            chapters: Vec::new(),
        };
        
        // Re-open to iterate through spine
        let mut doc = epub::doc::EpubDoc::new(&book.path).map_err(|_| BookError::ParseError)?;
        
        // Get spine item IDs
        let spine_ids: Vec<String> = doc.spine.iter().map(|item| item.idref.clone()).collect();
        
        // Collect all resource IDs for image lookup
        let resource_ids: Vec<String> = doc.resources.keys().cloned().collect();
        
        for id in spine_ids {
            if let Some((content, _mime)) = doc.get_resource(&id) {
                let html = String::from_utf8_lossy(&content).to_string();
                // Extract images referenced in this chapter
                let mut image_map: std::collections::HashMap<String, Vec<u8>> = std::collections::HashMap::new();
                for res_id in &resource_ids {
                    if let Some((data, mime)) = doc.get_resource(res_id) {
                        if mime.starts_with("image/") {
                            image_map.insert(res_id.clone(), data);
                        }
                    }
                }
                let chapter = parse_html_to_chapter(&html, &id, &image_map);
                book.chapters.push(chapter);
            }
        }
        
        Ok(book)
    }
    
    /// Load a plain text file
    pub fn open_text(path: PathBuf) -> Result<Self, BookError> {
        let content = std::fs::read_to_string(&path).map_err(|_| BookError::IoError)?;
        
        let title = path.file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".to_string());
        
        let mut paragraphs = Vec::new();
        for para in content.split("\n\n") {
            let trimmed = para.trim();
            if !trimmed.is_empty() {
                paragraphs.push(ContentBlock::Paragraph(trimmed.to_string()));
            }
        }
        
        Ok(Book {
            path,
            metadata: BookMetadata {
                title: title.clone(),
                author: String::new(),
                language: "en".to_string(),
                description: String::new(),
            },
            chapters: vec![Chapter {
                title,
                content: paragraphs,
            }],
        })
    }
    
    /// Get total chapter count
    pub fn chapter_count(&self) -> usize {
        self.chapters.len()
    }
}

#[derive(Debug)]
pub enum BookError {
    IoError,
    ParseError,
    UnsupportedFormat,
}

/// Helper to extract string from epub metadata
fn get_metadata_string(doc: &epub::doc::EpubDoc<std::io::BufReader<std::fs::File>>, key: &str) -> Option<String> {
    doc.mdata(key).map(|item| item.value.clone())
}

/// Parse HTML content to chapter
fn parse_html_to_chapter(html: &str, default_title: &str, images: &std::collections::HashMap<String, Vec<u8>>) -> Chapter {
    let dom = parse_document(RcDom::default(), Default::default())
        .from_utf8()
        .read_from(&mut html.as_bytes())
        .unwrap();
    
    let mut content = Vec::new();
    let mut title = default_title.to_string();
    
    extract_content(&dom.document, &mut content, &mut title, images);
    
    // Clean up title
    if title.is_empty() || title == default_title {
        // Try to use first heading as title
        for block in &content {
            if let ContentBlock::Heading { text, .. } = block {
                title = text.clone();
                break;
            }
        }
    }
    
    Chapter { title, content }
}

fn extract_content(handle: &Handle, content: &mut Vec<ContentBlock>, title: &mut String, images: &std::collections::HashMap<String, Vec<u8>>) {
    match &handle.data {
        NodeData::Element { name, attrs, .. } => {
            let tag = name.local.as_ref();
            
            match tag {
                "h1" => {
                    let text = get_text_content(handle);
                    if !text.is_empty() {
                        if title.is_empty() || !title.contains(&text) {
                            *title = text.clone();
                        }
                        content.push(ContentBlock::Heading { level: 1, text });
                    }
                }
                "h2" => {
                    let text = get_text_content(handle);
                    if !text.is_empty() {
                        content.push(ContentBlock::Heading { level: 2, text });
                    }
                }
                "h3" | "h4" | "h5" | "h6" => {
                    let text = get_text_content(handle);
                    if !text.is_empty() {
                        let level = tag.chars().last().unwrap().to_digit(10).unwrap() as u8;
                        content.push(ContentBlock::Heading { level, text });
                    }
                }
                "p" => {
                    // Check if paragraph contains an img
                    let mut has_img = false;
                    for child in handle.children.borrow().iter() {
                        if let NodeData::Element { name, .. } = &child.data {
                            if name.local.as_ref() == "img" {
                                extract_content(child, content, title, images);
                                has_img = true;
                            }
                        }
                    }
                    if !has_img {
                        let text = get_text_content(handle);
                        if !text.is_empty() {
                            content.push(ContentBlock::Paragraph(text));
                        }
                    }
                }
                "blockquote" => {
                    let text = get_text_content(handle);
                    if !text.is_empty() {
                        content.push(ContentBlock::Quote(text));
                    }
                }
                "pre" | "code" => {
                    let text = get_text_content(handle);
                    if !text.is_empty() {
                        content.push(ContentBlock::Code(text));
                    }
                }
                "li" => {
                    let text = get_text_content(handle);
                    if !text.is_empty() {
                        content.push(ContentBlock::ListItem(text));
                    }
                }
                "hr" => {
                    content.push(ContentBlock::HorizontalRule);
                }
                "img" | "image" => {
                    let attrs = attrs.borrow();
                    let src = attrs.iter()
                        .find(|a| a.name.local.as_ref() == "src" || a.name.local.as_ref() == "href")
                        .map(|a| a.value.to_string())
                        .unwrap_or_default();
                    let alt = attrs.iter()
                        .find(|a| a.name.local.as_ref() == "alt")
                        .map(|a| a.value.to_string())
                        .unwrap_or_else(|| "image".to_string());
                    
                    // Try to find image data by matching the src to a resource
                    let file_part = src.rsplit('/').next().unwrap_or(&src);
                    let data = images.iter()
                        .find(|(k, _)| k.contains(file_part) || file_part.contains(k.as_str()))
                        .map(|(_, v)| v.clone());
                    
                    content.push(ContentBlock::Image { alt, data });
                }
                "title" => {
                    let text = get_text_content(handle);
                    if !text.is_empty() && title.is_empty() {
                        *title = text;
                    }
                }
                "svg" => {
                    // Skip SVG subtrees
                    content.push(ContentBlock::Image { alt: "svg image".to_string(), data: None });
                }
                _ => {
                    // Recurse into children
                    for child in handle.children.borrow().iter() {
                        extract_content(child, content, title, images);
                    }
                }
            }
        }
        NodeData::Document => {
            for child in handle.children.borrow().iter() {
                extract_content(child, content, title, images);
            }
        }
        _ => {}
    }
}

fn get_text_content(handle: &Handle) -> String {
    let mut text = String::new();
    collect_text(handle, &mut text);
    // Normalize whitespace
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn collect_text(handle: &Handle, text: &mut String) {
    match &handle.data {
        NodeData::Text { contents } => {
            text.push_str(&contents.borrow());
        }
        NodeData::Element { .. } | NodeData::Document => {
            for child in handle.children.borrow().iter() {
                collect_text(child, text);
            }
        }
        _ => {}
    }
}
