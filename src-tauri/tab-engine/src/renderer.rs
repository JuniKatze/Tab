use typst::diag::{FileError, FileResult};
use typst::foundations::{Bytes, Datetime, Duration};
use typst::syntax::{FileId, Source, RootedPath, VirtualPath, VirtualRoot};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, LibraryExt, World};
use typst_layout::PagedDocument;
use typst_svg::{svg, SvgOptions};

/// Embedded New Computer Modern fonts for math rendering.
const NEWCM_MATH_REGULAR: &[u8] = include_bytes!("../fonts/NewCMMath-Regular.otf");
const NEWCM10_REGULAR: &[u8] = include_bytes!("../fonts/NewCM10-Regular.otf");

/// A minimal in-memory World implementation for rendering math formulas.
struct MathWorld {
    main_id: FileId,
    main_source: Source,
    library: LazyHash<Library>,
    font_book: LazyHash<FontBook>,
    fonts: Vec<Font>,
}

impl MathWorld {
    fn new(source: String) -> Self {
        let id = FileId::unique(RootedPath::new(
            VirtualRoot::Project,
            VirtualPath::new("math.typ").unwrap(),
        ));

        let main_source = Source::new(id, source);

        // Build font book with embedded NewCM fonts.
        let mut font_book = FontBook::new();
        let mut fonts: Vec<Font> = Vec::new();

        // Add NewCM10-Regular (text font)
        let text_font = Font::new(Bytes::new(NEWCM10_REGULAR), 0)
            .expect("failed to load NewCM10-Regular");
        font_book.push(text_font.info().clone());
        fonts.push(text_font);

        // Add NewCMMath-Regular (math font)
        let math_font = Font::new(Bytes::new(NEWCM_MATH_REGULAR), 0)
            .expect("failed to load NewCMMath-Regular");
        font_book.push(math_font.info().clone());
        fonts.push(math_font);

        Self {
            main_id: id,
            main_source,
            library: LazyHash::new(Library::default()),
            font_book: LazyHash::new(font_book),
            fonts,
        }
    }
}

impl World for MathWorld {
    fn library(&self) -> &LazyHash<Library> {
        &self.library
    }

    fn book(&self) -> &LazyHash<FontBook> {
        &self.font_book
    }

    fn main(&self) -> FileId {
        self.main_id
    }

    fn source(&self, id: FileId) -> FileResult<Source> {
        if id == self.main_id {
            Ok(self.main_source.clone())
        } else {
            Err(FileError::Other(Some("not available".into())))
        }
    }

    fn file(&self, _id: FileId) -> FileResult<Bytes> {
        Err(FileError::Other(Some("not available".into())))
    }

    fn font(&self, index: usize) -> Option<Font> {
        self.fonts.get(index).cloned()
    }

    fn today(&self, _offset: Option<Duration>) -> Option<Datetime> {
        None
    }
}

/// Render a math formula to SVG.
///
/// * `content` - The math formula content (without `$` delimiters)
/// * `display` - Whether this is display math (affects styling)
///
/// Returns an SVG string.
pub fn render_math(content: &str, display: bool) -> Result<String, String> {
    let source = build_math_document(content, display);
    let world = MathWorld::new(source);

    let warned = typst::compile::<PagedDocument>(&world);
    let document = warned
        .output
        .map_err(|errors| format_errors(&errors))?;

    let pages = document.pages();
    if pages.is_empty() {
        return Err("no pages produced".into());
    }

    let opts = SvgOptions::default();
    let svg_str = svg(&pages[0], &opts);
    Ok(extract_svg_inner(&svg_str))
}

/// Build a minimal typst document around a math formula.
///
/// In Typst, spacing around the math content determines the style:
/// - `$x$` (no spaces) → inline style (compact fractions, smaller operators)
/// - `$ x $` (spaces)  → display style (full-size fractions, larger operators)
///
/// We always trim user content and control style explicitly via the `display`
/// parameter, so the user's whitespace doesn't accidentally change the style.
fn build_math_document(content: &str, display: bool) -> String {
    let content = content.trim();
    if display {
        format!(
            "#set page(width: auto, height: auto, margin: 0pt)\n#set text(size: 14pt, font: \"NewCM10-Regular\")\n$ {content} $"
        )
    } else {
        format!(
            "#set page(width: auto, height: auto, margin: 0pt)\n#set text(size: 12pt, font: \"NewCM10-Regular\")\n${content}$"
        )
    }
}

/// Extract the SVG content, stripping only the XML declaration.
/// Keeps the <svg> tag so browsers can render it properly.
/// Also adds overflow="visible" to prevent viewBox clipping.
fn extract_svg_inner(svg_str: &str) -> String {
    let svg_str = svg_str.trim();
    // Strip XML declaration if present
    let svg_str = if let Some(rest) = svg_str.strip_prefix("<?xml") {
        if let Some(pos) = rest.find("?>") {
            rest[pos + 2..].trim()
        } else {
            svg_str
        }
    } else {
        svg_str
    };
    // Add overflow="visible" to the <svg> tag to prevent clipping
    if let Some(tag_end) = svg_str.find('>') {
        let tag = &svg_str[..tag_end];
        if !tag.contains("overflow") {
            let rest = &svg_str[tag_end..];
            return format!("{} overflow=\"visible\"{}", tag, rest);
        }
    }
    svg_str.to_string()
}

fn format_errors(errors: &[typst::diag::SourceDiagnostic]) -> String {
    errors
        .iter()
        .map(|e| format!("{}", e.message))
        .collect::<Vec<_>>()
        .join("; ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_inline_math() {
        let result = render_math("x^2", false);
        assert!(result.is_ok(), "render failed: {:?}", result.err());
        let svg = result.unwrap();
        assert!(!svg.is_empty(), "SVG should not be empty");
    }

    #[test]
    fn test_render_frac() {
        let result = render_math("frac(a, b)", true);
        assert!(result.is_ok(), "render frac failed: {:?}", result.err());
    }
}
