# Tab — Typst 数学公式 Markdown 编辑器

[![Rust](https://img.shields.io/badge/引擎-Rust-orange)](src-tauri/tab-engine)
[![Tauri](https://img.shields.io/badge/桌面-Tauri-blue)](https://tauri.app)
[![WASM](https://img.shields.io/badge/网页-WASM-purple)](web)

用 **Typst 语法**写数学公式的 Markdown 编辑器。告别 LaTeX，公式更简洁。

```
行内公式：$x = (-b +- sqrt(b^2 - 4 a c)) / (2 a)$          ← 紧凑排版

行间公式：$$ sum_(i=1)^n i = (n(n+1)) / 2 $$                ← 展开排版

矩阵：    $$ mat(a, b; c, d) $$                              ← 矩阵渲染
```

## 特性

- **Typst 数学语法** — 比 LaTeX 更直观，同样强大
- **实时预览** — 编辑即渲染，500ms 防抖
- **双端运行** — Tauri 桌面应用 + WASM 网页版
- **独立引擎** — `tab-engine` Rust crate，可在任何项目中使用

## 项目结构

```
Tab/
├── src/                              # React + CodeMirror 6 前端
├── src-tauri/
│   ├── tab-engine/                   # ★ 独立渲染引擎
│   │   ├── src/parser.rs             #   .mtyp 解析器（自定义状态机）
│   │   ├── src/renderer.rs           #   Typst 公式 → SVG（基于 typst-rs 0.15）
│   │   ├── src/lib.rs                #   公共 API + Markdown 渲染（pulldown-cmark）
│   │   ├── src/wasm.rs               #   WASM 绑定
│   │   ├── fonts/                    #   内嵌 NewCM 数学字体
│   │   └── tests/                    #   集成测试
│   └── src/lib.rs                    # Tauri 命令
├── web/                              # 独立的 WASM 网页版
│   ├── pkg/                          # 生成的 WASM + JS 绑定
│   └── build-wasm.sh                 # WASM 构建脚本
└── start.sh                          # 桌面应用一键启动
```

### tab-engine — API

```rust
use tab_engine;

// 解析 .mtyp 为块序列（Text, InlineMath, DisplayMath）
let blocks = tab_engine::parse("公式 $x^2$ 示例");

// 渲染单个数学公式为 SVG
let svg = tab_engine::render_math("x^2", false)?;

// 完整渲染：解析 + typst 公式 → SVG + Markdown → HTML
let result = tab_engine::render(source)?;
// result.html        → 完整 HTML，含内联 SVG
// result.math_count  → 公式数量
```

## 快速开始

### 桌面应用 (Tauri)

```bash
./start.sh
```

或手动：

```bash
npm run build
cd dist && python3 -m http.server 1420 -b 127.0.0.1 &
cd ../src-tauri && cargo run --release
```

### 网页版 (WASM)

```bash
cd web && bash build-wasm.sh && npm run dev
# 打开 http://127.0.0.1:5173
```

### 运行测试

```bash
cd src-tauri/tab-engine && cargo test
# 14 个测试：解析器、渲染器、Markdown、行内/行间区分
```

## 技术栈

| 层级 | 技术 |
|------|------|
| 桌面框架 | Tauri v2 + WebKit |
| 前端 | React 19, TypeScript, CodeMirror 6, Vite |
| 公式渲染 | [typst](https://github.com/typst/typst) 0.15, typst-svg |
| Markdown | pulldown-cmark |
| 字体 | New Computer Modern（内嵌） |
| WASM | wasm-bindgen, wasm32-unknown-unknown |

## 许可

MIT
