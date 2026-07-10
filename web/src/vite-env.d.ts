/// <reference types="vite/client" />

declare module "*.wasm" {
  const init: (options?: WebAssembly.Imports) => Promise<WebAssembly.Instance>;
  export default init;
}

declare module "../pkg/tab_engine" {
  export default function init(): Promise<void>;
  export function parse_mtyp(source: string): string;
  export function render_math_wasm(content: string, display: boolean): string;
  export function render_mtyp(source: string): string;
}
