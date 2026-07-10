import { useState, useEffect, useRef, useCallback } from "react";
import init, { render_mtyp } from "../pkg/tab_engine";
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

interface RenderResponse {
  html: string;
  math_count: number;
}

function App() {
  const [content, setContent] = useState(DEFAULT_CONTENT);
  const [html, setHtml] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [wasmReady, setWasmReady] = useState(false);
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Load WASM
  useEffect(() => {
    init()
      .then(() => {
        setWasmReady(true);
        setLoading(false);
      })
      .catch((e) => {
        setError(`Failed to load WASM: ${e}`);
        setLoading(false);
      });
  }, []);

  const render = useCallback(async (source: string) => {
    if (!wasmReady) return;
    setLoading(true);
    setError(null);
    try {
      const json = render_mtyp(source);
      const result: RenderResponse = JSON.parse(json);
      setHtml(result.html);
    } catch (e) {
      setError(String(e));
      setHtml("");
    } finally {
      setLoading(false);
    }
  }, [wasmReady]);

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

  if (!wasmReady && !error) {
    return (
      <div style={{
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        height: "100vh",
        background: "#1e1e1e",
        color: "#d4d4d4",
        fontFamily: "sans-serif",
        fontSize: "18px",
      }}>
        <div style={{ textAlign: "center" }}>
          <h2>Loading Tab Engine...</h2>
          <p style={{ color: "#888", fontSize: "14px" }}>
            Downloading and initializing WebAssembly (~32MB)
          </p>
          <div style={{
            width: "300px",
            height: "4px",
            background: "#333",
            borderRadius: "2px",
            margin: "16px auto",
            overflow: "hidden",
          }}>
            <div style={{
              width: "100%",
              height: "100%",
              background: "#4fc3f7",
              animation: "loading-bar 2s ease-in-out infinite",
            }} />
          </div>
        </div>
      </div>
    );
  }

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
