//! WASM bindings for the tab-engine.
//! Compiled only when the `wasm` feature is enabled.

use wasm_bindgen::prelude::*;

/// Parse a .mtyp source into blocks. Returns JSON.
#[wasm_bindgen]
pub fn parse_mtyp(source: &str) -> String {
    let blocks: Vec<crate::parser::Block> = crate::parse(source);
    serde_json::to_string(&blocks).unwrap_or_default()
}

/// Render a single math formula to SVG.
#[wasm_bindgen]
pub fn render_math_wasm(content: &str, display: bool) -> Result<String, JsValue> {
    crate::renderer::render_math(content, display).map_err(|e| JsValue::from_str(&e))
}

/// Render a complete .mtyp document. Returns JSON with html and math_count.
#[wasm_bindgen]
pub fn render_mtyp(source: &str) -> Result<String, JsValue> {
    let result = crate::render(source).map_err(|e| JsValue::from_str(&e))?;
    serde_json::to_string(&result).map_err(|e| JsValue::from_str(&e.to_string()))
}
