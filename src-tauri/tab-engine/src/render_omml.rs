//! MathML → OMML converter for embedding editable formulas in DOCX.
//! Uses typst's built-in HTML/MathML output.

use typst::diag::{FileError, FileResult};
use typst::foundations::{Bytes, Datetime, Duration};
use typst::syntax::{FileId, Source, RootedPath, VirtualPath, VirtualRoot};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, LibraryExt, World};
use typst_html::HtmlDocument;

struct HtmlWorld {
    main_id: FileId,
    main_source: Source,
    library: LazyHash<Library>,
    font_book: LazyHash<FontBook>,
    fonts: Vec<typst::text::Font>,
}

impl HtmlWorld {
    fn new(source: String) -> Self {
        let id = FileId::unique(RootedPath::new(
            VirtualRoot::Project,
            VirtualPath::new("math.typ").unwrap(),
        ));
        let main_source = Source::new(id, source);
        let mut font_book = FontBook::new();
        let mut fonts: Vec<typst::text::Font> = Vec::new();

        // Embed NewCM fonts
        let text_font = typst::text::Font::new(
            Bytes::new(crate::renderer::NEWCM10_REGULAR), 0,
        ).expect("failed to load NewCM10-Regular");
        font_book.push(text_font.info().clone());
        fonts.push(text_font);

        let math_font = typst::text::Font::new(
            Bytes::new(crate::renderer::NEWCM_MATH_REGULAR), 0,
        ).expect("failed to load NewCMMath-Regular");
        font_book.push(math_font.info().clone());
        fonts.push(math_font);

        Self {
            main_id: id,
            main_source,
            library: LazyHash::new(Library::builder().build()),
            font_book: LazyHash::new(font_book),
            fonts,
        }
    }
}

impl World for HtmlWorld {
    fn library(&self) -> &LazyHash<Library> { &self.library }
    fn book(&self) -> &LazyHash<FontBook> { &self.font_book }
    fn main(&self) -> FileId { self.main_id }
    fn source(&self, id: FileId) -> FileResult<Source> {
        if id == self.main_id { Ok(self.main_source.clone()) }
        else { Err(FileError::Other(Some("not available".into()))) }
    }
    fn file(&self, _id: FileId) -> FileResult<Bytes> {
        Err(FileError::Other(Some("not available".into())))
    }
    fn font(&self, index: usize) -> Option<typst::text::Font> {
        self.fonts.get(index).cloned()
    }
    fn today(&self, _offset: Option<Duration>) -> Option<Datetime> { None }
}

/// Render a math formula to HTML, then extract MathML.
pub fn render_math_to_mathml(content: &str, display: bool) -> Result<String, String> {
    use typst::foundations::Bytes;
    let content = content.trim();
    let source = if display {
        format!(
            "#set page(width: auto, height: auto, margin: 0pt)\n\
             #set text(size: 14pt, font: \"NewCM10-Regular\")\n\
             $ {content} $"
        )
    } else {
        format!(
            "#set page(width: auto, height: auto, margin: 0pt)\n\
             #set text(size: 12pt, font: \"NewCM10-Regular\")\n\
             ${content}$"
        )
    };

    let world = HtmlWorld::new(source);
    let warned = typst::compile::<HtmlDocument>(&world);
    let doc = warned.output.map_err(|e| {
        e.iter().map(|d| d.message.to_string()).collect::<Vec<_>>().join("; ")
    })?;

    // Convert HtmlDocument to string using typst_html's encoder
    let html = typst_html::html(&doc, &typst_html::HtmlOptions::default())
        .map_err(|e| e.iter().map(|d| d.message.to_string()).collect::<Vec<_>>().join("; "))?;
    extract_mathml(&html)
}

/// Extract MathML elements from HTML output.
fn extract_mathml(html: &str) -> Result<String, String> {
    // Find all <math> elements and concatenate
    let mut mathml = String::new();
    let mut start = 0;
    while let Some(pos) = html[start..].find("<math") {
        let abs = start + pos;
        if let Some(end) = html[abs..].find("</math>") {
            let element = &html[abs..abs + end + "</math>".len()];
            if !mathml.is_empty() {
                mathml.push('\n');
            }
            mathml.push_str(element);
            start = abs + end + "</math>".len();
        } else {
            break;
        }
    }
    if mathml.is_empty() {
        Err("No MathML found in HTML output".into())
    } else {
        Ok(mathml)
    }
}

/// Convert MathML to OMML XML string.
pub fn mathml_to_omml(mathml: &str) -> String {
    let mut omml = String::new();
    // Parse MathML and convert to OMML
    let doc = match roxmltree::Document::parse(mathml) {
        Ok(d) => d,
        Err(_) => return mathml_to_omml_fallback(mathml),
    };

    for node in doc.root().descendants() {
        if node.is_element() {
            match node.tag_name().name() {
                "math" => {
                    omml.push_str("<m:oMathPara xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\">");
                    omml.push_str("<m:oMath>");
                    for child in node.children() {
                        convert_node(child, &mut omml);
                    }
                    omml.push_str("</m:oMath></m:oMathPara>");
                }
                _ => {} // handled in recursion
            }
        }
    }
    omml
}

fn convert_node(node: roxmltree::Node, out: &mut String) {
    if !node.is_element() {
        if let Some(t) = node.text() {
            out.push_str(&escape_xml(t));
        }
        return;
    }
    match node.tag_name().name() {
        "mrow" => {
            for child in node.children() {
                convert_node(child, out);
            }
        }
        "mi" | "mn" => {
            let text = node.text().unwrap_or("");
            out.push_str(&format!("<m:r><m:t>{}</m:t></m:r>", escape_xml(text)));
        }
        "mo" => {
            let text = node.text().unwrap_or("");
            out.push_str(&format!("<m:r><m:t>{}</m:t></m:r>", escape_xml(text)));
        }
        "msup" => {
            out.push_str("<m:sSup><m:e>");
            for child in node.children() {
                if child.is_element() && child.has_tag_name("mrow") {
                    // skip mrow wrapper
                    for c in child.children() { convert_node(c, out); }
                } else {
                    convert_node(child, out);
                }
                break; // only first child is the base
            }
            out.push_str("</m:e><m:sup>");
            // The sup part is the second child element (skip whitespace)
            let mut found_first = false;
            for child in node.children() {
                if child.is_element() {
                    if !found_first { found_first = true; continue; }
                    convert_node(child, out);
                    break;
                }
            }
            out.push_str("</m:sup></m:sSup>");
        }
        "msub" => {
            out.push_str("<m:sSub><m:e>");
            for child in node.children() {
                if child.is_element() {
                    convert_node(child, out);
                    break;
                }
            }
            out.push_str("</m:e><m:sub>");
            let mut found_first = false;
            for child in node.children() {
                if child.is_element() {
                    if !found_first { found_first = true; continue; }
                    convert_node(child, out);
                    break;
                }
            }
            out.push_str("</m:sub></m:sSub>");
        }
        "mfrac" => {
            out.push_str("<m:f><m:num>");
            let mut count = 0;
            for child in node.children() {
                if child.is_element() {
                    if count == 0 {
                        convert_node(child, out);
                        out.push_str("</m:num><m:den>");
                    } else {
                        convert_node(child, out);
                    }
                    count += 1;
                }
            }
            out.push_str("</m:den></m:f>");
        }
        "msqrt" => {
            out.push_str("<m:rad><m:radPr><m:degHide m:val=\"1\"/></m:radPr><m:e>");
            for child in node.children() {
                convert_node(child, out);
            }
            out.push_str("</m:e></m:rad>");
        }
        "mover" => {
            out.push_str("<m:acc><m:accPr><m:chr m:val=\"&#x0302;\"/></m:accPr><m:e>");
            for child in node.children() {
                convert_node(child, out);
            }
            out.push_str("</m:e></m:acc>");
        }
        "munder" => {
            out.push_str("<m:limLow><m:e>");
            let mut found_first = false;
            for child in node.children() {
                if child.is_element() {
                    if !found_first { found_first = true; convert_node(child, out); }
                    else { out.push_str("</m:e><m:lim>"); convert_node(child, out); out.push_str("</m:lim>"); }
                }
            }
            out.push_str("</m:limLow>");
        }
        "munderover" => {
            out.push_str("<m:limUpp><m:limLow><m:e>");
            let elems: Vec<roxmltree::Node> = node.children().filter(|c| c.is_element()).collect();
            if !elems.is_empty() { convert_node(elems[0], out); }
            out.push_str("</m:e>");
            if elems.len() > 1 {
                out.push_str("<m:lim>"); convert_node(elems[1], out); out.push_str("</m:lim>");
            }
            out.push_str("</m:limLow>");
            if elems.len() > 2 {
                out.push_str("<m:lim>"); convert_node(elems[2], out); out.push_str("</m:lim>");
            }
            out.push_str("</m:limUpp>");
        }
        "mtable" => {
            // Matrix
            for child in node.children() {
                if child.has_tag_name("mtr") {
                    let mut first = true;
                    for cell in child.children() {
                        if cell.has_tag_name("mtd") {
                            if first { first = false; }
                            convert_node(cell, out);
                        }
                    }
                }
            }
        }
        _ => {
            // Unknown element — pass through children
            for child in node.children() {
                convert_node(child, out);
            }
        }
    }
}

/// Fallback: simple regex-based MathML → OMML conversion
fn mathml_to_omml_fallback(mathml: &str) -> String {
    // Remove <math> wrapper and use basic text extraction
    let inner = mathml
        .replace("<math xmlns=\"http://www.w3.org/1998/Math/MathML\">", "")
        .replace("<math>", "")
        .replace("</math>", "");
    format!(
        "<m:oMathPara xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\">\
         <m:oMath><m:r><m:t>{}</m:t></m:r></m:oMath></m:oMathPara>",
        escape_xml(&inner)
    )
}

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mathml_extraction() {
        let result = render_math_to_mathml("x^2", false);
        assert!(result.is_ok(), "render failed: {:?}", result.err());
        let mathml = result.unwrap();
        assert!(mathml.contains("<math"), "should contain math element");
        println!("{}", mathml);
    }

    #[test]
    fn test_mathml_to_omml() {
        let mathml = r#"<math xmlns="http://www.w3.org/1998/Math/MathML"><msup><mi>x</mi><mn>2</mn></msup></math>"#;
        let omml = mathml_to_omml(mathml);
        assert!(omml.contains("m:oMath"));
        assert!(omml.contains("m:sSup"));
    }
}
