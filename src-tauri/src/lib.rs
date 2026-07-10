use serde::Serialize;

#[derive(Serialize)]
struct RenderResponse {
    html: String,
    math_count: usize,
}

#[tauri::command]
fn render_mtyp(source: &str) -> Result<RenderResponse, String> {
    let result = tab_engine::render(source)?;
    Ok(RenderResponse {
        html: result.html,
        math_count: result.math_count,
    })
}

#[tauri::command]
fn render_math(formula: &str, display: bool) -> Result<String, String> {
    tab_engine::render_math(formula, display)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![render_mtyp, render_math])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
