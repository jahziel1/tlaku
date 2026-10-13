import { t as translate, type I18nKey } from '@/i18n';
import type { SliceCreator } from './types';

export type NoticeLevel = 'info' | 'success' | 'warning' | 'error';

export interface Notice {
  id: number;
  level: NoticeLevel;
  /** Texto ya traducido o mensaje del motor. */
  text: string;
  /** Detalles opcionales (por ejemplo, la lista de advertencias de una importación). */
  details?: string[];
  /** Persistente: no se va solo ni lo desplaza otro aviso; espera a que el usuario lo cierre. */
  sticky: boolean;
}

export interface NoticesSlice {
  notices: Notice[];
  notify: (level: NoticeLevel, text: string, details?: string[]) => number;
  notifyKey: (level: NoticeLevel, key: I18nKey) => number;
  dismissNotice: (id: number) => void;
}

let nextId = 1;
/** Avisos efímeros visibles a la vez. */
export const MAX_NOTICES = 4;
/** Tope de avisos persistentes; solo se alcanza con una avalancha de errores. */
export const MAX_STICKY = 12;

/** Los avisos y errores se quedan hasta que alguien los cierra: una advertencia nunca se pierde sola. */
export const isSticky = (level: NoticeLevel): boolean => level === 'warning' || level === 'error';

/**
 * Agrega un aviso. Si hay demasiados, se descartan primero los efímeros más viejos; un aviso
 * persistente solo cede ante el tope `MAX_STICKY`.
 */
export function pushNotice(list: Notice[], notice: Notice): Notice[] {
  // Un aviso idéntico al último (mismo nivel, texto y detalles) no se apila: es el mismo problema repetido.
  const same = list.find(
    (n) => n.level === notice.level && n.text === notice.text && (n.details ?? []).join('\n') === (notice.details ?? []).join('\n'),
  );
  const base = same ? list.filter((n) => n !== same) : list;
  let next = [...base, notice];
  const transient = next.filter((n) => !n.sticky);
  if (transient.length > MAX_NOTICES) {
    const drop = new Set(transient.slice(0, transient.length - MAX_NOTICES));
    next = next.filter((n) => !drop.has(n));
  }
  const sticky = next.filter((n) => n.sticky);
  if (sticky.length > MAX_STICKY) {
    const drop = new Set(sticky.slice(0, sticky.length - MAX_STICKY));
    next = next.filter((n) => !drop.has(n));
  }
  return next;
}

export const createNoticesSlice: SliceCreator<NoticesSlice> = (set, get) => ({
  notices: [],
  notify: (level, text, details) => {
    const id = nextId++;
    const notice: Notice = { id, level, text, details, sticky: isSticky(level) };
    set((s) => ({ notices: pushNotice(s.notices, notice) }));
    return id;
  },
  notifyKey: (level, key) => get().notify(level, translate(key)),
  dismissNotice: (id) => set((s) => ({ notices: s.notices.filter((n) => n.id !== id) })),
});
