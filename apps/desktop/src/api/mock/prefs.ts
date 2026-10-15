import type { LaserApi, PrefsBundle, PrefsFile, RecoveryInfo, TemplateInfo } from '../api';
import type { Document } from '../types';

type Lag = <T>(value: () => T) => Promise<T>;

/**
 * Preferencias y autoguardado en memoria (modo mock). Lo exportado se recuerda por ruta para poder
 * importarlo de vuelta; una ruta que contiene `no-existe` simula un archivo que ya no está.
 */
export function mockPrefs(lag: Lag): Pick<
  LaserApi,
  'loadPreferences' | 'savePreferences' | 'exportPreferences' | 'importPreferences' | 'pathsExist' | 'autosaveWrite' | 'autosaveList' | 'autosaveRead' | 'autosaveDiscard' | 'listTemplates' | 'saveTemplate' | 'loadTemplate' | 'deleteTemplate' | 'newWindow'
> {
  let stored: PrefsFile | null = null;
  const bundles = new Map<string, PrefsBundle>();
  const copies = new Map<string, { info: RecoveryInfo; doc: Document }>();
  const templates = new Map<string, { info: TemplateInfo; doc: Document }>();

  return {
    loadPreferences: () => lag(() => (stored ? structuredClone(stored) : null)),
    savePreferences: ({ data }) =>
      lag(() => {
        stored = structuredClone(data);
      }),
    exportPreferences: ({ path, bundle }) =>
      lag(() => {
        bundles.set(path, structuredClone(bundle));
      }),
    importPreferences: ({ path }) =>
      lag(() => {
        const bundle = bundles.get(path);
        if (!bundle) throw new Error(`${path}: no existe o no es un paquete de preferencias.`);
        return structuredClone(bundle);
      }),
    pathsExist: ({ paths }) => lag(() => paths.map((p) => !p.includes('no-existe'))),
    autosaveWrite: ({ id, name, path, doc }) =>
      lag(() => {
        const info: RecoveryInfo = { id, name, path, saved_at_ms: Date.now(), size_bytes: JSON.stringify(doc).length };
        copies.set(id, { info, doc: structuredClone(doc) });
        return info;
      }),
    autosaveList: () => lag(() => [...copies.values()].map((c) => c.info).sort((a, b) => b.saved_at_ms - a.saved_at_ms)),
    autosaveRead: ({ id }) =>
      lag(() => {
        const copy = copies.get(id);
        if (!copy) throw new Error(`No hay una copia de recuperación «${id}».`);
        return structuredClone(copy.doc);
      }),
    autosaveDiscard: ({ id }) =>
      lag(() => {
        copies.delete(id);
      }),
    listTemplates: () =>
      lag(() => [...templates.values()].map((t) => t.info).sort((a, b) => a.name.localeCompare(b.name))),
    saveTemplate: ({ name, doc }) =>
      lag(() => {
        const clean = name.trim();
        if (!clean || /[/\\:*?"<>|]/.test(clean)) throw new Error(`El nombre «${name}» no es válido para una plantilla.`);
        const info: TemplateInfo = { name: clean, saved_at_ms: Date.now(), size_bytes: JSON.stringify(doc).length };
        templates.set(clean, { info, doc: structuredClone(doc) });
        return info;
      }),
    loadTemplate: ({ name }) =>
      lag(() => {
        const found = templates.get(name);
        if (!found) throw new Error(`No existe la plantilla «${name}».`);
        return structuredClone(found.doc);
      }),
    newWindow: (args) =>
      lag(() => {
        // En el navegador (modo mock) una pestaña nueva hace de ventana nueva.
        if (typeof window !== 'undefined' && typeof window.open === 'function') {
          const url = new URL(window.location.href);
          url.search = args?.handoff ? `?handoff=${args.handoff}` : '';
          window.open(url.toString(), '_blank');
        }
        return 'doc-mock';
      }),
    deleteTemplate: ({ name }) =>
      lag(() => {
        templates.delete(name);
      }),
  };
}
