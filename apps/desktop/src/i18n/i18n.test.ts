import { afterEach, describe, expect, it, vi } from 'vitest';
import { base } from './es';
import { detectLanguage, en, es, getLanguage, locale, setLanguage, t, tn } from './index';
import { arrange } from './es/arrange';
import { design } from './es/design';
import { art } from './es/art';
import { camera } from './es/camera';
import { canvas } from './es/canvas';
import { catalog } from './es/catalog';
import { codes } from './es/codes';
import { devices } from './es/devices';
import { fileio } from './es/fileio';
import { help } from './es/help';
import { history } from './es/history';
import { layers } from './es/layers';
import { machine } from './es/machine';
import { mset } from './es/mset';
import { nodes } from './es/nodes';
import { preview } from './es/preview';
import { prefs } from './es/prefs';
import { remote } from './es/remote';
import { bundle } from './es/bundle';
import { text } from './es/text';
import { trace } from './es/trace';
import { variables } from './es/variables';
import { nest } from './es/nest';
import { printcut } from './es/printcut';
import { galvo } from './es/galvo';
import { lhymicro } from './es/lhymicro';
import { ruida } from './es/ruida';
import { shapes } from './es/shapes';
import { review } from './es/review';
import { tiling } from './es/tiling';
import { materials } from './es/materials';
import { simple } from './es/simple';
import { a11y } from './es/a11y';
import { tabs } from './es/tabs';
import { parts } from './es/parts';
import { costs } from './es/costs';
import { queue } from './es/queue';
import { arrange as en_arrange } from './en/arrange';
import { design as en_design } from './en/design';
import { art as en_art } from './en/art';
import { camera as en_camera } from './en/camera';
import { canvas as en_canvas } from './en/canvas';
import { catalog as en_catalog } from './en/catalog';
import { codes as en_codes } from './en/codes';
import { devices as en_devices } from './en/devices';
import { fileio as en_fileio } from './en/fileio';
import { help as en_help } from './en/help';
import { history as en_history } from './en/history';
import { layers as en_layers } from './en/layers';
import { machine as en_machine } from './en/machine';
import { mset as en_mset } from './en/mset';
import { materials as en_materials } from './en/materials';
import { nodes as en_nodes } from './en/nodes';
import { preview as en_preview } from './en/preview';
import { prefs as en_prefs } from './en/prefs';
import { remote as en_remote } from './en/remote';
import { bundle as en_bundle } from './en/bundle';
import { text as en_text } from './en/text';
import { trace as en_trace } from './en/trace';
import { variables as en_variables } from './en/variables';
import { nest as en_nest } from './en/nest';
import { printcut as en_printcut } from './en/printcut';
import { galvo as en_galvo } from './en/galvo';
import { lhymicro as en_lhymicro } from './en/lhymicro';
import { ruida as en_ruida } from './en/ruida';
import { shapes as en_shapes } from './en/shapes';
import { review as en_review } from './en/review';
import { tiling as en_tiling } from './en/tiling';
import { base as en_base } from './en/base';
import { simple as en_simple } from './en/simple';
import { a11y as en_a11y } from './en/a11y';
import { tabs as en_tabs } from './en/tabs';
import { parts as en_parts } from './en/parts';
import { costs as en_costs } from './en/costs';
import { queue as en_queue } from './en/queue';

describe('diccionarios por área', () => {
  it('cada clave lleva el prefijo de su área, así ningún archivo pisa a otro', () => {
    const areas = { review, tiling, art, canvas, help, history, layers, machine, mset, preview, arrange, design, text, nodes, trace, nest, printcut, ruida, lhymicro, galvo, shapes, camera, materials, devices, catalog, prefs, remote, bundle, codes, fileio, variables, simple, a11y, tabs, costs, parts, queue };
    for (const [area, dict] of Object.entries(areas)) {
      for (const key of Object.keys(dict)) expect(key.startsWith(`${area}.`), key).toBe(true);
    }
  });
});

type Dict = Record<string, string | undefined>;

const AREAS: Record<string, { es: Dict; en: Dict }> = {
  base: { es: base, en: en_base },
  arrange: { es: arrange, en: en_arrange },
  design: { es: design, en: en_design },
  art: { es: art, en: en_art },
  camera: { es: camera, en: en_camera },
  canvas: { es: canvas, en: en_canvas },
  catalog: { es: catalog, en: en_catalog },
  codes: { es: codes, en: en_codes },
  devices: { es: devices, en: en_devices },
  fileio: { es: fileio, en: en_fileio },
  help: { es: help, en: en_help },
  history: { es: history, en: en_history },
  layers: { es: layers, en: en_layers },
  machine: { es: machine, en: en_machine },
  mset: { es: mset, en: en_mset },
  materials: { es: materials, en: en_materials },
  nodes: { es: nodes, en: en_nodes },
  nest: { es: nest, en: en_nest },
  printcut: { es: printcut, en: en_printcut },
  ruida: { es: ruida, en: en_ruida },
  lhymicro: { es: lhymicro, en: en_lhymicro },
  galvo: { es: galvo, en: en_galvo },
  shapes: { es: shapes, en: en_shapes },
  tiling: { es: tiling, en: en_tiling },
  review: { es: review, en: en_review },
  preview: { es: preview, en: en_preview },
  prefs: { es: prefs, en: en_prefs },
  remote: { es: remote, en: en_remote },
  bundle: { es: bundle, en: en_bundle },
  text: { es: text, en: en_text },
  trace: { es: trace, en: en_trace },
  variables: { es: variables, en: en_variables },
  simple: { es: simple, en: en_simple },
  a11y: { es: a11y, en: en_a11y },
  tabs: { es: tabs, en: en_tabs },
  parts: { es: parts, en: en_parts },
  costs: { es: costs, en: en_costs },
  queue: { es: queue, en: en_queue },
};

/**
 * Claves cuyo texto en inglés puede ser idéntico al español, con el motivo. Todo lo demás debe estar
 * traducido de verdad. Además se acepta sin lista un texto que, sin marcadores ni unidades, no tiene
 * letras (por ejemplo «×{n}» o «0,1 mm»).
 */
const IDENTICAL_OK: Record<string, string> = {
  // base / layers / machine / preview / history / help
  'canvas.zoom': 'palabra idéntica en ambos idiomas',
  'canvas.obj.auto': 'abreviatura idéntica en ambos idiomas',
  'layers.dither.stucki': 'nombre propio del algoritmo',
  'layers.dither.jarvis': 'nombre propio del algoritmo',
  'layers.dither.atkinson': 'nombre propio del algoritmo',
  'layers.dither.floyd': 'nombre propio del algoritmo',
  'layers.gamma': 'término técnico igual en ambos idiomas',
  'layers.kerf': 'término del oficio, igual en ambos idiomas',
  'layers.overscan': 'término del oficio, igual en ambos idiomas',
  'layers.feature.z': 'nombre de un eje',
  'layers.group.general': 'palabra idéntica en ambos idiomas',
  'machine.dev.group.general': 'palabra idéntica en ambos idiomas',
  'machine.virtual.badge': 'palabra idéntica en ambos idiomas',
  'machine.virtual.name.falcon2': 'nombre de modelo',
  'machine.virtual.name.grbl11': 'nombre de firmware',
  'machine.virtual.name.grblhal': 'nombre de firmware',
  'machine.virtual.name.fluidnc': 'nombre de firmware',
  'machine.virtual.name.marlin': 'nombre de firmware',
  'machine.virtual.name.smoothie': 'nombre de firmware',
  'preview.legend.overscan': 'término del oficio, igual en ambos idiomas',
  'preview.speed.value': 'multiplicador, igual en ambos idiomas',
  'ruida.experimental': 'palabra idéntica en ambos idiomas',
  'lhymicro.experimental': 'palabra idéntica en ambos idiomas',
  'galvo.experimental': 'palabra idéntica en ambos idiomas',
  'galvo.panel.field.value': 'solo cifras, unidades y el nombre de la fuente, igual en ambos idiomas',
  'galvo.layer.group': 'nombre de la sección, igual en ambos idiomas',
  'galvo.panel.title': 'nombre de la familia de tarjetas, igual en ambos idiomas',
  'galvo.dev.group': 'nombre de la familia de tarjetas, igual en ambos idiomas',
  'lhymicro.virtual.name': 'nombre de la máquina',
  'ruida.virtual.name': 'nombre de familia de controladoras',
  'ruida.info.magic': 'término del protocolo, igual en ambos idiomas',
  'prefs.language.es': 'nombre del idioma en su propio idioma',
  'prefs.language.en': 'nombre del idioma en su propio idioma',
  'printcut.title': 'nombre de la función, igual en ambos idiomas',
  'printcut.cmd.open': 'nombre de la función, igual en ambos idiomas',
  'printcut.capture.saved': 'solo coordenadas, igual en ambos idiomas',
};

const placeholders = (text: string): string[] => [...text.matchAll(/\{(\w+)\}/g)].map((m) => m[1]!).sort();

const noLetters = (text: string): boolean =>
  !/[A-Za-zÀ-ÖØ-öø-ÿ]/.test(text.replace(/\{\w+\}/g, '').replace(/\b(mm|cm|px|DPI|s|min|G-code|GRBL)\b/g, ''));

/**
 * Palabras que se escriben igual en español e inglés: nombres propios, siglas, formatos y cognados
 * exactos. Un texto idéntico al español pasa solo si TODAS sus palabras están aquí (sin distinguir
 * mayúsculas) o llevan dígitos, son una letra suelta (A, B, X, Ø…) o una sigla en mayúsculas de 2 o más letras. Cualquier otra
 * palabra obliga a traducir o a justificar la clave en `IDENTICAL_OK`.
 */
const NOMBRES = new Set(
  [
    // firmware y marcas
    'grbl', 'grblhal', 'fluidnc', 'marlin', 'smoothieware', 'smoothie', 'falcon2', 'pro', 'creality', 'tlaku', 'lightburn',
    // formatos y códigos
    'qr', 'data', 'matrix', 'code', 'ean', 'upc', 'svg', 'dxf', 'pdf', 'png', 'jpg', 'bmp', 'dpi', 'tlakuprefs',
    // cognados exactos
    'offset', 'original', 'horizontal', 'vertical', 'firmware', 'general', 'material', 'error', 'no', 'virtual',
    'overscan', 'kerf', 'gamma', 'zoom', 'codabar', 'mm', 'cm', 'dx', 'dy', 'macro', 'macros',
  ].map((w) => w.toLowerCase()),
);

const soloNombres = (text: string): boolean => {
  const words = text.replace(/\{\w+\}/g, ' ').match(/[A-Za-zÀ-ÖØ-öø-ÿ0-9][A-Za-zÀ-ÖØ-öø-ÿ0-9.]*/g) ?? [];
  return words.every((w) => w.length === 1 || /\d/.test(w) || (/^[A-Z]{2,}$/.test(w)) || NOMBRES.has(w.toLowerCase().replace(/\.$/, '')));
};

describe('inglés: completitud por área', () => {
  for (const [area, { es: esDict, en: enDict }] of Object.entries(AREAS)) {
    describe(area, () => {
      it('tiene exactamente las mismas claves que el español', () => {
        const missing = Object.keys(esDict).filter((k) => !(k in enDict));
        const extra = Object.keys(enDict).filter((k) => !(k in esDict));
        expect({ missing, extra }).toEqual({ missing: [], extra: [] });
      });

      it('conserva los marcadores {x} de cada texto', () => {
        const bad = Object.keys(esDict)
          .filter((k) => typeof enDict[k] === 'string' && placeholders(enDict[k]!).join() !== placeholders(esDict[k]!).join())
          .map((k) => `${k}: ${placeholders(esDict[k]!).join()} -> ${placeholders(enDict[k]!).join()}`);
        expect(bad).toEqual([]);
      });

      it('no deja textos vacíos ni idénticos al español', () => {
        const empty = Object.keys(enDict).filter((k) => typeof enDict[k] === 'string' && enDict[k]!.trim() === '');
        const same = Object.keys(esDict).filter((k) => {
          const e = enDict[k];
          return typeof e === 'string' && e.trim() !== '' && e === esDict[k] && !(k in IDENTICAL_OK) && !noLetters(e) && !soloNombres(e);
        });
        expect({ empty, same }).toEqual({ empty: [], same: [] });
      });
    });
  }

  it('las excepciones de textos idénticos siguen existiendo y siguen siendo idénticas', () => {
    const all: Dict = Object.assign({}, ...Object.values(AREAS).map((a) => a.es));
    const allEn: Dict = Object.assign({}, ...Object.values(AREAS).map((a) => a.en));
    const stale = Object.keys(IDENTICAL_OK).filter((k) => !(k in all) || allEn[k] !== all[k]);
    expect(stale).toEqual([]);
  });

  it('el diccionario `en` unido tiene las mismas claves que `es`', () => {
    expect(Object.keys(en).sort()).toEqual(Object.keys(es).sort());
  });
});

describe('plurales: tn()', () => {
  afterEach(() => setLanguage('es'));

  it('elige .one con 1 y .other con 0 o con más', () => {
    expect(tn('review.count', 1)).toBe('Encontré 1 cosa por revisar.');
    expect(tn('review.count', 0)).toBe('Encontré 0 cosas por revisar.');
    expect(tn('review.count', 3)).toBe('Encontré 3 cosas por revisar.');
    setLanguage('en');
    expect(tn('review.count', 1)).toBe('Found 1 thing to look at.');
    expect(tn('review.count', 2)).toBe('Found 2 things to look at.');
  });

  it('pasa n y los demás marcadores al texto', () => {
    expect(tn('materials.notice.exported', 1, { path: '/a' })).toBe('Biblioteca exportada (1 ajuste) en /a.');
    expect(tn('materials.notice.exported', 12, { path: '/a' })).toBe('Biblioteca exportada (12 ajustes) en /a.');
  });

  it('cada `.one` tiene su `.other` (o su `.many`) y cada `.other` con hermano `.one` tiene el par completo, en los dos idiomas', () => {
    for (const [name, dict] of Object.entries({ es, en }) as [string, Dict][]) {
      const keys = Object.keys(dict);
      const strays = keys.filter((k) => k.endsWith('.one') && !keys.includes(`${k.slice(0, -4)}.other`) && !keys.includes(`${k.slice(0, -4)}.many`));
      expect({ name, strays }).toEqual({ name, strays: [] });
    }
  });
});

describe('idioma en uso', () => {
  afterEach(() => {
    setLanguage('es');
    vi.unstubAllGlobals();
  });

  it('t() traduce según el idioma y vuelve al español si falta la clave', () => {
    setLanguage('en');
    expect(getLanguage()).toBe('en');
    expect(t('cmd.file.save')).toBe('Save');
    expect(t('notice.saved', { path: '/a.tlaku' })).toBe('Saved to /a.tlaku');
    setLanguage('es');
    expect(t('cmd.file.save')).toBe('Guardar');
  });

  it('locale() sigue al idioma', () => {
    expect(locale()).toBe('es');
    setLanguage('en');
    expect(locale()).toBe('en-US');
  });

  it('detectLanguage() usa inglés solo si el sistema empieza por «en»', () => {
    vi.stubGlobal('window', {});
    for (const [nav, want] of [['en-US', 'en'], ['en', 'en'], ['EN-gb', 'en'], ['es-MX', 'es'], ['fr-FR', 'es'], ['', 'es']] as const) {
      vi.stubGlobal('navigator', { language: nav });
      expect(detectLanguage(), nav).toBe(want);
    }
  });

  it('detectLanguage() devuelve español fuera de un navegador', () => {
    vi.stubGlobal('window', undefined);
    expect(detectLanguage()).toBe('es');
  });
});
