# Tab — Typst 数学公式 Markdown 编辑器

[![Rust](https://img.shields.io/badge/引擎-Rust-orange)](engine)
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
- **实时预览** — 编辑即渲染，实时自动保存
- **三端运行** — Tauri 桌面应用 + WASM 网页版 + CLI 命令行
- **双格式输出** — HTML（内嵌 SVG 公式）+ DOCX（可编辑 OMML 公式）
- **独立引擎** — `tab-engine` Rust crate，可在任何项目中使用
- **中文支持** — CJK 字体内嵌，数学公式中的中文文本正常渲染

## 项目结构

```
Tab/
├── Cargo.toml                         # Cargo workspace 根
├── desktop/                           # 桌面应用
│   ├── src/                           #   React + CodeMirror 6 前端
│   │   ├── App.tsx                    #     桌面应用主组件
│   │   └── components/
│   │       ├── Editor.tsx             #     CodeMirror 6 编辑器（Emacs 键位）
│   │       └── Preview.tsx            #     实时 HTML 预览
│   ├── src-tauri/                     #   Tauri 桌面外壳
│   │   ├── src/
│   │   │   ├── main.rs               #     应用入口
│   │   │   └── lib.rs                #     Tauri 命令
│   │   └── Cargo.toml                #     依赖 engine/
│   ├── index.html
│   ├── package.json
│   └── start.sh                     #   一键启动脚本
├── engine/                           # ★ 独立渲染引擎
│   ├── src/
│   │   ├── parser.rs                 #   .mtyp 解析器
│   │   ├── renderer.rs               #   Typst 公式 → SVG
│   │   ├── render_omml.rs            #   MathML → OMML（DOCX）
│   │   ├── lib.rs                    #   公共 API + Markdown 渲染
│   │   ├── main.rs                   #   CLI 工具 + DOCX 导出
│   │   ├── wasm.rs                   #   WASM 绑定
│   │   └── style.css                 #   输出 HTML 默认样式表
│   ├── fonts/
│   ├── docx-tests/
│   ├── tests/
│   └── Cargo.toml
├── web/                              # WASM 网页版
│   ├── src/
│   ├── pkg/
│   └── build-wasm.sh
├── DESIGN.md
└── README.md
```

### tab-engine — API

```rust
use tab_engine;

// 解析 .mtyp 为块序列（Text, InlineMath, DisplayMath）
let blocks = tab_engine::parse("公式 $x^2$ 示例");

// 渲染单个数学公式为 SVG
let svg = tab_engine::render_math("x^2", false)?;

// 渲染单个数学公式为 OMML（DOCX 可编辑公式）
use tab_engine::render_omml;
let mathml = render_omml::render_math_to_mathml("x^2", false)?;
let omml = render_omml::mathml_to_omml(&mathml, false);

// 完整渲染：解析 + typst 公式 → SVG + Markdown → HTML
let result = tab_engine::render(source)?;
// result.html        → 完整 HTML，含内联 SVG
// result.math_count  → 公式数量
```

## 快速开始

### 桌面应用 (Tauri)

```bash
./desktop/start.sh
```

或手动：

```bash
cd desktop && npm run build
cd dist && python3 -m http.server 1420 -b 127.0.0.1 &
cd ../src-tauri && cargo run --release
```

### 网页版 (WASM)

```bash
cd web && bash build-wasm.sh && npm run dev
# 打开 http://127.0.0.1:5173
```

### CLI — 编译 .mtyp 文件

```bash
# HTML 输出
cargo run --release -- document.mtyp

# DOCX 输出（可编辑数学公式）
cargo run --features docx --release -- document.mtyp --docx
```

### 运行测试

```bash
cd engine && cargo test
# 48 个单元测试 + 3 个集成测试 = 51 tests
```

## 技术栈

| 层级 | 技术 |
|------|------|
| 桌面框架 | Tauri v2 + WebKit |
| 前端 | React 19, TypeScript, CodeMirror 6, Vite |
| 公式渲染 | [typst](https://github.com/typst/typst) 0.15, typst-svg, typst-html |
| Markdown | pulldown-cmark |
| DOCX 生成 | docx-rs + OMML（MathML → OMML 转换器） |
| 字体 | New Computer Modern + Noto Serif CJK SC（子集化） |
| WASM | wasm-bindgen, wasm32-unknown-unknown |
| XML 解析 | roxmltree, regex |

## 许可

MIT
