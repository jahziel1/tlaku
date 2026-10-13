import type { LaserApi } from '../api';
import type { ImportResult } from '../types';
import { sampleLayers, sampleObjects } from './sample';

type Lag = <T>(value: () => T) => Promise<T>;

/** Extensión en minúsculas, sin punto. */
const extOf = (path: string): string => /\.([^./\\]+)$/.exec(path)?.[1]?.toLowerCase() ?? '';

/**
 * Importar con opciones, exportar e imprimir simulados: no hay disco, así que importar devuelve
 * el diseño de ejemplo (escalado y agrupado según las opciones) y exportar devuelve un resumen
 * plausible sin escribir nada.
 */
export function mockFileio(lag: Lag): Pick<LaserApi, 'importFileWith' | 'exportDesign' | 'printDesign'> {
  return {
    importFileWith: ({ path, options }) =>
      lag((): ImportResult => {
        const scale = (options.scale_percent ?? 100) / 100;
        const group = options.group ? 'grp-mock' : null;
        const target = options.target_layer ?? null;
        const objects = sampleObjects().map((o) => {
          const base = { ...o, group: group ?? o.group, layer: target && o.kind.type !== 'image' ? target : o.layer };
          return scale === 1 ? base : { ...base, transform: [o.transform[0] * scale, o.transform[1] * scale, o.transform[2] * scale, o.transform[3] * scale, o.transform[4] * scale, o.transform[5] * scale] as typeof o.transform };
        });
        const warnings: string[] = [];
        if (extOf(path) === 'pdf' || extOf(path) === 'ai') warnings.push('Se omitieron 2 imagen(es) incrustada(s) del PDF: solo se importan los vectores.');
        if (['plt', 'hpgl', 'hpg'].includes(extOf(path))) warnings.push('HPGL: se supuso 40 unidades por mm (0,025 mm).');
        return { objects, new_layers: sampleLayers(), warnings };
      }),

    exportDesign: ({ doc, selection, path }) =>
      lag(() => {
        const count = selection ? selection.length : doc.objects.length;
        if (count === 0) throw new Error('El diseño no tiene nada que exportar.');
        return { path, bytes: 1024 * Math.max(1, count), size_mm: [120, 80] as [number, number], warnings: [] };
      }),

    printDesign: ({ doc, selection }) =>
      lag(() => {
        const count = selection ? selection.length : doc.objects.length;
        if (count === 0) throw new Error('El diseño no tiene nada que exportar.');
        return { path: '/tmp/tlaku-impresion.pdf', bytes: 2048, size_mm: [120, 80] as [number, number], warnings: [] };
      }),
  };
}
