use serde::Serialize;

/// A block in a .mtyp document.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(tag = "type", content = "content")]
pub enum Block {
    /// Plain Markdown text (may include formatting, headings, etc.)
    Text(String),
    /// Inline math: $...$
    InlineMath(String),
    /// Display math: $$...$$
    DisplayMath(String),
    /// Raw HTML (for SVG or other pre-rendered content)
    Html(String),
}

/// Parse a .mtyp source string into a sequence of blocks.
///
/// Rules:
/// - `$$...$$` delimits display math (must be on its own or starts at line beginning).
///   Actually, we treat `$$` at any position as display math opener.
/// - `$...$` delimits inline math.
/// - `\$` is an escaped dollar sign (literal `$`).
/// - Inside math mode, `\$` still escapes, and we track brace nesting
///   to correctly identify the closing `$`.
pub fn parse(source: &str) -> Vec<Block> {
    let chars: Vec<char> = source.chars().collect();
    let len = chars.len();
    let mut blocks = Vec::new();
    let mut i = 0;
    let mut text_buf = String::new();

    while i < len {
        // Check for escaped dollar
        if chars[i] == '\\' && i + 1 < len && chars[i + 1] == '$' {
            text_buf.push('$');
            i += 2;
            continue;
        }

        // Check for $$
        if chars[i] == '$' && i + 1 < len && chars[i + 1] == '$' {
            // Flush text buffer
            if !text_buf.is_empty() {
                blocks.push(Block::Text(std::mem::take(&mut text_buf)));
            }
            i += 2; // skip $$
            let math_content = extract_math(&chars, &mut i, len, false);
            blocks.push(Block::DisplayMath(math_content));
            continue;
        }

        // Check for single $ (inline math)
        if chars[i] == '$' {
            // Flush text buffer
            if !text_buf.is_empty() {
                blocks.push(Block::Text(std::mem::take(&mut text_buf)));
            }
            i += 1; // skip $
            let math_content = extract_math(&chars, &mut i, len, false);
            blocks.push(Block::InlineMath(math_content));
            continue;
        }

        text_buf.push(chars[i]);
        i += 1;
    }

    if !text_buf.is_empty() {
        blocks.push(Block::Text(std::mem::take(&mut text_buf)));
    }

    blocks
}

/// Extract math content until the matching closing `$` or `$$`.
/// `i` points past the opening delimiter.
/// Returns the inner math content (without surrounding delimiters).
fn extract_math(chars: &[char], i: &mut usize, len: usize, _display: bool) -> String {
    let mut content = String::new();
    let mut brace_depth = 0u32;

    while *i < len {
        // Handle escaped characters inside math
        if chars[*i] == '\\' && *i + 1 < len {
            content.push(chars[*i]);
            content.push(chars[*i + 1]);
            *i += 2;
            continue;
        }

        // Track brace nesting
        if chars[*i] == '{' {
            brace_depth += 1;
            content.push(chars[*i]);
            *i += 1;
            continue;
        }
        if chars[*i] == '}' {
            brace_depth = brace_depth.saturating_sub(1);
            content.push(chars[*i]);
            *i += 1;
            continue;
        }

        // Check for closing $$
        if chars[*i] == '$' && *i + 1 < len && chars[*i + 1] == '$' && brace_depth == 0 {
            *i += 2; // skip $$
            return content;
        }

        // Check for closing single $
        if chars[*i] == '$' && brace_depth == 0 {
            *i += 1; // skip $
            return content;
        }

        content.push(chars[*i]);
        *i += 1;
    }

    // EOF reached without closing — treat everything as content
    content
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plain_text() {
        let result = parse("Hello world");
        assert_eq!(result, vec![Block::Text("Hello world".into())]);
    }

    #[test]
    fn test_inline_math() {
        let result = parse("The formula $x^2$ is quadratic.");
        assert_eq!(
            result,
            vec![
                Block::Text("The formula ".into()),
                Block::InlineMath("x^2".into()),
                Block::Text(" is quadratic.".into()),
            ]
        );
    }

    #[test]
    fn test_display_math() {
        let result = parse("Before $$x^2$$ after");
        assert_eq!(
            result,
            vec![
                Block::Text("Before ".into()),
                Block::DisplayMath("x^2".into()),
                Block::Text(" after".into()),
            ]
        );
    }

    #[test]
    fn test_escaped_dollar() {
        let result = parse(r"Price is \$100, formula $x^2$");
        assert_eq!(
            result,
            vec![
                Block::Text("Price is $100, formula ".into()),
                Block::InlineMath("x^2".into()),
            ]
        );
    }

    #[test]
    fn test_nested_braces_in_math() {
        let result = parse(r"$f(x) = \frac{1}{2}$");
        assert_eq!(
            result,
            vec![Block::InlineMath(r"f(x) = \frac{1}{2}".into())]
        );
    }

    #[test]
    fn test_multiple_math_blocks() {
        let result = parse("$a$ and $b$ and $c$");
        assert_eq!(
            result,
            vec![
                Block::InlineMath("a".into()),
                Block::Text(" and ".into()),
                Block::InlineMath("b".into()),
                Block::Text(" and ".into()),
                Block::InlineMath("c".into()),
            ]
        );
    }
}
