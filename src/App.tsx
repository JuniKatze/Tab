import { useState, useEffect, useRef, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";
import MtypEditor from "./components/Editor";
import Preview from "./components/Preview";
import "./App.css";

const DEFAULT_CONTENT = "";

function App() {
  const [content, setContent] = useState(DEFAULT_CONTENT);
  const [html, setHtml] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [filePath, setFilePath] = useState<string | null>(null);
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const render = useCallback(async (source: string) => {
    setLoading(true);
    setError(null);
    try {
      const result = await invoke<{ html: string; math_count: number }>(
        "render_mtyp",
        { source }
      );
      setHtml(result.html);
    } catch (e) {
      setError(String(e));
      setHtml("");
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    if (timerRef.current) {
      clearTimeout(timerRef.current);
    }
    timerRef.current = setTimeout(() => {
      render(content);
    }, 500);
    return () => {
      if (timerRef.current) {
        clearTimeout(timerRef.current);
      }
    };
  }, [content, render]);

  const handleOpenFile = async () => {
    const selected = await open({
      filters: [
        {
          name: "Typst-Markdown",
          extensions: ["mtyp", "md", "typ"],
        },
      ],
      multiple: false,
    });

    if (selected) {
      try {
        const result = await invoke<{ content: string; path: string }>(
          "read_file",
          { path: selected }
        );
        setContent(result.content);
        setFilePath(result.path);
      } catch (e) {
        setError(String(e));
      }
    }
  };

  const handleSave = async () => {
    let targetPath = filePath;
    if (!targetPath) {
      // New file — ask where to save
      targetPath = await save({
        filters: [
          {
            name: "Typst-Markdown",
            extensions: ["mtyp"],
          },
        ],
      });
      if (!targetPath) return;
      setFilePath(targetPath);
    }

    try {
      await invoke("save_file", { path: targetPath, content });
    } catch (e) {
      setError(String(e));
    }
  };

  // Ctrl+S / Cmd+S shortcut
  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key === "s") {
        e.preventDefault();
        handleSave();
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [filePath, content]);

  return (
    <div className="app-container">
      <div className="editor-pane">
        <div className="pane-header">
          <span>{filePath ? filePath : "Editor (.mtyp)"}</span>
          <div className="header-btns">
            <button className="open-btn" onClick={handleOpenFile}>
              打开
            </button>
            <button className="open-btn" onClick={handleSave}>
              保存
            </button>
          </div>
        </div>
        <MtypEditor value={content} onChange={setContent} />
      </div>
      <div className="preview-pane">
        <div className="pane-header">Preview</div>
        <Preview html={html} error={error} loading={loading} />
      </div>
    </div>
  );
}

export default App;
