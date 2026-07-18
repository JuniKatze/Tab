//! MathML → OMML converter for embedding editable formulas in DOCX.
//! Uses typst's built-in HTML/MathML output.

use typst::diag::{FileError, FileResult};
use typst::foundations::{Bytes, Datetime, Duration};
use typst::syntax::{FileId, RootedPath, Source, VirtualPath, VirtualRoot};
use typst::text::FontBook;
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
        let text_font = typst::text::Font::new(Bytes::new(crate::renderer::NEWCM10_REGULAR), 0)
            .expect("failed to load NewCM10-Regular");
        font_book.push(text_font.info().clone());
        fonts.push(text_font);

        let math_font = typst::text::Font::new(Bytes::new(crate::renderer::NEWCM_MATH_REGULAR), 0)
            .expect("failed to load NewCMMath-Regular");
        font_book.push(math_font.info().clone());
        fonts.push(math_font);

        // Embed CJK font for text inside math formulas
        let cjk_font = typst::text::Font::new(
            Bytes::new(crate::renderer::NOTO_SERIF_CJK_SC_REGULAR),
            0,
        )
        .expect("failed to load NotoSerifCJKsc-Regular");
        font_book.push(cjk_font.info().clone());
        fonts.push(cjk_font);

        Self {
            main_id: id,
            main_source,
            library: LazyHash::new(
                Library::builder()
                    .with_features(typst::Features::all())
                    .build(),
            ),
            font_book: LazyHash::new(font_book),
            fonts,
        }
    }
}

impl World for HtmlWorld {
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
    fn font(&self, index: usize) -> Option<typst::text::Font> {
        self.fonts.get(index).cloned()
    }
    fn today(&self, _offset: Option<Duration>) -> Option<Datetime> {
        None
    }
}

/// Render a math formula to HTML, then extract MathML.
pub fn render_math_to_mathml(content: &str, display: bool) -> Result<String, String> {
    let content = content.trim();
    let source = if display {
        format!(
            "#set page(width: auto, height: auto, margin: 0pt)\n\
             #set text(size: 12pt, font: (\"NewCM10-Regular\", \"Noto Serif CJK SC\"))\n\
             $ {content} $"
        )
    } else {
        format!(
            "#set page(width: auto, height: auto, margin: 0pt)\n\
             #set text(size: 12pt, font: (\"NewCM10-Regular\", \"Noto Serif CJK SC\"))\n\
             ${content}$"
        )
    };

    let world = HtmlWorld::new(source);
    let warned = typst::compile::<HtmlDocument>(&world);
    let doc = warned.output.map_err(|e| {
        e.iter()
            .map(|d| d.message.to_string())
            .collect::<Vec<_>>()
            .join("; ")
    })?;

    // Convert HtmlDocument to string using typst_html's encoder
    let html = typst_html::html(&doc, &typst_html::HtmlOptions::default()).map_err(|e| {
        e.iter()
            .map(|d| d.message.to_string())
            .collect::<Vec<_>>()
            .join("; ")
    })?;
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
/// `display`: true for block/display math (m:oMathPara), false for inline (m:oMath).
pub fn mathml_to_omml(mathml: &str, display: bool) -> String {
    let mut omml = String::new();
    let doc = match roxmltree::Document::parse(mathml) {
        Ok(d) => d,
        Err(_) => return mathml_to_omml_fallback(mathml),
    };

    for node in doc.root().descendants() {
        if node.is_element() && node.tag_name().name() == "math" {
            let ns = "xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\"";
            if display {
                omml.push_str(&format!("<m:oMathPara {}><m:oMath>", ns));
            } else {
                omml.push_str(&format!("<m:oMath {}>", ns));
            }

            // Collect element children so we can look ahead for msubsup/munderover
            let children: Vec<roxmltree::Node> =
                node.children().filter(|c| c.is_element()).collect();
            let mut i = 0;
            while i < children.len() {
                // Check if this mrow wraps a n-ary operator as its first-or-later child.
                // Typst sometimes wraps `[∑ ...]` in an mrow, hiding the n-ary from us.
                if children[i].has_tag_name("mrow") {
                    let mrow_kids: Vec<roxmltree::Node> =
                        children[i].children().filter(|c| c.is_element()).collect();
                    if let Some(nary_pos) = mrow_kids.iter().position(|c| {
                        (c.has_tag_name("munderover") || c.has_tag_name("msubsup"))
                            && nary_child_is_operator(c)
                    }) {
                        let nary_node = mrow_kids[nary_pos];
                        let nk: Vec<roxmltree::Node> =
                            nary_node.children().filter(|c| c.is_element()).collect();
                        if nk.len() >= 3 {
                            let op_text: String = nk[0]
                                .descendants()
                                .filter(|d| d.is_text())
                                .map(|d| d.text().unwrap_or(""))
                                .collect();
                            let op = op_text.trim();
                            if is_nary_operator(op) {
                                // Convert mrow children BEFORE the n-ary (e.g. opening bracket)
                                for j in 0..nary_pos {
                                    convert_node(mrow_kids[j], &mut omml);
                                }
                                omml.push_str(&format!(
                                    "<m:nary><m:naryPr><m:chr m:val=\"{}\"/><m:limLoc m:val=\"subSup\"/></m:naryPr>",
                                    escape_xml(op)));
                                omml.push_str("<m:e>");
                                // Body: mrow children after n-ary plus math root siblings
                                for j in (nary_pos + 1)..mrow_kids.len() {
                                    convert_node(mrow_kids[j], &mut omml);
                                }
                                for j in (i + 1)..children.len() {
                                    convert_node(children[j], &mut omml);
                                }
                                omml.push_str("</m:e>");
                                omml.push_str("<m:sub>");
                                convert_node(nk[1], &mut omml);
                                omml.push_str("</m:sub>");
                                omml.push_str("<m:sup>");
                                convert_node(nk[2], &mut omml);
                                omml.push_str("</m:sup>");
                                omml.push_str("</m:nary>");
                                break; // all remaining children consumed
                            }
                        }
                    }
                    convert_node(children[i], &mut omml);
                    i += 1;
                    continue;
                }

                let is_nary = children[i].has_tag_name("msubsup")
                    || children[i].has_tag_name("munderover");
                if is_nary {
                    // msubsup / munderover may represent an n-ary operator
                    // (∫, ∑, ∏, etc.) or a combined sub+superscript (e.g. a_b^c).
                    let nary_children: Vec<roxmltree::Node> =
                        children[i].children().filter(|c| c.is_element()).collect();
                    if nary_children.len() >= 3 {
                        // Extract operator text from descendants (not just direct text)
                        let op_text: String = nary_children[0]
                            .descendants()
                            .filter(|d| d.is_text())
                            .map(|d| d.text().unwrap_or(""))
                            .collect();
                        let op = op_text.trim();
                        if is_nary_operator(op) {
                            omml.push_str(&format!(
                                "<m:nary><m:naryPr><m:chr m:val=\"{}\"/><m:limLoc m:val=\"subSup\"/></m:naryPr>",
                                escape_xml(op)));
                            omml.push_str("<m:e>");
                            // All remaining siblings are the integrand/body
                            for j in (i + 1)..children.len() {
                                convert_node(children[j], &mut omml);
                            }
                            omml.push_str("</m:e>");
                            omml.push_str("<m:sub>");
                            convert_node(nary_children[1], &mut omml);
                            omml.push_str("</m:sub>");
                            omml.push_str("<m:sup>");
                            convert_node(nary_children[2], &mut omml);
                            omml.push_str("</m:sup>");
                            omml.push_str("</m:nary>");
                            break; // all remaining children consumed
                        }
                    }
                    // Not an n-ary operator: fall through to normal conversion
                    convert_node(children[i], &mut omml);
                } else {
                    convert_node(children[i], &mut omml);
                }
                i += 1;
            }

            if display {
                omml.push_str("</m:oMath></m:oMathPara>");
            } else {
                omml.push_str("</m:oMath>");
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
            // Check if this is a matrix/cases wrapper (mrow containing mtable
            // with optional delimiter mo siblings).
            let child_elems: Vec<roxmltree::Node> =
                node.children().filter(|c| c.is_element()).collect();
            let has_mtable = child_elems.iter().any(|c| c.has_tag_name("mtable"));
            if has_mtable {
                // Let the mtable handler output proper OMML delimiters.
                // Skip mo children here to avoid double delimiters.
                for child in node.children() {
                    if child.is_element() && child.has_tag_name("mo") {
                        continue; // handled by mtable
                    }
                    convert_node(child, out);
                }
            } else if child_elems.len() == 1 && child_elems[0].has_tag_name("mtable") {
                // Single mtable child (no explicit delimiters)
                convert_node(child_elems[0], out);
            } else {
                // General mrow: look ahead for n-ary operators so their body
                // (subsequent siblings) is placed inside <m:e> instead of after.
                let all_children: Vec<roxmltree::Node> =
                    node.children().collect();
                let mut j = 0;
                while j < all_children.len() {
                    let child = all_children[j];
                    if child.is_element()
                        && (child.has_tag_name("munderover") || child.has_tag_name("msubsup"))
                    {
                        let nk: Vec<roxmltree::Node> =
                            child.children().filter(|c| c.is_element()).collect();
                        if nk.len() >= 3 {
                            let op_text: String = nk[0]
                                .descendants()
                                .filter(|d| d.is_text())
                                .map(|d| d.text().unwrap_or(""))
                                .collect();
                            if is_nary_operator(op_text.trim()) {
                                let op = op_text.trim();
                                out.push_str(&format!(
                                    "<m:nary><m:naryPr><m:chr m:val=\"{}\"/><m:limLoc m:val=\"subSup\"/></m:naryPr>",
                                    escape_xml(op)));
                                out.push_str("<m:e>");
                                // Consume subsequent siblings into <m:e>
                                j += 1;
                                while j < all_children.len() {
                                    convert_node(all_children[j], out);
                                    j += 1;
                                }
                                out.push_str("</m:e>");
                                out.push_str("<m:sub>");
                                convert_node(nk[1], out);
                                out.push_str("</m:sub>");
                                out.push_str("<m:sup>");
                                convert_node(nk[2], out);
                                out.push_str("</m:sup>");
                                out.push_str("</m:nary>");
                                break;
                            }
                        }
                    }
                    convert_node(child, out);
                    j += 1;
                }
            }
        }
        "mspace" => {
            // Use non-breaking spaces (U+00A0) instead of em-spaces (U+2003).
            // Em-spaces can skew column-width calculations inside matrix cells.
            // U+00A0 is preserved in math zones and gives a consistent advance.
            let width_str = node.attribute("width").unwrap_or("");
            let n_em = parse_em_width(width_str);
            let n_nbsp = n_em * 3; // approximate 1em ≈ 3 narrow spaces in math font
            let spaces: String = std::iter::repeat('\u{00A0}').take(n_nbsp).collect();
            out.push_str(&format!("<m:r><m:t xml:space=\"preserve\">{}</m:t></m:r>", spaces));
        }
        "mi" | "mn" | "mtext" | "ms" => {
            let text = node.text().unwrap_or("");
            out.push_str(&format!("<m:r><m:rPr><m:sty m:val=\"p\"/></m:rPr><m:t>{}</m:t></m:r>", escape_xml(text)));
        }
        "mo" => {
            let text = node.text().unwrap_or("");
            out.push_str(&format!("<m:r><m:rPr><m:sty m:val=\"p\"/></m:rPr><m:t>{}</m:t></m:r>", escape_xml(text)));
        }
        "msup" => {
            out.push_str("<m:sSup><m:e>");
            for child in node.children() {
                if child.is_element() && child.has_tag_name("mrow") {
                    // skip mrow wrapper
                    for c in child.children() {
                        convert_node(c, out);
                    }
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
                    if !found_first {
                        found_first = true;
                        continue;
                    }
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
                    if !found_first {
                        found_first = true;
                        continue;
                    }
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
        "mroot" => {
            let elems: Vec<roxmltree::Node> =
                node.children().filter(|c| c.is_element()).collect();
            out.push_str("<m:rad><m:radPr><m:degHide m:val=\"0\"/></m:radPr><m:deg>");
            if elems.len() >= 2 {
                convert_node(elems[1], out); // root degree (index)
            }
            out.push_str("</m:deg><m:e>");
            if !elems.is_empty() {
                convert_node(elems[0], out); // radicand (base)
            }
            out.push_str("</m:e></m:rad>");
        }
        "mover" => {
            let elems: Vec<roxmltree::Node> =
                node.children().filter(|c| c.is_element()).collect();
            // Map accent character to combining Unicode for OMML <m:chr>
            let accent_char = if elems.len() >= 2 {
                elems[1].text().unwrap_or("^")
            } else {
                "^"
            };
            let combining = accent_to_combining(accent_char);
            out.push_str(&format!(
                "<m:acc><m:accPr><m:chr m:val=\"{}\"/></m:accPr><m:e>",
                combining
            ));
            if !elems.is_empty() {
                convert_node(elems[0], out);
            }
            out.push_str("</m:e></m:acc>");
        }
        "munder" => {
            out.push_str("<m:limLow><m:e>");
            let mut found_first = false;
            for child in node.children() {
                if child.is_element() {
                    if !found_first {
                        found_first = true;
                        convert_node(child, out);
                    } else {
                        out.push_str("</m:e><m:lim>");
                        convert_node(child, out);
                        out.push_str("</m:lim>");
                    }
                }
            }
            out.push_str("</m:limLow>");
        }
        "munderover" => {
            let elems: Vec<roxmltree::Node> = node.children().filter(|c| c.is_element()).collect();
            // Detect n-ary operator (∑, ∏, ∫, etc.) vs combined sub+superscript
            let is_nary = if !elems.is_empty() {
                let op_text: String = elems[0]
                    .descendants()
                    .filter(|d| d.is_text())
                    .map(|d| d.text().unwrap_or(""))
                    .collect();
                is_nary_operator(op_text.trim())
            } else {
                false
            };
            if is_nary {
                // N-ary operator nested inside a fraction/group:
                // emit <m:nary> with sub/sup as limits (body siblings handled by caller).
                // <m:e/> is required by OMML spec even when empty — without it Word
                // renders a dashed placeholder box.
                let op_text: String = elems[0]
                    .descendants()
                    .filter(|d| d.is_text())
                    .map(|d| d.text().unwrap_or(""))
                    .collect();
                let op = op_text.trim();
                out.push_str(&format!(
                    "<m:nary><m:naryPr><m:chr m:val=\"{}\"/><m:limLoc m:val=\"subSup\"/></m:naryPr>",
                    escape_xml(op)));
                out.push_str("<m:e/>");
                out.push_str("<m:sub>");
                if elems.len() > 1 {
                    convert_node(elems[1], out);
                }
                out.push_str("</m:sub>");
                out.push_str("<m:sup>");
                if elems.len() > 2 {
                    convert_node(elems[2], out);
                }
                out.push_str("</m:sup>");
                out.push_str("</m:nary>");
            } else {
                // Regular combined sub+superscript (e.g. a_b^c)
                out.push_str("<m:sSubSup><m:e>");
                if !elems.is_empty() {
                    convert_node(elems[0], out);
                }
                out.push_str("</m:e><m:sub>");
                if elems.len() > 1 {
                    convert_node(elems[1], out);
                }
                out.push_str("</m:sub><m:sup>");
                if elems.len() > 2 {
                    convert_node(elems[2], out);
                }
                out.push_str("</m:sup></m:sSubSup>");
            }
        }
        "msubsup" => {
            let elems: Vec<roxmltree::Node> = node.children().filter(|c| c.is_element()).collect();
            if elems.len() >= 3 {
                // Detect n-ary operator (∫, ∑, ∏, etc.) vs regular sub+superscript
                let op_text: String = elems[0]
                    .descendants()
                    .filter(|d| d.is_text())
                    .map(|d| d.text().unwrap_or(""))
                    .collect();
                let op = op_text.trim();
                if is_nary_operator(op) {
                    // N-ary operator nested inside a fraction/group
                    out.push_str(&format!(
                        "<m:nary><m:naryPr><m:chr m:val=\"{}\"/><m:limLoc m:val=\"subSup\"/></m:naryPr>",
                        escape_xml(op)));
                    out.push_str("<m:e/>");
                    out.push_str("<m:sub>");
                    convert_node(elems[1], out);
                    out.push_str("</m:sub>");
                    out.push_str("<m:sup>");
                    convert_node(elems[2], out);
                    out.push_str("</m:sup>");
                    out.push_str("</m:nary>");
                } else {
                    // Regular sub+superscript (e.g. r_i^2, x_i^2)
                    out.push_str("<m:sSubSup><m:e>");
                    convert_node(elems[0], out);
                    out.push_str("</m:e><m:sub>");
                    convert_node(elems[1], out);
                    out.push_str("</m:sub><m:sup>");
                    convert_node(elems[2], out);
                    out.push_str("</m:sup></m:sSubSup>");
                }
            } else {
                for e in &elems {
                    convert_node(*e, out);
                }
            }
        }
        "mtable" => {
            // Detect delimiter characters from surrounding mrow/mo context.
            let parent = node.parent().unwrap();
            let is_cases = node.attribute("class").map_or(false, |c| c == "cases");
            let wrap_delim = parent.has_tag_name("mrow");

            if wrap_delim {
                let (left, right) = detect_delimiters(parent, node, is_cases);
                out.push_str("<m:d><m:dPr>");
                out.push_str(&format!("<m:begChr m:val=\"{}\"/>", escape_xml(&left)));
                if let Some(r) = &right {
                    out.push_str(&format!("<m:endChr m:val=\"{}\"/>", escape_xml(r)));
                }
                out.push_str("</m:dPr><m:e><m:m>");
                if is_cases {
                    let num_cols = node
                        .children()
                        .find(|c| c.has_tag_name("mtr"))
                        .map(|first_row| {
                            first_row
                                .children()
                                .filter(|c| c.has_tag_name("mtd"))
                                .count()
                        })
                        .unwrap_or(1);
                    out.push_str("<m:mPr><m:mcs>");
                    for _ in 0..num_cols {
                        out.push_str(
                            "<m:mc><m:mcPr><m:count m:val=\"1\"/><m:mcJc m:val=\"left\"/></m:mcPr></m:mc>",
                        );
                    }
                    out.push_str("</m:mcs></m:mPr>");
                }
            } else {
                // Standalone mtable (e.g. multi-line formula with \ line break):
                // use <m:eqArr> for equation array — <m:m> as a direct child of
                // <m:oMath> has inconsistent support across office suites.
                out.push_str("<m:eqArr>");
            }
            for child in node.children() {
                if child.has_tag_name("mtr") {
                    if wrap_delim {
                        out.push_str("<m:mr>");
                    }
                    for cell in child.children() {
                        if cell.has_tag_name("mtd") {
                            out.push_str("<m:e>");
                            convert_node(cell, out);
                            out.push_str("</m:e>");
                        }
                    }
                    if wrap_delim {
                        out.push_str("</m:mr>");
                    }
                }
            }
            if wrap_delim {
                out.push_str("</m:m></m:e></m:d>");
            } else {
                out.push_str("</m:eqArr>");
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

/// Determine the left and right delimiter characters for an `mtable`
/// inside an `mrow`. For cases (class="cases"), left is `{` and right is
/// an explicit empty string (Word defaults to `)` when `endChr` is absent).
/// For matrices, detects delimiters from sibling `mo` elements.
fn detect_delimiters(
    parent: roxmltree::Node,
    mtable: roxmltree::Node,
    is_cases: bool,
) -> (String, Option<String>) {
    if is_cases {
        // Explicit empty endChr prevents Word from defaulting to ")"
        return ("{".to_string(), Some(String::new()));
    }
    let children: Vec<roxmltree::Node> = parent.children().filter(|c| c.is_element()).collect();
    let mtable_pos = children.iter().position(|c| *c == mtable);
    let left = mtable_pos
        .and_then(|pos| {
            if pos > 0 && children[pos - 1].has_tag_name("mo") {
                children[pos - 1].text().map(|t| t.to_string())
            } else {
                None
            }
        })
        .unwrap_or_else(|| "[".to_string());
    let right = mtable_pos.and_then(|pos| {
        if pos + 1 < children.len() && children[pos + 1].has_tag_name("mo") {
            children[pos + 1].text().map(|t| t.to_string())
        } else {
            None
        }
    });
    (left, right)
}

/// Map a MathML accent character to its combining Unicode equivalent for OMML.
fn accent_to_combining(ch: &str) -> String {
    match ch {
        "^" | "\u{0302}" => "\u{0302}".to_string(),   // hat
        "~" | "\u{0303}" => "\u{0303}".to_string(),   // tilde
        "\u{2192}" | "\u{20D7}" => "\u{20D7}".to_string(), // vector arrow
        "\u{02D9}" | "\u{0307}" => "\u{0307}".to_string(), // dot
        "\u{00A8}" | "\u{0308}" => "\u{0308}".to_string(), // ddot
        "\u{00AF}" | "\u{0304}" => "\u{0304}".to_string(), // bar / macron
        "\u{02D8}" | "\u{0306}" => "\u{0306}".to_string(), // breve
        "\u{02C7}" | "\u{030C}" => "\u{030C}".to_string(), // caron
        "\u{0301}" => "\u{0301}".to_string(), // acute
        "\u{0300}" => "\u{0300}".to_string(), // grave
        // For unrecognized characters, use as-is (may already be combining)
        other => escape_xml(other),
    }
}

/// Check if an munderover/msubsup node represents a n-ary operator (by
/// inspecting its first child's text).
fn nary_child_is_operator(node: &roxmltree::Node) -> bool {
    let kids: Vec<roxmltree::Node> = node.children().filter(|c| c.is_element()).collect();
    if kids.is_empty() {
        return false;
    }
    let op_text: String = kids[0]
        .descendants()
        .filter(|d| d.is_text())
        .map(|d| d.text().unwrap_or(""))
        .collect();
    is_nary_operator(op_text.trim())
}

/// Check if a character is a known n-ary/large operator that should use <m:nary>.
fn is_nary_operator(op: &str) -> bool {
    matches!(
        op,
        "∫" | "∬" | "∭" | "∮" | "∯" | "∰" | "∱" | "∲" | // integrals
        "∑" | "∏" | "∐" |                                     // sum, product, coproduct
        "⋀" | "⋁" | "⋂" | "⋃" |                               // big wedge/vee/cap/cup
        "⨀" | "⨁" | "⨂" | "⨄" | "⨆"                           // big odot/oplus/otimes/cup/sqcup
    )
}

/// Parse a CSS-like width value (e.g. "2em", "0.2222em") and return an
/// approximate number of space characters to represent that width in OMML.
fn parse_em_width(s: &str) -> usize {
    let num_str = s.trim_end_matches("em").trim();
    let em: f64 = num_str.parse().unwrap_or(0.0);
    if em <= 0.0 {
        0
    } else {
        // 1 em-space (U+2003) ≈ 1em
        em.ceil() as usize
    }
}

/// Fallback: when MathML parsing fails, produce a readable error run
/// instead of escaping XML tags into visible garbage.
fn mathml_to_omml_fallback(mathml: &str) -> String {
    // Extract plain text from the MathML by stripping tags
    let plain: String = mathml
        .replace("<math xmlns=\"http://www.w3.org/1998/Math/MathML\">", "")
        .replace("<math>", "")
        .replace("</math>", "");
    // Strip remaining XML tags, keep only text content
    let mut text = String::new();
    let mut in_tag = false;
    for ch in plain.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => text.push(ch),
            _ => {}
        }
    }
    let content = text.trim();
    if content.is_empty() {
        return "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\"><m:r><m:t>(formula)</m:t></m:r></m:oMath>".to_string();
    }
    format!(
        "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\"><m:r><m:rPr><m:sty m:val=\"p\"/></m:rPr><m:t>{}</m:t></m:r></m:oMath>",
        escape_xml(content)
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

    // ============================================================
    // Helper: formula → MathML → OMML round-trip with assertions
    // ============================================================

    /// Render formula to MathML and assert it contains expected elements.
    fn assert_mathml(formula: &str, display: bool, expected_patterns: &[&str]) {
        let result = render_math_to_mathml(formula, display);
        let mathml =
            result.unwrap_or_else(|e| panic!("render_math_to_mathml({formula:?}) failed: {e}"));
        for pat in expected_patterns {
            assert!(
                mathml.contains(pat),
                "MathML for '{formula}' should contain '{pat}', got:\n{mathml}"
            );
        }
    }

    /// Render formula all the way to OMML and assert it contains expected elements.
    fn assert_omml(formula: &str, display: bool, expected_patterns: &[&str]) {
        let mathml = render_math_to_mathml(formula, display)
            .unwrap_or_else(|e| panic!("render_math_to_mathml({formula:?}) failed: {e}"));
        let omml = mathml_to_omml(&mathml, display);
        for pat in expected_patterns {
            assert!(
                omml.contains(pat),
                "OMML for '{formula}' should contain '{pat}', got:\n{omml}"
            );
        }
    }

    // ============================================================
    // Basic MathML extraction
    // ============================================================

    #[test]
    fn test_mathml_extraction() {
        let result = render_math_to_mathml("x^2", false);
        assert!(result.is_ok(), "render failed: {:?}", result.err());
        let mathml = result.unwrap();
        assert!(mathml.contains("<math"), "should contain math element");
    }

    // ============================================================
    // Superscript / Subscript
    // ============================================================

    #[test]
    fn test_superscript_x2() {
        // Typst outputs Unicode math italic (e.g. 𝑥 = U+1D465) instead of ASCII x
        assert_mathml("x^2", false, &["<msup>", "<mi>", "<mn>2</mn>"]);
        assert_omml("x^2", false, &["m:oMath", "m:sSup"]);
    }

    #[test]
    fn test_superscript_complex() {
        assert_omml("e^(i pi) + 1 = 0", false, &["m:oMath", "m:sSup"]);
    }

    #[test]
    fn test_subscript() {
        assert_omml("x_i", false, &["m:oMath", "m:sSub"]);
    }

    #[test]
    fn test_subscript_complex() {
        assert_omml("a_(n+1)", false, &["m:oMath", "m:sSub"]);
    }

    // ============================================================
    // Fractions
    // ============================================================

    #[test]
    fn test_fraction_simple() {
        assert_mathml("frac(a, b)", true, &["<mfrac>"]);
        assert_omml("frac(a, b)", true, &["m:oMath", "m:f", "m:num", "m:den"]);
    }

    #[test]
    fn test_fraction_nested() {
        assert_omml("frac(frac(1, 2), 3)", true, &["m:oMath", "m:f"]);
    }

    #[test]
    fn test_quadratic_formula() {
        let formula = "x = (-b +- sqrt(b^2 - 4 a c)) / (2 a)";
        assert_omml(formula, true, &["m:oMath", "m:f", "m:rad"]);
    }

    // ============================================================
    // Square roots / radicals
    // ============================================================

    #[test]
    fn test_sqrt_simple() {
        assert_omml("sqrt(x)", false, &["m:oMath", "m:rad"]);
    }

    #[test]
    fn test_sqrt_complex() {
        assert_omml("sqrt(frac(a, b))", true, &["m:oMath", "m:rad", "m:f"]);
    }

    #[test]
    fn test_sqrt_nested() {
        assert_omml("sqrt(1 + sqrt(2))", true, &["m:oMath", "m:rad"]);
    }

    // ============================================================
    // Sum / Product with limits
    // ============================================================

    #[test]
    fn test_sum_with_limits() {
        assert_omml("sum_(i=1)^n i", true, &["m:oMath", "m:nary"]);
    }

    #[test]
    fn test_sum_complex() {
        assert_omml("sum_(i=1)^n i^2", true, &["m:oMath", "m:nary", "m:sSup"]);
    }

    #[test]
    fn test_prod_with_limits() {
        assert_omml("product_(i=1)^n i", true, &["m:oMath", "m:nary"]);
    }

    // ============================================================
    // Integrals
    // ============================================================

    #[test]
    fn test_integral_definite() {
        assert_omml("integral_0^1 f(x) dif x", true, &["m:oMath", "m:nary"]);
    }

    #[test]
    fn test_integral_indefinite() {
        assert_omml("integral f(x) dif x", true, &["m:oMath"]);
    }

    #[test]
    fn test_nested_subsup_not_nary() {
        // r_i^2 should be sSubSup, NOT nary (regression test for nested msubsup)
        assert_omml("(r_i^2 m_i^2)/9", true, &["m:oMath", "m:sSubSup", "m:f"]);
    }

    #[test]
    fn test_sigma_sub_superscript() {
        // sigma_i^2 — Greek letter with sub+superscript becomes msubsup → sSubSup
        assert_omml("sigma_i^2", true, &["m:oMath", "m:sSubSup"]);
    }

    // ============================================================
    // Matrices
    // ============================================================

    #[test]
    fn test_matrix_2x2() {
        let formula = "mat(a, b; c, d)";
        assert_omml(formula, true, &["m:oMath", "m:m", "m:mr", "m:e"]);
    }

    #[test]
    fn test_matrix_3x3() {
        let formula = "mat(1, 2, 3; 4, 5, 6; 7, 8, 9)";
        assert_omml(formula, true, &["m:oMath", "m:m", "m:d"]);
    }

    // ============================================================
    // Accents
    // ============================================================

    #[test]
    fn test_hat_accent() {
        assert_omml("hat(x)", false, &["m:oMath", "m:acc"]);
    }

    // ============================================================
    // Brackets / delimiters
    // ============================================================

    #[test]
    fn test_parens_nested() {
        assert_omml("(a + b)(c + d)", false, &["m:oMath"]);
    }

    #[test]
    fn test_abs_value() {
        assert_omml("abs(x)", false, &["m:oMath"]);
    }

    // ============================================================
    // Greek letters & special symbols
    // ============================================================

    #[test]
    fn test_greek_letters() {
        assert_omml("alpha + beta + gamma", false, &["m:oMath"]);
    }

    #[test]
    fn test_pi_and_infinity() {
        assert_omml("pi = 3.14159", false, &["m:oMath"]);
    }

    // ============================================================
    // Display vs Inline mode
    // ============================================================

    #[test]
    fn test_display_mode_uses_omathpara() {
        let mathml = render_math_to_mathml("x", true).unwrap();
        let omml = mathml_to_omml(&mathml, true);
        assert!(
            omml.contains("m:oMathPara"),
            "Display mode should use oMathPara"
        );
    }

    #[test]
    fn test_inline_mode_no_omathpara() {
        let mathml = render_math_to_mathml("x", false).unwrap();
        let omml = mathml_to_omml(&mathml, false);
        assert!(
            !omml.contains("m:oMathPara"),
            "Inline mode should NOT use oMathPara"
        );
    }

    // ============================================================
    // Edge cases
    // ============================================================

    #[test]
    fn test_empty_formula() {
        // Empty string may succeed (typst renders it as empty) or fail — both are acceptable
        let result = render_math_to_mathml("", false);
        if let Ok(mathml) = result {
            // Should still produce valid OMML without panicking
            let omml = mathml_to_omml(&mathml, false);
            assert!(
                !omml.is_empty() || omml.contains("m:oMath"),
                "Empty OMML should be valid XML"
            );
        }
    }

    #[test]
    fn test_single_number() {
        assert_omml("42", false, &["m:oMath"]);
    }

    #[test]
    fn test_single_variable() {
        assert_omml("x", false, &["m:oMath"]);
    }

    #[test]
    fn test_complex_nested_expression() {
        let formula = "integral_0^oo e^(-x^2) dif x = sqrt(pi) / 2";
        let result = render_math_to_mathml(formula, true);
        assert!(
            result.is_ok(),
            "Complex expression should render: {:?}",
            result.err()
        );
        let mathml = result.unwrap();
        let omml = mathml_to_omml(&mathml, true);
        assert!(
            omml.contains("m:oMath"),
            "Complex expression OMML should contain oMath"
        );
        assert!(
            !omml.is_empty(),
            "OMML should not be empty for complex expression"
        );
    }

    #[test]
    fn test_fallback_on_broken_input() {
        // mathml_to_omml should not panic on invalid XML
        let omml = mathml_to_omml("not valid xml at all <<<>>>", false);
        assert!(!omml.is_empty(), "Fallback should produce non-empty output");
    }
}

