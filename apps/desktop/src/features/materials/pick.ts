/**
 * Elección del ajuste más adecuado para una máquina, material, grosor y operación.
 *
 * Orden de preferencia:
 * 1. el ajuste de **esa máquina** (`device_id`);
 * 2. el de su **clase** (por ejemplo "diodo 10 W");
 * 3. un ajuste **sin máquina ni clase** (vale para cualquiera);
 * 4. el de la clase **más cercana por potencia** del mismo tipo de láser (en un empate, la de
 *    menos potencia), avisando de que hay que probar.
 * Dentro de cada paso gana el grosor exacto, luego los ajustes del usuario sobre los de fábrica y
 * luego la fuente más confiable. Si la potencia de la máquina se desconoce se parte de la clase
 * más baja de su tipo (la más prudente) y también se avisa.
 */
import type { Confidence, LaserKind, MachineClass, MaterialLibrary, MaterialPreset, MaterialRestriction, MaterialRole, MaterialWarning } from '@/api/types';
import type { I18nKey } from '@/i18n';
import { classDistance, classKind, classWatts, classesOfKind } from './machineClass';
import { classLabel } from './labels';
import { fold } from './library';
import { effectiveRole } from './role';

export { effectiveRole };

export type PickMatch = 'device' | 'class' | 'generic' | 'nearest' | 'none';

export interface PickQuery {
  /** Clave del material de fábrica ("mdf") o su nombre ("MDF"). */
  material: string;
  /** `null` = sin grosor (grabado). */
  thickness: number | null;
  role: MaterialRole;
  deviceId: string | null;
  /** Tipo de láser de la máquina; `null` = no se sabe (solo valen los ajustes genéricos). */
  kind: LaserKind | null;
  /** Clase de la máquina; `null` = se desconoce la potencia. */
  machineClass: MachineClass | null;
}

export interface PickResult {
  preset: MaterialPreset | null;
  match: PickMatch;
  /** Hay que hacer una prueba de materiales antes de confiar en el resultado. */
  mustTest: boolean;
  /** Clase del ajuste elegido (si tiene). */
  usedClass: MachineClass | null;
  /** No se conocía la potencia de la máquina. */
  powerUnknown: boolean;
  /** Lo que el tipo de láser no puede hacer con ese material, si aplica (aunque haya ajuste). */
  restriction: MaterialRestriction | null;
  warnings: MaterialWarning[];
}

const CONFIDENCE_RANK: Record<Confidence, number> = { official: 0, guide: 1, forum: 2 };

const sameThickness = (a: number, b: number) => Math.abs(a - b) < 0.01;

function isMaterial(p: Pick<MaterialPreset, 'material' | 'material_key'>, wanted: string): boolean {
  const w = fold(wanted);
  return fold(p.material_key ?? '') === w || fold(p.material) === w;
}

/** 0 = grosor exacto, 1 = el ajuste no depende del grosor; `null` = no vale. */
function thicknessRank(p: MaterialPreset, wanted: number | null): number | null {
  if (wanted === null) return p.thickness === null ? 0 : 1;
  if (p.thickness === null) return 1;
  return sameThickness(p.thickness, wanted) ? 0 : null;
}

interface Candidate {
  preset: MaterialPreset;
  fromUser: boolean;
  thick: number;
}

const preference = (a: Candidate, b: Candidate) =>
  a.thick - b.thick ||
  Number(!a.fromUser) - Number(!b.fromUser) ||
  (a.preset.confidence ? CONFIDENCE_RANK[a.preset.confidence] : 3) - (b.preset.confidence ? CONFIDENCE_RANK[b.preset.confidence] : 3);

function best(list: Candidate[]): Candidate | null {
  return [...list].sort(preference)[0] ?? null;
}

/** Restricción de `material` para ese tipo de láser y operación, si la biblioteca de fábrica trae una. */
export function restrictionFor(
  factory: Pick<MaterialLibrary, 'restrictions'>,
  material: string,
  kind: LaserKind | null,
  role: MaterialRole | null = null,
): MaterialRestriction | null {
  if (kind === null) return null;
  const w = fold(material);
  return (factory.restrictions ?? []).find((r) => fold(r.material) === w && r.laser === kind && (r.role == null || role === null || r.role === role)) ?? null;
}

export function pickPreset(user: readonly MaterialPreset[], factory: Pick<MaterialLibrary, 'presets' | 'restrictions'>, q: PickQuery): PickResult {
  const powerUnknown = q.machineClass === null && q.kind !== null;
  // Sin potencia conocida: la clase más baja del tipo de láser.
  const cls = q.machineClass ?? (q.kind ? (classesOfKind(q.kind)[0] ?? null) : null);

  const all: Candidate[] = [];
  const add = (list: readonly MaterialPreset[], fromUser: boolean) => {
    for (const preset of list) {
      if (!isMaterial(preset, q.material) || effectiveRole(preset) !== q.role) continue;
      const thick = thicknessRank(preset, q.thickness);
      if (thick !== null) all.push({ preset, fromUser, thick });
    }
  };
  add(user, true);
  add(factory.presets, false);

  const restriction = restrictionFor(factory, q.material, q.kind, q.role);
  const done = (c: Candidate | null, match: PickMatch, extraTest = false): PickResult => ({
    preset: c?.preset ?? null,
    match: c ? match : 'none',
    mustTest: c ? extraTest || powerUnknown || match === 'nearest' || match === 'generic' : false,
    usedClass: c?.preset.machine_class ?? null,
    powerUnknown: c ? powerUnknown : false,
    restriction,
    warnings: c?.preset.warnings ?? [],
  });

  // 1. La máquina misma.
  if (q.deviceId !== null) {
    const hit = best(all.filter((c) => c.preset.device_id === q.deviceId));
    if (hit) return done(hit, 'device');
  }
  const untargeted = all.filter((c) => c.preset.device_id === null);
  // 2. Su clase.
  if (cls !== null) {
    const hit = best(untargeted.filter((c) => c.preset.machine_class === cls));
    if (hit) return done(hit, 'class');
  }
  // 3. Sin máquina ni clase: vale para cualquiera.
  const generic = best(untargeted.filter((c) => !c.preset.machine_class));
  if (generic) return done(generic, 'generic');
  // 4. La clase más cercana por potencia del mismo tipo de láser.
  if (cls !== null) {
    const sameKind = untargeted.filter((c) => c.preset.machine_class && classKind(c.preset.machine_class) === classKind(cls));
    const distance = (c: Candidate) => classDistance(c.preset.machine_class!, cls);
    const watts = (c: Candidate) => classWatts(c.preset.machine_class!);
    const sorted = [...sameKind].sort((a, b) => distance(a) - distance(b) || watts(a) - watts(b) || preference(a, b));
    if (sorted[0]) return done(sorted[0], 'nearest');
  }
  return done(null, 'none');
}

// ---------------------------------------------------------------------------
// Texto del resultado (para el panel y para el modo sencillo)
// ---------------------------------------------------------------------------

/** Clave de i18n y parámetros que explican de dónde salió el ajuste y qué hacer. */
export function describePick(result: PickResult, requested: MachineClass | null): { key: I18nKey; params: Record<string, string> } {
  const params = { class: requested ? classLabel(requested) : '', used: result.usedClass ? classLabel(result.usedClass) : '' };
  if (result.match === 'none') return { key: result.restriction ? 'materials.pick.restricted' : 'materials.pick.none', params };
  if (result.powerUnknown) return { key: 'materials.pick.powerUnknown', params };
  switch (result.match) {
    case 'device':
      return { key: 'materials.pick.device', params };
    case 'class':
      return { key: 'materials.pick.class', params };
    case 'generic':
      return { key: 'materials.pick.generic', params };
    default:
      return { key: 'materials.pick.nearest', params };
  }
}
