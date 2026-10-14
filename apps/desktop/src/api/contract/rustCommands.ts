/**
 * Lector de texto de los comandos de Tauri de `src-tauri/src` (sin `syn`).
 *
 * Solo sirve a la prueba de contrato (`../tauri.contract.test.ts`): saca de `lib.rs` los comandos
 * registrados en `generate_handler![...]`, ubica la función de cada uno y calcula los nombres de
 * argumento que Tauri espera en `invoke`. Si una firma no se reconoce, lanza un error: nunca
 * adivina, porque una firma mal leída escondería justo el fallo que la prueba debe encontrar.
 */
import { existsSync, readdirSync, readFileSync, statSync } from 'node:fs';
import { join } from 'node:path';

export type RenameAll = 'camelCase' | 'snake_case';

export interface RustParam {
  /** Nombre del parámetro en Rust (snake_case). */
  name: string;
  /** Nombre que Tauri espera en el objeto de `invoke`. */
  key: string;
  /** `Option<...>`: la interfaz puede omitirlo. */
  optional: boolean;
}

export interface RustCommand {
  /** Ruta tal como está en `generate_handler!` (`commands::fire_test`). */
  path: string;
  /** Nombre de la función = nombre del comando para `invoke`. */
  name: string;
  /** Archivo donde se encontró (relativo a `src-tauri/src`). */
  file: string;
  rename: RenameAll;
  params: RustParam[];
}

/** Tipos que Tauri inyecta solo y que no viajan en el objeto de `invoke`. */
const INJECTED = /^(?:&\s*)?(?:(?:tauri::)?(?:ipc::)?)(?:State|AppHandle|Window|WebviewWindow|Webview|Request)\b/;

/** `power_pct` -> `powerPct` (igual que la conversión por defecto de Tauri). */
export function toCamelCase(snake: string): string {
  return snake.replace(/_+([a-zA-Z0-9])/g, (_m, c: string) => c.toUpperCase());
}

// Quita los comentarios de línea y de bloque (las firmas no llevan cadenas con barras dobles).
function stripComments(text: string): string {
  return text.replace(/\/\*[\s\S]*?\*\//g, '').replace(/\/\/[^\n]*/g, '');
}

/** Índice del cierre que corresponde al apertura en `open` (cuenta anidamiento). */
function matchClose(text: string, open: number, a: string, b: string): number {
  let depth = 0;
  for (let i = open; i < text.length; i++) {
    const c = text[i];
    if (c === a) depth++;
    else if (c === b && --depth === 0) return i;
  }
  throw new Error(`falta el cierre "${b}" que corresponde a la posición ${open}`);
}

/** Divide por comas de nivel superior (ignora las de `<>`, `()`, `[]`, `{}`). */
function splitTopLevel(text: string): string[] {
  const out: string[] = [];
  let depth = 0;
  let start = 0;
  for (let i = 0; i < text.length; i++) {
    const c = text[i];
    if ('(<[{'.includes(c)) depth++;
    else if (')>]}'.includes(c)) depth--;
    else if (c === ',' && depth === 0) {
      out.push(text.slice(start, i));
      start = i + 1;
    }
  }
  out.push(text.slice(start));
  return out.map((s) => s.trim()).filter((s) => s.length > 0);
}

/** Rutas dentro de `generate_handler![...]` de `lib.rs`. */
export function parseHandlerList(libRs: string): string[] {
  const code = stripComments(libRs);
  const at = code.indexOf('generate_handler![');
  if (at < 0) throw new Error('no se encontró `generate_handler![` en lib.rs');
  const open = code.indexOf('[', at);
  const close = matchClose(code, open, '[', ']');
  const items = splitTopLevel(code.slice(open + 1, close));
  const seen = new Set<string>();
  for (const item of items) {
    if (!/^[a-z_][a-z0-9_]*(?:::[a-z_][a-z0-9_]*)+$/.test(item)) {
      throw new Error(`entrada irreconocible en generate_handler!: "${item}"`);
    }
    if (seen.has(item)) throw new Error(`comando repetido en generate_handler!: ${item}`);
    seen.add(item);
  }
  if (items.length === 0) throw new Error('generate_handler! está vacío');
  return items;
}

function parseAttr(attr: string): RenameAll {
  // `attr` es el contenido tras `tauri::command`: vacío o `(...)`.
  const rest = attr.trim();
  if (rest === '') return 'camelCase';
  if (!rest.startsWith('(') || !rest.endsWith(')')) {
    throw new Error(`atributo tauri::command irreconocible: "${attr}"`);
  }
  let rename: RenameAll = 'camelCase';
  for (const part of splitTopLevel(rest.slice(1, -1))) {
    const m = /^rename_all\s*=\s*"([^"]+)"$/.exec(part);
    if (!m) throw new Error(`opción de tauri::command no reconocida: "${part}"`);
    if (m[1] !== 'snake_case' && m[1] !== 'camelCase') {
      throw new Error(`rename_all "${m[1]}" no soportado por la prueba de contrato`);
    }
    rename = m[1];
  }
  return rename;
}

function parseParam(raw: string, fn: string, rename: RenameAll): RustParam | null {
  const m = /^(?:mut\s+)?([a-z][a-z0-9_]*)\s*:\s*([\s\S]+)$/.exec(raw);
  if (!m) throw new Error(`parámetro irreconocible en ${fn}: "${raw}"`);
  const [, name, type] = m;
  if (INJECTED.test(type.trim())) return null;
  return {
    name,
    key: rename === 'snake_case' ? name : toCamelCase(name),
    optional: /^Option\s*</.test(type.trim()),
  };
}

/**
 * Todas las funciones `#[tauri::command...]` de un texto Rust, por nombre. Lanza si una no se
 * puede leer.
 */
export function parseCommandFns(
  rustSource: string,
  file = '<texto>',
): Map<string, { rename: RenameAll; params: RustParam[] }> {
  const code = stripComments(rustSource);
  const found = new Map<string, { rename: RenameAll; params: RustParam[] }>();
  const attrRe = /#\[\s*tauri::command\b/g;
  let m: RegExpExecArray | null;
  while ((m = attrRe.exec(code))) {
    const open = code.indexOf('[', m.index);
    const close = matchClose(code, open, '[', ']');
    const attrBody = code.slice(m.index, close).replace(/^#\[\s*tauri::command/, '');
    const rename = parseAttr(attrBody);
    // Salta otros atributos (`#[allow(...)]`) hasta la firma.
    let pos = close + 1;
    for (;;) {
      const rest = code.slice(pos);
      const attr = /^\s*#\[/.exec(rest);
      if (!attr) break;
      pos += attr[0].length - 1;
      pos = matchClose(code, pos, '[', ']') + 1;
    }
    const sig = /^\s*(?:pub(?:\([a-z]+\))?\s+)?(?:async\s+)?fn\s+([a-z_][a-z0-9_]*)\s*(<[^(]*>)?\s*\(/.exec(
      code.slice(pos),
    );
    if (!sig) throw new Error(`${file}: no se reconoce la firma tras #[tauri::command]`);
    const name = sig[1];
    if (sig[2]) throw new Error(`${file}: ${name} es genérico; la prueba de contrato no lo soporta`);
    const parenOpen = pos + sig[0].length - 1;
    const parenClose = matchClose(code, parenOpen, '(', ')');
    const params: RustParam[] = [];
    for (const raw of splitTopLevel(code.slice(parenOpen + 1, parenClose))) {
      const p = parseParam(raw, `${file}::${name}`, rename);
      if (p) params.push(p);
    }
    if (found.has(name)) throw new Error(`${file}: comando ${name} repetido`);
    found.set(name, { rename, params });
  }
  return found;
}

/** Archivos que pueden definir las funciones del módulo `mods` (p. ej. `['edit', 'camera']`). */
function moduleFiles(srcDir: string, mods: string[]): string[] {
  const base = join(srcDir, ...mods);
  const files: string[] = [];
  if (existsSync(`${base}.rs`)) files.push(`${base}.rs`);
  if (existsSync(join(base, 'mod.rs'))) files.push(join(base, 'mod.rs'));
  // Submódulos con `pub use sub::*` (edit/camera/*.rs); se ignoran las carpetas de pruebas.
  if (existsSync(base) && statSync(base).isDirectory()) {
    for (const f of readdirSync(base)) {
      if (f.endsWith('.rs') && f !== 'mod.rs') files.push(join(base, f));
    }
  }
  if (files.length === 0) throw new Error(`no se encontró el módulo ${mods.join('::')} en ${srcDir}`);
  return files;
}

/** Lee `lib.rs` y devuelve cada comando registrado con su firma. */
export function loadRustCommands(srcDir: string): RustCommand[] {
  const paths = parseHandlerList(readFileSync(join(srcDir, 'lib.rs'), 'utf8'));
  const cache = new Map<string, Map<string, { rename: RenameAll; params: RustParam[] }>>();
  return paths.map((path) => {
    const segs = path.split('::');
    const name = segs[segs.length - 1];
    const mods = segs.slice(0, -1);
    const hits: RustCommand[] = [];
    for (const file of moduleFiles(srcDir, mods)) {
      let fns = cache.get(file);
      if (!fns) {
        fns = parseCommandFns(readFileSync(file, 'utf8'), file);
        cache.set(file, fns);
      }
      const fn = fns.get(name);
      if (fn) hits.push({ path, name, file: file.slice(srcDir.length + 1), ...fn });
    }
    if (hits.length === 0) {
      throw new Error(`${path}: no se encontró una función #[tauri::command] llamada ${name}`);
    }
    if (hits.length > 1) {
      throw new Error(`${path}: función ambigua, definida en ${hits.map((h) => h.file).join(' y ')}`);
    }
    return hits[0];
  });
}
