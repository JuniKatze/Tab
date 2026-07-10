import { useState, useEffect, useRef, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import MtypEditor from "./components/Editor";
import Preview from "./components/Preview";
import "./App.css";

const DEFAULT_CONTENT = `# Typst-Markdown Demo

This is a **Typst-Markdown** document (.mtyp).

## Inline Math

The quadratic formula is $x = (-b +- sqrt(b^2 - 4 a c)) / (2 a)$.

## Display Math

$$ sum_(i=1)^n i = (n(n+1)) / 2 $$

$$ integral_0^oo e^(-x^2) dif x = sqrt(pi) / 2 $$

## Matrix

$$ mat(a, b; c, d) $$

## Text

You can write normal Markdown text here, with **bold**, *italic*, and
\`code\`. The math parts use Typst syntax instead of LaTeX!
`;

function App() {
  const [content, setContent] = useState(DEFAULT_CONTENT);
  const [html, setHtml] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
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
    // Debounce rendering by 500ms
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

  return (
    <div className="app-container">
      <div className="editor-pane">
        <div className="pane-header">Editor (.mtyp)</div>
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
