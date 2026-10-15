import { flattenBez, transformBez } from '@/lib/bezpath';
import { DEFAULT_VERSIONS_CONFIG, type ProjectSummary, type VersionInfo, type VersionsConfig } from '@/features/versions/types';
import type { LaserApi } from '../api';
import type { Document } from '../types';

type Lag = <T>(value: () => T) => Promise<T>;

interface Entry {
  info: VersionInfo;
  doc: Document;
  thumb: string | null;
}

const clone = <T>(v: T): T => structuredClone(v);

/** Miniatura SVG de los vectores del documento (el motor real usa `tlaku-export`; aquí basta un trazo). */
export function mockThumbnail(doc: Document): string | null {
  const lines: string[] = [];
  let minX = Infinity;
  let minY = Infinity;
  let maxX = -Infinity;
  let maxY = -Infinity;
  for (const o of doc.objects) {
    if (o.kind.type !== 'vector') continue;
    const color = typeof o.layer === 'string' ? o.layer : '#ff0000';
    for (const p of o.kind.paths) {
      for (const flat of flattenBez(transformBez(p, o.transform), 0.2)) {
        if (flat.points.length < 2) continue;
        for (const [x, y] of flat.points) {
          minX = Math.min(minX, x);
          maxX = Math.max(maxX, x);
          minY = Math.min(minY, y);
          maxY = Math.max(maxY, y);
        }
        const d = flat.points.map(([x, y], i) => `${i ? 'L' : 'M'}${x.toFixed(2)} ${(-y).toFixed(2)}`).join('');
        lines.push(`<path d="${d}${flat.closed ? 'Z' : ''}" stroke="${color}"/>`);
      }
    }
  }
  if (lines.length === 0) return null;
  const w = maxX - minX || 1;
  const h = maxY - minY || 1;
  const pad = Math.max(w, h) / 40;
  const stroke = Math.max(w, h) / 120;
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="${minX - pad} ${-maxY - pad} ${w + 2 * pad} ${h + 2 * pad}" fill="none" stroke-width="${stroke}" stroke-linecap="round" stroke-linejoin="round">${lines.join('')}</svg>`;
}

/**
 * Historial de versiones en memoria (modo mock). Imita al motor: no repite el contenido de la última
 * versión, una manual con el mismo contenido nombra a la última, `versionsCapture` guarda lo que tenía
 * `projects` (los «archivos» del mock) para esa ruta. No aplica retención por tiempo.
 */
export function mockVersions(
  lag: Lag,
  projects: Map<string, Document>,
): Pick<
  LaserApi,
  'versionsList' | 'versionsThumb' | 'versionsRead' | 'versionsCapture' | 'versionsSnapshot' | 'versionsDelete' | 'versionsClear' | 'versionsProjects' | 'versionsConfigGet' | 'versionsConfigSet'
> {
  const store = new Map<string, Entry[]>();
  let config: VersionsConfig = { ...DEFAULT_VERSIONS_CONFIG };
  let counter = 0;

  const entries = (path: string): Entry[] => {
    let list = store.get(path);
    if (!list) {
      list = [];
      store.set(path, list);
    }
    return list;
  };
  const latest = (list: Entry[]): Entry | undefined => [...list].sort((a, b) => b.info.created_ms - a.info.created_ms)[0];

  const add = (path: string, doc: Document, kind: VersionInfo['kind'], label: string | null, createdMs: number): VersionInfo | null => {
    const list = entries(path);
    const text = JSON.stringify(doc);
    const top = latest(list);
    if (top && JSON.stringify(top.doc) === text) {
      if (kind !== 'manual' || (top.info.kind === 'manual' && top.info.label === label)) return null;
      top.info = { ...top.info, kind: 'manual', label };
      return clone(top.info);
    }
    counter += 1;
    const info: VersionInfo = {
      id: `${String(createdMs).padStart(13, '0')}-${counter.toString(16).padStart(8, '0')}`,
      kind,
      label,
      created_ms: createdMs,
      size_bytes: text.length,
      stored_bytes: Math.ceil(text.length / 6),
      hash: String(counter),
      objects: doc.objects.length,
      layers: doc.layers.length,
      has_thumb: false,
    };
    const thumb = mockThumbnail(doc);
    info.has_thumb = thumb !== null;
    list.push({ info, doc: clone(doc), thumb });
    return clone(info);
  };

  const sorted = (path: string): VersionInfo[] =>
    entries(path)
      .map((e) => e.info)
      .sort((a, b) => b.created_ms - a.created_ms || (a.id < b.id ? 1 : -1))
      .map(clone);

  const find = (path: string, id: string): Entry => {
    const e = entries(path).find((x) => x.info.id === id);
    if (!e) throw new Error(`No existe la versión «${id}» en el historial de este proyecto.`);
    return e;
  };

  return {
    versionsList: ({ path }) =>
      lag(() => {
        const versions = sorted(path);
        return { versions, total_bytes: versions.reduce((n, v) => n + v.stored_bytes, 0), dir: `/mock/versiones/${path.split('/').pop()}` };
      }),
    versionsThumb: ({ path, id }) => lag(() => find(path, id).thumb),
    versionsRead: ({ path, id }) => lag(() => clone(find(path, id).doc)),
    versionsCapture: ({ path }) =>
      lag(() => {
        const file = projects.get(path);
        if (!config.enabled || !file) return null;
        return add(path, file, 'saved', null, Date.now());
      }),
    versionsSnapshot: ({ path, doc, kind, label }) =>
      lag(() => {
        if (kind === 'auto' && (!config.enabled || config.auto_minutes === 0)) return null;
        const name = label?.trim() || null;
        if (kind === 'manual' && !name) throw new Error('Una versión manual necesita un nombre.');
        return add(path, doc, kind, kind === 'manual' ? name : null, Date.now());
      }),
    versionsDelete: ({ path, ids }) =>
      lag(() => {
        const list = entries(path);
        const keep = list.filter((e) => !ids.includes(e.info.id));
        const n = list.length - keep.length;
        store.set(path, keep);
        return n;
      }),
    versionsClear: ({ path }) =>
      lag(() => {
        const n = entries(path).length;
        store.delete(path);
        return n;
      }),
    versionsProjects: () =>
      lag((): ProjectSummary[] =>
        [...store.entries()]
          .filter(([, list]) => list.length > 0)
          .map(([path, list]) => ({
            path,
            name: path.split('/').pop() ?? path,
            versions: list.length,
            total_bytes: list.reduce((n, e) => n + e.info.stored_bytes, 0),
            latest_ms: Math.max(...list.map((e) => e.info.created_ms)),
            exists: projects.has(path),
          }))
          .sort((a, b) => b.latest_ms - a.latest_ms),
      ),
    versionsConfigGet: () => lag(() => ({ ...config })),
    versionsConfigSet: ({ config: next }) =>
      lag(() => {
        config = {
          ...next,
          auto_minutes: Math.min(240, Math.max(0, Math.round(next.auto_minutes))),
          recent_hours: Math.min(720, Math.max(1, Math.round(next.recent_hours))),
          daily_days: Math.min(365, Math.max(0, Math.round(next.daily_days))),
          weekly_weeks: Math.min(520, Math.max(0, Math.round(next.weekly_weeks))),
          max_mb: Math.min(10240, Math.max(1, Math.round(next.max_mb))),
          max_versions: Math.min(5000, Math.max(10, Math.round(next.max_versions))),
        };
        return { ...config };
      }),
  };
}
