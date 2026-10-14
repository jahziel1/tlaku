import { describe, expect, it } from 'vitest';
import { parseCommandFns, parseHandlerList, toCamelCase } from './rustCommands';

describe('lector de comandos de Tauri', () => {
  it('convierte a camelCase como Tauri', () => {
    expect(toCamelCase('power_pct')).toBe('powerPct');
    expect(toCamelCase('s_max')).toBe('sMax');
    expect(toCamelCase('path')).toBe('path');
  });

  it('lee firmas multilínea, omite lo inyectado y respeta rename_all', () => {
    const src = `
      /// doc
      #[tauri::command(rename_all = "snake_case")]
      pub async fn fire_test(
          state: State<'_, AppState>,
          app: tauri::AppHandle,
          power_pct: f64, // comentario
          duration_ms: Option<u32>,
          m: HashMap<String, Vec<u8>>,
      ) -> Result<(), String> { Ok(()) }

      #[tauri::command]
      fn plain(mut my_arg: String, w: WebviewWindow) -> String { my_arg }
    `;
    const fns = parseCommandFns(src);
    expect(fns.get('fire_test')).toEqual({
      rename: 'snake_case',
      params: [
        { name: 'power_pct', key: 'power_pct', optional: false },
        { name: 'duration_ms', key: 'duration_ms', optional: true },
        { name: 'm', key: 'm', optional: false },
      ],
    });
    expect(fns.get('plain')?.params).toEqual([{ name: 'my_arg', key: 'myArg', optional: false }]);
  });

  it('falla fuerte si no reconoce algo', () => {
    expect(() => parseCommandFns('#[tauri::command]\nstruct X;')).toThrow(/firma/);
    expect(() => parseCommandFns('#[tauri::command(async)]\nfn a() {}')).toThrow(/no reconocida/);
    expect(() => parseCommandFns('#[tauri::command]\nfn a((x, y): (u8, u8)) {}')).toThrow(/irreconocible/);
    expect(() => parseCommandFns('#[tauri::command(rename_all = "kebab-case")]\nfn a() {}')).toThrow(/soportado/);
  });

  it('lee la lista de generate_handler!', () => {
    const lib = 'x(generate_handler![\n a::b, // c\n edit::t::d,\n])';
    expect(parseHandlerList(lib)).toEqual(['a::b', 'edit::t::d']);
    expect(() => parseHandlerList('generate_handler![a::b, a::b]')).toThrow(/repetido/);
    expect(() => parseHandlerList('generate_handler![solo]')).toThrow(/irreconocible/);
  });
});
