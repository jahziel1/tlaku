/// <reference types="vite/client" />

interface ImportMetaEnv {
  /** `mock` (por defecto) o `tauri`. */
  readonly VITE_API?: 'mock' | 'tauri';
  /** `off` evita cargar el diseño de ejemplo al arrancar en modo mock. */
  readonly VITE_DEMO?: 'on' | 'off';
}
interface ImportMeta {
  readonly env: ImportMetaEnv;
}
