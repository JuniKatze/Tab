//! Tab Engine CLI — convert .mtyp files to HTML or DOCX.
//! Build with: cargo build                (HTML + CLI)
//!             cargo build --features docx (HTML + DOCX with OMML)

use std::fs;
use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("用法: tab-engine <输入.mtyp> [--docx] [--stdout]");
        eprintln!("  tab-engine document.mtyp          输出 HTML 文件");
        eprintln!("  tab-engine document.mtyp --stdout 输出 HTML 到终端");
        eprintln!("  tab-engine document.mtyp --docx   输出 DOCX 文件（可编辑公式）");
        std::process::exit(1);
    }

    let input_path = PathBuf::from(&args[1]);
    let to_docx = args.iter().skip(2).any(|s| s == "--docx");
    let to_stdout = args.iter().skip(2).any(|s| s == "--stdout");

    let source = fs::read_to_string(&input_path).unwrap_or_else(|e| {
        eprintln!("无法读取文件 {}: {}", input_path.display(), e);
        std::process::exit(1);
    });

    if to_docx {
        #[cfg(feature = "docx")]
        {
            let docx_path = input_path.with_extension("docx");
            export_docx(&source, &docx_path);
            eprintln!("已输出: {}", docx_path.display());
        }
        #[cfg(not(feature = "docx"))]
        {
            eprintln!("DOCX 功能未启用。请用以下命令编译：");
            eprintln!("  cargo build --features docx --bin tab-engine --release");
            std::process::exit(1);
        }
    } else {
        let result = tab_engine::render(&source).unwrap_or_else(|e| {
            eprintln!("渲染失败: {}", e);
            std::process::exit(1);
        });
        let title = input_path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("Tabdown Document");
        let html = html_document(title, &result.html);
        if to_stdout {
            println!("{}", html);
        } else {
            let html_path = input_path.with_extension("html");
            fs::write(&html_path, html).unwrap_or_else(|e| {
                eprintln!("无法写入文件 {}: {}", html_path.display(), e);
                std::process::exit(1);
            });
            eprintln!("已输出: {}", html_path.display());
        }
    }
}

fn html_document(title: &str, body: &str) -> String {
    format!(
        "<!doctype html>\n<html lang=\"zh-CN\">\n<head>\n<meta charset=\"utf-8\">\n<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n<title>{}</title>\n</head>\n<body>\n{}\n</body>\n</html>\n",
        escape_html_text(title),
        body
    )
}

fn escape_html_text(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(feature = "docx")]
fn export_docx(source: &str, output_path: &std::path::Path) {
    use docx_rs::*;
    use pulldown_cmark::{CodeBlockKind, Event, Options, Tag, TagEnd};
    use std::collections::HashMap;

    let blocks = tab_engine::parse(source);
    const BODY_FONT: &str = "Noto Serif CJK SC";
    const HEADING_FONT: &str = "Noto Sans CJK SC";
    const CODE_FONT: &str = "Courier New";
    const BODY_SIZE: usize = 24; // 12pt (小四)
    const PAGE_MARGIN_2_5CM: i32 = 1417;
    const WORD_SINGLE_LINE: i32 = 240;
    const BODY_LINE_MULTIPLE: i32 = 360; // Word "Multiple" line spacing, 1.5x (360/240).
    const BODY_AFTER: u32 = 120; // 6pt.
    const LIST_AFTER: u32 = 60; // 3pt.
    const DISPLAY_MATH_BEFORE: u32 = 80; // 4pt.
    const DISPLAY_MATH_AFTER: u32 = 120; // 6pt.
    const BULLET_NUMBERING_ID: usize = 2;

    let mut doc = with_tabdown_numbering(
        Docx::new()
            .page_margin(
                PageMargin::new()
                    .top(PAGE_MARGIN_2_5CM)
                    .right(PAGE_MARGIN_2_5CM)
                    .bottom(PAGE_MARGIN_2_5CM)
                    .left(PAGE_MARGIN_2_5CM),
            )
            .default_size(BODY_SIZE)
            .default_fonts(body_fonts())
            .default_line_spacing(body_spacing()),
    );
    let mut omml_map: HashMap<String, String> = HashMap::new();
    let mut counter = 0u32;

    #[derive(Clone, Debug, PartialEq)]
    enum StyleKind {
        Bold,
        Italic,
        Strike,
    }

    enum ParaPart {
        Run(docx_rs::Run),
        InlineMath(String), // placeholder key
    }

    #[derive(Copy, Clone)]
    struct ListContext {
        ordered: bool,
    }

    #[derive(Copy, Clone)]
    enum ParaKind {
        Body,
        CodeBlock,
        List { ordered: bool, level: usize },
    }

    fn fonts(name: &str) -> docx_rs::RunFonts {
        docx_rs::RunFonts::new()
            .ascii(name)
            .hi_ansi(name)
            .east_asia(name)
            .cs(name)
    }

    fn body_fonts() -> docx_rs::RunFonts {
        fonts(BODY_FONT)
    }

    fn heading_fonts() -> docx_rs::RunFonts {
        fonts(HEADING_FONT)
    }

    fn code_fonts() -> docx_rs::RunFonts {
        fonts(CODE_FONT)
    }

    fn with_tabdown_numbering(doc: docx_rs::Docx) -> docx_rs::Docx {
        doc.add_abstract_numbering(
            docx_rs::AbstractNumbering::new(BULLET_NUMBERING_ID).add_level(
                docx_rs::Level::new(
                    0,
                    docx_rs::Start::new(1),
                    docx_rs::NumberFormat::new("bullet"),
                    docx_rs::LevelText::new("•"),
                    docx_rs::LevelJc::new("left"),
                )
                .indent(
                    Some(420),
                    Some(docx_rs::SpecialIndentType::Hanging(420)),
                    None,
                    None,
                ),
            ),
        )
        .add_numbering(docx_rs::Numbering::new(
            BULLET_NUMBERING_ID,
            BULLET_NUMBERING_ID,
        ))
    }

    fn body_spacing() -> docx_rs::LineSpacing {
        docx_rs::LineSpacing::new()
            .line(BODY_LINE_MULTIPLE)
            .line_rule(docx_rs::LineSpacingType::Auto)
            .after(BODY_AFTER)
    }

    fn list_spacing() -> docx_rs::LineSpacing {
        docx_rs::LineSpacing::new()
            .line(BODY_LINE_MULTIPLE)
            .line_rule(docx_rs::LineSpacingType::Auto)
            .after(LIST_AFTER)
    }

    fn code_block_spacing() -> docx_rs::LineSpacing {
        docx_rs::LineSpacing::new()
            .line(WORD_SINGLE_LINE)
            .line_rule(docx_rs::LineSpacingType::Auto)
            .before(80)
            .after(BODY_AFTER)
    }

    fn display_math_spacing() -> docx_rs::LineSpacing {
        docx_rs::LineSpacing::new()
            .line(WORD_SINGLE_LINE)
            .line_rule(docx_rs::LineSpacingType::Auto)
            .before(DISPLAY_MATH_BEFORE)
            .after(DISPLAY_MATH_AFTER)
    }

    fn heading_spacing(level: usize) -> docx_rs::LineSpacing {
        let (before, after) = match level {
            1 => (240, 160), // 12pt / 8pt
            2 => (220, 120), // 11pt / 6pt
            3 => (180, 100), // 9pt / 5pt
            _ => (160, 80),  // 8pt / 4pt
        };

        docx_rs::LineSpacing::new()
            .line(276)
            .line_rule(docx_rs::LineSpacingType::Auto)
            .before(before)
            .after(after)
    }

    fn heading_size(level: usize) -> usize {
        match level {
            1 => 32,        // 16pt 三号
            2 => 30,        // 15pt 小三
            3 => 28,        // 14pt 四号
            4 => 24,        // 12pt 小四
            _ => 21,        // 10.5pt 五号
        }
    }

    fn apply_styles(run: docx_rs::Run, styles: &[StyleKind]) -> docx_rs::Run {
        let mut run = run;
        for style in styles {
            match style {
                StyleKind::Bold => run = run.bold(),
                StyleKind::Italic => run = run.italic(),
                StyleKind::Strike => run = run.strike(),
            }
        }
        run
    }

    fn body_text_run(text: &str, styles: &[StyleKind]) -> docx_rs::Run {
        apply_styles(
            docx_rs::Run::new()
                .add_text(text)
                .fonts(body_fonts())
                .size(BODY_SIZE),
            styles,
        )
    }

    fn heading_text_run(run: docx_rs::Run, level: usize) -> docx_rs::Run {
        run.fonts(heading_fonts()).size(heading_size(level)).bold()
    }

    fn code_block_base_run() -> docx_rs::Run {
        docx_rs::Run::new().fonts(code_fonts()).size(BODY_SIZE)
    }

    fn code_inline_run(text: &str) -> docx_rs::Run {
        docx_rs::Run::new()
            .add_text(text)
            .fonts(code_fonts())
            .size(BODY_SIZE)
    }

    fn find_placeholder(text: &str) -> Option<(usize, usize)> {
        let start = text.find("OMMLPLACEHOLDER")?;
        let digits_start = start + "OMMLPLACEHOLDER".len();
        let bytes = text.as_bytes();
        let mut end = digits_start;
        while end < bytes.len() && bytes[end].is_ascii_digit() {
            end += 1;
        }
        (end > digits_start).then_some((start, end))
    }

    fn push_text_runs(runs: &mut Vec<ParaPart>, text: &str, styles: &[StyleKind]) {
        let mut rest = text;
        while let Some((start, end)) = find_placeholder(rest) {
            if start > 0 {
                let run = body_text_run(&rest[..start], styles);
                runs.push(ParaPart::Run(run));
            }
            runs.push(ParaPart::InlineMath(rest[start..end].to_string()));
            rest = &rest[end..];
        }
        if !rest.is_empty() {
            let run = body_text_run(rest, styles);
            runs.push(ParaPart::Run(run));
        }
    }

    fn push_code_runs(runs: &mut Vec<ParaPart>, text: &str) {
        for segment in text.split_inclusive('\n') {
            let has_break = segment.ends_with('\n');
            let line = segment.strip_suffix('\n').unwrap_or(segment);
            if !line.is_empty() {
                runs.push(ParaPart::Run(code_block_base_run().add_text(line)));
            }
            if has_break {
                runs.push(ParaPart::Run(
                    code_block_base_run().add_break(docx_rs::BreakType::TextWrapping),
                ));
            }
        }
    }

    fn current_para_kind(list_stack: &[ListContext]) -> ParaKind {
        match list_stack.last() {
            Some(context) => ParaKind::List {
                ordered: context.ordered,
                level: list_stack.len().saturating_sub(1).min(8),
            },
            None => ParaKind::Body,
        }
    }

    fn paragraph_for_kind(kind: ParaKind) -> docx_rs::Paragraph {
        let para = docx_rs::Paragraph::new()
            .fonts(body_fonts())
            .size(BODY_SIZE);

        match kind {
            ParaKind::Body => para.line_spacing(body_spacing()),
            ParaKind::CodeBlock => para.line_spacing(code_block_spacing()).keep_lines(true),
            ParaKind::List { ordered, level } => para.line_spacing(list_spacing()).numbering(
                docx_rs::NumberingId::new(if ordered { 1 } else { BULLET_NUMBERING_ID }),
                docx_rs::IndentLevel::new(level),
            ),
        }
    }

    fn flush_paragraph(
        doc: docx_rs::Docx,
        runs: &mut Vec<ParaPart>,
        kind: ParaKind,
    ) -> docx_rs::Docx {
        if runs.is_empty() {
            return doc;
        }
        let parts: Vec<ParaPart> = runs.drain(..).collect();
        let mut para = paragraph_for_kind(kind);
        for part in parts {
            match part {
                ParaPart::Run(run) => {
                    para = para.add_run(run);
                }
                ParaPart::InlineMath(placeholder) => {
                    para = para.add_run(docx_rs::Run::new().add_text(&placeholder));
                }
            }
        }
        doc.add_paragraph(para)
    }

    fn flush_heading(doc: docx_rs::Docx, runs: &mut Vec<ParaPart>, level: usize) -> docx_rs::Docx {
        let parts: Vec<ParaPart> = runs.drain(..).collect();
        if parts.is_empty() {
            return doc;
        }
        let mut para = docx_rs::Paragraph::new()
            .fonts(heading_fonts())
            .size(heading_size(level))
            .bold()
            .line_spacing(heading_spacing(level))
            .keep_next(true);
        if level == 1 {
            para = para.align(docx_rs::AlignmentType::Center);
        }
        for part in parts {
            match part {
                ParaPart::Run(run) => {
                    para = para.add_run(heading_text_run(run, level));
                }
                ParaPart::InlineMath(placeholder) => {
                    para = para.add_run(docx_rs::Run::new().add_text(&placeholder));
                }
            }
        }
        doc.add_paragraph(para)
    }

    fn code_language(kind: &CodeBlockKind<'_>) -> Option<String> {
        match kind {
            CodeBlockKind::Fenced(info) => info
                .split_whitespace()
                .next()
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string()),
            CodeBlockKind::Indented => None,
        }
    }

    fn render_markdown_to_docx(mut doc: docx_rs::Docx, markdown: &str) -> docx_rs::Docx {
        if markdown.trim().is_empty() {
            return doc;
        }

        let mut options = Options::empty();
        options.insert(Options::ENABLE_STRIKETHROUGH);
        let parser = pulldown_cmark::Parser::new_ext(markdown, options);
        let mut para_runs: Vec<ParaPart> = Vec::new();
        let mut style_stack: Vec<StyleKind> = Vec::new();
        let mut list_stack: Vec<ListContext> = Vec::new();
        let mut in_code_block = false;

        for event in parser {
            match event {
                Event::Start(tag) => match tag {
                    Tag::Paragraph => {}
                    Tag::Heading { .. } => {
                        doc = flush_paragraph(doc, &mut para_runs, current_para_kind(&list_stack));
                    }
                    Tag::Strong => style_stack.push(StyleKind::Bold),
                    Tag::Emphasis => style_stack.push(StyleKind::Italic),
                    Tag::Strikethrough => style_stack.push(StyleKind::Strike),
                    Tag::CodeBlock(kind) => {
                        doc = flush_paragraph(doc, &mut para_runs, current_para_kind(&list_stack));
                        in_code_block = true;
                        if let Some(lang) = code_language(&kind) {
                            para_runs.push(ParaPart::Run(
                                code_block_base_run()
                                    .bold()
                                    .add_text(format!("language: {}", lang)),
                            ));
                            para_runs.push(ParaPart::Run(
                                code_block_base_run().add_break(docx_rs::BreakType::TextWrapping),
                            ));
                        }
                    }
                    Tag::Item => {}
                    Tag::List(start) => {
                        doc = flush_paragraph(doc, &mut para_runs, current_para_kind(&list_stack));
                        list_stack.push(ListContext {
                            ordered: start.is_some(),
                        });
                    }
                    _ => {}
                },
                Event::End(tag_end) => match tag_end {
                    TagEnd::Paragraph => {
                        doc = flush_paragraph(doc, &mut para_runs, current_para_kind(&list_stack));
                    }
                    TagEnd::Heading(level) => {
                        doc = flush_heading(doc, &mut para_runs, level as u8 as usize);
                    }
                    TagEnd::Strong => {
                        style_stack.pop();
                    }
                    TagEnd::Emphasis => {
                        style_stack.pop();
                    }
                    TagEnd::Strikethrough => {
                        style_stack.pop();
                    }
                    TagEnd::CodeBlock => {
                        doc = flush_paragraph(doc, &mut para_runs, ParaKind::CodeBlock);
                        in_code_block = false;
                    }
                    TagEnd::Item => {
                        doc = flush_paragraph(doc, &mut para_runs, current_para_kind(&list_stack));
                    }
                    TagEnd::List(_) => {
                        list_stack.pop();
                    }
                    _ => {}
                },
                Event::Text(text) => {
                    if in_code_block {
                        push_code_runs(&mut para_runs, &text);
                    } else {
                        push_text_runs(&mut para_runs, &text, &style_stack);
                    }
                }
                Event::Code(text) => {
                    para_runs.push(ParaPart::Run(code_inline_run(&text)));
                }
                Event::SoftBreak => {
                    if in_code_block {
                        push_code_runs(&mut para_runs, "\n");
                    } else {
                        para_runs.push(ParaPart::Run(body_text_run(" ", &style_stack)));
                    }
                }
                Event::HardBreak => {
                    para_runs.push(ParaPart::Run(
                        docx_rs::Run::new().add_break(docx_rs::BreakType::TextWrapping),
                    ));
                }
                _ => {}
            }
        }
        flush_paragraph(doc, &mut para_runs, current_para_kind(&list_stack))
    }

    fn math_placeholder(
        counter: &mut u32,
        omml_map: &mut HashMap<String, String>,
        content: &str,
        display: bool,
    ) -> String {
        let placeholder = format!("OMMLPLACEHOLDER{}", counter);
        *counter += 1;
        let omml = tab_engine::render_omml::render_math_to_mathml(content, display)
            .map(|ml| tab_engine::render_omml::mathml_to_omml(&ml, display))
            .unwrap_or_else(|_| {
                if display {
                    format!(
                        "<m:oMathPara xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\"><m:oMath><m:r><m:t>{}</m:t></m:r></m:oMath></m:oMathPara>",
                        escape_xml(content)
                    )
                } else {
                    format!(
                        "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\"><m:r><m:t>{}</m:t></m:r></m:oMath>",
                        escape_xml(content)
                    )
                }
            });
        omml_map.insert(placeholder.clone(), omml);
        placeholder
    }

    let mut markdown_buf = String::new();

    for block in &blocks {
        match block {
            tab_engine::parser::Block::Text(text) => {
                markdown_buf.push_str(text);
            }
            tab_engine::parser::Block::InlineMath(content) => {
                let placeholder = math_placeholder(&mut counter, &mut omml_map, content, false);
                markdown_buf.push_str(&placeholder);
            }
            tab_engine::parser::Block::DisplayMath(content) => {
                doc = render_markdown_to_docx(doc, &markdown_buf);
                markdown_buf.clear();
                let placeholder = math_placeholder(&mut counter, &mut omml_map, content, true);
                doc = doc.add_paragraph(
                    docx_rs::Paragraph::new()
                        .align(docx_rs::AlignmentType::Center)
                        .line_spacing(display_math_spacing())
                        .add_run(docx_rs::Run::new().add_text(&placeholder)),
                );
            }
            tab_engine::parser::Block::Html(html) => {
                doc = render_markdown_to_docx(doc, &markdown_buf);
                markdown_buf.clear();
                doc = doc.add_paragraph(
                    docx_rs::Paragraph::new()
                        .fonts(body_fonts())
                        .size(BODY_SIZE)
                        .line_spacing(body_spacing())
                        .add_run(body_text_run(html, &[])),
                );
            }
        }
    }
    doc = render_markdown_to_docx(doc, &markdown_buf);

    let docx = doc.build();
    let file = fs::File::create(output_path).expect("无法创建 DOCX 文件");
    docx.pack(file).expect("无法打包 DOCX");

    // Post-process: replace placeholders with actual OMML
    inject_omml(output_path, &omml_map);
}

/// Post-process the DOCX to replace OMML placeholders with actual OMML XML.
#[cfg(feature = "docx")]
fn inject_omml(path: &std::path::Path, omml_map: &std::collections::HashMap<String, String>) {
    use std::collections::HashMap;
    use std::io::{Cursor, Read, Write};

    let data = fs::read(path).expect("无法读取 DOCX 进行后处理");
    let cursor = Cursor::new(data);
    let mut archive = zip::ZipArchive::new(cursor).expect("无法打开 DOCX ZIP");

    let mut entries: HashMap<String, Vec<u8>> = HashMap::new();

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).expect("无法读取 ZIP 条目");
        let name = entry.name().to_string();
        let mut buf = Vec::new();
        entry.read_to_end(&mut buf).expect("无法读取条目内容");

        if name == "word/document.xml" {
            let mut xml = String::from_utf8(buf).expect("无效的 UTF-8");
            for (placeholder, omml) in omml_map {
                // Use regex for robust matching regardless of docx-rs XML formatting.
                // Matches <w:r> containing <w:rPr/> followed by <w:t> with the placeholder.
                let pattern = format!(
                    r"<w:r[^>]*>\s*<w:rPr[^>]*/?>\s*<w:t[^>]*>{}</w:t>\s*</w:r>",
                    regex::escape(placeholder)
                );
                let re = regex::Regex::new(&pattern)
                    .unwrap_or_else(|e| panic!("无效的正则表达式: {e}"));
                let count = re.find_iter(&xml).count();
                if count == 0 {
                    eprintln!(
                        "警告: 未找到占位符 '{}'，OMML 注入失败（docx-rs 格式可能已变更）",
                        placeholder
                    );
                }
                xml = re.replace_all(&xml, omml.as_str()).into_owned();
            }
            entries.insert(name, xml.into_bytes());
        } else {
            entries.insert(name, buf);
        }
    }

    let out_file = fs::File::create(path).expect("无法创建输出文件");
    let mut zip_writer = zip::ZipWriter::new(out_file);
    let options =
        zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    for (name, data) in &entries {
        zip_writer
            .start_file(name, options)
            .expect("无法创建 ZIP 条目");
        zip_writer.write_all(data).expect("无法写入 ZIP 条目");
    }
    zip_writer.finish().expect("无法完成 ZIP");
}

#[cfg(feature = "docx")]
fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
