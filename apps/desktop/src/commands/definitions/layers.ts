import { openEditorFor, pickPaletteEntry, selectLayerObjects } from '@/features/layers/actions';
import { LAYER_PALETTE } from '@/lib/palette';
import type { AppState } from '@/store';
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
];
