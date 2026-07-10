//! Tab CLI — convert .mtyp files to HTML or DOCX.
//! Build with: cargo build                (HTML + CLI)
//!             cargo build --features docx (HTML + DOCX with OMML)

use std::fs;
use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("用法: tab <输入.mtyp> [--docx]");
        eprintln!("  tab document.mtyp         输出 HTML 到终端");
        eprintln!("  tab document.mtyp --docx   输出 DOCX 文件（可编辑公式）");
        std::process::exit(1);
    }

    let input_path = PathBuf::from(&args[1]);
    let to_docx = args.get(2).map(|s| s.as_str()) == Some("--docx");

    let source = fs::read_to_string(&input_path)
        .unwrap_or_else(|e| {
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
            eprintln!("  cargo build --features docx --bin tab --release");
            std::process::exit(1);
        }
    } else {
        let result = tab_engine::render(&source)
            .unwrap_or_else(|e| {
                eprintln!("渲染失败: {}", e);
                std::process::exit(1);
            });
        println!("{}", result.html);
    }
}

#[cfg(feature = "docx")]
fn export_docx(source: &str, output_path: &std::path::Path) {
    use std::collections::HashMap;

    let blocks = tab_engine::parse(source);
    let mut doc = docx_rs::Docx::new();
    let mut omml_map: HashMap<String, String> = HashMap::new();
    let mut counter = 0u32;

    for block in &blocks {
        match block {
            tab_engine::parser::Block::Text(text) => {
                for line in text.lines() {
                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        doc = doc.add_paragraph(docx_rs::Paragraph::new());
                    } else {
                        doc = doc.add_paragraph(
                            docx_rs::Paragraph::new()
                                .add_run(docx_rs::Run::new().add_text(trimmed))
                        );
                    }
                }
            }
            tab_engine::parser::Block::InlineMath(content) => {
                let placeholder = format!("OMMLPLACEHOLDER{}", counter);
                counter += 1;
                match tab_engine::render_omml::render_math_to_mathml(content, false) {
                    Ok(mathml) => {
                        let omml = tab_engine::render_omml::mathml_to_omml(&mathml, false);
                        omml_map.insert(placeholder.clone(), omml);
                    }
                    Err(_) => {
                        omml_map.insert(placeholder.clone(), format!(
                            "<m:oMathPara xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\"><m:oMath><m:r><m:t>{}</m:t></m:r></m:oMath></m:oMathPara>",
                            escape_xml(content)
                        ));
                    }
                }
                doc = doc.add_paragraph(
                    docx_rs::Paragraph::new()
                        .add_run(docx_rs::Run::new().add_text(&placeholder))
                );
            }
            tab_engine::parser::Block::DisplayMath(content) => {
                let placeholder = format!("OMMLPLACEHOLDER{}", counter);
                counter += 1;
                match tab_engine::render_omml::render_math_to_mathml(content, true) {
                    Ok(mathml) => {
                        let omml = tab_engine::render_omml::mathml_to_omml(&mathml, true);
                        omml_map.insert(placeholder.clone(), omml);
                    }
                    Err(_) => {
                        omml_map.insert(placeholder.clone(), format!(
                            "<m:oMathPara xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\"><m:oMath><m:r><m:t>{}</m:t></m:r></m:oMath></m:oMathPara>",
                            escape_xml(content)
                        ));
                    }
                }
                doc = doc.add_paragraph(
                    docx_rs::Paragraph::new()
                        .align(docx_rs::AlignmentType::Center)
                        .add_run(docx_rs::Run::new().add_text(&placeholder))
                );
            }
            tab_engine::parser::Block::Html(html) => {
                doc = doc.add_paragraph(
                    docx_rs::Paragraph::new()
                        .add_run(docx_rs::Run::new().add_text(html))
                );
            }
        }
    }

    let docx = doc.build();
    let file = fs::File::create(output_path).expect("无法创建 DOCX 文件");
    docx.pack(file).expect("无法打包 DOCX");

    // Post-process: replace placeholders with actual OMML
    inject_omml(output_path, &omml_map);
}

/// Post-process the DOCX to replace OMML placeholders with actual OMML XML.
#[cfg(feature = "docx")]
fn inject_omml(path: &std::path::Path, omml_map: &std::collections::HashMap<String, String>) {
    use std::io::{Cursor, Read, Write};
    use std::collections::HashMap;

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
                // Replace placeholder text run with OMML paragraph
                // docx-rs generates <w:rPr /> (self-closing) not <w:rPr></w:rPr>
                let old = format!("<w:r><w:rPr /><w:t xml:space=\"preserve\">{}</w:t></w:r>", placeholder);
                xml = xml.replace(&old, omml);
            }
            entries.insert(name, xml.into_bytes());
        } else {
            entries.insert(name, buf);
        }
    }

    // Write new DOCX
    let out_file = fs::File::create(path).expect("无法创建输出文件");
    let mut zip_writer = zip::ZipWriter::new(out_file);
    let options = zip::write::FileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    for (name, data) in &entries {
        zip_writer.start_file(name, options).expect("无法创建 ZIP 条目");
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
