import type { AppState } from '@/store';
import type { CommandDef } from '../types';

/** Los controles de reproducción solo existen con la vista previa abierta y calculada. */
const hasPreview = (s: AppState) => (s.previewVisible && s.preview !== null && s.preview.moves.length > 0 ? true : 'preview.reason.unavailable');

/**
 * Atajos de la reproducción. Espacio no se usa: en el lienzo sirve para mover la vista, y
 * `Home`/`End` son del desplazamiento de los paneles. Una tecla, un significado.
 */
export const previewCommands: CommandDef[] = [
  // Orden de corte y recorridos: se aplican a la vista previa, el encuadre, el envío y la exportación.
  { id: 'preview.optimization', label: 'preview.cmd.optimization', category: 'machine', run: (s) => s.openDialog({ kind: 'optimization' }) },
  {
    id: 'view.preview',
    label: 'cmd.view.preview',
    category: 'view',
    shortcuts: ['P'],
    checked: (s) => s.previewVisible,
    // El cálculo lo dispara `usePreviewSync` al abrir (o ya estaba al día).
    run: (s) => s.togglePreview(),
  },
  {
    id: 'preview.play',
    label: 'preview.cmd.play',
    labelFor: (s) => (s.playing ? 'preview.cmd.pause' : 'preview.cmd.play'),
    category: 'view',
    shortcuts: ['K'],
    enabled: hasPreview,
    checked: (s) => s.playing,
    run: (s) => s.togglePlaying(),
  },
  {
    id: 'preview.slower',
    label: 'preview.cmd.slower',
    category: 'view',
    shortcuts: [','],
    enabled: hasPreview,
    run: (s) => s.stepPlaySpeed(-1),
  },
  {
    id: 'preview.faster',
    label: 'preview.cmd.faster',
    category: 'view',
    shortcuts: ['.'],
    enabled: hasPreview,
    run: (s) => s.stepPlaySpeed(1),
  },
];
