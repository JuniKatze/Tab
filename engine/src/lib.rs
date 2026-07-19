pub mod parser;
pub mod render_omml;
pub mod renderer;

#[cfg(feature = "wasm")]
pub mod wasm;

use parser::Block;
use pulldown_cmark::{CowStr, Event, Parser, Tag, TagEnd};
use serde::Serialize;

/// Default CSS stylesheet for rendered HTML output.
pub const STYLE_CSS: &str = include_str!("style.css");

/// Return the default CSS stylesheet for .mtyp rendered HTML.
pub fn style_css() -> &'static str {
    STYLE_CSS
}

/// Result of rendering a .mtyp document.
#[derive(Debug, Clone, Serialize)]
pub struct RenderResult {
    /// Rendered HTML string (Markdown HTML + inline SVG for math).
    pub html: String,
    /// Number of math blocks rendered.
    pub math_count: usize,
}

/// Parse a .mtyp source string into a list of blocks.
pub fn parse(source: &str) -> Vec<Block> {
    parser::parse(source)
}

/// Render a single math formula to SVG.
pub fn render_math(content: &str, display: bool) -> Result<String, String> {
    renderer::render_math(content, display)
}

/// Render a complete .mtyp document.
///
/// Consecutive Text + InlineMath blocks are merged before Markdown rendering,
/// so inline formulas stay within their paragraphs instead of breaking lines.
pub fn render(source: &str) -> Result<RenderResult, String> {
    let blocks = parse(source);
    let mut html = String::new();
    let mut math_count = 0;

    // Accumulate consecutive Text + InlineMath blocks into a single paragraph.
    // Each entry: (placeholder_number, svg_html)
    let mut para_text = String::new();
    let mut para_svgs: Vec<(u32, String)> = Vec::new();
    let mut next_ph = 0u32;

    fn flush_paragraph(html: &mut String, para_text: &mut String, svgs: &mut Vec<(u32, String)>) {
        if para_text.is_empty() && svgs.is_empty() {
            return;
        }
        // Only math, no text: output SVGs directly
        if para_text.trim().is_empty() && !svgs.is_empty() {
            for (_, svg) in svgs.iter() {
                html.push_str(&format!(r#"<span class="math-inline">{}</span>"#, svg));
            }
        } else {
            // Render combined text through Markdown, then substitute placeholders
            let md_html = markdown_to_html(para_text);
            let mut result = md_html;
            for (num, svg) in svgs.iter() {
                let placeholder = format!("MATHPLACEHOLDER{}", num);
                let replacement = format!(r#"<span class="math-inline">{}</span>"#, svg);
                result = result.replace(&placeholder, &replacement);
            }
            html.push_str(&result);
        }
        para_text.clear();
        svgs.clear();
    }

    for block in &blocks {
        match block {
            Block::Text(text) => {
                para_text.push_str(text);
            }
            Block::InlineMath(content) => {
                let ph_num = next_ph;
                next_ph += 1;
                para_text.push_str(&format!("MATHPLACEHOLDER{}", ph_num));

                match renderer::render_math(content, false) {
                    Ok(svg) => {
                        para_svgs.push((ph_num, svg));
                        math_count += 1;
                    }
                    Err(e) => {
                        para_svgs.push((
                            ph_num,
                            format!(
                                r#"<span class="math-error" title="{}">${}$</span>"#,
                                escape_html(&e),
                                escape_html(content)
                            ),
                        ));
                    }
                }
            }
            Block::DisplayMath(content) => {
                flush_paragraph(&mut html, &mut para_text, &mut para_svgs);
                match renderer::render_math(content, true) {
                    Ok(svg) => {
                        html.push_str(&format!(r#"<div class="math-display">{}</div>"#, svg));
                        math_count += 1;
                    }
                    Err(e) => {
                        html.push_str(&format!(
                            r#"<div class="math-error" title="{}">$${}$$</div>"#,
                            escape_html(&e),
                            escape_html(content)
                        ));
                    }
                }
            }
            Block::Html(raw) => {
                flush_paragraph(&mut html, &mut para_text, &mut para_svgs);
                html.push_str(raw);
            }
        }
    }

    flush_paragraph(&mut html, &mut para_text, &mut para_svgs);

    let styled_html = format!(
        "<style>{}</style>\n<div class=\"preview-content\">\n{}\n</div>",
        STYLE_CSS, html
    );

    Ok(RenderResult {
        html: styled_html,
        math_count,
    })
}

/// Render Markdown text to HTML using pulldown-cmark.
fn markdown_to_html(text: &str) -> String {
    let parser = Parser::new(text).into_offset_iter();
    let mut events = Vec::new();
    let mut current_paragraph_start = None;
    let mut current_paragraph_text_end = 0;
    let mut last_paragraph = None;
    let mut list_stack = Vec::new();

    for (event, range) in parser {
        match event {
            Event::Start(Tag::Paragraph) => {
                current_paragraph_start = Some(events.len());
                current_paragraph_text_end = range.start;
                events.push(Event::Start(Tag::Paragraph));
            }
            Event::End(TagEnd::Paragraph) => {
                let end_idx = events.len();
                events.push(Event::End(TagEnd::Paragraph));

                if let Some(start_idx) = current_paragraph_start.take() {
                    last_paragraph = Some((start_idx, end_idx, current_paragraph_text_end));
                }
            }
            Event::Start(Tag::List(start)) => {
                let ordered = start.is_some();
                let attached_to_previous_paragraph = last_paragraph
                    .map(|(_, _, paragraph_end)| {
                        !has_blank_line_between(&text[paragraph_end..range.start])
                    })
                    .unwrap_or(false);

                if attached_to_previous_paragraph {
                    if let Some((start_idx, end_idx, _)) = last_paragraph {
                        events[start_idx] =
                            Event::Html(CowStr::Borrowed(r#"<p class="list-lead">"#));
                        events[end_idx] = Event::Html(CowStr::Borrowed("</p>"));
                    }
                    events.push(Event::Html(attached_list_start_html(start).into()));
                    list_stack.push(Some(ordered));
                } else {
                    events.push(Event::Start(Tag::List(start)));
                    list_stack.push(None);
                }
                last_paragraph = None;
            }
            Event::End(TagEnd::List(ordered)) => match list_stack.pop() {
                Some(Some(true)) => events.push(Event::Html(CowStr::Borrowed("</ol>"))),
                Some(Some(false)) => events.push(Event::Html(CowStr::Borrowed("</ul>"))),
                _ => events.push(Event::End(TagEnd::List(ordered))),
            },
            _ => {
                if current_paragraph_start.is_some() {
                    current_paragraph_text_end = current_paragraph_text_end.max(range.end);
                }
                events.push(event);
            }
        }
    }

    let mut html_output = String::new();
    pulldown_cmark::html::push_html(&mut html_output, events.into_iter());
    html_output
}

fn attached_list_start_html(start: Option<u64>) -> String {
    match start {
        Some(1) => r#"<ol class="list-attached">"#.to_string(),
        Some(number) => format!(r#"<ol class="list-attached" start="{}">"#, number),
        None => r#"<ul class="list-attached">"#.to_string(),
    }
}

fn has_blank_line_between(gap: &str) -> bool {
    let mut newlines = 0;

    for ch in gap.chars() {
        match ch {
            '\n' => {
                newlines += 1;
                if newlines >= 2 {
                    return true;
                }
            }
            '\r' | '\t' | ' ' => {}
            _ => newlines = 0,
        }
    }

    false
}

/// Basic HTML escaping for error messages.
fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple() {
        let blocks = parse("Hello $x^2$ world");
        assert_eq!(blocks.len(), 3);
        assert!(matches!(blocks[1], Block::InlineMath(_)));
    }

    #[test]
    fn test_parse_display_math() {
        let blocks = parse("Before $$x^2 + y^2$$ after");
        assert_eq!(blocks.len(), 3);
        assert!(matches!(blocks[1], Block::DisplayMath(_)));
    }

    #[test]
    fn test_markdown_rendering() {
        let html = markdown_to_html("**bold** and *italic*");
        assert!(html.contains("<strong>bold</strong>"));
        assert!(html.contains("<em>italic</em>"));
    }

    #[test]
    fn test_inline_math_in_paragraph() {
        let result = render("The formula $x^2$ is quadratic.").unwrap();
        // Inline math should be inside the <p> tag, not after it
        assert!(
            !result.html.contains("</p><span"),
            "math should be inside paragraph, not after it"
        );
        assert!(
            result.html.contains("math-inline"),
            "should contain math-inline span"
        );
    }

    #[test]
    fn test_render_includes_current_stylesheet() {
        let result = render("Text").unwrap();
        assert!(result.html.contains("--tab-body-line-height: 1.6"));
        assert!(result.html.contains("--tab-paragraph-gap: 1em"));
        assert!(result.html.contains("vertical-align: 0em"));
    }

    #[test]
    fn test_attached_list_uses_paragraph_spacing_class() {
        let html = markdown_to_html("理由如下：\n1. xxx\n2. xxx");
        assert!(html.contains(r#"<p class="list-lead">理由如下：</p>"#));
        assert!(html.contains(r#"<ol class="list-attached">"#));
    }

    #[test]
    fn test_blank_line_before_list_keeps_block_spacing() {
        let html = markdown_to_html("理由如下：\n\n1. xxx\n2. xxx");
        assert!(html.contains("<p>理由如下：</p>"));
        assert!(html.contains("<ol>"));
        assert!(!html.contains("list-lead"));
        assert!(!html.contains("list-attached"));
    }
}

#[test]
fn debug_render() {
    let result = render("The formula $x^2$ is quadratic.").unwrap();
    println!("=== HTML ===");
    println!("{}", result.html);
}
