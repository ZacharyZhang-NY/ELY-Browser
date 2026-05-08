use ely_design_system::colors;
use gpui::{AnyElement, IntoElement, ParentElement, Styled, div, px, rgb};
use gpui_component::StyledExt;

const PREVIEW_BLOCK_LIMIT: usize = 4;
const CODE_LINE_LIMIT: usize = 3;
const TEXT_LIMIT: usize = 120;

#[derive(Clone, Debug, Eq, PartialEq)]
enum MarkdownPreviewBlock {
    Heading { level: u8, text: String },
    Bullet(String),
    Quote(String),
    Code(String),
    Paragraph(String),
}

pub(super) fn render_markdown_preview(body: &str) -> AnyElement {
    div()
        .max_h(px(96.0))
        .overflow_hidden()
        .flex()
        .flex_col()
        .gap_1()
        .children(markdown_preview_blocks(body).into_iter().map(render_markdown_block))
        .into_any_element()
}

fn render_markdown_block(block: MarkdownPreviewBlock) -> AnyElement {
    match block {
        MarkdownPreviewBlock::Heading { level, text } => render_heading(level, text),
        MarkdownPreviewBlock::Bullet(text) => render_bullet(text),
        MarkdownPreviewBlock::Quote(text) => render_quote(text),
        MarkdownPreviewBlock::Code(text) => render_code(text),
        MarkdownPreviewBlock::Paragraph(text) => render_paragraph(text),
    }
}

fn render_heading(level: u8, text: String) -> AnyElement {
    let size = match level {
        1 => 14.0,
        2 => 13.0,
        _ => 12.0,
    };

    div()
        .text_size(px(size))
        .font_semibold()
        .truncate()
        .text_color(rgb(colors::INK))
        .child(text)
        .into_any_element()
}

fn render_bullet(text: String) -> AnyElement {
    div()
        .min_w_0()
        .flex()
        .items_start()
        .gap_2()
        .text_xs()
        .text_color(rgb(colors::MUTED_SOFT))
        .child(div().flex_none().child("-"))
        .child(div().min_w_0().truncate().child(text))
        .into_any_element()
}

fn render_quote(text: String) -> AnyElement {
    div()
        .min_w_0()
        .border_l_2()
        .border_color(rgb(colors::HAIRLINE_STRONG))
        .pl_2()
        .text_xs()
        .truncate()
        .text_color(rgb(colors::MUTED))
        .child(text)
        .into_any_element()
}

fn render_code(text: String) -> AnyElement {
    div()
        .min_w_0()
        .rounded_md()
        .border_1()
        .border_color(rgb(colors::HAIRLINE))
        .bg(rgb(colors::CANVAS_SOFT))
        .px_2()
        .py_1()
        .text_xs()
        .text_color(rgb(colors::BODY))
        .child(truncate_text(&text, TEXT_LIMIT))
        .into_any_element()
}

fn render_paragraph(text: String) -> AnyElement {
    div().text_xs().truncate().text_color(rgb(colors::MUTED_SOFT)).child(text).into_any_element()
}

fn markdown_preview_blocks(body: &str) -> Vec<MarkdownPreviewBlock> {
    let mut blocks = Vec::new();
    let mut code_lines = Vec::new();
    let mut in_code_block = false;

    for line in body.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("```") {
            if in_code_block {
                push_code_block(&mut blocks, &mut code_lines);
                if blocks.len() >= PREVIEW_BLOCK_LIMIT {
                    break;
                }
            }
            in_code_block = !in_code_block;
            continue;
        }

        if in_code_block {
            if !trimmed.is_empty() && code_lines.len() < CODE_LINE_LIMIT {
                code_lines.push(trimmed.to_string());
            }
            continue;
        }

        let Some(block) = parse_markdown_line(trimmed) else {
            continue;
        };

        blocks.push(block);
        if blocks.len() >= PREVIEW_BLOCK_LIMIT {
            break;
        }
    }

    if in_code_block && blocks.len() < PREVIEW_BLOCK_LIMIT {
        push_code_block(&mut blocks, &mut code_lines);
    }

    blocks
}

fn parse_markdown_line(line: &str) -> Option<MarkdownPreviewBlock> {
    if line.is_empty() {
        return None;
    }

    if let Some((level, text)) = heading(line) {
        return Some(MarkdownPreviewBlock::Heading {
            level,
            text: truncate_text(text, TEXT_LIMIT),
        });
    }

    if let Some(text) = line.strip_prefix("- ").or_else(|| line.strip_prefix("* ")) {
        return non_empty_block(text, MarkdownPreviewBlock::Bullet);
    }

    if let Some(text) = line.strip_prefix("> ") {
        return non_empty_block(text, MarkdownPreviewBlock::Quote);
    }

    Some(MarkdownPreviewBlock::Paragraph(truncate_text(line, TEXT_LIMIT)))
}

fn heading(line: &str) -> Option<(u8, &str)> {
    let level = line.chars().take_while(|character| *character == '#').count();
    if !(1..=3).contains(&level) {
        return None;
    }

    let text = line[level..].strip_prefix(' ')?;
    (!text.trim().is_empty()).then_some((level as u8, text.trim()))
}

fn non_empty_block(
    value: &str,
    block: impl FnOnce(String) -> MarkdownPreviewBlock,
) -> Option<MarkdownPreviewBlock> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| block(truncate_text(trimmed, TEXT_LIMIT)))
}

fn push_code_block(blocks: &mut Vec<MarkdownPreviewBlock>, code_lines: &mut Vec<String>) {
    if code_lines.is_empty() {
        return;
    }

    blocks.push(MarkdownPreviewBlock::Code(code_lines.join(" ")));
    code_lines.clear();
}

fn truncate_text(value: &str, limit: usize) -> String {
    let mut chars = value.chars();
    let truncated: String = chars.by_ref().take(limit).collect();
    if chars.next().is_some() { format!("{truncated}...") } else { truncated }
}

#[cfg(test)]
mod tests {
    use super::{MarkdownPreviewBlock, markdown_preview_blocks, truncate_text};

    #[test]
    fn markdown_preview_blocks_parse_headings_bullets_and_quotes() {
        let blocks = markdown_preview_blocks("# Heading\n- first\n> quoted\nplain");

        assert_eq!(
            blocks,
            vec![
                MarkdownPreviewBlock::Heading { level: 1, text: "Heading".to_string() },
                MarkdownPreviewBlock::Bullet("first".to_string()),
                MarkdownPreviewBlock::Quote("quoted".to_string()),
                MarkdownPreviewBlock::Paragraph("plain".to_string()),
            ]
        );
    }

    #[test]
    fn markdown_preview_blocks_parse_fenced_code() {
        let blocks = markdown_preview_blocks("```rust\nlet value = 1;\nvalue + 1\n```\nnext");

        assert_eq!(
            blocks,
            vec![
                MarkdownPreviewBlock::Code("let value = 1; value + 1".to_string()),
                MarkdownPreviewBlock::Paragraph("next".to_string()),
            ]
        );
    }

    #[test]
    fn markdown_preview_blocks_limit_visible_blocks() {
        let blocks = markdown_preview_blocks("one\ntwo\nthree\nfour\nfive");

        assert_eq!(blocks.len(), 4);
        assert_eq!(blocks[3], MarkdownPreviewBlock::Paragraph("four".to_string()));
    }

    #[test]
    fn truncate_text_appends_marker_for_long_text() {
        let truncated = truncate_text(&"a".repeat(130), 120);

        assert_eq!(truncated.len(), 123);
        assert!(truncated.ends_with("..."));
    }
}
