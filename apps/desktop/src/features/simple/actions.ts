import { useMemo } from 'react';
import type { Affine, Document, LaserKind, LayerSettings, MachineClass, MaterialLibrary, MaterialPreset } from '@/api/types';
import { moveDoc } from '@/features/arrange/align';
import { BUILTIN_ART } from '@/features/art/catalog';
import { insertArt } from '@/features/art/actions';
import { useArtPanel } from '@/features/art/artStore';
import { importDesign } from '@/features/history/files';
import { isVirtualPort } from '@/features/machine/virtualPort';
import { portsForDevice } from '@/features/machine/ruidaPorts';
import { chooseMachineClass, loadMaterials, persistMaterials } from '@/features/materials/actions';
import { resolveMachineClass, storedClassFor } from '@/features/materials/machineClass';
import { idsOf, presetFromLayer, uniqueId, type PresetForm } from '@/features/materials/library';
import { useMaterials } from '@/features/materials/state';
import { openTiling } from '@/features/tiling/store';
import { t } from '@/i18n';
import { multiplyAffine } from '@/lib/geometry';
import { selectActiveDevice, useStore } from '@/store';
import {
  actionBlock,
  allBox,
  applyActionsToDoc,
  colorGroups,
  cutBurnLess,
  cutMorePasses,
  designBox,
  fitFactor,
  impossibleActions,
  knobsOf,
  materialChoices,
  moveToAnchor,
  pickFor,
  sameColor,
  tuneEngrave,
  validate,
  type Knobs,
  type MaterialChoice,
  type SimpleAction,
} from './model';
import { stepName, useSimple } from './store';

const app = () => useStore.getState();
const ui = () => useSimple.getState();

// ---------------------------------------------------------------------------
// Lo que el modo sencillo lee del estado
// ---------------------------------------------------------------------------

/** Tipo de láser y clase (potencia) de la máquina activa. La clase es `null` si no se conoce la potencia. */
export interface MachineInfo {
  kind: LaserKind | null;
  machineClass: MachineClass | null;
}

/** Lo que se sabe de la máquina activa: la potencia que dio el usuario, o la que se deduce del perfil. */
export function currentMachine(): MachineInfo {
  return resolveMachineClass(selectActiveDevice(app()));
}

/** La máquina activa como estado de React (se actualiza al cambiar de máquina o al elegir su potencia). */
export function useMachineInfo(): MachineInfo {
  const device = useStore(selectActiveDevice);
  // Suscribirse a la clase elegida hace que este hook se vuelva a calcular cuando el usuario dice los watts.
  useMaterials((s) => s.machineClass);
  const stored = storedClassFor(device.id);
  return useMemo(() => resolveMachineClass(device), [device, stored]); // eslint-disable-line react-hooks/exhaustive-deps
}

/** Los materiales (del usuario y de fábrica) con sus ajustes para la máquina activa, con los datos ya leídos. */
export function choicesFor(user: readonly MaterialPreset[], factory: MaterialLibrary, deviceId: string, machine: MachineInfo): MaterialChoice[] {
  return materialChoices({ user, factory, deviceId, kind: machine.kind, machineClass: machine.machineClass });
}

/** Los materiales que se ofrecen: los del usuario más los de fábrica, para la máquina activa. */
export function useMaterialChoices(): MaterialChoice[] {
  const user = useMaterials((s) => s.library.presets);
  const factory = useMaterials((s) => s.factory);
  const deviceId = useStore((s) => selectActiveDevice(s).id);
  const machine = useMachineInfo();
  return useMemo(() => choicesFor(user, factory, deviceId, machine), [user, factory, deviceId, machine]);
}

/** El material elegido ahora mismo (`null` si no hay o ya no está en la biblioteca). */
export function currentChoice(): MaterialChoice | null {
  const key = ui().materialKey;
  if (!key) return null;
  const m = useMaterials.getState();
  return choicesFor(m.library.presets, m.factory, selectActiveDevice(app()).id, currentMachine()).find((c) => c.key === key) ?? null;
}

/** El usuario dice de cuántos watts es su láser (la clase más cercana); `null` = no lo sabe. Se recuerda para esa máquina. */
export function chooseWatts(machineClass: MachineClass | null): void {
  chooseMachineClass(selectActiveDevice(app()).id, machineClass);
  // Con otra potencia puede cambiar el ajuste elegido; lo que ya no tenga, se quita.
  reconcile(false);
}

const layerOf = (doc: Document, color: string): LayerSettings | undefined => doc.layers.find((l) => sameColor(l.color, color));

/** Valores del material para el color (según su acción), o `null` si no hay. */
export function baseKnobs(color: string): Knobs | null {
  const action = ui().actions[color.toLowerCase()];
  const pick = action ? pickFor(currentChoice(), action) : null;
  return pick ? knobsOf(pick.preset.settings) : null;
}

// ---------------------------------------------------------------------------
// Pasos
// ---------------------------------------------------------------------------

/** Va al paso `n` y deja el lienzo como ese paso lo necesita (vista previa, herramienta, selección). */
export function goToStep(n: number): void {
  ui().setStep(n);
  const s = app();
  const name = stepName(ui().step);
  const wantsPreview = name === 'review' || name === 'start';
  s.setPreviewVisible(wantsPreview);
  if (name === 'place') {
    s.setTool('select');
    s.selectAll();
    s.fitBed();
  }
  if (wantsPreview && (s.previewStale || s.previewStatus !== 'ready')) void s.computePreview();
  if (name === 'start' && !s.connection.connected) void s.refreshPorts();
}

export const nextStep = (): void => goToStep(ui().step + 1);
export const prevStep = (): void => goToStep(ui().step - 1);

// ---------------------------------------------------------------------------
// Paso 1: abrir
// ---------------------------------------------------------------------------

/** Abre el diálogo de importar un archivo de diseño. */
export const openFile = (): Promise<void> => importDesign();

/** Abre la biblioteca de arte. */
export function openArtLibrary(): void {
  useArtPanel.getState().setOpen(true);
}

/** Pone un diseño de ejemplo (un corazón de la biblioteca de arte) para probar el flujo. */
export async function openExample(): Promise<boolean> {
  const item = BUILTIN_ART.find((a) => a.id === 'corazones-clasico') ?? BUILTIN_ART[0];
  if (!item) return false;
  const ok = await insertArt(item);
  if (ok) app().fitContent();
  return ok;
}

// ---------------------------------------------------------------------------
// Paso 2: material y acción por color
// ---------------------------------------------------------------------------

/** Documento con la salida apagada en estas capas (para un color que se quedó sin acción). */
const silence = (doc: Document, colors: ReadonlySet<string>): Document => {
  let changed = false;
  const layers = doc.layers.map((l) => {
    if (!colors.has(l.color.toLowerCase()) || !l.output) return l;
    changed = true;
    return { ...l, output: false };
  });
  return changed ? { ...doc, layers } : doc;
};

/**
 * Los colores cuya acción ya no tiene ajuste (otro material, otra máquina u otra potencia) vuelven a
 * "sin decidir" y su capa queda sin salida: nunca se queda con los valores del material anterior.
 * Con `reapply`, a los demás se les vuelven a poner los ajustes del material. Todo es un solo paso
 * de deshacer. Devuelve cuántos colores perdieron su acción.
 */
function reconcile(reapply: boolean): number {
  const { actions } = ui();
  const choice = currentChoice();
  const groups = colorGroups(app().doc);
  const broken = impossibleActions(groups, actions, choice);
  const gone = new Set(broken.map((g) => g.color.toLowerCase()));
  const kept = Object.fromEntries(Object.entries(actions).filter(([c]) => !gone.has(c)));
  if (broken.length > 0) ui().dropActions(broken.map((g) => ({ color: g.color, name: g.name || g.color })));
  if (!reapply && broken.length === 0) return 0;
  app().applyDocChange('simple.history.apply', (doc) => silence(reapply ? applyActionsToDoc(doc, kept, choice) : doc, gone));
  return broken.length;
}

/** Elige el material: lo que ya se decidió por color se vuelve a aplicar con sus ajustes; lo que ya no se puede, se quita. */
export function chooseMaterial(key: string): void {
  ui().setMaterialKey(key);
  ui().clearTune();
  reconcile(true);
}

/** Cambia la máquina activa y revisa que lo decidido siga teniendo ajuste. */
export function changeMachine(id: string): void {
  app().setActiveDevice(id);
  ui().clearTune();
  reconcile(false);
}

/**
 * Elige qué hacer con un color y lo aplica a su capa. Si el material de ahora no tiene ajuste para
 * esa acción (sin ajuste, con restricción, o cortar una imagen) no cambia nada y avisa. Devuelve si se aplicó.
 */
export function chooseAction(color: string, action: SimpleAction): boolean {
  const group = colorGroups(app().doc).find((g) => sameColor(g.color, color));
  const choice = currentChoice();
  const why = group ? actionBlock(action, group, choice) : null;
  if (why) {
    app().notify('warning', t(why));
    return false;
  }
  ui().setAction(color, action);
  ui().setTune(color, 0);
  const only = { [color.toLowerCase()]: action };
  app().applyDocChange('simple.history.action', (doc) => applyActionsToDoc(doc, only, choice));
  return true;
}

/** Cambia velocidad / potencia / pasadas de una capa. Teclear seguido es un solo paso de deshacer. */
function patchLayer(color: string, patch: Partial<Knobs>, key?: string): void {
  app().applyDocChange(
    'simple.history.tune',
    (doc) => {
      const cur = layerOf(doc, color);
      if (!cur || (Object.keys(patch) as (keyof Knobs)[]).every((k) => cur[k] === patch[k])) return doc;
      return { ...doc, layers: doc.layers.map((l) => (l === cur ? { ...l, ...patch } : l)) };
    },
    key,
  );
}

const maxRate = (): number => selectActiveDevice(app()).max_rate[0];

/** "Más claro ↔ más oscuro": mueve potencia y velocidad juntas. */
export function setDarkness(color: string, step: number): void {
  const base = baseKnobs(color);
  if (!base) return;
  const k = tuneEngrave(base, step, maxRate());
  ui().setTune(color, step);
  patchLayer(color, { speed: k.speed, power: k.power });
}

/** Corte: "No atraviesa" (una pasada más). */
export function moreCutPasses(color: string): void {
  const layer = layerOf(app().doc, color);
  if (layer) patchLayer(color, { passes: cutMorePasses(knobsOf(layer)).passes });
}

/** Corte: "Quema mucho" (menos potencia y más velocidad). */
export function lessBurn(color: string): void {
  const layer = layerOf(app().doc, color);
  if (!layer) return;
  const k = cutBurnLess(knobsOf(layer), maxRate());
  patchLayer(color, { speed: k.speed, power: k.power });
}

/** Un valor escrito a mano en "Ver números". */
export function setNumber(color: string, field: keyof Knobs, value: number): void {
  ui().setTune(color, null);
  patchLayer(color, { [field]: value }, `simple:${color}:${field}`);
}

/** Vuelve la capa a los valores del material. */
export function resetToMaterial(color: string): void {
  const action = ui().actions[color.toLowerCase()];
  if (!action) return;
  const base = baseKnobs(color);
  if (!base) return;
  ui().setTune(color, 0);
  patchLayer(color, base);
}

/**
 * Guarda los valores de la capa como un material del usuario (para esta máquina), con el mismo
 * material y grosor. Devuelve si se guardó.
 */
export async function saveAsMyMaterial(color: string): Promise<boolean> {
  const choice = currentChoice();
  const action = ui().actions[color.toLowerCase()];
  const layer = layerOf(app().doc, color);
  if (!choice || !layer || !action || action === 'ignore') return false;
  await loadMaterials();
  const lib = useMaterials.getState();
  if (lib.loadError) {
    app().notify('error', t('materials.err.load', { error: lib.loadError }));
    return false;
  }
  const form: PresetForm = {
    material: choice.material,
    thickness: choice.thickness,
    title: t(action === 'cut' ? 'simple.mine.cut' : 'simple.mine.engrave'),
    notes: t('simple.mine.notes'),
    device_id: selectActiveDevice(app()).id,
    role: action,
    machine_class: currentMachine().machineClass,
  };
  const preset = presetFromLayer(layer, form, uniqueId(idsOf(lib.library.presets)));
  lib.setLibrary({ ...lib.library, presets: [...lib.library.presets, preset] });
  await persistMaterials();
  app().notify('success', t('simple.mine.saved', { name: `${preset.material} · ${preset.title}` }));
  return true;
}

// ---------------------------------------------------------------------------
// Paso 3: colocar
// ---------------------------------------------------------------------------

/** Lleva el diseño a un punto de la cama: 0..8 (fila por fila desde arriba a la izquierda), 4 = centro. */
export function placeAt(index: number): boolean {
  const s = app();
  const box = designBox(s.doc) ?? allBox(s.doc);
  if (!box) return false;
  const [dx, dy] = moveToAnchor(box, selectActiveDevice(s).bed, index);
  if (Math.abs(dx) < 1e-9 && Math.abs(dy) < 1e-9) return true;
  const ids = s.doc.objects.map((o) => o.id);
  s.applyDocChange('simple.history.place', (doc) => moveDoc(doc, [{ ids, dx, dy }]));
  return true;
}

/** Reduce el diseño (sin deformarlo) hasta que quepa en la cama y lo centra. Devuelve si cambió algo. */
export function fitToBed(): boolean {
  const s = app();
  const box = designBox(s.doc) ?? allBox(s.doc);
  if (!box) return false;
  const k = fitFactor(box, selectActiveDevice(s).bed);
  if (k >= 1) return false;
  const cx = (box[0][0] + box[1][0]) / 2;
  const cy = (box[0][1] + box[1][1]) / 2;
  const m: Affine = [k, 0, 0, k, cx * (1 - k), cy * (1 - k)];
  s.applyDocChange('simple.history.fit', (doc) => ({
    ...doc,
    objects: doc.objects.map((o) => (o.locked ? o : { ...o, transform: multiplyAffine(m, o.transform) })),
  }));
  placeAt(4);
  return true;
}

/** Abre el reparto en tramos (para diseños más grandes que la cama). */
export const splitInTiles = (): void => openTiling();

// ---------------------------------------------------------------------------
// Pasos 4 y 5: máquina
// ---------------------------------------------------------------------------

/** Conecta con la máquina activa (el puerto elegido o la dirección de su perfil). */
export const connectMachine = (): Promise<void> => app().connect();

/** Conecta con la máquina virtual que corresponde al perfil activo. Devuelve si había una. */
export async function connectVirtual(): Promise<boolean> {
  await app().refreshPorts();
  const s = app();
  const virtual = portsForDevice(selectActiveDevice(s), s.ports).find((p) => isVirtualPort(p));
  if (!virtual) {
    s.notify('warning', t('simple.noVirtual'));
    return false;
  }
  s.setSelectedPort(virtual);
  await s.connect();
  return app().connection.connected;
}

/** El botón Iniciar: abre "¿Láser listo y área despejada?" con la lista de comprobación de siempre. */
export function startJob(): void {
  // Defensa extra: con un color sin ajuste no se abre el diálogo de inicio.
  const s = app();
  const problems = validate({ doc: s.doc, device: selectActiveDevice(s), connected: s.connection.connected, choice: currentChoice(), actions: ui().actions });
  if (problems.some((p) => p.id === 'actionNoSettings')) {
    s.notify('warning', t('simple.need.actions'));
    return;
  }
  s.requestStart();
}
