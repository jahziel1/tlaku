import { readPref, writePref } from '@/lib/platform';
import { MAX_ITEMS, makeItem, type QueueItem } from './queueModel';

/**
 * La definición de la cola (qué archivos, cuántas copias, qué aviso) se recuerda entre sesiones en
 * `localStorage` (clave `tlaku.queue`); el estado de la ejecución NO: al abrir la app la cola siempre empieza
 * detenida y sin copias hechas, y nada se envía hasta que una persona la arranque y confirme cada paso. Solo se
 * recuerdan los trabajos que tienen archivo (una pestaña sin guardar no se puede reabrir).
 */
export const QUEUE_KEY = 'queue';

interface Saved {
  name: string;
  path: string;
  copies: number;
  note: string;
}

/** Guarda la definición de los trabajos con archivo. */
export function saveQueue(items: readonly QueueItem[]): void {
  const saved: Saved[] = items
    .filter((i): i is QueueItem & { path: string } => i.path !== null && i.status !== 'skipped')
    .map((i) => ({ name: i.name, path: i.path, copies: i.copies, note: i.note }));
  writePref(QUEUE_KEY, JSON.stringify(saved));
}

/** Trabajos recordados (lo dañado se descarta). Salen como archivos por abrir, con 0 copias hechas. */
export function loadQueue(): QueueItem[] {
  let raw: unknown;
  try {
    raw = JSON.parse(readPref(QUEUE_KEY) ?? '[]');
  } catch {
    return [];
  }
  if (!Array.isArray(raw)) return [];
  const out: QueueItem[] = [];
  for (const r of raw.slice(0, MAX_ITEMS)) {
    const o = r && typeof r === 'object' ? (r as Record<string, unknown>) : null;
    if (!o || typeof o.path !== 'string' || !o.path) continue;
    out.push(
      makeItem({
        source: 'file',
        path: o.path.slice(0, 1000),
        name: typeof o.name === 'string' && o.name ? o.name.slice(0, 200) : (o.path.split(/[\\/]/).pop() ?? o.path),
        copies: typeof o.copies === 'number' ? o.copies : 1,
        note: typeof o.note === 'string' ? o.note : '',
      }),
    );
  }
  return out;
}
