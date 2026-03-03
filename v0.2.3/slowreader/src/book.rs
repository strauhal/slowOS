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

        // Build image map keyed by both resource ID and resource path for matching
        let mut all_images: std::collections::HashMap<String, Vec<u8>> = std::collections::HashMap::new();
        for res_id in &resource_ids {
            if let Some((data, mime)) = doc.get_resource(res_id) {
                if mime.starts_with("image/") {
                    // Key by resource ID
                    all_images.insert(res_id.clone(), data.clone());
                    // Also key by resource path (href) for src matching
                    if let Some(res) = doc.resources.get(res_id) {
                        let href = res.path.to_string_lossy().to_string();
                        all_images.insert(href.clone(), data.clone());
                        // Also key by just the filename component
                        if let Some(fname) = res.path.file_name() {
                            all_images.insert(fname.to_string_lossy().to_string(), data);
                        }
                    }
                }
            }
        }

        for id in spine_ids {
            if let Some((content, _mime)) = doc.get_resource(&id) {
                let html = String::from_utf8_lossy(&content).to_string();
                let chapter = parse_html_to_chapter(&html, &id, &all_images);
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
                    // Capture SVG content as XML for rendering
                    let svg_xml = serialize_svg_node(handle);
                    if !svg_xml.is_empty() {
                        content.push(ContentBlock::Image {
                            alt: "svg image".to_string(),
                            data: Some(svg_xml.into_bytes())
                        });
                    } else {
                        content.push(ContentBlock::Image { alt: "svg image".to_string(), data: None });
                    }
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
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    // Normalize Unicode characters that may not render in the system font
    normalize_unicode(&text)
}

/// Replace Unicode characters that the system fonts can't render with
/// ASCII/Latin equivalents so they don't appear as empty boxes.
fn normalize_unicode(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            // Smart quotes -> straight quotes
            '\u{201C}' | '\u{201D}' | '\u{201E}' | '\u{201F}' => result.push('"'),
            '\u{2018}' | '\u{2019}' | '\u{201A}' | '\u{201B}' => result.push('\''),
            // Dashes
            '\u{2013}' => result.push_str("--"),  // en-dash
            '\u{2014}' => result.push_str("---"), // em-dash
            '\u{2015}' => result.push_str("---"), // horizontal bar
            // Ellipsis
            '\u{2026}' => result.push_str("..."),
            // Bullets
            '\u{2022}' => result.push_str("* "),
            '\u{2023}' => result.push_str("> "),
            // Spaces
            '\u{00A0}' => result.push(' '), // non-breaking space
            '\u{2002}' | '\u{2003}' | '\u{2004}' | '\u{2005}' | '\u{2006}' |
            '\u{2007}' | '\u{2008}' | '\u{2009}' | '\u{200A}' | '\u{200B}' |
            '\u{FEFF}' => result.push(' '),
            // Guillemets
            '\u{00AB}' => result.push_str("<<"),
            '\u{00BB}' => result.push_str(">>"),
            '\u{2039}' => result.push('<'),
            '\u{203A}' => result.push('>'),
            // Dagger/double dagger (footnote markers)
            '\u{2020}' => result.push('*'),
            '\u{2021}' => result.push_str("**"),
            // Section/paragraph marks
            '\u{00B6}' => result.push_str("[P]"),
            // Misc symbols that IBMPlexSans may lack
            '\u{2212}' => result.push('-'), // minus sign
            '\u{00D7}' => result.push('x'), // multiplication sign
            '\u{00F7}' => result.push('/'), // division sign
            // Soft hyphen (invisible)
            '\u{00AD}' => {}
            // Zero-width joiners/non-joiners (invisible)
            '\u{200C}' | '\u{200D}' => {}
            // Everything else: pass through (the font may or may not have it)
            _ => result.push(ch),
        }
    }
    result
}

fn collect_text(handle: &Handle, text: &mut String) {
    match &handle.data {
        NodeData::Text { contents } => {
            text.push_str(&contents.borrow());
        }
        NodeData::Element { name, .. } => {
            let tag = name.local.as_ref();
            match tag {
                "sup" => {
                    let mut inner = String::new();
                    for child in handle.children.borrow().iter() {
                        collect_text(child, &mut inner);
                    }
                    for ch in inner.chars() {
                        text.push(to_superscript(ch));
                    }
                }
                "sub" => {
                    let mut inner = String::new();
                    for child in handle.children.borrow().iter() {
                        collect_text(child, &mut inner);
                    }
                    for ch in inner.chars() {
                        text.push(to_subscript(ch));
                    }
                }
                _ => {
                    for child in handle.children.borrow().iter() {
                        collect_text(child, text);
                    }
                }
            }
        }
        NodeData::Document => {
            for child in handle.children.borrow().iter() {
                collect_text(child, text);
            }
        }
        _ => {}
    }
}

/// Convert a character to its Unicode superscript equivalent
fn to_superscript(ch: char) -> char {
    match ch {
        '0' => '\u{2070}', '1' => '\u{00B9}', '2' => '\u{00B2}', '3' => '\u{00B3}',
        '4' => '\u{2074}', '5' => '\u{2075}', '6' => '\u{2076}', '7' => '\u{2077}',
        '8' => '\u{2078}', '9' => '\u{2079}',
        '+' => '\u{207A}', '-' => '\u{207B}', '=' => '\u{207C}',
        '(' => '\u{207D}', ')' => '\u{207E}', 'n' => '\u{207F}', 'i' => '\u{2071}',
        _ => ch,
    }
}

/// Convert a character to its Unicode subscript equivalent
fn to_subscript(ch: char) -> char {
    match ch {
        '0' => '\u{2080}', '1' => '\u{2081}', '2' => '\u{2082}', '3' => '\u{2083}',
        '4' => '\u{2084}', '5' => '\u{2085}', '6' => '\u{2086}', '7' => '\u{2087}',
        '8' => '\u{2088}', '9' => '\u{2089}',
        '+' => '\u{208A}', '-' => '\u{208B}', '=' => '\u{208C}',
        '(' => '\u{208D}', ')' => '\u{208E}',
        _ => ch,
    }
}

/// Serialize an SVG node back to XML string
fn serialize_svg_node(handle: &Handle) -> String {
    let mut result = String::new();
    serialize_node_recursive(handle, &mut result);
    result
}

fn serialize_node_recursive(handle: &Handle, output: &mut String) {
    match &handle.data {
        NodeData::Element { name, attrs, .. } => {
            let tag = name.local.as_ref();
            output.push('<');
            output.push_str(tag);

            // Add attributes
            for attr in attrs.borrow().iter() {
                output.push(' ');
                output.push_str(attr.name.local.as_ref());
                output.push_str("=\"");
                // Escape attribute values
                for c in attr.value.chars() {
                    match c {
                        '"' => output.push_str("&quot;"),
                        '&' => output.push_str("&amp;"),
                        '<' => output.push_str("&lt;"),
                        '>' => output.push_str("&gt;"),
                        _ => output.push(c),
                    }
                }
                output.push('"');
            }
            output.push('>');

            // Recurse into children
            for child in handle.children.borrow().iter() {
                serialize_node_recursive(child, output);
            }

            // Close tag
            output.push_str("</");
            output.push_str(tag);
            output.push('>');
        }
        NodeData::Text { contents } => {
            // Escape text content
            for c in contents.borrow().chars() {
                match c {
                    '&' => output.push_str("&amp;"),
                    '<' => output.push_str("&lt;"),
                    '>' => output.push_str("&gt;"),
                    _ => output.push(c),
                }
            }
        }
        _ => {}
    }
}
