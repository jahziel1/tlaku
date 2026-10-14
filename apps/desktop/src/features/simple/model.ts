/**
 * Lógica pura del modo sencillo (sin interfaz ni API): agrupar el diseño por color, elegir qué ajuste
 * del material corresponde a "Cortar" y a "Grabar", aplicarlo a las capas reales, los ajustes finos
 * (más claro / más oscuro, más pasadas...) y las validaciones amigables.
 */
import type { BezPath, DesignObject, DeviceProfile, Document, LaserKind, LayerSettings, MachineClass, MaterialLibrary, MaterialPreset, MaterialRestriction, MaterialRole, MaterialWarning } from '@/api/types';
import { designBounds } from '@/features/machine/placement';
import { classLabel, materialName, warningText } from '@/features/materials/labels';
import { fold, presetPatch, sameValue, thicknessLabel } from '@/features/materials/library';
import { effectiveRole, pickPreset, type PickMatch, type PickResult } from '@/features/materials/pick';
import { FIELD_SPECS } from '@/features/layers/fields';
import { locale, type I18nKey } from '@/i18n';
import { normalizeLayer } from '@/lib/layers';
import { isGuideColor } from '@/lib/palette';
import { objectPolylines, unionBounds, type Bounds } from '@/lib/geometry';
import { objectBoundsWithCopies } from '@/lib/virtualArray';

export type SimpleAction = 'cut' | 'engrave' | 'ignore';

/** Los cinco pasos, en orden. */
export const STEPS = ['open', 'material', 'place', 'review', 'start'] as const;
export type SimpleStep = (typeof STEPS)[number];

export const sameColor = (a: string, b: string): boolean => a.toLowerCase() === b.toLowerCase();

// ---------------------------------------------------------------------------
// El diseño agrupado por color
// ---------------------------------------------------------------------------

export interface ColorGroup {
  color: string;
  name: string;
  /** Objetos visibles de esa capa. */
  count: number;
  /** Solo imágenes: se graban siempre, o se ignoran. */
  isImage: boolean;
  /** Textos que siguen siendo texto (no se convirtieron a curvas). */
  texts: number;
  /** Trazos que no se cierran sobre sí mismos. */
  openPaths: number;
  layer: LayerSettings;
}

/** ¿El trazo vuelve a su punto de partida (tiene un segmento de cierre)? */
const isClosed = (p: BezPath): boolean => p.some((s) => s.type === 'close');

const objectsOf = (doc: Pick<Document, 'objects'>, color: string): DesignObject[] =>
  doc.objects.filter((o) => !o.hidden && sameColor(o.layer, color));

/** Un grupo por cada capa que tiene objetos visibles (las guías no cuentan). */
export function colorGroups(doc: Pick<Document, 'objects' | 'layers'>): ColorGroup[] {
  const groups: ColorGroup[] = [];
  for (const layer of doc.layers) {
    if (isGuideColor(layer.color)) continue;
    const objs = objectsOf(doc, layer.color);
    if (objs.length === 0) continue;
    const isImage = layer.mode === 'image' || objs.every((o) => o.kind.type === 'image');
    groups.push({
      color: layer.color,
      name: layer.name,
      count: objs.length,
      isImage,
      texts: objs.filter((o) => o.kind.type === 'text').length,
      openPaths: objs.reduce((n, o) => n + (o.kind.type === 'vector' ? o.kind.paths.filter((p) => !isClosed(p)).length : 0), 0),
      layer,
    });
  }
  return groups;
}

/** Contorno de una capa como un trazo SVG para la muestra; `null` si no hay formas (por ejemplo, solo imágenes). */
export function thumbnail(doc: Pick<Document, 'objects'>, color: string, maxPoints = 400): { d: string; viewBox: string } | null {
  const lines: { points: [number, number][]; closed: boolean }[] = [];
  for (const o of objectsOf(doc, color)) {
    if (o.kind.type === 'image') continue;
    lines.push(...objectPolylines(o, 0.5));
    if (lines.length > 200) break;
  }
  const all = lines.flatMap((l) => l.points);
  if (all.length === 0) return null;
  const stride = Math.max(1, Math.ceil(all.length / maxPoints));
  const xs = all.map((p) => p[0]);
  const ys = all.map((p) => p[1]);
  const [x0, x1, y0, y1] = [Math.min(...xs), Math.max(...xs), Math.min(...ys), Math.max(...ys)];
  const w = Math.max(x1 - x0, 1e-6);
  const h = Math.max(y1 - y0, 1e-6);
  const r = (n: number) => Math.round(n * 100) / 100;
  // El eje Y del documento va hacia arriba; en SVG hacia abajo.
  const d = lines
    .map((l) => {
      const pts = l.points.filter((_, i) => i % stride === 0 || i === l.points.length - 1);
      return pts.map(([x, y], i) => `${i === 0 ? 'M' : 'L'}${r(x - x0)} ${r(y1 - y)}`).join('') + (l.closed ? 'Z' : '');
    })
    .join('');
  return { d, viewBox: `-${r(w * 0.05)} -${r(h * 0.05)} ${r(w * 1.1)} ${r(h * 1.1)}` };
}

// ---------------------------------------------------------------------------
// El material: qué ajuste sirve para cortar y cuál para grabar
// ---------------------------------------------------------------------------

/** Para qué sirve un ajuste (el `role` que trae, o el que se deduce del modo de la capa). */
export const roleOf = effectiveRole;

/** El ajuste elegido para una operación y de dónde salió. */
export interface RolePick {
  preset: MaterialPreset;
  /** Es un ajuste guardado para esta máquina (no uno de su clase o general). */
  specific: boolean;
  match: PickMatch;
  /** No se conocía la potencia del láser. */
  powerUnknown: boolean;
  /** Clase del ajuste elegido, si tiene. */
  usedClass: MachineClass | null;
  warnings: MaterialWarning[];
  /** El resultado completo de `pickPreset`. */
  result: PickResult;
}

export interface MaterialChoice {
  key: string;
  /** Nombre ya traducido ("MDF"). */
  material: string;
  thickness: number | null;
  /** "MDF 3 mm". */
  label: string;
  cut: RolePick | null;
  engrave: RolePick | null;
  /** Por qué no hay ajuste de corte / de grabado para esta máquina (clave de texto), o `null` si lo hay. */
  cutWhy: I18nKey | null;
  engraveWhy: I18nKey | null;
  /** Lo que el tipo de láser no puede hacer con este material, si aplica. */
  restriction: MaterialRestriction | null;
}

/** De dónde salen los ajustes: los del usuario, los de fábrica y la máquina para la que se elige. */
export interface PickContext {
  user: readonly MaterialPreset[];
  factory: Pick<MaterialLibrary, 'presets' | 'restrictions'>;
  deviceId: string | null;
  kind: LaserKind | null;
  machineClass: MachineClass | null;
}

const collator = (): Intl.Collator => new Intl.Collator(locale(), { sensitivity: 'base', numeric: true });

/**
 * Los materiales que se pueden ofrecer: los del usuario y los de fábrica, uno por material y grosor.
 * Para cada uno, Cortar y Grabar salen de `pickPreset` (los del usuario primero). Un material sin
 * ningún ajuste ni restricción para esta máquina no se ofrece.
 */
export function materialChoices(ctx: PickContext): MaterialChoice[] {
  // Los materiales del usuario con el nombre de uno de fábrica se juntan con él (misma clave).
  const keyByName = new Map<string, string>();
  for (const p of ctx.factory.presets) {
    if (!p.material_key) continue;
    keyByName.set(fold(p.material), p.material_key);
    keyByName.set(fold(materialName(p)), p.material_key);
  }
  const user = ctx.user.map((p) => {
    const key = p.material_key ?? keyByName.get(fold(p.material));
    return key && !p.material_key ? { ...p, material_key: key } : p;
  });
  const all = [...user, ...ctx.factory.presets];
  const identity = (p: MaterialPreset) => p.material_key ?? fold(p.material);

  const byMaterial = new Map<string, { name: string; thicknesses: Set<number>; presets: MaterialPreset[] }>();
  for (const p of all) {
    const id = identity(p);
    const g = byMaterial.get(id) ?? { name: materialName(p), thicknesses: new Set<number>(), presets: [] };
    if (p.thickness !== null) g.thicknesses.add(p.thickness);
    g.presets.push(p);
    byMaterial.set(id, g);
  }

  const choices: MaterialChoice[] = [];
  for (const [id, g] of byMaterial) {
    const thicknesses: (number | null)[] = g.thicknesses.size > 0 ? [...g.thicknesses].sort((a, b) => a - b) : [null];
    for (const thickness of thicknesses) {
      const query = (role: MaterialRole): PickResult =>
        pickPreset(user, ctx.factory, { material: id, thickness, role, deviceId: ctx.deviceId, kind: ctx.kind, machineClass: ctx.machineClass });
      // Sin grosor no hay corte: un corte siempre es de un grosor.
      const cutResult = thickness === null ? null : query('cut');
      const engraveResult = query('engrave');
      const toPick = (r: PickResult | null): RolePick | null =>
        r?.preset
          ? { preset: r.preset, specific: r.match === 'device', match: r.match, powerUnknown: r.powerUnknown, usedClass: r.usedClass, warnings: r.warnings, result: r }
          : null;
      const restriction = cutResult?.restriction ?? engraveResult.restriction;
      // Una restricción que vale para todas las operaciones o para el corte impide cortar.
      const cutBlocked = restriction !== null && (restriction.role === null || restriction.role === 'cut');
      const engraveBlocked = restriction !== null && restriction.role === 'engrave';
      const cut = cutBlocked ? null : toPick(cutResult);
      const engrave = engraveBlocked ? null : toPick(engraveResult);
      if (!cut && !engrave && !restriction) continue;
      const exists = (role: MaterialRole) =>
        g.presets.some((p) => effectiveRole(p) === role && (thickness === null || p.thickness === null || Math.abs(p.thickness - thickness) < 0.01));
      const why = (role: MaterialRole, blocked: boolean): I18nKey =>
        blocked ? 'simple.why.restricted' : exists(role) ? 'simple.why.otherMachine' : role === 'cut' ? 'simple.why.noCut' : 'simple.why.noEngrave';
      const thick = thicknessLabel(thickness);
      choices.push({
        key: `${id}|${thickness ?? ''}`,
        material: g.name,
        thickness,
        label: thick ? `${g.name} ${thick}` : g.name,
        cut,
        engrave,
        cutWhy: cut ? null : why('cut', cutBlocked),
        engraveWhy: engrave ? null : why('engrave', engraveBlocked),
        restriction,
      });
    }
  }
  const c = collator();
  return choices.sort((a, b) => c.compare(a.material, b.material) || (a.thickness ?? Infinity) - (b.thickness ?? Infinity));
}

/** El ajuste de un material para una acción (`null` para ignorar). */
export function pickFor(choice: MaterialChoice | null, action: SimpleAction): RolePick | null {
  if (!choice || action === 'ignore') return null;
  return action === 'cut' ? choice.cut : choice.engrave;
}

/**
 * La capa con la acción aplicada. "Cortar" y "Grabar" copian los ajustes del material (menos color,
 * nombre, visibilidad y posición) y encienden la salida; "Ignorar" solo apaga la salida. Una capa de
 * imágenes sigue siendo de imágenes; un ajuste de imagen puesto en formas pasa a relleno.
 */
export function layerWithAction(layer: LayerSettings, action: SimpleAction, pick: RolePick | null): LayerSettings {
  const base = normalizeLayer(layer);
  if (action === 'ignore') return base.output ? { ...base, output: false } : layer;
  if (!pick) return layer;
  const patch: Record<string, unknown> = { ...presetPatch(pick.preset) };
  const presetMode = pick.preset.settings.mode;
  if (action === 'cut') {
    patch.mode = 'line';
  } else if (base.mode === 'image') {
    patch.mode = 'image';
    // Sin ajuste de imagen propio en el material, se conserva el tramado de la capa.
    if (presetMode !== 'image') patch.image = base.image;
  } else if (presetMode === 'image') {
    patch.mode = 'fill';
  }
  return { ...base, ...patch, output: true } as LayerSettings;
}

/** El documento con las acciones aplicadas a sus capas; el mismo documento si no cambia nada. */
export function applyActionsToDoc(
  doc: Document,
  actions: Readonly<Record<string, SimpleAction>>,
  choice: MaterialChoice | null,
): Document {
  let changed = false;
  const layers = doc.layers.map((l) => {
    const action = actions[l.color.toLowerCase()];
    if (!action) return l;
    const next = layerWithAction(l, action, pickFor(choice, action));
    if (sameValue(next, l)) return l;
    changed = true;
    return next;
  });
  return changed ? { ...doc, layers } : doc;
}

// ---------------------------------------------------------------------------
// Ajustes finos
// ---------------------------------------------------------------------------

/** Pasos de "más claro / más oscuro" a cada lado del valor del material. */
export const DARK_STEPS = 3;
/** Pasadas que ofrece el botón "No atraviesa" antes de sugerir revisar el material. */
export const MAX_EASY_PASSES = 10;

const round1 = (n: number) => Math.round(n * 10) / 10;

/** Valores de un ajuste que el modo sencillo muestra y toca. */
export interface Knobs {
  speed: number;
  power: number;
  passes: number;
}

export const knobsOf = (l: Pick<LayerSettings, 'speed' | 'power' | 'passes'>): Knobs => ({ speed: l.speed, power: l.power, passes: l.passes });

/** Velocidad máxima razonable: la de la máquina, o la del material si ya la pasaba. */
function speedCeiling(maxRate: number, base: number): number {
  return Math.min(FIELD_SPECS.speed.max, Math.max(maxRate, base));
}

/**
 * "Más claro ↔ más oscuro": `step` va de -3 a 3 y 0 es el valor del material. La energía por
 * milímetro se multiplica por 1,25 por paso; se reparte entre potencia y velocidad (a partes iguales,
 * y lo que no cabe en el 100 % de potencia lo pone la velocidad). Con paso 0 devuelve el material tal cual.
 */
export function tuneEngrave(base: Knobs, step: number, maxRate: number): Knobs {
  const s = Math.max(-DARK_STEPS, Math.min(DARK_STEPS, Math.round(step)));
  if (s === 0) return { ...base };
  const m = 1.25 ** s;
  const power = Math.max(1, Math.min(100, base.power * Math.sqrt(m)));
  const factor = m / (power / base.power); // lo que le toca a la velocidad
  const speed = Math.max(FIELD_SPECS.speed.min, Math.min(speedCeiling(maxRate, base.speed), base.speed / factor));
  return { speed: Math.round(speed), power: round1(power), passes: base.passes };
}

/** "No atraviesa": una pasada más (hasta `MAX_EASY_PASSES`). */
export function cutMorePasses(cur: Knobs): Knobs {
  return { ...cur, passes: Math.min(MAX_EASY_PASSES, cur.passes + 1) };
}

/** "Quema mucho": un poco menos de potencia y un poco más de velocidad. */
export function cutBurnLess(cur: Knobs, maxRate: number): Knobs {
  const speed = Math.min(speedCeiling(maxRate, cur.speed), cur.speed * 1.1);
  return { ...cur, power: Math.max(1, round1(cur.power * 0.95)), speed: Math.round(speed) };
}

export interface KnobChange {
  field: keyof Knobs;
  from: number;
  to: number;
}

/** Qué se cambió respecto del material. */
export function knobChanges(base: Knobs, cur: Knobs): KnobChange[] {
  return (['speed', 'power', 'passes'] as const).flatMap((field) =>
    Math.abs(base[field] - cur[field]) > 1e-9 ? [{ field, from: base[field], to: cur[field] }] : [],
  );
}

// ---------------------------------------------------------------------------
// Validaciones amigables
// ---------------------------------------------------------------------------

export type ProblemFix = 'connect' | 'virtual' | 'tiling' | 'fit' | 'test';

export interface Problem {
  id: string;
  /** `block` impide seguir; `warn` solo avisa. */
  level: 'block' | 'warn';
  key: I18nKey;
  params?: Record<string, string | number>;
  /** Pasos donde se muestra. */
  steps: readonly SimpleStep[];
  fix?: ProblemFix;
}

export interface ValidateInput {
  doc: Document;
  device: Pick<DeviceProfile, 'bed'>;
  connected: boolean;
  choice: MaterialChoice | null;
  actions: Readonly<Record<string, SimpleAction>>;
}

/** Tamaño del diseño que sale a la máquina, o `null` si no hay nada. */
export function designBox(doc: Pick<Document, 'objects' | 'layers'>): Bounds | null {
  return designBounds(doc, null);
}

/** Todo lo que conviene avisar con el estado de ahora. Los textos están en `simple.problem.*`. */
export function validate({ doc, device, connected, choice, actions }: ValidateInput): Problem[] {
  const out: Problem[] = [];
  const groups = colorGroups(doc);
  const active = groups.filter((g) => actions[g.color.toLowerCase()] !== 'ignore' && g.layer.output);
  const box = designBox(doc);

  if (!box) out.push({ id: 'nothing', level: 'block', key: 'simple.problem.nothing', steps: ['material', 'place', 'review', 'start'] });

  if (box) {
    const [[x0, y0], [x1, y1]] = box;
    const [bw, bh] = device.bed;
    if (x0 < -1e-6 || y0 < -1e-6 || x1 > bw + 1e-6 || y1 > bh + 1e-6) {
      out.push({
        id: 'outside',
        level: 'block',
        key: 'simple.problem.outside',
        params: { w: Math.round(x1 - x0), h: Math.round(y1 - y0), bw: Math.round(bw), bh: Math.round(bh) },
        steps: ['place', 'review', 'start'],
        fix: 'fit',
      });
    }
  }

  if (!connected) out.push({ id: 'noMachine', level: 'block', key: 'simple.problem.noMachine', steps: ['review', 'start'], fix: 'connect' });

  if (choice) {
    if (!choice.cut && !choice.engrave) {
      // Con una restricción, el aviso de abajo ya lo explica.
      if (!choice.restriction) out.push({ id: 'noSettings', level: 'block', key: 'simple.problem.noSettings', params: { material: choice.label }, steps: ['material', 'review', 'start'], fix: 'test' });
    } else {
      const used = groups.flatMap((g) => {
        const a = actions[g.color.toLowerCase()];
        return a && a !== 'ignore' ? [pickFor(choice, a)] : [];
      });
      const picks = (used.length > 0 ? used : [choice.cut, choice.engrave]).filter((p): p is RolePick => p !== null);
      // Siempre se recuerda que son valores de partida.
      out.push({ id: 'startingPoint', level: 'warn', key: 'simple.problem.startingPoint', params: { material: choice.label }, steps: ['material', 'review', 'start'], fix: 'test' });
      if (picks.some((p) => p.powerUnknown)) {
        out.push({ id: 'powerUnknown', level: 'warn', key: 'simple.problem.powerUnknown', steps: ['material', 'review', 'start'] });
      }
      const nearest = picks.find((p) => p.match === 'nearest' && !p.powerUnknown);
      if (nearest) {
        out.push({
          id: 'nearest',
          level: 'warn',
          key: 'simple.problem.nearest',
          params: { used: nearest.usedClass ? classLabel(nearest.usedClass) : '' },
          steps: ['material', 'review', 'start'],
        });
      }
      for (const w of new Set(picks.flatMap((p) => p.warnings))) {
        out.push({ id: `warning:${w}`, level: 'warn', key: 'simple.problem.text', params: { text: warningText(w) }, steps: ['material', 'review', 'start'] });
      }
    }
    if (choice.restriction) {
      out.push({
        id: 'restricted',
        level: choice.cut || choice.engrave ? 'warn' : 'block',
        key: 'simple.problem.restricted',
        params: { material: choice.label, text: warningText(choice.restriction.warning) },
        steps: ['material', 'review', 'start'],
      });
    }
  }

  const texts = active.reduce((n, g) => n + g.texts, 0);
  if (texts > 0) out.push({ id: 'texts', level: 'warn', key: 'simple.problem.texts', params: { n: texts }, steps: ['review', 'start'] });

  const open = groups.filter((g) => actions[g.color.toLowerCase()] === 'cut' && g.openPaths > 0);
  if (open.length > 0) {
    out.push({
      id: 'openCut',
      level: 'warn',
      key: 'simple.problem.openCut',
      params: { n: open.reduce((n, g) => n + g.openPaths, 0), names: open.map((g) => g.name || g.color).join(', ') },
      steps: ['review', 'start'],
    });
  }
  return out;
}

/** Colores sin decidir (no cuentan las capas de imágenes si ya se eligió algo). */
export function undecided(groups: readonly ColorGroup[], actions: Readonly<Record<string, SimpleAction>>): ColorGroup[] {
  return groups.filter((g) => !actions[g.color.toLowerCase()]);
}

// ---------------------------------------------------------------------------
// Colocar
// ---------------------------------------------------------------------------

/** Margen en mm que se deja al pegar el diseño a un borde de la cama. */
export const EDGE_MARGIN_MM = 5;

/**
 * Cuánto hay que mover el diseño para llevar su punto `index` (0..8, fila por fila desde arriba a la
 * izquierda; 4 = centro) al mismo punto de la cama. Los bordes dejan un margen mientras quepa.
 */
export function moveToAnchor(box: Bounds, bed: readonly [number, number], index: number): [number, number] {
  const fx = (index % 3) / 2;
  const fy = Math.floor(index / 3) / 2; // 0 = arriba
  const w = box[1][0] - box[0][0];
  const h = box[1][1] - box[0][1];
  const mx = Math.min(EDGE_MARGIN_MM, Math.max(0, (bed[0] - w) / 2));
  const my = Math.min(EDGE_MARGIN_MM, Math.max(0, (bed[1] - h) / 2));
  const x = mx + fx * (bed[0] - w - 2 * mx);
  const yTop = my + fy * (bed[1] - h - 2 * my);
  const y = bed[1] - h - yTop; // el eje Y del documento va hacia arriba
  return [x - box[0][0], y - box[0][1]];
}

/** Factor (≤ 1) para que el diseño quepa en la cama con un pequeño margen. */
export function fitFactor(box: Bounds, bed: readonly [number, number]): number {
  const w = box[1][0] - box[0][0];
  const h = box[1][1] - box[0][1];
  if (w <= 0 || h <= 0) return 1;
  return Math.min(1, (bed[0] - 2 * EDGE_MARGIN_MM) / w, (bed[1] - 2 * EDGE_MARGIN_MM) / h);
}

/** Caja de todo lo visible (también lo ignorado): lo que se mueve al colocar. */
export function allBox(doc: Pick<Document, 'objects'>): Bounds | null {
  return unionBounds(doc.objects.filter((o) => !o.hidden).map(objectBoundsWithCopies));
}

// ---------------------------------------------------------------------------
// Avanzar de paso
// ---------------------------------------------------------------------------

export interface GateInput {
  doc: Document;
  choice: MaterialChoice | null;
  actions: Readonly<Record<string, SimpleAction>>;
  problems: readonly Problem[];
  /** La vista previa está calculada y al día. */
  previewReady: boolean;
}

/** Por qué todavía no se puede pasar al paso siguiente (clave de texto), o `null` si se puede. */
export function advanceBlock(step: SimpleStep, g: GateInput): I18nKey | null {
  const groups = colorGroups(g.doc);
  switch (step) {
    case 'open':
      return groups.length === 0 ? 'simple.need.design' : null;
    case 'material': {
      if (!g.choice) return 'simple.need.material';
      if (undecided(groups, g.actions).length > 0) return 'simple.need.actions';
      if (groups.every((x) => g.actions[x.color.toLowerCase()] === 'ignore')) return 'simple.need.oneAction';
      return null;
    }
    case 'place':
      return g.problems.some((p) => p.id === 'outside') ? 'simple.need.fit' : null;
    case 'review':
      return g.previewReady ? null : 'simple.need.preview';
    case 'start':
      return null;
  }
}
