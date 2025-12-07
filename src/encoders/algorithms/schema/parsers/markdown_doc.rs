use crate::encoders::algorithms::schema::parsers::InputParser;
use crate::encoders::algorithms::schema::types::*;
use markdown::mdast::{Blockquote, Code, Heading, List, ListItem, Node, Table};
use markdown::{Constructs, ParseOptions, to_mdast};

pub struct MarkdownDocParser;

impl InputParser for MarkdownDocParser {
    type Error = SchemaError;

    fn parse(input: &str) -> Result<IntermediateRepresentation, Self::Error> {
        // Parse markdown to mdast with GFM (GitHub Flavored Markdown) for tables
        let options = ParseOptions {
            constructs: Constructs::gfm(),
            ..Default::default()
        };

        let mdast = to_mdast(input, &options)
            .map_err(|e| SchemaError::InvalidInput(format!("Failed to parse markdown: {}", e)))?;

        // Walk tree and collect blocks
        let blocks = extract_blocks(&mdast)?;

        if blocks.is_empty() {
            return Err(SchemaError::InvalidInput(
                "No content blocks found in markdown document.".to_string(),
            ));
        }

        // Build IR schema: type, content, meta
        let fields = vec![
            FieldDef::new("type", FieldType::String),
            FieldDef::new("content", FieldType::String),
            FieldDef::new("meta", FieldType::String),
        ];

        let row_count = blocks.len();
        let mut header = SchemaHeader::new(row_count, fields);

        // Track nulls in meta column
        let mut has_nulls = false;
        let total_values = row_count * 3;
        let bitmap_bytes = total_values.div_ceil(8);
        let mut null_bitmap = vec![0u8; bitmap_bytes];

        let mut values = Vec::with_capacity(total_values);

        for (idx, block) in blocks.iter().enumerate() {
            // type
            values.push(SchemaValue::String(block.block_type.clone()));

            // content
            values.push(SchemaValue::String(block.content.clone()));

            // meta (nullable)
            if let Some(meta) = &block.meta {
                values.push(SchemaValue::String(meta.clone()));
            } else {
                values.push(SchemaValue::Null);
                let meta_idx = idx * 3 + 2; // meta is third column
                set_null_bit(&mut null_bitmap, meta_idx);
                has_nulls = true;
            }
        }

        if has_nulls {
            header.null_bitmap = Some(null_bitmap);
            header.set_flag(FLAG_HAS_NULLS);
        }

        IntermediateRepresentation::new(header, values)
    }
}

/// Simplified block representation
#[derive(Debug)]
struct Block {
    block_type: String,
    content: String,
    meta: Option<String>,
}

impl Block {
    fn new(block_type: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            block_type: block_type.into(),
            content: content.into(),
            meta: None,
        }
    }

    fn with_meta(
        block_type: impl Into<String>,
        content: impl Into<String>,
        meta: impl Into<String>,
    ) -> Self {
        Self {
            block_type: block_type.into(),
            content: content.into(),
            meta: Some(meta.into()),
        }
    }
}

/// Extract blocks from mdast
fn extract_blocks(node: &Node) -> Result<Vec<Block>, SchemaError> {
    let mut blocks = Vec::new();
    walk_node(node, &mut blocks)?;
    Ok(blocks)
}

/// Recursively walk mdast nodes
fn walk_node(node: &Node, blocks: &mut Vec<Block>) -> Result<(), SchemaError> {
    match node {
        Node::Root(root) => {
            for child in &root.children {
                walk_node(child, blocks)?;
            }
        }
        Node::Heading(heading) => {
            blocks.push(heading_to_block(heading)?);
        }
        Node::Paragraph(para) => {
            let text = extract_text(&Node::Paragraph(para.clone()));
            if !text.trim().is_empty() {
                blocks.push(Block::new("p", text));
            }
        }
        Node::List(list) => {
            blocks.push(list_to_block(list)?);
        }
        Node::Code(code) => {
            blocks.push(code_to_block(code));
        }
        Node::Blockquote(quote) => {
            blocks.push(quote_to_block(quote)?);
        }
        Node::Table(table) => {
            blocks.push(table_to_block(table)?);
        }
        Node::ThematicBreak(_) => {
            blocks.push(Block::new("hr", ""));
        }
        Node::Link(link) => {
            let text = extract_text(&Node::Link(link.clone()));
            blocks.push(Block::with_meta("link", text, &link.url));
        }
        Node::Image(img) => {
            blocks.push(Block::with_meta("image", &img.alt, &img.url));
        }
        // Container nodes - recurse into children
        Node::ListItem(item) => {
            for child in &item.children {
                walk_node(child, blocks)?;
            }
        }
        Node::TableRow(_) | Node::TableCell(_) => {
            // Handled by table_to_block
        }
        // Inline nodes and other types - skip, they're handled by text extraction
        _ => {}
    }
    Ok(())
}

/// Convert heading to block
fn heading_to_block(heading: &Heading) -> Result<Block, SchemaError> {
    let level = heading.depth;
    let block_type = format!("h{}", level);
    let content = extract_text(&Node::Heading(heading.clone()));
    Ok(Block::new(block_type, content))
}

/// Convert list to block
fn list_to_block(list: &List) -> Result<Block, SchemaError> {
    let block_type = if list.ordered { "ol" } else { "ul" };
    let mut items = Vec::new();

    for child in &list.children {
        if let Node::ListItem(item) = child {
            let item_text = list_item_to_text(item, 0);
            items.push(item_text);
        }
    }

    let content = items.join(";");
    Ok(Block::new(block_type, content))
}

/// Extract list item text with nested lists
fn list_item_to_text(item: &ListItem, depth: usize) -> String {
    let mut parts = Vec::new();
    let indent = "  ".repeat(depth);

    for child in &item.children {
        match child {
            Node::Paragraph(para) => {
                let text = extract_text(&Node::Paragraph(para.clone()));
                parts.push(format!("{}{}", indent, text.trim()));
            }
            Node::List(nested_list) => {
                for nested_child in &nested_list.children {
                    if let Node::ListItem(nested_item) = nested_child {
                        parts.push(list_item_to_text(nested_item, depth + 1));
                    }
                }
            }
            _ => {
                let text = extract_text(child);
                if !text.trim().is_empty() {
                    parts.push(format!("{}{}", indent, text.trim()));
                }
            }
        }
    }

    parts.join("\n")
}

/// Convert code block to block
fn code_to_block(code: &Code) -> Block {
    let lang = code.lang.as_deref().unwrap_or("");
    if lang.is_empty() {
        Block::new("code", &code.value)
    } else {
        Block::with_meta("code", &code.value, lang)
    }
}

/// Convert blockquote to block
fn quote_to_block(quote: &Blockquote) -> Result<Block, SchemaError> {
    let mut parts = Vec::new();
    for child in &quote.children {
        let text = extract_text(child);
        if !text.trim().is_empty() {
            parts.push(text);
        }
    }
    Ok(Block::new("quote", parts.join("\n")))
}

/// Convert table to block
fn table_to_block(table: &Table) -> Result<Block, SchemaError> {
    let mut rows = Vec::new();

    for row_node in &table.children {
        if let Node::TableRow(row) = row_node {
            let mut cells = Vec::new();
            for cell_node in &row.children {
                if let Node::TableCell(cell) = cell_node {
                    let text = extract_text(&Node::TableCell(cell.clone()));
                    cells.push(text);
                }
            }
            rows.push(cells.join(","));
        }
    }

    let content = rows.join(";");
    let meta = format!(
        "{}x{}",
        rows.len(),
        table
            .children
            .first()
            .and_then(|r| if let Node::TableRow(row) = r {
                Some(row.children.len())
            } else {
                None
            })
            .unwrap_or(0)
    );

    Ok(Block::with_meta("table", content, meta))
}

/// Extract plain text from any node (handles inline formatting)
fn extract_text(node: &Node) -> String {
    match node {
        Node::Text(text) => text.value.clone(),
        Node::InlineCode(code) => format!("`{}`", code.value),
        Node::Emphasis(em) => {
            let inner = em
                .children
                .iter()
                .map(extract_text)
                .collect::<Vec<_>>()
                .join("");
            format!("*{}*", inner)
        }
        Node::Strong(strong) => {
            let inner = strong
                .children
                .iter()
                .map(extract_text)
                .collect::<Vec<_>>()
                .join("");
            format!("**{}**", inner)
        }
        Node::Link(link) => {
            let text = link
                .children
                .iter()
                .map(extract_text)
                .collect::<Vec<_>>()
                .join("");
            format!("[{}]({})", text, link.url)
        }
        Node::Image(img) => {
            format!("![{}]({})", img.alt, img.url)
        }
        Node::Break(_) => " ".to_string(),
        Node::Paragraph(para) => para
            .children
            .iter()
            .map(extract_text)
            .collect::<Vec<_>>()
            .join(""),
        Node::Heading(heading) => heading
            .children
            .iter()
            .map(extract_text)
            .collect::<Vec<_>>()
            .join(""),
        Node::TableCell(cell) => cell
            .children
            .iter()
            .map(extract_text)
            .collect::<Vec<_>>()
            .join(""),
        Node::ListItem(item) => item
            .children
            .iter()
            .map(extract_text)
            .collect::<Vec<_>>()
            .join(""),
        Node::Blockquote(quote) => quote
            .children
            .iter()
            .map(extract_text)
            .collect::<Vec<_>>()
            .join("\n"),
        Node::Delete(del) => {
            let inner = del
                .children
                .iter()
                .map(extract_text)
                .collect::<Vec<_>>()
                .join("");
            format!("~~{}~~", inner)
        }
        // Container nodes
        Node::Root(root) => root
            .children
            .iter()
            .map(extract_text)
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

/// Set a bit in the null bitmap
fn set_null_bit(bitmap: &mut [u8], index: usize) {
    let byte_idx = index / 8;
    let bit_idx = index % 8;
    bitmap[byte_idx] |= 1 << bit_idx;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_headings() {
        let input = "# Title\n## Subtitle\n### Section";
        let ir = MarkdownDocParser::parse(input).unwrap();

        assert_eq!(ir.header.row_count, 3);
        assert_eq!(
            ir.get_value(0, 0),
            Some(&SchemaValue::String("h1".to_string()))
        );
        assert_eq!(
            ir.get_value(0, 1),
            Some(&SchemaValue::String("Title".to_string()))
        );
        assert_eq!(
            ir.get_value(1, 0),
            Some(&SchemaValue::String("h2".to_string()))
        );
        assert_eq!(
            ir.get_value(1, 1),
            Some(&SchemaValue::String("Subtitle".to_string()))
        );
        assert_eq!(
            ir.get_value(2, 0),
            Some(&SchemaValue::String("h3".to_string()))
        );
    }

    #[test]
    fn test_paragraph() {
        let input = "This is a paragraph with **bold** and *italic* text.";
        let ir = MarkdownDocParser::parse(input).unwrap();

        assert_eq!(ir.header.row_count, 1);
        assert_eq!(
            ir.get_value(0, 0),
            Some(&SchemaValue::String("p".to_string()))
        );
        let content = if let Some(SchemaValue::String(s)) = ir.get_value(0, 1) {
            s
        } else {
            panic!("Expected string content");
        };
        assert!(content.contains("**bold**"));
        assert!(content.contains("*italic*"));
    }

    #[test]
    fn test_unordered_list() {
        let input = "- Item 1\n- Item 2\n- Item 3";
        let ir = MarkdownDocParser::parse(input).unwrap();

        assert_eq!(ir.header.row_count, 1);
        assert_eq!(
            ir.get_value(0, 0),
            Some(&SchemaValue::String("ul".to_string()))
        );
        let content = if let Some(SchemaValue::String(s)) = ir.get_value(0, 1) {
            s
        } else {
            panic!("Expected string content");
        };
        assert!(content.contains("Item 1"));
        assert!(content.contains("Item 2"));
        assert!(content.contains("Item 3"));
    }

    #[test]
    fn test_ordered_list() {
        let input = "1. First\n2. Second\n3. Third";
        let ir = MarkdownDocParser::parse(input).unwrap();

        assert_eq!(ir.header.row_count, 1);
        assert_eq!(
            ir.get_value(0, 0),
            Some(&SchemaValue::String("ol".to_string()))
        );
    }

    #[test]
    fn test_code_block_with_language() {
        let input = "```rust\nfn main() {}\n```";
        let ir = MarkdownDocParser::parse(input).unwrap();

        assert_eq!(ir.header.row_count, 1);
        assert_eq!(
            ir.get_value(0, 0),
            Some(&SchemaValue::String("code".to_string()))
        );
        assert_eq!(
            ir.get_value(0, 1),
            Some(&SchemaValue::String("fn main() {}".to_string()))
        );
        assert_eq!(
            ir.get_value(0, 2),
            Some(&SchemaValue::String("rust".to_string()))
        );
    }

    #[test]
    fn test_code_block_no_language() {
        let input = "```\ncode here\n```";
        let ir = MarkdownDocParser::parse(input).unwrap();

        assert_eq!(ir.header.row_count, 1);
        assert_eq!(
            ir.get_value(0, 0),
            Some(&SchemaValue::String("code".to_string()))
        );
        assert_eq!(ir.get_value(0, 2), Some(&SchemaValue::Null));
        assert!(ir.is_null(0, 2));
    }

    #[test]
    fn test_blockquote() {
        let input = "> This is a quote\n> with multiple lines";
        let ir = MarkdownDocParser::parse(input).unwrap();

        assert_eq!(ir.header.row_count, 1);
        assert_eq!(
            ir.get_value(0, 0),
            Some(&SchemaValue::String("quote".to_string()))
        );
    }

    #[test]
    fn test_horizontal_rule() {
        let input = "---";
        let ir = MarkdownDocParser::parse(input).unwrap();

        assert_eq!(ir.header.row_count, 1);
        assert_eq!(
            ir.get_value(0, 0),
            Some(&SchemaValue::String("hr".to_string()))
        );
        assert_eq!(
            ir.get_value(0, 1),
            Some(&SchemaValue::String("".to_string()))
        );
    }

    #[test]
    fn test_table() {
        let input = "| A | B |\n|---|---|\n| 1 | 2 |\n| 3 | 4 |";
        let ir = MarkdownDocParser::parse(input).unwrap();

        assert_eq!(ir.header.row_count, 1);
        assert_eq!(
            ir.get_value(0, 0),
            Some(&SchemaValue::String("table".to_string()))
        );
        let meta = if let Some(SchemaValue::String(s)) = ir.get_value(0, 2) {
            s
        } else {
            panic!("Expected string meta");
        };
        assert!(meta.contains("x2")); // 2 columns
    }

    #[test]
    fn test_link_as_block() {
        let input = "[Link Text](https://example.com)";
        let ir = MarkdownDocParser::parse(input).unwrap();

        // Links in paragraphs are treated as paragraph content with inline markdown
        assert_eq!(ir.header.row_count, 1);
        assert_eq!(
            ir.get_value(0, 0),
            Some(&SchemaValue::String("p".to_string()))
        );
        let content = if let Some(SchemaValue::String(s)) = ir.get_value(0, 1) {
            s
        } else {
            panic!("Expected string content");
        };
        assert!(content.contains("[Link Text](https://example.com)"));
    }

    #[test]
    fn test_image_as_block() {
        let input = "![Alt Text](image.jpg)";
        let ir = MarkdownDocParser::parse(input).unwrap();

        assert_eq!(ir.header.row_count, 1);
        assert_eq!(
            ir.get_value(0, 0),
            Some(&SchemaValue::String("p".to_string()))
        );
    }

    #[test]
    fn test_mixed_document() {
        let input = "# Header\n\nSome text.\n\n- Item 1\n- Item 2\n\n```\ncode\n```";
        let ir = MarkdownDocParser::parse(input).unwrap();

        assert!(ir.header.row_count >= 3);
        // Should have: h1, p, ul, code
    }

    #[test]
    fn test_inline_code_preserved() {
        let input = "This has `inline code` in it.";
        let ir = MarkdownDocParser::parse(input).unwrap();

        assert_eq!(ir.header.row_count, 1);
        let content = if let Some(SchemaValue::String(s)) = ir.get_value(0, 1) {
            s
        } else {
            panic!("Expected string content");
        };
        assert!(content.contains("`inline code`"));
    }

    #[test]
    fn test_nested_list() {
        let input = "- Level 1\n  - Level 2\n    - Level 3";
        let ir = MarkdownDocParser::parse(input).unwrap();

        assert_eq!(ir.header.row_count, 1);
        assert_eq!(
            ir.get_value(0, 0),
            Some(&SchemaValue::String("ul".to_string()))
        );
        let content = if let Some(SchemaValue::String(s)) = ir.get_value(0, 1) {
            s
        } else {
            panic!("Expected string content");
        };
        // Should preserve indentation
        assert!(content.contains("Level 1"));
        assert!(content.contains("Level 2"));
    }

    #[test]
    fn test_empty_document() {
        let input = "";
        let result = MarkdownDocParser::parse(input);
        assert!(result.is_err());
    }

    #[test]
    fn test_null_bitmap_for_code_without_lang() {
        let input = "```\ntest\n```\n\n```python\nprint()\n```";
        let ir = MarkdownDocParser::parse(input).unwrap();

        assert!(ir.header.has_flag(FLAG_HAS_NULLS));
        assert!(ir.is_null(0, 2)); // First code block has no language
        assert!(!ir.is_null(1, 2)); // Second has language
    }
}
