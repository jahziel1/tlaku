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
}

export interface NoticesSlice {
  notices: Notice[];
  notify: (level: NoticeLevel, text: string, details?: string[]) => number;
  notifyKey: (level: NoticeLevel, key: I18nKey) => number;
  dismissNotice: (id: number) => void;
}

let nextId = 1;
const MAX_NOTICES = 4;

export const createNoticesSlice: SliceCreator<NoticesSlice> = (set, get) => ({
  notices: [],
  notify: (level, text, details) => {
    const id = nextId++;
    set((s) => ({ notices: [...s.notices, { id, level, text, details }].slice(-MAX_NOTICES) }));
    return id;
  },
  notifyKey: (level, key) => get().notify(level, translate(key)),
  dismissNotice: (id) => set((s) => ({ notices: s.notices.filter((n) => n.id !== id) })),
});
