/**
 * Contrato entre la interfaz y los comandos de Tauri.
 *
 * El modo simulado (mock) no pasa por Tauri, así que no ve un argumento mal nombrado: Tauri
 * espera camelCase salvo que el comando lleve `#[tauri::command(rename_all = "snake_case")]`,
 * y si la interfaz manda `power_pct` a un comando camelCase, la app real falla. Esta prueba:
 *
 * 1. lee de `src-tauri/src/lib.rs` los comandos registrados y, de cada función Rust, los
 *    parámetros propios (sin `State`/`AppHandle`/`Window`) y su `rename_all`
 *    (`contract/rustCommands.ts`);
 * 2. saca de `LaserApi` (compilador de TypeScript) las claves que la interfaz manda por método;
 * 3. llama cada método de la API real de Tauri (`createTauriApi`) con un objeto con esas claves,
 *    interceptando `invoke`, y registra lo que realmente se le pasa;
 * 4. compara ambos lados, y también `COMMAND_NAMES` contra `lib.rs`.
 *
 * Las claves de campos anidados dentro de cada argumento (p. ej. `Document`) las decide serde, no
 * Tauri, y no entran aquí.
 */
import { join } from 'node:path';
import ts from 'typescript';
import { beforeAll, describe, expect, it, vi } from 'vitest';

const invoke = vi.fn(async (_cmd: string, _args?: unknown) => undefined);
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (cmd: string, args?: unknown) => invoke(cmd, args),
  convertFileSrc: (p: string) => p,
}));
vi.mock('@tauri-apps/api/event', () => ({ listen: async () => () => undefined }));

import { COMMAND_NAMES } from './api';
import { loadRustCommands, type RustCommand } from './contract/rustCommands';
import { createTauriApi } from './tauri';

const ROOT = join(import.meta.dirname, '..', '..'); // apps/desktop
const SRC_TAURI = join(ROOT, 'src-tauri', 'src');

interface UiKeys {
  /** Todas las claves que acepta el argumento del método. */
  all: string[];
  /** Las que no son opcionales (`?`). */
  required: string[];
}

/** Claves del primer argumento de cada método de `LaserApi`, leídas con el compilador. */
function readUiKeys(): Record<string, UiKeys> {
  const cfgPath = join(ROOT, 'tsconfig.json');
  const cfg = ts.readConfigFile(cfgPath, ts.sys.readFile);
  const parsed = ts.parseJsonConfigFileContent(cfg.config, ts.sys, ROOT);
  const apiFile = join(ROOT, 'src', 'api', 'api.ts');
  const program = ts.createProgram({ rootNames: [apiFile], options: parsed.options });
  const checker = program.getTypeChecker();
  const source = program.getSourceFile(apiFile);
  if (!source) throw new Error('no se pudo abrir api.ts');
  let iface: ts.InterfaceDeclaration | undefined;
  source.forEachChild((n) => {
    if (ts.isInterfaceDeclaration(n) && n.name.text === 'LaserApi') iface = n;
  });
  if (!iface) throw new Error('no se encontró `interface LaserApi` en api.ts');
  const out: Record<string, UiKeys> = {};
  const apiType = checker.getTypeAtLocation(iface);
  for (const prop of checker.getPropertiesOfType(apiType)) {
    if (prop.name === 'on') continue;
    const type = checker.getTypeOfSymbolAtLocation(prop, iface);
    const sigs = type.getCallSignatures();
    if (sigs.length !== 1) throw new Error(`${prop.name}: se esperaba una sola firma`);
    const params = sigs[0].getParameters();
    if (params.length === 0) {
      out[prop.name] = { all: [], required: [] };
      continue;
    }
    if (params.length > 1) throw new Error(`${prop.name}: más de un argumento (invoke solo manda un objeto)`);
    const argType = checker.getNonNullableType(checker.getTypeOfSymbolAtLocation(params[0], iface));
    const props = checker.getPropertiesOfType(argType);
    const isObject =
      (argType.flags & (ts.TypeFlags.Object | ts.TypeFlags.Intersection)) !== 0 && !checker.isArrayType(argType);
    if (!isObject || argType.isUnion()) {
      throw new Error(`${prop.name}: el argumento debe ser un objeto con claves, no ${checker.typeToString(argType)}`);
    }
    out[prop.name] = {
      all: props.map((p) => p.name).sort(),
      required: props.filter((p) => (p.flags & ts.SymbolFlags.Optional) === 0).map((p) => p.name).sort(),
    };
  }
  return out;
}

const methods = Object.keys(COMMAND_NAMES) as (keyof typeof COMMAND_NAMES)[];

describe('contrato interfaz <-> comandos de Tauri', () => {
  let rust: RustCommand[];
  let ui: Record<string, UiKeys>;
  beforeAll(() => {
    rust = loadRustCommands(SRC_TAURI);
    ui = readUiKeys();
  }, 60_000);

  it('COMMAND_NAMES y generate_handler! registran los mismos comandos', () => {
    const names = Object.values(COMMAND_NAMES) as string[];
    const rustNames = rust.map((c) => c.name);
    expect(new Set(names).size, 'COMMAND_NAMES repite un comando').toBe(names.length);
    expect({
      sinRegistrarEnLibRs: names.filter((n) => !rustNames.includes(n)).sort(),
      sinEntradaEnCommandNames: rustNames.filter((n) => !names.includes(n)).sort(),
    }).toEqual({ sinRegistrarEnLibRs: [], sinEntradaEnCommandNames: [] });
  });

  it('LaserApi, COMMAND_NAMES y createTauriApi tienen los mismos métodos', () => {
    const api = createTauriApi() as unknown as Record<string, unknown>;
    const apiMethods = Object.keys(api).filter((k) => k !== 'on').sort();
    expect(apiMethods).toEqual([...methods].sort());
    expect(Object.keys(ui).sort()).toEqual([...methods].sort());
  });

  it('cada método manda a invoke las claves que el comando de Rust espera', () => {
    const api = createTauriApi() as unknown as Record<string, (a?: object) => Promise<unknown>>;
    const byName = new Map(rust.map((c) => [c.name, c]));
    const problems: string[] = [];
    for (const method of methods) {
      const cmd = COMMAND_NAMES[method];
      const keys = ui[method];
      const fn = byName.get(cmd);
      if (!keys || !fn) continue; // ya lo reporta la prueba de nombres
      // Llamada real a la API de Tauri, con todas las claves del tipo, e intercepción de invoke.
      invoke.mockClear();
      const args = Object.fromEntries(keys.all.map((k) => [k, 1]));
      void api[method](keys.all.length > 0 ? args : undefined);
      expect(invoke, `${method} debe llamar a invoke una vez`).toHaveBeenCalledTimes(1);
      const [sentCmd, sent] = invoke.mock.calls[0];
      if (sentCmd !== cmd) problems.push(`${method}: invoke recibió "${sentCmd}", se esperaba "${cmd}"`);
      const sentKeys = sent && typeof sent === 'object' ? Object.keys(sent).sort() : [];
      if (sentKeys.join() !== keys.all.join()) {
        problems.push(`${method}: tauri.ts altera las claves (tipo: ${keys.all}; invoke: ${sentKeys})`);
      }

      const expected = new Set(fn.params.map((p) => p.key));
      const hint = fn.rename === 'snake_case' ? 'snake_case (rename_all)' : 'camelCase';
      for (const k of sentKeys) {
        if (!expected.has(k)) {
          problems.push(
            `${method} -> ${cmd}: la interfaz manda "${k}" y Rust espera [${[...expected]}] en ${hint} ` +
              `(${fn.file}). Agrega #[tauri::command(rename_all = "snake_case")] o cambia la clave.`,
          );
        }
      }
      for (const p of fn.params) {
        if (!p.optional && !sentKeys.includes(p.key)) {
          problems.push(
            `${method} -> ${cmd}: Rust exige "${p.key}" (${hint}) y la interfaz no lo manda (${fn.file}).`,
          );
        } else if (!p.optional && !keys.required.includes(p.key)) {
          problems.push(
            `${method} -> ${cmd}: "${p.key}" es obligatorio en Rust pero opcional (?) en LaserApi (${fn.file}).`,
          );
        }
      }
    }
    expect(problems).toEqual([]);
  });

  it('las claves obligatorias del tipo no son opcionales en Rust sin motivo', () => {
    // Una clave obligatoria en TypeScript que Rust ignora ya la atrapa la prueba anterior; aquí solo
    // se comprueba que el lector vio parámetros en los comandos con argumentos.
    const conArgs = rust.filter((c) => c.params.length > 0).length;
    expect(conArgs).toBeGreaterThan(40);
    expect(rust.filter((c) => c.rename === 'snake_case').length).toBeGreaterThan(10);
  });
});
