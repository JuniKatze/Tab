import { useState, useEffect, useRef, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";
import MtypEditor from "./components/Editor";
import Preview from "./components/Preview";
import "./App.css";

const DEFAULT_CONTENT = "";

type SaveStatus = "saved" | "saving" | "unsaved";

function App() {
  const [content, setContent] = useState(DEFAULT_CONTENT);
  const [html, setHtml] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [filePath, setFilePath] = useState<string | null>(null);
  const [saveStatus, setSaveStatus] = useState<SaveStatus>("saved");
  const renderTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const saveTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  // Track the last content that was successfully saved
  const savedContentRef = useRef(DEFAULT_CONTENT);
  // Keep a stable reference to current content for the save timer
  const contentRef = useRef(content);
  contentRef.current = content;
  const filePathRef = useRef(filePath);
  filePathRef.current = filePath;

  const doSave = useCallback(async (path: string, text: string) => {
    setSaveStatus("saving");
    try {
      await invoke("save_file", { path, content: text });
      savedContentRef.current = text;
      setSaveStatus("saved");
    } catch (e) {
      setError(String(e));
      setSaveStatus("unsaved");
    }
  }, []);

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

  // Render with debounce
  useEffect(() => {
    if (renderTimerRef.current) {
      clearTimeout(renderTimerRef.current);
    }
    renderTimerRef.current = setTimeout(() => {
      render(content);
    }, 500);
    return () => {
      if (renderTimerRef.current) {
        clearTimeout(renderTimerRef.current);
      }
    };
  }, [content, render]);

  // Auto-save with debounce when filePath is set
  useEffect(() => {
    if (!filePath) {
      // New file without path — mark unsaved if content differs from default
      if (content !== savedContentRef.current) {
        setSaveStatus("unsaved");
      }
      return;
    }
    // Don't auto-save if content hasn't changed from last save
    if (content === savedContentRef.current) {
      setSaveStatus("saved");
      return;
    }

    setSaveStatus("unsaved");

    if (saveTimerRef.current) {
      clearTimeout(saveTimerRef.current);
    }
    saveTimerRef.current = setTimeout(() => {
      const currentContent = contentRef.current;
      const currentPath = filePathRef.current;
      if (currentPath && currentContent !== savedContentRef.current) {
        doSave(currentPath, currentContent);
      }
    }, 300);

    return () => {
      if (saveTimerRef.current) {
        clearTimeout(saveTimerRef.current);
      }
    };
  }, [content, filePath, doSave]);

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
        savedContentRef.current = result.content;
        setSaveStatus("saved");
      } catch (e) {
        setError(String(e));
      }
    }
  };

  const handleSave = async () => {
    // Flush any pending auto-save
    if (saveTimerRef.current) {
      clearTimeout(saveTimerRef.current);
    }

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

    await doSave(targetPath, content);
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

  const saveStatusLabel: Record<SaveStatus, string> = {
    saved: "💾 已保存",
    saving: "⏳ 保存中...",
    unsaved: "⚠ 未保存",
  };

  return (
    <div className="app-container">
      <div className="editor-pane">
        <div className="pane-header">
          <span>{filePath ? filePath : "Editor (.mtyp)"}</span>
          <div className="header-btns">
            <span className={`save-status save-status--${saveStatus}`}>
              {saveStatusLabel[saveStatus]}
            </span>
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
