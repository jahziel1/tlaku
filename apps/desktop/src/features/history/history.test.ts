import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setApi } from '@/api';
import { createMockApi } from '@/api/mock';
import type { Document } from '@/api/types';
import { useStore } from '@/store';
import type { HistoryEntry } from '@/store/document';
import { importPaths, openProject, openRecent, saveProject } from './files';
import { baseName, extensionOf, isImportable, withExtension } from './paths';
import { askChoice, usePrompts } from './prompt';
import { addRecent, MAX_RECENT, parseRecents, removeRecent, useRecents } from './recent';
import { buildTimeline } from './timeline';
import { guardReducer, guardUnsaved, type GuardChoice, type GuardIo, type GuardState } from './unsaved';
import { canGuardClose, canSetNativeTitle, declaredPermissions, documentTitle } from './windowEvents';

const s = () => useStore.getState();

describe('archivos recientes', () => {
  it('pone el más nuevo al frente, sin repetir y con tope', () => {
    expect(addRecent(['a', 'b', 'c'], 'b')).toEqual(['b', 'a', 'c']);
    const many = Array.from({ length: 15 }, (_, i) => `p${i}`).reduce<string[]>((l, p) => addRecent(l, p), []);
    expect(many).toHaveLength(MAX_RECENT);
    expect(many[0]).toBe('p14');
    expect(removeRecent(['a', 'b'], 'a')).toEqual(['b']);
  });

  it('tolera datos guardados rotos', () => {
    expect(parseRecents(null)).toEqual([]);
    expect(parseRecents('{no es json')).toEqual([]);
    expect(parseRecents('{"a":1}')).toEqual([]);
    expect(parseRecents(JSON.stringify(['a', 3, '', 'a', 'b']))).toEqual(['a', 'b']);
  });

  describe('persistencia en localStorage', () => {
    const data = new Map<string, string>();
    beforeEach(() => {
      data.clear();
      vi.stubGlobal('localStorage', {
        getItem: (k: string) => data.get(k) ?? null,
        setItem: (k: string, v: string) => void data.set(k, v),
      });
      useRecents.getState().reload();
    });
    afterEach(() => vi.unstubAllGlobals());

    it('guarda y recarga la lista', () => {
      useRecents.getState().add('/a/uno.tlaku');
      useRecents.getState().add('/a/dos.tlaku');
      expect(JSON.parse(data.get('tlaku.recent')!)).toEqual(['/a/dos.tlaku', '/a/uno.tlaku']);
      useRecents.setState({ paths: [] });
      useRecents.getState().reload();
      expect(useRecents.getState().paths).toEqual(['/a/dos.tlaku', '/a/uno.tlaku']);
      useRecents.getState().clear();
      expect(JSON.parse(data.get('tlaku.recent')!)).toEqual([]);
    });

    it('sigue funcionando sin almacenamiento (private mode, bloqueado)', () => {
      vi.stubGlobal('localStorage', {
        getItem: () => {
          throw new Error('bloqueado');
        },
        setItem: () => {
          throw new Error('bloqueado');
        },
      });
      expect(() => useRecents.getState().reload()).not.toThrow();
      expect(() => useRecents.getState().add('/x.tlaku')).not.toThrow();
      expect(useRecents.getState().paths).toEqual(['/x.tlaku']);
    });
  });
});

describe('flujo de cambios sin guardar (máquina de estados)', () => {
  const idle: GuardState = { phase: 'idle' };

  it('sin cambios la acción continúa sin preguntar', () => {
    expect(guardReducer(idle, { type: 'request', intent: 'new', dirty: false })).toEqual({ phase: 'proceed', intent: 'new' });
  });

  it('con cambios pregunta y cada respuesta lleva a su destino', () => {
    const asking = guardReducer(idle, { type: 'request', intent: 'open', dirty: true });
    expect(asking).toEqual({ phase: 'asking', intent: 'open' });
    expect(guardReducer(asking, { type: 'choose', choice: 'discard' }).phase).toBe('proceed');
    expect(guardReducer(asking, { type: 'choose', choice: 'cancel' }).phase).toBe('cancelled');
    const saving = guardReducer(asking, { type: 'choose', choice: 'save' });
    expect(saving.phase).toBe('saving');
    expect(guardReducer(saving, { type: 'saved' }).phase).toBe('proceed');
    expect(guardReducer(saving, { type: 'saveAborted' }).phase).toBe('cancelled');
  });

  it('ignora eventos que no corresponden al estado', () => {
    expect(guardReducer(idle, { type: 'choose', choice: 'save' })).toBe(idle);
    expect(guardReducer(idle, { type: 'saved' })).toBe(idle);
    const asking: GuardState = { phase: 'asking', intent: 'close' };
    expect(guardReducer(asking, { type: 'request', intent: 'new', dirty: true })).toBe(asking);
    expect(guardReducer(asking, { type: 'saved' })).toBe(asking);
  });

  function makeIo(opts: { dirty: boolean; choice?: GuardChoice; saves?: boolean }) {
    const log: string[] = [];
    const io: GuardIo = {
      isDirty: () => opts.dirty,
      ask: async () => {
        log.push('ask');
        return opts.choice ?? 'cancel';
      },
      save: async () => {
        log.push('save');
        return opts.saves ?? true;
      },
    };
    return { io, log };
  }

  it('guardUnsaved: limpio continúa, descartar continúa, cancelar frena', async () => {
    const clean = makeIo({ dirty: false });
    expect(await guardUnsaved('new', clean.io)).toBe(true);
    expect(clean.log).toEqual([]);
    expect(await guardUnsaved('new', makeIo({ dirty: true, choice: 'discard' }).io)).toBe(true);
    expect(await guardUnsaved('new', makeIo({ dirty: true, choice: 'cancel' }).io)).toBe(false);
  });

  it('guardUnsaved: guardar continúa solo si el guardado se completó', async () => {
    const ok = makeIo({ dirty: true, choice: 'save', saves: true });
    expect(await guardUnsaved('close', ok.io)).toBe(true);
    expect(ok.log).toEqual(['ask', 'save']);
    expect(await guardUnsaved('close', makeIo({ dirty: true, choice: 'save', saves: false }).io)).toBe(false);
  });

  it('guardUnsaved: mientras hay una pregunta abierta, otra petición se rechaza', async () => {
    let release!: (c: GuardChoice) => void;
    const first: GuardIo = {
      isDirty: () => true,
      ask: () => new Promise<GuardChoice>((r) => (release = r)),
      save: async () => true,
    };
    const pending = guardUnsaved('close', first);
    expect(await guardUnsaved('new', makeIo({ dirty: false }).io)).toBe(false);
    release('discard');
    expect(await pending).toBe(true);
    expect(await guardUnsaved('new', makeIo({ dirty: false }).io)).toBe(true); // se liberó
  });
});

describe('preguntas modales', () => {
  it('resuelve con la opción elegida y encola las siguientes', async () => {
    const mk = (title: string) => askChoice({ title, message: '', choices: [{ id: 'ok', label: 'Ok' }], cancelId: 'ok' });
    const a = mk('a');
    const b = mk('b');
    expect(usePrompts.getState().current?.title).toBe('a');
    usePrompts.getState().answer('ok');
    await a;
    expect(usePrompts.getState().current?.title).toBe('b');
    usePrompts.getState().answer('ok');
    await b;
    expect(usePrompts.getState().current).toBeNull();
  });
});

describe('línea de tiempo del historial', () => {
  const entry = (label: HistoryEntry['label'], at: number): HistoryEntry => ({ label, doc: {} as Document, at });

  it('incluye el estado inicial y marca el actual y lo deshecho', () => {
    const rows = buildTimeline([entry('history.import', 1), entry('history.move', 2)], [entry('history.delete', 3)]);
    expect(rows.map((r) => [r.position, r.status])).toEqual([
      [0, 'applied'],
      [1, 'applied'],
      [2, 'current'],
      [3, 'undone'],
    ]);
    expect(rows[0]!.label).toBe('history.initial');
    expect(rows[0]!.at).toBeNull();
    expect(rows[3]!.at).toBe(3);
  });

  it('sin pasos, el estado inicial es el actual', () => {
    expect(buildTimeline([], []).map((r) => r.status)).toEqual(['current']);
  });

  it('las posiciones coinciden con jumpTo', () => {
    useStore.getState().newDocument();
    s().applyImport({ objects: [], new_layers: [], warnings: [] });
    s().ensureLayer('#ff0000');
    s().ensureLayer('#00ff00');
    const rows = buildTimeline(s().past, s().future);
    expect(rows).toHaveLength(s().past.length + 1);
    s().jumpTo(rows[1]!.position);
    expect(s().past).toHaveLength(1);
  });
});

describe('rutas y formatos', () => {
  it('reconoce los formatos de importación sin importar mayúsculas', () => {
    for (const f of ['a.svg', 'A.DXF', 'x.png', 'f.jpg', 'f.JPEG', 'b.bmp']) expect(isImportable(f), f).toBe(true);
    for (const f of ['a.pdf', 'a', '.svg', 'a.tlaku']) expect(isImportable(f), f).toBe(false);
  });
  it('nombre, extensión y extensión por defecto', () => {
    expect(baseName('C:\\x\\y\\pieza.svg')).toBe('pieza.svg');
    expect(baseName('/a/b/c.tlaku')).toBe('c.tlaku');
    expect(extensionOf('/a/b.c/archivo')).toBe('');
    expect(withExtension('/a/proyecto', 'tlaku')).toBe('/a/proyecto.tlaku');
    expect(withExtension('/a/proyecto.tlaku', 'tlaku')).toBe('/a/proyecto.tlaku');
  });
});

describe('título de la ventana y permisos', () => {
  it('marca los cambios sin guardar', () => {
    expect(documentTitle(null, false)).toBe('Sin título — Tlaku');
    expect(documentTitle('/a/placa.tlaku', true)).toBe('● placa.tlaku — Tlaku');
  });

  it('solo se protege el cierre si el permiso de destruir la ventana está declarado', () => {
    const none = declaredPermissions({ a: { permissions: ['core:event:allow-listen', 'dialog:allow-open'] } });
    expect(canGuardClose(none)).toBe(false);
    expect(canSetNativeTitle(none)).toBe(false);
    const all = declaredPermissions({ a: { permissions: ['core:window:allow-destroy', { identifier: 'core:window:allow-set-title' }] } });
    expect(canGuardClose(all)).toBe(true);
    expect(canSetNativeTitle(all)).toBe(true);
    expect(declaredPermissions({ a: 7, b: null, c: { permissions: 'x' } }).size).toBe(0);
  });

  it('la capacidad real de src-tauri declara lo que usa el shell (si se quita, el cierre protegido se apaga)', () => {
    expect(canGuardClose()).toBe(true);
    expect(canSetNativeTitle()).toBe(true);
  });
});

describe('importar, abrir y guardar con la API simulada', () => {
  beforeEach(() => {
    setApi(createMockApi({ latencyMs: 0 }));
    s().newDocument();
    for (const n of s().notices) s().dismissNotice(n.id);
    useRecents.setState({ paths: [] });
  });

  it('las advertencias del importador salen en un aviso persistente con su detalle', async () => {
    const n = await importPaths(['ejemplos/placa.svg']);
    expect(n).toBeGreaterThan(0);
    const warn = s().notices.find((x) => x.level === 'warning');
    expect(warn).toBeDefined();
    expect(warn!.sticky).toBe(true);
    expect(warn!.details!.length).toBeGreaterThan(0);
    expect(warn!.text).toContain('placa.svg');
  });

  it('lo importado queda seleccionado (aunque se renombren los ids)', async () => {
    await importPaths(['a.png']);
    const first = [...s().selectedIds];
    expect(first.length).toBeGreaterThan(0);
    await importPaths(['b.png']); // mismos ids del mock: se renombran
    expect(s().selectedIds).toHaveLength(first.length);
    expect(s().selectedIds.every((id) => !first.includes(id))).toBe(true);
    for (const id of s().selectedIds) expect(s().doc.objects.some((o) => o.id === id)).toBe(true);
  });

  it('un formato no admitido se avisa y no importa nada', async () => {
    expect(await importPaths(['plano.pdf'])).toBe(0);
    expect(s().notices.at(-1)).toMatchObject({ level: 'error' });
    expect(s().doc.objects).toHaveLength(0);
  });

  it('guarda con la extensión .tlaku, limpia el indicador y recuerda el archivo', async () => {
    await importPaths(['a.png']);
    expect(s().dirty).toBe(true);
    expect(await saveProject(true)).toBe(true);
    expect(s().dirty).toBe(false);
    expect(s().filePath).toMatch(/\.tlaku$/);
    expect(useRecents.getState().paths[0]).toBe(s().filePath);
  });

  it('abrir con cambios y cancelar la pregunta no toca el documento', async () => {
    await importPaths(['a.png']);
    const before = s().doc;
    const pending = openProject('ejemplos/placa.tlaku');
    await vi.waitFor(() => expect(usePrompts.getState().current).not.toBeNull());
    usePrompts.getState().answer('cancel');
    expect(await pending).toBe(false);
    expect(s().doc).toBe(before);
  });

  it('abrir descartando los cambios reemplaza el documento y limpia el historial', async () => {
    await importPaths(['a.png']);
    const pending = openProject('ejemplos/placa.tlaku');
    await vi.waitFor(() => expect(usePrompts.getState().current).not.toBeNull());
    usePrompts.getState().answer('discard');
    expect(await pending).toBe(true);
    expect(s().dirty).toBe(false);
    expect(s().past).toHaveLength(0);
    expect(s().filePath).toBe('ejemplos/placa.tlaku');
  });

  it('un reciente que ya no se puede abrir se quita de la lista y se avisa', async () => {
    const api = createMockApi({ latencyMs: 0 });
    setApi({ ...api, loadProject: () => Promise.reject('No existe el archivo') });
    useRecents.setState({ paths: ['/x/perdido.tlaku', '/x/otro.tlaku'] });
    expect(await openRecent('/x/perdido.tlaku')).toBe(false);
    expect(useRecents.getState().paths).toEqual(['/x/otro.tlaku']);
    expect(s().notices.at(-1)).toMatchObject({ level: 'error' });
    expect(s().notices.at(-1)!.text).toContain('No existe el archivo');
  });
});
