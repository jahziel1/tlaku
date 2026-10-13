import { copyLayerSettings, deleteLayerWithObjects, lockedInLayer, openEditorFor, pasteLayerSettings, pickPaletteEntry, selectLayerObjects } from '@/features/layers/actions';
import { resetLayerDefaults, saveLayerAsDefaults } from '@/features/layers/defaults';
import { useLayerUi } from '@/features/layers/layerUi';
import { isGuideColor } from '@/lib/palette';
import { LAYER_PALETTE } from '@/lib/palette';
import { useStore, type AppState } from '@/store';
import type { I18nKey } from '@/i18n';
import type { CommandDef } from '../types';

/** Capa activa si existe en el documento. */
const activeLayerOf = (s: AppState) => s.doc.layers.find((l) => l.color.toLowerCase() === s.activeLayer?.toLowerCase());

const hasActive = (s: AppState): true | I18nKey => (activeLayerOf(s) ? true : 'layers.reason.noActiveLayer');

/**
 * Elegir una muestra de la paleta con el teclado (`Alt+1`..`Alt+0` y `Alt+G` para las guías).
 * Hacen lo mismo que un clic en la franja de colores. Ocultos: viven en la franja, no en menús.
 */
const pickCommands: CommandDef[] = LAYER_PALETTE.map((entry) => ({
  id: `layer.pick.${entry.id}`,
  label: 'layers.cmd.pick',
  category: 'edit',
  shortcuts: entry.shortcut ? [entry.shortcut] : undefined,
  hidden: true,
  run: (s) => pickPaletteEntry(s, entry),
}));

export const layerCommands: CommandDef[] = [
  ...pickCommands,
  {
    id: 'layer.edit',
    label: 'layers.cmd.edit',
    category: 'edit',
    shortcuts: ['Alt+Enter'],
    enabled: hasActive,
    run: (s) => openEditorFor(s, activeLayerOf(s)!.color),
  },
  {
    id: 'layer.selectObjects',
    label: 'layers.cmd.selectObjects',
    category: 'edit',
    enabled: (s) => {
      const layer = activeLayerOf(s);
      if (!layer) return 'layers.reason.noActiveLayer';
      return s.doc.objects.some((o) => o.layer.toLowerCase() === layer.color.toLowerCase()) ? true : 'layers.reason.noObjects';
    },
    run: (s) => selectLayerObjects(s, activeLayerOf(s)!.color),
  },
  {
    id: 'layer.copySettings',
    label: 'layers.cmd.copySettings',
    category: 'edit',
    enabled: (s) => {
      const layer = activeLayerOf(s);
      if (!layer) return 'layers.reason.noActiveLayer';
      return isGuideColor(layer.color) ? 'layers.reason.guideNoSettings' : true;
    },
    run: (s) => copyLayerSettings(s, activeLayerOf(s)!.color),
  },
  {
    id: 'layer.pasteSettings',
    label: 'layers.cmd.pasteSettings',
    category: 'edit',
    enabled: (s) => {
      const layer = activeLayerOf(s);
      if (!layer) return 'layers.reason.noActiveLayer';
      if (isGuideColor(layer.color)) return 'layers.reason.guideNoSettings';
      return useLayerUi.getState().copied ? true : 'layers.reason.nothingCopied';
    },
    run: (s) => pasteLayerSettings(s, activeLayerOf(s)!.color),
  },
  {
    id: 'layer.delete',
    label: 'layers.cmd.delete',
    category: 'edit',
    enabled: (s) => {
      const layer = activeLayerOf(s);
      if (!layer) return 'layers.reason.noActiveLayer';
      return lockedInLayer(s.doc, layer.color) > 0 ? 'layers.reason.lockedObjects' : true;
    },
    run: async (s) => {
      await deleteLayerWithObjects(useStore.getState, activeLayerOf(s)!.color);
    },
  },
  {
    id: 'layer.saveDefaults',
    label: 'layers.cmd.saveDefaults',
    category: 'edit',
    enabled: (s) => {
      const layer = activeLayerOf(s);
      if (!layer) return 'layers.reason.noActiveLayer';
      return isGuideColor(layer.color) ? 'layers.reason.guideNoSettings' : true;
    },
    run: (s) => saveLayerAsDefaults(s, activeLayerOf(s)!.color),
  },
  {
    id: 'layer.resetDefaults',
    label: 'layers.cmd.resetDefaults',
    category: 'edit',
    enabled: (s) => (s.prefs.layerDefaults ? true : 'layers.reason.noCustomDefaults'),
    run: (s) => resetLayerDefaults(s),
  },
];
