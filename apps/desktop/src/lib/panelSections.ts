/**
 * Secciones plegables de los paneles de la derecha (`ui/PanelSection.tsx`): qué ids hay, cuáles
 * abren por omisión y cómo se sanea lo que se recuerda (preferencia `panelSections`). Sin DOM.
 */

/** Lo que se recuerda: solo las secciones que el usuario abrió (`true`) o cerró (`false`) a mano. */
export type PanelSections = Record<string, boolean>;

export const NO_PANEL_SECTIONS: PanelSections = {};

/** Tope de entradas guardadas (hay unas 30 secciones; el resto sería basura de un archivo ajeno). */
const MAX_SECTIONS = 80;
const ID = /^[a-z][a-zA-Z0-9.-]{0,47}$/;

/** Lo guardado, saneado: solo ids con forma de id y valores booleanos. */
export function sanitizePanelSections(raw: unknown): PanelSections {
  if (!raw || typeof raw !== 'object' || Array.isArray(raw)) return NO_PANEL_SECTIONS;
  const out: PanelSections = {};
  for (const [k, v] of Object.entries(raw as Record<string, unknown>)) {
    if (Object.keys(out).length >= MAX_SECTIONS) break;
    if (ID.test(k) && typeof v === 'boolean') out[k] = v;
  }
  return Object.keys(out).length ? out : NO_PANEL_SECTIONS;
}

/** ¿Está abierta la sección? Lo que el usuario eligió manda; si no, el valor por omisión. */
export function isSectionOpen(saved: PanelSections, id: string, defaultOpen: boolean): boolean {
  return saved[id] ?? defaultOpen;
}

/**
 * Secciones anidadas dentro de otra: para mostrar una hay que abrir también la que la contiene.
 * Las claves son los ids de `PanelSection`.
 */
export const SECTION_PARENT: Readonly<Record<string, string>> = {
  'machine.job.startFrom': 'machine.job',
  'machine.job.selection': 'machine.job',
  'machine.job.frame': 'machine.job',
  'machine.status.detail': 'machine.status',
};

/** Ids de las secciones del panel Máquina que se piden desde otros lugares (comandos, avisos). */
export const SECTION_IDS = {
  job: 'machine.job',
  jog: 'machine.jog',
  positions: 'machine.positions',
  macros: 'machine.macros',
  overrides: 'machine.overrides',
  settings: 'machine.settings',
  connection: 'machine.connection',
} as const;

/** Pestaña del panel donde vive cada sección (por el prefijo del id). */
export function sectionTab(id: string): 'machine' | 'camera' | null {
  if (id.startsWith('machine.')) return 'machine';
  if (id.startsWith('camera.')) return 'camera';
  return null;
}
