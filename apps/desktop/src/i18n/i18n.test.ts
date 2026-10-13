import { afterEach, describe, expect, it, vi } from 'vitest';
import { base } from './es';
import { detectLanguage, en, es, getLanguage, locale, setLanguage, t } from './index';
import { arrange } from './es/arrange';
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
import { nodes } from './es/nodes';
import { preview } from './es/preview';
import { prefs } from './es/prefs';
import { text } from './es/text';
import { trace } from './es/trace';
import { materials } from './es/materials';
import { arrange as en_arrange } from './en/arrange';
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
import { materials as en_materials } from './en/materials';
import { nodes as en_nodes } from './en/nodes';
import { preview as en_preview } from './en/preview';
import { prefs as en_prefs } from './en/prefs';
import { text as en_text } from './en/text';
import { trace as en_trace } from './en/trace';
import { base as en_base } from './en/base';

describe('diccionarios por área', () => {
  it('cada clave lleva el prefijo de su área, así ningún archivo pisa a otro', () => {
    const areas = { canvas, help, history, layers, machine, preview, arrange, text, nodes, trace, camera, materials, devices, catalog, prefs, codes, fileio };
    for (const [area, dict] of Object.entries(areas)) {
      for (const key of Object.keys(dict)) expect(key.startsWith(`${area}.`), key).toBe(true);
    }
  });
});

type Dict = Record<string, string | undefined>;

const AREAS: Record<string, { es: Dict; en: Dict }> = {
  base: { es: base, en: en_base },
  arrange: { es: arrange, en: en_arrange },
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
  materials: { es: materials, en: en_materials },
  nodes: { es: nodes, en: en_nodes },
  preview: { es: preview, en: en_preview },
  prefs: { es: prefs, en: en_prefs },
  text: { es: text, en: en_text },
  trace: { es: trace, en: en_trace },
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
  'prefs.language.es': 'nombre del idioma en su propio idioma',
  'prefs.language.en': 'nombre del idioma en su propio idioma',
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
    'overscan', 'kerf', 'gamma', 'zoom', 'codabar', 'mm', 'cm', 'dx', 'dy',
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
