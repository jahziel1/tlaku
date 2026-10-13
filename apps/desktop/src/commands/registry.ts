import type { I18nKey } from '@/i18n';
import type { AppState } from '@/store';
import { normalizeCombo, formatCombo } from './shortcuts';
import type { CommandDef } from './types';

export class ShortcutConflictError extends Error {}

/**
 * Registro central de comandos. Menús, barras de herramientas, paleta de herramientas y atajos
 * leen de aquí: un comando tiene un solo id, una etiqueta, sus atajos y su acción.
 * Al construirlo se rechazan ids repetidos y atajos asignados a dos comandos.
 */
export class CommandRegistry {
  private byId = new Map<string, CommandDef>();
  private byCombo = new Map<string, CommandDef>();

  constructor(
    defs: readonly CommandDef[],
    private getState: () => AppState,
  ) {
    for (const def of defs) {
      if (this.byId.has(def.id)) throw new Error(`Comando repetido: ${def.id}`);
      this.byId.set(def.id, def);
      for (const raw of def.shortcuts ?? []) {
        const combo = normalizeCombo(raw);
        const other = this.byCombo.get(combo);
        if (other) throw new ShortcutConflictError(`El atajo ${combo} está en "${other.id}" y en "${def.id}"`);
        this.byCombo.set(combo, def);
      }
    }
  }

  list(): CommandDef[] {
    return [...this.byId.values()];
  }

  has(id: string): boolean {
    return this.byId.has(id);
  }

  get(id: string): CommandDef {
    const def = this.byId.get(id);
    if (!def) throw new Error(`Comando desconocido: ${id}`);
    return def;
  }

  /** Comando asignado a un atajo canónico, si lo hay. */
  match(combo: string): CommandDef | undefined {
    return this.byCombo.get(combo);
  }

  label(id: string, state: AppState = this.getState()): I18nKey {
    const def = this.get(id);
    return def.labelFor ? def.labelFor(state) : def.label;
  }

  /** `true` o la clave del motivo por el que no se puede ejecutar. */
  availability(id: string, state: AppState = this.getState()): true | I18nKey {
    return this.get(id).enabled?.(state) ?? true;
  }

  checked(id: string, state: AppState = this.getState()): boolean {
    return this.get(id).checked?.(state) ?? false;
  }

  /** Atajo principal, listo para mostrar; cadena vacía si no tiene. */
  shortcutLabel(id: string, mac: boolean): string {
    const first = this.get(id).shortcuts?.[0];
    return first ? formatCombo(first, mac) : '';
  }

  /** Ejecuta un comando. Devuelve false si no estaba disponible. Los errores se avisan, no se lanzan. */
  async run(id: string): Promise<boolean> {
    const state = this.getState();
    const def = this.get(id);
    const avail = this.availability(id, state);
    if (avail !== true) return false;
    try {
      await def.run(state);
    } catch (e) {
      state.notify('error', e instanceof Error ? e.message : String(e));
    }
    return true;
  }
}
