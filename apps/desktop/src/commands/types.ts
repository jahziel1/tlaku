import type { I18nKey } from '@/i18n';
import type { AppState } from '@/store';

export type CommandCategory = 'file' | 'edit' | 'tools' | 'view' | 'machine' | 'panels' | 'arrange' | 'camera';

export interface CommandDef {
  /** Identificador estable, `categoria.nombre`. Lo usan menús, barras y atajos. */
  id: string;
  label: I18nKey;
  /** Etiqueta que depende del estado (por ejemplo Pausar / Reanudar). Tiene prioridad sobre `label`. */
  labelFor?: (s: AppState) => I18nKey;
  category: CommandCategory;
  /** Atajos (ver `shortcuts.ts`). El primero es el que se muestra. */
  shortcuts?: string[];
  run: (s: AppState) => void | Promise<void>;
  /** `true` si se puede ejecutar; si no, la clave del motivo (se muestra en el tooltip). */
  enabled?: (s: AppState) => true | I18nKey;
  /** Estado de interruptor (herramienta activa, panel visible...). */
  checked?: (s: AppState) => boolean;
  /** Se repite al mantener la tecla pulsada (mover con flechas). */
  repeatable?: boolean;
  /** No aparece en menús ni buscadores. */
  hidden?: boolean;
}
