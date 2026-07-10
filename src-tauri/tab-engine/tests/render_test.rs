use tab_engine;

#[test]
fn test_full_render() {
    let source = r#"# Hello

This is $x^2 + y^2 = z^2$ inline math.

Display math:

$$ sum_(i=1)^n i = (n(n+1)) / 2 $$

**Bold text** and *italic*.
"#;

    let result = tab_engine::render(source);
    match result {
        Ok(r) => {
            println!("=== math_count: {} ===", r.math_count);
            println!("=== HTML output ===");
            println!("{}", r.html);
            assert!(r.math_count >= 2, "Expected at least 2 math blocks, got {}", r.math_count);
            assert!(r.html.contains("<svg"), "HTML should contain SVG");
            assert!(r.html.contains("<strong>"), "HTML should contain bold");
        }
        Err(e) => {
            panic!("Render failed: {}", e);
        }
    }
}

#[test]
fn test_inline_vs_display_differs() {
    let inline = tab_engine::render_math("frac(a, b)", false).unwrap();
    let display = tab_engine::render_math("frac(a, b)", true).unwrap();

    println!("=== INLINE ===");
    println!("{}", &inline[..inline.len().min(300)]);
    println!("=== DISPLAY ===");
    println!("{}", &display[..display.len().min(300)]);

    // Extract viewBox dimensions
    fn get_viewbox(svg: &str) -> (f64, f64) {
        let start = svg.find("viewBox=\"0 0 ").unwrap() + "viewBox=\"0 0 ".len();
        let end = svg[start..].find('"').unwrap();
        let dims: Vec<f64> = svg[start..start + end]
            .split_whitespace()
            .filter_map(|s| s.parse().ok())
            .collect();
        (dims[0], dims[1])
    }

    let (_iw, ih) = get_viewbox(&inline);
    let (_dw, dh) = get_viewbox(&display);
    println!("\nInline  height: {:.2}", ih);
    println!("Display height: {:.2}", dh);

    // Display should be taller
    assert!(dh > ih, "Display should be taller: dh={}, ih={}", dh, ih);
    println!("PASS: display ({}pt) > inline ({}pt)", dh, ih);
}

#[test]
fn test_inline_inside_paragraph() {
    let result = tab_engine::render(
        "The equation $x^2$ is important.\n\n$$ sum_(i=1)^n i $$"
    ).unwrap();

    // Inline math must NOT be between </p> and next tag
    let bad_pattern = "</p><span class=\"math-inline\"";
    assert!(
        !result.html.contains(bad_pattern),
        "Inline math should be inside <p>, not after </p>"
    );

    // But it should contain math-inline inside a <p> context
    assert!(result.html.contains("<p>"), "Should have paragraph tags");
    assert!(result.html.contains("math-inline"), "Should have inline math");
    assert!(result.html.contains("math-display"), "Should have display math");

    println!("PASS: inline math is inside paragraph");
}
