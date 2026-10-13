import { useMemo } from 'react';
import { t, type I18nKey } from '@/i18n';
import { isMac } from '@/lib/platform';
import { useStore } from '@/store';
import { commands } from './index';

export interface CommandView {
  id: string;
  label: string;
  shortcut: string;
  enabled: boolean;
  /** Motivo traducido si está deshabilitado. */
  reason: string | null;
  checked: boolean;
  /** Texto para `title`: etiqueta, atajo y, si no se puede, por qué. */
  tooltip: string;
  run: () => void;
}

/**
 * Vista de un comando para botones y menús. Cada selector devuelve un valor primitivo, así que
 * el componente solo se vuelve a pintar si cambia su disponibilidad, etiqueta o marca
 * (no con cada movimiento del cursor).
 */
export function useCommand(id: string): CommandView {
  const avail = useStore((s) => commands.availability(id, s));
  const labelKey = useStore((s) => commands.label(id, s));
  const checked = useStore((s) => commands.checked(id, s));
  // Atajo principal como texto: cambia si el usuario lo reasigna, y entonces el tooltip se recalcula.
  const shortcut = useStore(() => commands.shortcutLabel(id, isMac()));
  return useMemo(() => {
    const label = t(labelKey);
    const reason = avail === true ? null : t(avail as I18nKey);
    const base = shortcut ? `${label} (${shortcut})` : label;
    return {
      id,
      label,
      shortcut,
      enabled: avail === true,
      reason,
      checked,
      tooltip: reason ? `${base}. ${reason}` : base,
      run: () => void commands.run(id),
    };
  }, [id, avail, labelKey, checked, shortcut]);
}
