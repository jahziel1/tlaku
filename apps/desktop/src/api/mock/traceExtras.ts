import type { LaserApi } from '../api';
import type { ImportResult } from '../types';
import { polylineBez } from '@/lib/bezpath';
import type { UserArt, UserComponent } from '@/features/art/types';

type Lag = <T>(value: () => T) => Promise<T>;

/** Resolución mínima y máxima que acepta el motor para convertir vectores en imagen. */
const DPI_RANGE = [10, 2400] as const;

/**
 * Simulados de "convertir a mapa de bits", "guardar imagen procesada" y la biblioteca de arte: no
 * hay disco ni rasterizador, así que devuelven resultados plausibles.
 */
export function mockTraceExtras(
  lag: Lag,
): Pick<
  LaserApi,
  'vectorsToBitmap' | 'saveProcessedImage' | 'artImportSvg' | 'artListUser' | 'artAddUser' | 'artRemoveUser' | 'artSaveUser' | 'componentListUser' | 'componentSaveUser' | 'componentRenameUser' | 'componentRemoveUser'
> {
  let user: UserArt[] = [];
  let components: UserComponent[] = [];
  return {
    vectorsToBitmap: ({ doc, selection, dir, stem, options }) =>
      lag(() => {
        if (!(options.dpi >= DPI_RANGE[0] && options.dpi <= DPI_RANGE[1])) {
          throw new Error(`Resolución inválida: ${options.dpi} ppp (entre 10 y 2400).`);
        }
        const picked = doc.objects.filter((o) => selection.includes(o.id) && o.kind.type !== 'image');
        if (picked.length === 0) throw new Error('La selección no tiene formas vectoriales que convertir.');
        // Caja aproximada: la de los puntos de control de las formas.
        const pts = picked.flatMap((o) =>
          o.kind.type === 'vector'
            ? o.kind.paths.flatMap((p) => p.flatMap((s) => (s.type === 'close' ? [] : [[s.to[0] + o.transform[4], s.to[1] + o.transform[5]]])))
            : [[o.transform[4], o.transform[5]]],
        );
        const xs = pts.map((p) => p[0]!);
        const ys = pts.map((p) => p[1]!);
        const w = Math.max(1, Math.max(...xs) - Math.min(...xs));
        const h = Math.max(1, Math.max(...ys) - Math.min(...ys));
        const px = 25.4 / options.dpi;
        const width_px = Math.ceil(w / px);
        const height_px = Math.ceil(h / px);
        return {
          path: `${dir ?? 'tmp/tlaku-bitmaps'}/${stem}-bitmap-1.png`,
          width_px,
          height_px,
          size_mm: [width_px * px, height_px * px] as [number, number],
          origin_mm: [Math.min(...xs), Math.min(...ys)] as [number, number],
          in_temp_dir: dir === null,
        };
      }),

    saveProcessedImage: ({ image, path }) =>
      lag(() => {
        if (!path.trim()) throw new Error('Falta la ruta del archivo.');
        return { path, bytes: 2048, width_px: image.width_px, height_px: image.height_px };
      }),

    artImportSvg: ({ svg, target_layer }) =>
      lag((): ImportResult => {
        if (!/<svg[\s>]/i.test(svg)) throw new Error('SVG inválido: falta el elemento <svg>.');
        const layer = target_layer ?? '#000000';
        return {
          objects: [
            {
              id: 'arte',
              layer,
              transform: [1, 0, 0, 1, 0, 0],
              kind: { type: 'vector', paths: [polylineBez([[0, 0], [30, 0], [30, 30], [0, 30]], true)] },
              locked: false,
              hidden: false,
              group: null,
            },
          ],
          new_layers: [],
          warnings: [],
        };
      }),

    artListUser: () => lag(() => user.map((a) => ({ ...a }))),

    artAddUser: ({ paths }) =>
      lag(() => {
        const added: UserArt[] = [];
        const errors: string[] = [];
        for (const p of paths) {
          const file = p.split(/[/\\]/).pop() ?? p;
          if (!/\.svg$/i.test(file)) {
            errors.push(`${file}: no es un archivo SVG.`);
            continue;
          }
          const base = file.replace(/\.svg$/i, '');
          let name = base;
          for (let k = 2; user.some((a) => a.name === name) || added.some((a) => a.name === name); k++) name = `${base}-${k}`;
          added.push({ name, svg: '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><rect x="1" y="1" width="8" height="8" fill="none" stroke="#000000"/></svg>' });
        }
        user = [...user, ...added];
        return { added, errors };
      }),

    artRemoveUser: ({ name }) =>
      lag(() => {
        user = user.filter((a) => a.name !== name);
      }),

    artSaveUser: ({ items }) =>
      lag(() => {
        const added: UserArt[] = [];
        const errors: string[] = [];
        for (const it of items) {
          const base = it.name.trim().replace(/\.svg$/i, '').replace(/[/\\:*?"<>|]/g, '-').replace(/^\.+|\.+$/g, '').trim();
          if (base === '') {
            errors.push(`«${it.name}»: el nombre no es válido.`);
            continue;
          }
          if (!/<svg[\s>]/i.test(it.svg)) {
            errors.push(`${it.name}: no es un SVG.`);
            continue;
          }
          let name = base;
          for (let k = 2; user.some((a) => a.name === name) || added.some((a) => a.name === name); k++) name = `${base}-${k}`;
          added.push({ name, svg: it.svg });
        }
        user = [...user, ...added];
        return { added, errors };
      }),

    componentListUser: () => lag(() => components.map((c) => structuredClone(c)).sort((a, b) => a.name.toLowerCase().localeCompare(b.name.toLowerCase()))),

    componentSaveUser: ({ items }) =>
      lag(() => {
        const added: UserComponent[] = [];
        const errors: string[] = [];
        for (const it of items) {
          const base = it.name.trim().replace(/[/\\:*?"<>|]/g, '-').replace(/^\.+|\.+$/g, '').trim();
          if (base === '') {
            errors.push(`«${it.name}»: el nombre no es válido.`);
            continue;
          }
          if (it.data?.format !== 'tlaku-component' || !Array.isArray(it.data.objects) || it.data.objects.length === 0) {
            errors.push(`«${it.name}»: no es un componente válido.`);
            continue;
          }
          let name = base;
          for (let k = 2; components.some((c) => c.name === name) || added.some((c) => c.name === name); k++) name = `${base}-${k}`;
          added.push({ name, data: structuredClone(it.data) });
        }
        components = [...components, ...added];
        return { added: added.map((c) => structuredClone(c)), errors };
      }),

    componentRenameUser: ({ name, new_name }) =>
      lag(() => {
        const base = new_name.trim().replace(/[/\\:*?"<>|]/g, '-').replace(/^\.+|\.+$/g, '').trim();
        if (base === '') throw new Error(`Nombre de componente inválido: «${new_name}».`);
        if (!components.some((c) => c.name === name)) throw new Error(`No existe el componente «${name}».`);
        if (base === name) return base;
        let final = base;
        for (let k = 2; components.some((c) => c.name === final); k++) final = `${base}-${k}`;
        components = components.map((c) => (c.name === name ? { ...c, name: final } : c));
        return final;
      }),

    componentRemoveUser: ({ name }) =>
      lag(() => {
        if (!components.some((c) => c.name === name)) throw new Error(`No existe el componente «${name}».`);
        components = components.filter((c) => c.name !== name);
      }),
  };
}
