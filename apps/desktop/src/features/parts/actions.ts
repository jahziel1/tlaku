import { getApi } from '@/api';
import type { Document, LayerSettings, Point } from '@/api/types';
import type { GeneratorRecord } from '@/bindings/GeneratorRecord';
import { t, tn } from '@/i18n';
import { errorMessage } from '@/lib/errors';
import { groupMembers } from '@/lib/groups';
import { LAYER_PALETTE } from '@/lib/palette';
import { pinTab, selectActiveDevice, useStore } from '@/store';
import {
  applyInsert,
  BED_MARGIN_MM,
  engraveLayerFor,
  freeSpot,
  freshGroupId,
  groupOrigin,
  occupiedBounds,
  partObjects,
  PART_GAP_MM,
  recordFor,
  unusedColor,
} from './partsModel';
import { NEW_ENGRAVE, NO_ENGRAVE, usePartsUi, valuesFor } from './partsStore';

const state = () => useStore.getState();

/** Pide los generadores al motor la primera vez. */
export async function loadGenerators(): Promise<void> {
  if (usePartsUi.getState().loaded) return;
  try {
    usePartsUi.getState().setGenerators(await getApi().listGenerators());
  } catch (e) {
    state().notify('error', t('parts.failed', { error: errorMessage(e) }));
  }
}

/** Ancho útil (mm) de la cama para una fila de piezas. */
export function bedWidthForParts(): number {
  return Math.max(10, selectActiveDevice(state()).bed[0] - 2 * BED_MARGIN_MM);
}

/** Capa de corte de partida: la activa si es de líneas o rellenos; si no, la primera, o el primer color de la paleta. */
export function defaultCutLayer(doc: Document, active: string | null): string {
  const usable = (c: string | null) => doc.layers.find((l) => c && l.color.toLowerCase() === c.toLowerCase() && l.mode === 'line');
  return usable(active)?.color ?? doc.layers.find((l) => l.mode === 'line')?.color ?? LAYER_PALETTE[0]!.color;
}

export function openPartsDialog(generator?: string): void {
  void loadGenerators();
  usePartsUi.getState().openDialog(state().activeTabId, generator);
}

/** «Regenerar»: reabre el diálogo con los parámetros del grupo seleccionado. */
export async function openRegenerateDialog(record: GeneratorRecord): Promise<void> {
  await loadGenerators();
  usePartsUi.getState().openRegenerate(state().activeTabId, record);
}

/** El registro de generador de la selección actual, si la selección es un grupo generado. */
export function selectedRecord(): GeneratorRecord | null {
  const s = state();
  return recordFor(s.doc, s.selectedIds);
}

/**
 * Inserta (o regenera) las piezas con los valores del diálogo. Pide el cálculo al motor y escribe el resultado
 * en la pestaña donde se abrió el diálogo (`pinTab`), aunque mientras tanto se haya cambiado de pestaña.
 */
export async function commitParts(): Promise<boolean> {
  const ui = usePartsUi.getState();
  const inTab = pinTab(state());
  const params = valuesFor(ui, ui.generator);
  ui.setBusy(true);
  try {
    const result = await getApi().generatePart({ generator: ui.generator, params, max_width_mm: bedWidthForParts(), gap_mm: PART_GAP_MM });
    if (result.error) {
      state().notify('error', t('parts.failed', { error: result.error.message }));
      return false;
    }
    const regen = ui.regenGroup;
    let count = 0;
    let missingSpot = false;
    const created = inTab(() => {
      const s = state();
      const doc = s.doc;
      const cutLayer = ui.cutLayer ?? defaultCutLayer(doc, s.activeLayer);
      const engraveWanted = ui.engraveLayer !== NO_ENGRAVE && result.parts.some((p) => p.engraves.length > 0);
      const newLayers: LayerSettings[] = [];
      let engraveLayer: string | null = null;
      if (engraveWanted) {
        if (ui.engraveLayer === NEW_ENGRAVE) {
          const color = unusedColor(doc.layers, [cutLayer]);
          newLayers.push(engraveLayerFor(color, t('parts.layers.newEngrave')));
          engraveLayer = color;
        } else {
          engraveLayer = ui.engraveLayer;
        }
      }
      // Regenerar: mismo lugar y misma cadena de grupos; si el grupo ya no existe, es una inserción nueva.
      const members = regen ? groupMembers(doc.objects, regen) : [];
      const first = members.length > 0 ? doc.objects.find((o) => o.id === members[0]) : undefined;
      const replacing = regen && members.length > 0 ? regen : null;
      const group = replacing && first?.group ? first.group : freshGroupId(doc.objects);
      const groupId = replacing ?? group;
      let origin: Point | null = replacing ? groupOrigin(doc, replacing) : null;
      if (!origin) {
        const bed = selectActiveDevice(s).bed;
        origin = freeSpot(occupiedBounds(doc), result.width, result.height, bed);
        if (!origin) {
          origin = [BED_MARGIN_MM, BED_MARGIN_MM];
          missingSpot = true;
        }
      }
      const taken = new Set(doc.objects.filter((o) => !replacing || !members.includes(o.id)).map((o) => o.id));
      const objects = partObjects(result, { generator: ui.generator, cutLayer, engraveLayer, group, origin, taken });
      count = result.parts.length;
      const record: GeneratorRecord = {
        group: groupId,
        generator: ui.generator,
        params,
        cut_layer: cutLayer,
        engrave_layer: engraveLayer,
      };
      s.applyDocChange(replacing ? 'parts.history.regenerate' : 'parts.history.insert', (d) => applyInsert(d, { objects, record, newLayers }, replacing));
      s.select(objects.map((o) => o.id));
      return objects.length;
    });
    if (created === undefined) return false;
    state().notify('success', tn(regen ? 'parts.regenerated' : 'parts.inserted', count));
    if (missingSpot) state().notify('warning', t('parts.warn.spot'));
    void state().computePreview();
    return true;
  } catch (e) {
    state().notify('error', t('parts.failed', { error: errorMessage(e) }));
    return false;
  } finally {
    usePartsUi.getState().setBusy(false);
  }
}
