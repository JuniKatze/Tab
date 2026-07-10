# Tab — Typst-Markdown Editor

[![Rust](https://img.shields.io/badge/engine-rust-orange)](src-tauri/tab-engine)
[![Tauri](https://img.shields.io/badge/desktop-tauri-blue)](https://tauri.app)
[![WASM](https://img.shields.io/badge/web-wasm-purple)](web)

Write Markdown with **Typst-powered math formulas**. No LaTeX — just clean Typst syntax.

```
$x = (-b +- sqrt(b^2 - 4 a c)) / (2 a)$          ← inline (compact)

$$ sum_(i=1)^n i = (n(n+1)) / 2 $$                ← display (expanded)

$$ mat(a, b; c, d) $$                              ← matrix
```

## Features

- **Typst math, not LaTeX** — more readable syntax, same power
- **Live preview** — 500ms debounce, see results as you type
- **Dual output** — Tauri desktop app + WASM website
- **Standalone engine** — `tab-engine` Rust crate usable in any project

## Architecture

```
Tab/
├── src/                              # React + CodeMirror 6 frontend
├── src-tauri/
│   ├── tab-engine/                   # ★ Standalone rendering engine
│   │   ├── src/parser.rs             #   .mtyp parser (custom state machine)
│   │   ├── src/renderer.rs           #   Typst → SVG (typst-rs 0.15)
│   │   ├── src/lib.rs                #   Public API + Markdown (pulldown-cmark)
│   │   ├── src/wasm.rs               #   WASM bindings
│   │   ├── fonts/                    #   Embedded NewCM fonts
│   │   └── tests/                    #   Integration tests
│   └── src/lib.rs                    # Tauri commands
├── web/                              # Standalone WASM website
│   ├── pkg/                          # Generated WASM + JS bindings
│   └── build-wasm.sh                 # One-shot WASM build script
└── start.sh                          # Desktop app launcher
```

### tab-engine — API

```rust
use tab_engine;

// Parse .mtyp into blocks (Text, InlineMath, DisplayMath)
let blocks = tab_engine::parse("The formula $x^2$ is quadratic.");

// Render a single math formula to SVG
let svg = tab_engine::render_math("x^2", false)?;

// Full render: parse + typst math → SVG + Markdown → HTML
let result = tab_engine::render(source)?;
// result.html  → complete HTML with inline SVGs
// result.math_count → number of formulas rendered
```

## Quick Start

### Desktop (Tauri)

```bash
./start.sh
```

### Web (WASM)

```bash
cd web && bash build-wasm.sh && npm run dev
# Opens http://127.0.0.1:5173
```

### Tests

```bash
cd src-tauri/tab-engine && cargo test
# 14 tests: parser, renderer, Markdown, inline-vs-display
```

## Tech Stack

| Layer | Technology |
|-------|-----------|
| Desktop | Tauri v2 + WebKit |
| Frontend | React 19, TypeScript, CodeMirror 6, Vite |
| Math rendering | [typst](https://github.com/typst/typst) 0.15, typst-svg |
| Markdown | pulldown-cmark |
| Fonts | New Computer Modern (embedded) |
| WASM | wasm-bindgen, wasm32-unknown-unknown |

## License

MIT
