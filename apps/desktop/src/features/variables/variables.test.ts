import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createMockApi } from '@/api/mock';
import { mockTextOutline } from '@/api/mock/text';
import type { CsvTable, DesignObject, Document, TextObject, VariableText } from '@/api/types';
import { startOutlineSync } from '@/features/text/outlineSync';
import { isTextObject, resolveText } from '@/features/text/signature';
import { setLanguage } from '@/i18n';
import { objectBounds, unionBounds } from '@/lib/geometry';
import { useStore } from '@/store';
import { advanceAfterJob, advanceSettings, atCsvEnd, jobSpans } from './advance';
import { copiesPerRow } from './copies';
import { EMPTY_CSV, displayContext, useVariablesUi } from './context';
import { formatDate, formatSerial, parseSerialOption } from './format';
import { insertOptions, insertSnippet } from './insert';
import { resolveDocForOutput, VariableError } from './jobResolve';
import { csvColumnIndex, makeContext, resolveTemplate, type VarContext } from './resolve';
import { DEFAULT_VARIABLE_TEXT, editVariableText } from './settings';
import { csvColumnsUsed, escapeLiteral, hasVariables, isTemplate, parseTemplate, serializeTemplate, shiftTemplate, usesCsv } from './syntax';
import { CsvTableView, windowStart } from './VariablesDialog';

const CSV: CsvTable = {
  headers: ['Nombre', 'Cargo', 'Código'],
  rows: [
    ['Ana', 'Jefa', 'A-1'],
    ['Luis', 'Técnico', 'A-2'],
    ['Eva', 'Diseño', 'A-3'],
  ],
  delimiter: ',',
};

// Miércoles 3 de septiembre de 2025, 14:05:09 (hora local).
const NOW = new Date(2025, 8, 3, 14, 5, 9);

const ctx = (patch: Partial<VarContext> = {}): VarContext => ({
  serial: 7, serialStep: 1, serialDigits: 0, csv: CSV, csvRow: 0, now: NOW, lang: 'es', ...patch,
});

const textKind = (text: string): TextObject => ({
  text, font_family: 'Arial', bold: false, italic: false, size_mm: 10, letter_spacing_mm: 0,
  line_spacing: 1.2, align: 'left', on_path: null, outline: [],
});
const textObj = (id: string, text: string, extra: Partial<DesignObject> = {}): DesignObject => ({
  id, layer: '#ff0000', transform: [1, 0, 0, 1, 0, 0], locked: false, hidden: false, group: null,
  kind: { type: 'text', ...textKind(text) }, ...extra,
});
const doc = (objects: DesignObject[], vt?: Partial<VariableText>): Document => ({
  version: 2, objects, layers: [], device_id: null, ...(vt ? { variable_text: { ...DEFAULT_VARIABLE_TEXT, ...vt } } : {}),
});

describe('sintaxis: análisis', () => {
  it('separa texto y variables con opción y desplazamiento', () => {
    const t = parseTemplate('N° {serie:5} de {csv:Nombre@2}!');
    expect(t.map((x) => x.kind)).toEqual(['text', 'var', 'text', 'var', 'text']);
    expect(t[1]).toMatchObject({ kind: 'var', name: 'serial', arg: '5', offset: 0 });
    expect(t[3]).toMatchObject({ kind: 'var', name: 'csv', arg: 'Nombre', offset: 2 });
  });

  it('acepta los nombres en español e inglés sin distinguir mayúsculas', () => {
    const names = ['serie', 'SERIAL', 'Fecha', 'date', 'hora', 'Time', 'copia', 'copy', 'fila', 'row', 'csv'];
    const got = names.map((n) => (parseTemplate(`{${n}${n === 'csv' ? ':x' : ''}}`)[0] as { name: string }).name);
    expect(got).toEqual(['serial', 'serial', 'date', 'date', 'time', 'time', 'copy', 'copy', 'row', 'row', 'csv']);
  });

  it('las llaves dobles son llaves literales y no abren una variable', () => {
    expect(parseTemplate('{{serie}}')).toEqual([{ kind: 'text', text: '{serie}' }]);
    expect(parseTemplate('a {{ b }} c')).toEqual([{ kind: 'text', text: 'a { b } c' }]);
    expect(parseTemplate('{{{serie}}}').map((x) => x.kind)).toEqual(['text', 'var', 'text']);
  });

  it('lo que no se entiende queda marcado y no se pierde', () => {
    expect(parseTemplate('{foo}')[0]).toMatchObject({ kind: 'bad', reason: 'unknown', raw: '{foo}' });
    expect(parseTemplate('{}')[0]).toMatchObject({ kind: 'bad', reason: 'empty' });
    expect(parseTemplate('abc {serie')[1]).toMatchObject({ kind: 'bad', reason: 'unclosed', raw: '{serie' });
    // Una llave de cierre suelta es un carácter normal.
    expect(parseTemplate('a } b')).toEqual([{ kind: 'text', text: 'a } b' }]);
  });

  it('detecta plantillas, variables, columnas y uso del CSV', () => {
    expect(isTemplate('Hola')).toBe(false);
    expect(isTemplate('{{x}}')).toBe(true);
    expect(hasVariables('{{x}}')).toBe(false);
    expect(hasVariables('N {serie}')).toBe(true);
    expect(csvColumnsUsed('{csv:Nombre} {csv:Cargo} {csv:Nombre@1} {serie}')).toEqual(['Nombre', 'Cargo']);
    expect(usesCsv('{fila}')).toBe(true);
    expect(usesCsv('{serie}{fecha}')).toBe(false);
  });

  it('ida y vuelta: escribir lo analizado devuelve la misma plantilla', () => {
    for (const src of ['{serie}', 'A {csv:Nombre@3} B', '{{x}} {fecha:dd/MM/yyyy} }}', "{fecha:d 'de' MMMM}", '{copia@-1}']) {
      expect(serializeTemplate(parseTemplate(src)), src).toBe(src);
    }
    // Una llave de cierre suelta se normaliza a la forma escapada, con el mismo valor.
    const norm = serializeTemplate(parseTemplate('a } b'));
    expect(norm).toBe('a }} b');
    expect(resolveTemplate(norm, ctx()).text).toBe('a } b');
  });

  it('escapar un literal y resolverlo devuelve el literal', () => {
    const literal = 'caja {A} y }B{';
    expect(resolveTemplate(escapeLiteral(literal), ctx())).toEqual({ text: literal, issues: [] });
  });
});

describe('formato de serie y de fecha', () => {
  it('serie con ceros, signo y hexadecimal', () => {
    expect(formatSerial(7, 4)).toBe('0007');
    expect(formatSerial(12345, 3)).toBe('12345');
    expect(formatSerial(-5, 3)).toBe('-005');
    expect(formatSerial(255, 4, 'lower')).toBe('00ff');
    expect(formatSerial(255, 0, 'upper')).toBe('FF');
    expect(parseSerialOption('hex4')).toEqual({ digits: 4, hex: 'lower' });
    expect(parseSerialOption('HEX')).toEqual({ digits: null, hex: 'upper' });
    expect(parseSerialOption('abc')).toBeNull();
  });

  it('fecha en español e inglés, con las marcas de ambos idiomas', () => {
    expect(formatDate(NOW, 'dd/MM/yyyy', 'es')).toBe('03/09/2025');
    expect(formatDate(NOW, 'dd/mm/aaaa', 'es')).toBe('03/09/2025'); // mm junto a una fecha es el mes
    expect(formatDate(NOW, 'd/M/aa', 'es')).toBe('3/9/25');
    expect(formatDate(NOW, "dddd d 'de' MMMM 'de' yyyy", 'es')).toBe('miércoles 3 de septiembre de 2025');
    expect(formatDate(NOW, 'ddd, MMM d', 'en')).toBe('Wed, Sep 3');
    expect(formatDate(NOW, 'MMMM d, yyyy', 'en')).toBe('September 3, 2025');
  });

  it('hora: 24 y 12 horas, minutos junto a la hora y segundos', () => {
    expect(formatDate(NOW, 'HH:mm', 'es')).toBe('14:05');
    expect(formatDate(NOW, 'HH:mm:ss', 'es')).toBe('14:05:09');
    expect(formatDate(NOW, 'h:mm AP', 'en')).toBe('2:05 PM');
    expect(formatDate(NOW, 'hh:mm ap', 'en')).toBe('02:05 pm');
    expect(formatDate(NOW, 'mm:ss', 'es')).toBe('05:09');
    expect(formatDate(new Date(2025, 0, 1, 0, 0, 0), 'h AP', 'en')).toBe('12 AM');
  });

  it('el texto entre comillas simples sale tal cual', () => {
    expect(formatDate(NOW, "'día' d", 'es')).toBe('día 3');
    expect(formatDate(NOW, "yyyy'T'HH", 'es')).toBe('2025T14');
    expect(formatDate(NOW, "d''M", 'es')).toBe("3'9");
  });
});

describe('resolución', () => {
  it('serie con dígitos del documento, de la opción y desplazada', () => {
    expect(resolveTemplate('{serie}', ctx({ serialDigits: 4 })).text).toBe('0007');
    expect(resolveTemplate('{serie:3}', ctx()).text).toBe('007');
    expect(resolveTemplate('{serial:hex2}', ctx({ serial: 255 })).text).toBe('ff');
    expect(resolveTemplate('{serie@3}', ctx({ serialStep: 5 })).text).toBe('22');
    expect(resolveTemplate('{serie:xyz}', ctx()).issues[0]).toMatchObject({ code: 'badFormat', detail: 'xyz' });
  });

  it('datos del CSV por nombre, sin distinguir mayúsculas, por número y con desplazamiento', () => {
    const c = ctx({ csvRow: 1 });
    expect(resolveTemplate('{csv:Nombre} - {csv:cargo}', c).text).toBe('Luis - Técnico');
    expect(resolveTemplate('{csv:3}', c).text).toBe('A-2');
    expect(resolveTemplate('{csv:Nombre@1}', c).text).toBe('Eva');
    expect(resolveTemplate('{csv:Nombre@-1}', c).text).toBe('Ana');
    expect(resolveTemplate('fila {fila} / {row@1}', c).text).toBe('fila 2 / 3');
    expect(csvColumnIndex(CSV, 'Código')).toBe(2);
    expect(csvColumnIndex(CSV, '9')).toBe(-1);
  });

  it('avisa de columnas que no existen, falta de CSV y fin de los datos, dejando el texto escrito', () => {
    expect(resolveTemplate('{csv:Edad}', ctx())).toEqual({
      text: '{csv:Edad}',
      issues: [{ code: 'noColumn', raw: '{csv:Edad}', detail: 'Edad' }],
    });
    expect(resolveTemplate('{csv:Nombre}', ctx({ csv: null })).issues[0]!.code).toBe('noCsv');
    expect(resolveTemplate('{csv}', ctx()).issues[0]!.code).toBe('noColumnArg');
    const end = resolveTemplate('{csv:Nombre}', ctx({ csvRow: 3 }));
    expect(end.issues[0]).toMatchObject({ code: 'rowEnd', detail: '4' });
    expect(end.text).toBe('{csv:Nombre}');
    expect(resolveTemplate('{nada}', ctx()).issues[0]!.code).toBe('unknown');
  });

  it('fecha, hora y copia con sus formatos por defecto y el idioma', () => {
    expect(resolveTemplate('{fecha} {hora}', ctx()).text).toBe('03/09/2025 14:05');
    expect(resolveTemplate('{date:MMMM} {time:HH}h', ctx({ lang: 'en' })).text).toBe('September 14h');
    expect(resolveTemplate('{copia} {copia@2}', ctx()).text).toBe('1 3');
  });

  it('un texto sin llaves no se toca', () => {
    expect(resolveTemplate('Hola', ctx())).toEqual({ text: 'Hola', issues: [] });
  });

  it('makeContext usa los valores de fábrica si el documento no tiene ajustes', () => {
    const c = makeContext(undefined, null, NOW, 'es');
    expect(c).toMatchObject({ serial: 1, serialStep: 1, serialDigits: 0, csvRow: 0 });
  });
});

describe('copias por fila: plantilla desplazada', () => {
  it('suma k a serie, csv, fila y copia, pero no a fecha y hora', () => {
    expect(shiftTemplate('{csv:Nombre} {serie:3} {fecha} {copia@1}', 2)).toBe('{csv:Nombre@2} {serie:3@2} {fecha} {copia@3}');
    expect(shiftTemplate('Hola', 3)).toBe('Hola');
    expect(shiftTemplate('{serie}', 0)).toBe('{serie}');
  });

  it('cada copia resuelve una fila distinta', () => {
    const base = '{csv:Nombre}';
    const out = [0, 1, 2].map((k) => resolveTemplate(shiftTemplate(base, k), ctx()).text);
    expect(out).toEqual(['Ana', 'Luis', 'Eva']);
  });

  it('copiesPerRow pone las copias en rejilla con ids nuevos y plantillas desplazadas', () => {
    const rect = (id: string): DesignObject => ({
      id, layer: '#ff0000', transform: [1, 0, 0, 1, 0, 0], locked: false, hidden: false, group: null,
      kind: { type: 'vector', paths: [[{ type: 'move_to', to: [0, 0] }, { type: 'line_to', to: [10, 0] }, { type: 'line_to', to: [10, 5] }, { type: 'line_to', to: [0, 5] }, { type: 'close' }]] },
    });
    const d = doc([textObj('t', '{csv:Nombre}'), rect('r')], { csv_path: 'x.csv' });
    const r = copiesPerRow(d, ['t', 'r'], { count: 4, cols: 2, gap: 2 });
    expect(r.created).toHaveLength(6);
    expect(new Set(r.doc.objects.map((o) => o.id)).size).toBe(8);
    const texts = r.doc.objects.filter(isTextObject).map((o) => o.kind.text);
    expect(texts).toEqual(['{csv:Nombre}', '{csv:Nombre@1}', '{csv:Nombre@2}', '{csv:Nombre@3}']);
    const xs = r.doc.objects.filter(isTextObject).map((o) => [o.transform[4], o.transform[5]]);
    // El paso es el tamaño de la caja de la selección más el espacio entre copias.
    const box = unionBounds(d.objects.map(objectBounds))!;
    const [stepX, stepY] = [box[1][0] - box[0][0] + 2, box[1][1] - box[0][1] + 2];
    expect(stepX).toBeGreaterThan(12);
    expect(xs).toEqual([[0, 0], [stepX, 0], [0, -stepY], [stepX, -stepY]]);
    // El original no cambia.
    expect(d.objects).toHaveLength(2);
    expect(copiesPerRow(d, ['t'], { count: 1, cols: 1, gap: 0 }).created).toEqual([]);
  });
});

describe('avance', () => {
  const vt = (patch: Partial<VariableText> = {}): VariableText => ({ ...DEFAULT_VARIABLE_TEXT, ...patch });

  it('suma el paso a la serie y 1 a la fila solo si hay CSV', () => {
    expect(advanceSettings(vt({ serial: 10, serial_step: 5 }))).toMatchObject({ serial: 15, csv_row: 0 });
    expect(advanceSettings(vt({ csv_path: 'a.csv', csv_row: 2 }))).toMatchObject({ serial: 2, csv_row: 3 });
  });

  it('un trabajo con N copias gasta N pasos', () => {
    const d = doc([textObj('a', '{serie}'), textObj('b', '{serie@3} {csv:Nombre@2}')]);
    expect(jobSpans(d)).toEqual({ serial: 4, row: 3 });
    expect(advanceSettings(vt({ csv_path: 'a.csv', serial_step: 2 }), jobSpans(d))).toMatchObject({ serial: 9, csv_row: 3 });
    expect(jobSpans(doc([textObj('a', 'Hola')]))).toEqual({ serial: 1, row: 1 });
  });

  it('la fila se detiene pasada la última para no repetir etiquetas', () => {
    const a = advanceSettings(vt({ csv_path: 'a.csv', csv_row: 2 }), { serial: 1, row: 1 }, 3);
    expect(a.csv_row).toBe(3);
    expect(atCsvEnd(a, 3)).toBe(true);
    expect(advanceSettings(a, { serial: 1, row: 1 }, 3).csv_row).toBe(3);
    expect(atCsvEnd(vt({ csv_row: 9 }), 3)).toBe(false); // sin CSV no hay final
  });

  describe('al terminar un trabajo', () => {
    beforeEach(() => {
      useVariablesUi.setState({ csv: EMPTY_CSV });
    });

    it('avanza con un paso de deshacer; el documento guardado sigue con las plantillas', () => {
      useStore.getState().replaceDocument(doc([textObj('t', 'N° {serie:4}')], { serial: 5, serial_step: 2 }), null);
      expect(advanceAfterJob(useStore)).toBe(true);
      const s = useStore.getState();
      expect(s.doc.variable_text).toMatchObject({ serial: 7 });
      expect(s.doc.objects.filter(isTextObject)[0]!.kind.text).toBe('N° {serie:4}');
      expect(s.dirty).toBe(true);
      s.undo();
      expect(useStore.getState().doc.variable_text).toMatchObject({ serial: 5 });
    });

    it('no avanza si está apagado ni si no hay textos con variables', () => {
      useStore.getState().replaceDocument(doc([textObj('t', '{serie}')], { advance_after_job: false }), null);
      expect(advanceAfterJob(useStore)).toBe(false);
      useStore.getState().replaceDocument(doc([textObj('t', 'Fijo')], { serial: 5 }), null);
      expect(advanceAfterJob(useStore)).toBe(false);
      expect(useStore.getState().doc.variable_text).toMatchObject({ serial: 5 });
    });

    it('avisa al llegar al final del CSV', () => {
      useStore.getState().replaceDocument(doc([textObj('t', '{csv:Nombre}')], { csv_path: 'a.csv', csv_row: 2 }), null);
      useVariablesUi.setState({ csv: { path: 'a.csv', table: CSV, error: null, loading: false } });
      const before = useStore.getState().notices?.length ?? 0;
      advanceAfterJob(useStore);
      expect(useStore.getState().doc.variable_text?.csv_row).toBe(3);
      expect(useStore.getState().notices?.length ?? 0).toBeGreaterThan(before);
    });

    it('el evento finished del trabajo avanza; cancelled y error no', async () => {
      const { createAppStore } = await import('@/store');
      const store = createAppStore();
      store.getState().replaceDocument(doc([textObj('t', '{serie}')], { serial: 10 }), null);
      const job = (state: 'finished' | 'cancelled' | 'error') => store.getState().onJob({ state, message: '', position_lost: false });
      job('cancelled');
      job('error');
      expect(store.getState().doc.variable_text?.serial).toBe(10);
      job('finished');
      expect(store.getState().doc.variable_text?.serial).toBe(11);
    });

    it('editVariableText cambia un campo con historial y no crea pasos sin cambios', () => {
      useStore.getState().replaceDocument(doc([]), null);
      editVariableText({ serial: 42 }, 'serial');
      expect(useStore.getState().doc.variable_text?.serial).toBe(42);
      const steps = useStore.getState().past.length;
      editVariableText({ serial: 42 }, 'serial');
      expect(useStore.getState().past.length).toBe(steps);
    });
  });
});

describe('resolución al enviar', () => {
  const fake = () => {
    const calls: TextObject[] = [];
    return {
      calls,
      deps: {
        api: () => ({
          textOutline: async ({ text, path }: { text: TextObject; path: unknown }) => {
            calls.push(text);
            return mockTextOutline(text, path as never);
          },
          readCsv: async ({ required }: { path: string; required?: string[] }) => {
            const missing = (required ?? []).filter((c) => !CSV.headers.includes(c) && !/^\d+$/.test(c));
            if (missing.length) throw new Error(`faltan columnas: ${missing.join(', ')}`);
            return CSV;
          },
        }),
        fonts: async () => [],
        now: () => NOW,
      },
    };
  };

  it('regenera el contorno con el texto resuelto y deja intacto el documento original', async () => {
    const d = doc([textObj('a', 'Ficha {serie:3}'), textObj('b', 'Fijo')], { serial: 12 });
    const f = fake();
    const out = await resolveDocForOutput(d, f.deps);
    expect(f.calls.map((c) => c.text)).toEqual(['Ficha 012']);
    const a = out.objects.find((o) => o.id === 'a')!;
    expect(isTextObject(a) && a.kind.outline.length).toBe('Ficha 012'.replace(/\s/g, '').length);
    // El texto enviado conserva la plantilla; el original no cambia; lo no variable es el mismo objeto.
    expect(isTextObject(a) && a.kind.text).toBe('Ficha {serie:3}');
    expect(isTextObject(d.objects[0]!) && d.objects[0]!.kind.outline).toEqual([]);
    expect(out.objects[1]).toBe(d.objects[1]);
  });

  it('sin textos variables devuelve el mismo documento y no pide nada', async () => {
    const d = doc([textObj('b', 'Fijo')]);
    const f = fake();
    expect(await resolveDocForOutput(d, f.deps)).toBe(d);
    expect(f.calls).toHaveLength(0);
  });

  it('lee el CSV del disco con las columnas usadas y toma la fila actual', async () => {
    const d = doc([textObj('a', '{csv:Nombre} / {csv:Cargo}')], { csv_path: 'x.csv', csv_row: 1 });
    const f = fake();
    await resolveDocForOutput(d, f.deps);
    expect(f.calls[0]!.text).toBe('Luis / Técnico');
  });

  it('no reenvía al motor un contorno que ya corresponde a los valores de ahora', async () => {
    const d = doc([textObj('a', 'N {serie}')], { serial: 3 });
    const f = fake();
    const first = await resolveDocForOutput(d, f.deps);
    expect(f.calls).toHaveLength(1);
    expect(await resolveDocForOutput(first, f.deps)).toBe(first);
    expect(f.calls).toHaveLength(1);
    // Con otro número sí se regenera.
    await resolveDocForOutput({ ...first, variable_text: { ...DEFAULT_VARIABLE_TEXT, serial: 4 } }, f.deps);
    expect(f.calls).toHaveLength(2);
    expect(f.calls[1]!.text).toBe('N 4');
  });

  it('bloquea el envío si algo no se resuelve, nombrando todos los problemas', async () => {
    const f = fake();
    const bad = doc([textObj('a', '{csv:Edad}'), textObj('b', '{foo}')], { csv_path: 'x.csv' });
    await expect(resolveDocForOutput(bad, f.deps)).rejects.toThrow(/faltan columnas: Edad/);
    const unknown = doc([textObj('b', '{foo} y {csv}')]);
    const err = await resolveDocForOutput(unknown, f.deps).catch((e: unknown) => e);
    expect(err).toBeInstanceOf(VariableError);
    expect((err as VariableError).issues.map((i) => i.code)).toEqual(['unknown', 'noColumnArg']);
    expect(f.calls).toHaveLength(0);
    // Se acabó el CSV: no se graba la última etiqueta otra vez.
    const end = doc([textObj('a', '{csv:Nombre}')], { csv_path: 'x.csv', csv_row: 3 });
    await expect(resolveDocForOutput(end, f.deps)).rejects.toBeInstanceOf(VariableError);
  });

  it('un texto con solo llaves escapadas sale con las llaves sencillas', async () => {
    const f = fake();
    await resolveDocForOutput(doc([textObj('a', '{{A}}')]), f.deps);
    expect(f.calls[0]!.text).toBe('{A}');
  });
});

describe('vista previa y envío con texto variable', () => {
  it('el trabajo se calcula con el texto ya resuelto y el documento sigue con la plantilla', async () => {
    const { setApi } = await import('@/api');
    const { createAppStore } = await import('@/store');
    const base = createMockApi({ latencyMs: 0 });
    const seen: Document[] = [];
    setApi({ ...base, computeJob: (args) => (seen.push(args.doc), base.computeJob(args)) });
    const store = createAppStore();
    store.getState().replaceDocument(doc([textObj('t', 'Pieza {serie:3}')], { serial: 7 }), null);
    await store.getState().computePreview();
    expect(store.getState().previewStatus).toBe('ready');
    const sent = seen[0]!.objects[0]!;
    expect(isTextObject(sent) && sent.kind.text).toBe('Pieza {serie:3}');
    expect(isTextObject(sent) && sent.kind.outline.length).toBe('Pieza007'.length);
    const kept = store.getState().doc.objects[0]!;
    expect(isTextObject(kept) && kept.kind.outline).toEqual([]);
  });

  it('un CSV que se acabó deja el error visible en la vista previa y no calcula', async () => {
    const { setApi } = await import('@/api');
    const { createAppStore } = await import('@/store');
    const base = createMockApi({ latencyMs: 0 });
    let computed = 0;
    setApi({ ...base, computeJob: (a) => (computed++, base.computeJob(a)) });
    const store = createAppStore();
    store.getState().replaceDocument(doc([textObj('t', '{csv:Nombre}')], { csv_path: 'etiquetas.csv', csv_row: 99 }), null);
    await store.getState().computePreview();
    expect(store.getState().previewStatus).toBe('error');
    expect(store.getState().previewError).toMatch(/fila 100/);
    expect(computed).toBe(0);
  });
});

describe('lienzo: firma del contorno y sincronización', () => {
  beforeEach(() => {
    setLanguage('es');
    useVariablesUi.setState({ csv: EMPTY_CSV, showTemplate: false, clock: NOW.getTime() });
  });
  afterEach(() => setLanguage('es'));

  it('la firma cambia con los valores y con "ver plantilla"', () => {
    const t = textObj('t', 'N {serie}');
    if (!isTextObject(t)) throw new Error();
    const sig = (serial: number) => {
      const r = resolveText(t, doc([t], { serial }));
      return r.ok ? r.sig : '';
    };
    expect(sig(1)).not.toBe(sig(2));
    expect(sig(1)).toBe(sig(1));
    const resolved = resolveText(t, doc([t], { serial: 1 }));
    expect(resolved.ok && resolved.text).toBe('N 1');
    useVariablesUi.setState({ showTemplate: true });
    const shown = resolveText(t, doc([t], { serial: 1 }));
    expect(shown.ok && shown.text).toBe('N {serie}');
  });

  it('un texto sin llaves no trae texto resuelto (camino de siempre)', () => {
    const t = textObj('t', 'Hola');
    if (!isTextObject(t)) throw new Error();
    expect(resolveText(t, doc([t]))).toEqual({ ok: true, sig: expect.any(String), path: null });
  });

  it('displayContext usa el CSV cargado solo si es el del documento', () => {
    const d = doc([], { csv_path: 'a.csv' });
    expect(displayContext(d).csv).toBeNull();
    useVariablesUi.setState({ csv: { path: 'otro.csv', table: CSV, error: null, loading: false } });
    expect(displayContext(d).csv).toBeNull();
    useVariablesUi.setState({ csv: { path: 'a.csv', table: CSV, error: null, loading: false } });
    expect(displayContext(d).csv).toBe(CSV);
  });

  describe('con el sincronizador de contornos', () => {
    let sync: ReturnType<typeof startOutlineSync>;
    const calls: string[] = [];
    beforeEach(() => {
      vi.useFakeTimers();
      calls.length = 0;
      useStore.getState().replaceDocument(doc([textObj('t', 'N {serie}')], { serial: 1 }), null);
      const mock = createMockApi({ latencyMs: 0 });
      sync = startOutlineSync({
        api: () => ({
          textOutline: async (a) => {
            calls.push(a.text.text);
            return mock.textOutline(a);
          },
        }),
        fonts: async () => [],
        debounceMs: 10,
      });
    });
    afterEach(() => {
      sync.stop();
      vi.useRealTimers();
    });

    it('el contorno sigue a la serie, a "ver plantilla" y al CSV', async () => {
      await vi.advanceTimersByTimeAsync(50);
      expect(calls).toEqual(['N 1']);
      useStore.getState().applyDocChange('variables.history.edit', (d) => ({ ...d, variable_text: { ...DEFAULT_VARIABLE_TEXT, serial: 2 } }));
      await vi.advanceTimersByTimeAsync(50);
      expect(calls).toEqual(['N 1', 'N 2']);
      useVariablesUi.getState().setShowTemplate(true);
      await vi.advanceTimersByTimeAsync(50);
      expect(calls.at(-1)).toBe('N {serie}');
      // El texto del documento sigue siendo la plantilla en todo momento.
      const obj = useStore.getState().doc.objects[0]!;
      expect(isTextObject(obj) && obj.kind.text).toBe('N {serie}');
    });

    it('un contorno guardado con otros valores no se da por bueno al abrir el archivo', async () => {
      const stale = textObj('u', 'N {serie}');
      if (!isTextObject(stale)) throw new Error();
      const withOutline = { ...stale, kind: { ...stale.kind, outline: mockTextOutline(textKind('N 99'), null).paths } };
      useStore.getState().replaceDocument(doc([withOutline], { serial: 1 }), null);
      await vi.advanceTimersByTimeAsync(50);
      expect(calls.at(-1)).toBe('N 1');
    });
  });
});

describe('menú Insertar variable', () => {
  it('ofrece los nombres del idioma y las columnas del CSV', () => {
    const es = insertOptions('es', ['Nombre', 'A@B', '2']);
    expect(es.find((o) => o.id === 'serial')!.snippet).toBe('{serie}');
    expect(es.find((o) => o.id === 'csv:0')!.snippet).toBe('{csv:Nombre}');
    // Un nombre que rompería la sintaxis, o que parece número, se inserta por su posición.
    expect(es.find((o) => o.id === 'csv:1')!.snippet).toBe('{csv:2}');
    expect(es.find((o) => o.id === 'csv:2')!.snippet).toBe('{csv:3}');
    expect(insertOptions('en', []).find((o) => o.id === 'time')!.snippet).toBe('{time}');
    expect(insertOptions('en', []).some((o) => o.id === 'csv')).toBe(true);
  });

  it('todo lo que inserta se entiende al resolver', () => {
    for (const lang of ['es', 'en'] as const) {
      for (const o of insertOptions(lang, ['Nombre'])) {
        if (o.id === 'braceOpen' || o.id === 'braceClose') continue;
        expect(resolveTemplate(o.snippet.replace('{csv:Columna}', '{csv:Nombre}').replace('{csv:Column}', '{csv:Nombre}'), ctx({ lang })).issues, o.id).toEqual([]);
      }
    }
    expect(resolveTemplate('{{', ctx()).text).toBe('{');
  });

  it('inserta en el cursor o reemplaza la selección', () => {
    expect(insertSnippet('Hola ', 5, 5, '{serie}')).toEqual({ text: 'Hola {serie}', caret: 12 });
    expect(insertSnippet('Hola mundo', 5, 10, '{serie}')).toEqual({ text: 'Hola {serie}', caret: 12 });
    expect(insertSnippet('ab', 99, 99, 'x').text).toBe('abx');
  });
});

describe('tabla del CSV', () => {
  it('la ventana mantiene a la vista la fila actual', () => {
    expect(windowStart(0, 10)).toBe(0);
    expect(windowStart(500, 1000)).toBe(496);
    expect(windowStart(990, 1000)).toBe(900);
  });

  it('resalta la fila actual', async () => {
    const { renderToStaticMarkup } = await import('react-dom/server');
    const { createElement } = await import('react');
    const html = renderToStaticMarkup(createElement(CsvTableView, { table: CSV, current: 1, onPick: () => undefined }));
    expect(html.match(/aria-current="true"/g)).toHaveLength(1);
    expect(html).toMatch(/<tr class="vt-current" aria-current="true"><td class="num">2<\/td><td>Luis<\/td>/);
    expect(renderToStaticMarkup(createElement(CsvTableView, { table: null, current: 0, onPick: () => undefined }))).toContain('vt-empty');
  });
});
