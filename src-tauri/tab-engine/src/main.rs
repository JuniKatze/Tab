//! Tab CLI — convert .mtyp files to HTML or DOCX.
//! Build with: cargo build --features cli     (HTML only)
//!             cargo build --features docx    (HTML + DOCX)

use std::fs;
use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("用法: tab <输入.mtyp> [--docx]");
        eprintln!("  tab document.mtyp         输出 HTML 到终端");
        eprintln!("  tab document.mtyp --docx   输出 DOCX 文件到同目录");
        std::process::exit(1);
    }

    let input_path = PathBuf::from(&args[1]);
    let to_docx = args.get(2).map(|s| s.as_str()) == Some("--docx");

    let source = fs::read_to_string(&input_path)
        .unwrap_or_else(|e| {
            eprintln!("无法读取文件 {}: {}", input_path.display(), e);
            std::process::exit(1);
        });

    let result = tab_engine::render(&source)
        .unwrap_or_else(|e| {
            eprintln!("渲染失败: {}", e);
            std::process::exit(1);
        });

    if to_docx {
        #[cfg(feature = "docx")]
        {
            let docx_path = input_path.with_extension("docx");
            export_docx(&result.html, &docx_path);
            eprintln!("已输出: {}", docx_path.display());
        }
        #[cfg(not(feature = "docx"))]
        {
            eprintln!("DOCX 功能未启用。请用以下命令编译：");
            eprintln!("  cargo build --features docx --bin tab --release");
            std::process::exit(1);
        }
    } else {
        println!("{}", result.html);
    }
}

#[cfg(feature = "docx")]
fn export_docx(html: &str, output_path: &std::path::Path) {
    use docx_rs::*;

    let mut doc = Docx::new();
    let parts = split_html(html);

    for part in &parts {
        if part.starts_with("<svg") {
            // Convert SVG to PNG and embed
            let svg_data = part.as_bytes();
            match svg_to_png(svg_data) {
                Ok(png_data) => {
                    let img = Pic::new(&png_data)
                        .size(200, 50)
                        .floating();
                    doc = doc.add_paragraph(Paragraph::new().add_run(Run::new().add_image(img)));
                }
                Err(_) => {
                    // Fallback: insert alt text
                    doc = doc.add_paragraph(
                        Paragraph::new()
                            .add_run(Run::new().add_text("[数学公式]"))
                    );
                }
            }
        } else if !part.trim().is_empty() {
            let text = strip_tags(part);
            for line in text.lines() {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    doc = doc.add_paragraph(Paragraph::new());
                } else {
                    doc = doc.add_paragraph(
                        Paragraph::new().add_run(Run::new().add_text(trimmed))
                    );
                }
            }
        }
    }

    let docx = doc.build();
    let file = fs::File::create(output_path).expect("无法创建 DOCX 文件");
    docx.pack(file).expect("无法打包 DOCX");
}

/// Split HTML into text and SVG blocks.
#[cfg(feature = "docx")]
fn split_html(html: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut in_svg = false;
    let mut i = 0;
    let chars: Vec<char> = html.chars().collect();

    while i < chars.len() {
        if !in_svg && chars[i] == '<' {
            let rest: String = chars[i..].iter().take(100).collect();
            if rest.starts_with("<svg") || rest.starts_with("<span class=\"math-inline\"><svg")
                || rest.starts_with("<div class=\"math-display\"><svg")
            {
                if !current.trim().is_empty() {
                    parts.push(std::mem::take(&mut current));
                }
                in_svg = true;
            }
        }
        if in_svg {
            current.push(chars[i]);
            if chars[i] == '>' && current.ends_with("</svg>") {
                parts.push(std::mem::take(&mut current));
                in_svg = false;
            }
        } else {
            current.push(chars[i]);
        }
        i += 1;
    }
    if !current.is_empty() {
        parts.push(current);
    }
    parts
}

#[cfg(feature = "docx")]
fn svg_to_png(svg_data: &[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let tree = usvg::Tree::from_data(svg_data, &usvg::Options::default())?;
    let size = tree.size().to_int_size();
    let mut pixmap = resvg::tiny_skia::Pixmap::new(size.width(), size.height())
        .ok_or("无法创建 pixmap")?;
    resvg::render(&tree, resvg::tiny_skia::Transform::default(), &mut pixmap.as_mut());
    Ok(pixmap.encode_png()?)
}

#[cfg(feature = "docx")]
fn strip_tags(html: &str) -> String {
    let mut text = String::new();
    let mut in_tag = false;
    for ch in html.chars() {
        if ch == '<' {
            in_tag = true;
        } else if ch == '>' {
            in_tag = false;
        } else if !in_tag {
            text.push(ch);
        }
    }
    text
}
