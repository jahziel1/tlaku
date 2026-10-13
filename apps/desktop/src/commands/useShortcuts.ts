import { useEffect } from 'react';
import { isMac } from '@/lib/platform';
import type { CommandRegistry } from './registry';
import { eventToCombo, isEditableTarget, NATIVE_TEXT_COMBOS } from './shortcuts';

/**
 * Atajos globales. Reglas (documentadas en el README):
 *  - Valen en toda la ventana, no solo con el lienzo enfocado.
 *  - En un campo de texto solo actúan los que llevan Cmd/Ctrl y no son de edición de texto.
 *  - Un atajo ocupado por un comando deshabilitado no hace nada, pero tampoco pasa al navegador.
 */
export function useShortcuts(registry: CommandRegistry): void {
  useEffect(() => {
    const mac = isMac();
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.defaultPrevented || e.isComposing) return;
      const combo = eventToCombo(e, mac);
      if (!combo) return;
      const def = registry.match(combo);
      if (!def) return;
      if (isEditableTarget(e.target) && (!combo.startsWith('Mod+') || NATIVE_TEXT_COMBOS.has(combo))) return;
      if (e.repeat && !def.repeatable) return;
      e.preventDefault();
      void registry.run(def.id);
    };
    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, [registry]);
}
