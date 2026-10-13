import type { I18nKey } from '@/i18n';
import { useArtPanel } from '@/features/art/artStore';
import { bitmapEnabled } from '@/features/trace/bitmap';
import { useBitmapDialog } from '@/features/trace/bitmapStore';
import { useBatchDialog } from '@/features/trace/batchStore';
import { imageOpEnabled, replaceImage, saveProcessedImage } from '@/features/trace/imageOps';
import { applyMaskToDoc, maskEnabled, removeMaskFromDoc, splitMaskSelection, unmaskEnabled } from '@/features/trace/mask';
import { useTraceDialog } from '@/features/trace/traceStore';
import { selectSelectedObjects } from '@/store';
import type { AppState } from '@/store';
import type { CommandDef } from '../types';

/** Solo se vectoriza con exactamente una imagen seleccionada; si no, dice qué falta. */
export function traceEnabled(s: AppState): true | I18nKey {
  const picked = selectSelectedObjects(s);
  if (picked.length === 0) return 'trace.reason.needImage';
  if (picked.length > 1) return 'trace.reason.oneImage';
  return picked[0]!.kind.type === 'image' ? true : 'trace.reason.needImage';
}

export const traceCommands: CommandDef[] = [
  {
    id: 'tools.traceImage',
    label: 'trace.cmd.image',
    category: 'tools',
    enabled: traceEnabled,
    run: (s) => {
      if (traceEnabled(s) !== true) return;
      useTraceDialog.getState().open(s.selectedIds[0]!);
    },
  },
  {
    // D80: varias imágenes con los mismos ajustes; no necesita selección.
    id: 'tools.traceBatch',
    label: 'trace.cmd.batch',
    category: 'tools',
    run: () => useBatchDialog.getState().show(useTraceDialog.getState().settings),
  },
  {
    // D81: la imagen más una forma; la forma pasa a ser la máscara de la imagen.
    id: 'tools.maskImage',
    label: 'trace.cmd.mask',
    category: 'tools',
    enabled: (s) => maskEnabled(selectSelectedObjects(s)),
    run: (s) => {
      const picked = selectSelectedObjects(s);
      if (maskEnabled(picked) !== true) return;
      const parts = splitMaskSelection(picked);
      if (!parts) return;
      const imageId = parts.image.id;
      s.applyDocChange('trace.mask.history', (doc) =>
        applyMaskToDoc(doc, imageId, parts.shapes.map((o) => o.id)),
      );
      s.select([imageId]);
      void s.computePreview();
    },
  },
  {
    id: 'tools.unmaskImage',
    label: 'trace.cmd.unmask',
    category: 'tools',
    enabled: (s) => unmaskEnabled(selectSelectedObjects(s)),
    run: (s) => {
      const picked = selectSelectedObjects(s);
      if (unmaskEnabled(picked) !== true) return;
      const ids = picked.filter((o) => o.kind.type === 'image').map((o) => o.id);
      s.applyDocChange('trace.unmask.history', (doc) => removeMaskFromDoc(doc, ids));
      void s.computePreview();
    },
  },
  {
    // D82: vectores a imagen PNG a la resolución elegida, guardada junto al proyecto.
    id: 'tools.toBitmap',
    label: 'trace.cmd.bitmap',
    category: 'tools',
    enabled: (s) => bitmapEnabled(selectSelectedObjects(s)),
    run: (s) => {
      if (bitmapEnabled(selectSelectedObjects(s)) !== true) return;
      useBitmapDialog.getState().show();
    },
  },
  {
    id: 'tools.saveProcessedImage',
    label: 'trace.cmd.saveProcessed',
    category: 'tools',
    enabled: imageOpEnabled,
    run: async () => {
      await saveProcessedImage();
    },
  },
  {
    id: 'tools.replaceImage',
    label: 'trace.cmd.replace',
    category: 'tools',
    enabled: imageOpEnabled,
    run: async () => {
      await replaceImage();
    },
  },
  {
    // D47: biblioteca de arte (formas listas para insertar).
    id: 'tools.artLibrary',
    label: 'art.cmd.open',
    category: 'tools',
    checked: () => useArtPanel.getState().open,
    run: () => {
      const a = useArtPanel.getState();
      a.setOpen(!a.open);
    },
  },
];
