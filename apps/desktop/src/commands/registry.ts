import type { I18nKey } from '@/i18n';
import type { AppState } from '@/store';
import { normalizeCombo, formatCombo, sameCombos, tryNormalizeCombo, type ShortcutOverrides } from './shortcuts';
import type { CommandDef } from './types';

export class ShortcutConflictError extends Error {}

/** Atajos efectivos (fábrica + usuario) para un conjunto de cambios del usuario. */
interface Resolved {
  overrides: ShortcutOverrides;
  byCombo: Map<string, CommandDef>;
  byId: Map<string, string[]>;
}

const NO_OVERRIDES: ShortcutOverrides = {};

/**
 * Registro central de comandos. Menús, barras de herramientas, paleta de herramientas y atajos
 * leen de aquí: un comando tiene un solo id, una etiqueta, sus atajos y su acción.
 * Al construirlo se rechazan ids repetidos y atajos asignados a dos comandos.
 *
 * Los atajos que el usuario cambia (`shortcutOverrides` del estado) se aplican encima de los de
 * fábrica (`def.shortcuts`): un comando con cambios usa solo los suyos, y si uno de ellos choca con
 * un atajo de fábrica de otro comando, gana el del usuario. `def.shortcuts` nunca cambia: el
 * diálogo de atajos y el README siguen mostrando los de fábrica.
 */
export class CommandRegistry {
  private byId = new Map<string, CommandDef>();
  private byCombo = new Map<string, CommandDef>();
  private resolved: Resolved | null = null;

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

  /** Atajos efectivos según los cambios del usuario del estado actual. Se recalcula solo si cambian. */
  private resolve(): Resolved {
    const overrides = this.getState().shortcutOverrides ?? NO_OVERRIDES;
    if (this.resolved?.overrides === overrides) return this.resolved;
    const byCombo = new Map<string, CommandDef>();
    const byId = new Map<string, string[]>();
    const claim = (def: CommandDef, raw: string) => {
      const combo = tryNormalizeCombo(raw);
      if (combo === null || byCombo.has(combo)) return;
      byCombo.set(combo, def);
      byId.set(def.id, [...(byId.get(def.id) ?? []), combo]);
    };
    // Primero los comandos con cambios del usuario, para que ganen ante los de fábrica.
    for (const def of this.byId.values()) {
      byId.set(def.id, []);
      for (const raw of overrides[def.id] ?? []) claim(def, raw);
    }
    for (const def of this.byId.values()) {
      if (def.id in overrides) continue;
      for (const raw of def.shortcuts ?? []) claim(def, raw);
    }
    this.resolved = { overrides, byCombo, byId };
    return this.resolved;
  }

  /** Comando asignado a un atajo canónico, si lo hay (con los cambios del usuario). */
  match(combo: string): CommandDef | undefined {
    return this.resolve().byCombo.get(combo);
  }

  /** Atajos efectivos de un comando, en forma canónica. */
  shortcutsOf(id: string): string[] {
    this.get(id);
    return this.resolve().byId.get(id) ?? [];
  }

  /** Atajos de fábrica de un comando, en forma canónica. */
  factoryShortcuts(id: string): string[] {
    return (this.get(id).shortcuts ?? []).map(normalizeCombo);
  }

  /** ¿El usuario cambió los atajos de este comando respecto a los de fábrica? */
  isCustomized(id: string): boolean {
    const own = this.getState().shortcutOverrides?.[id];
    return own !== undefined && !sameCombos(own, this.factoryShortcuts(id));
  }

  /** Comando que tiene hoy este atajo, salvo `except`. Sirve para avisar de conflictos al reasignar. */
  conflictOf(combo: string, except?: string): CommandDef | undefined {
    const def = this.match(normalizeCombo(combo));
    return def && def.id !== except ? def : undefined;
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

  /** Atajo principal (el del usuario si lo cambió), listo para mostrar; cadena vacía si no tiene. */
  shortcutLabel(id: string, mac: boolean): string {
    const first = this.shortcutsOf(id)[0];
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
