pub mod parser;
pub mod renderer;
pub mod render_omml;

#[cfg(feature = "wasm")]
pub mod wasm;

use parser::Block;
use serde::Serialize;

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

    fn flush_paragraph(
        html: &mut String,
        para_text: &mut String,
        svgs: &mut Vec<(u32, String)>,
    ) {
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
                        para_svgs.push((ph_num, format!(
                            r#"<span class="math-error" title="{}">${}$</span>"#,
                            escape_html(&e),
                            escape_html(content)
                        )));
                    }
                }
            }
            Block::DisplayMath(content) => {
                flush_paragraph(&mut html, &mut para_text, &mut para_svgs);
                match renderer::render_math(content, true) {
                    Ok(svg) => {
                        html.push_str(&format!(
                            r#"<div class="math-display">{}</div>"#,
                            svg
                        ));
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

    Ok(RenderResult { html, math_count })
}

/// Render Markdown text to HTML using pulldown-cmark.
fn markdown_to_html(text: &str) -> String {
    let parser = pulldown_cmark::Parser::new(text);
    let mut html_output = String::new();
    pulldown_cmark::html::push_html(&mut html_output, parser);
    html_output
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
        assert!(!result.html.contains("</p><span"), "math should be inside paragraph, not after it");
        assert!(result.html.contains("math-inline"), "should contain math-inline span");
    }
}

    #[test]
    fn debug_render() {
        let result = render("The formula $x^2$ is quadratic.").unwrap();
        println!("=== HTML ===");
        println!("{}", result.html);
    }
