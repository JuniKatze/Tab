use serde::Serialize;
use std::fs;

#[derive(Serialize)]
struct RenderResponse {
    html: String,
    math_count: usize,
}

#[derive(Serialize)]
struct FileContent {
    content: String,
    path: String,
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

#[tauri::command]
fn read_file(path: &str) -> Result<FileContent, String> {
    let content = fs::read_to_string(path)
        .map_err(|e| format!("无法读取文件: {}", e))?;
    Ok(FileContent {
        content,
        path: path.to_string(),
    })
}

#[tauri::command]
fn save_file(path: &str, content: &str) -> Result<(), String> {
    fs::write(path, content)
        .map_err(|e| format!("无法保存文件: {}", e))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            render_mtyp,
            render_math,
            read_file,
            save_file,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
