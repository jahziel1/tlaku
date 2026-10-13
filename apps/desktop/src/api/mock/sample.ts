import { ellipseBez, polylineBez, roundedRectBez } from '@/lib/bezpath';
import { DEFAULT_ROTARY, dialectForKind } from '@/lib/device';
import { shapes, translation } from '@/lib/geometry';
import { makeLayer } from '@/lib/layers';
import type { DesignObject, DeviceProfile, Document, LayerSettings } from '../types';

export const SAMPLE_COLORS = {
  cut: '#ff3b4a',
  engrave: '#2f7bff',
  mark: '#ffb020',
  photo: '#9aa4b2',
} as const;

/** Capas que trae el diseño de ejemplo. */
export function sampleLayers(): LayerSettings[] {
  return [
    makeLayer(SAMPLE_COLORS.cut, 'Corte', { speed: 420, power: 85, passes: 2, air: true }),
    makeLayer(SAMPLE_COLORS.engrave, 'Grabado', { mode: 'fill', speed: 3000, power: 32, interval: 0.4, overscan: 'auto' }),
    makeLayer(SAMPLE_COLORS.mark, 'Marcas', { speed: 1500, power: 20 }),
    makeLayer(SAMPLE_COLORS.photo, 'Foto', { mode: 'image', speed: 4500, power: 40, interval: 0.5 }),
  ];
}

/**
 * Diseño de ejemplo: una placa con engranaje grabado, perforaciones, una estrella de marcas
 * y una foto. Cabe en una cama de 400 x 400 mm.
 */
export function sampleObjects(): DesignObject[] {
  const base = { locked: false, hidden: false, group: null };
  return [
    {
      id: 'obj-placa',
      layer: SAMPLE_COLORS.cut,
      transform: translation(60, 70),
      kind: {
        type: 'vector',
        paths: [
          roundedRectBez(0, 0, 220, 140, 14),
          ...[
            [14, 14],
            [206, 14],
            [14, 126],
            [206, 126],
          ].map(([x, y]) => ellipseBez(x!, y!, 3.2, 3.2)),
        ],
      },
      ...base,
    },
    {
      id: 'obj-engranaje',
      layer: SAMPLE_COLORS.engrave,
      transform: translation(60, 70),
      kind: {
        type: 'vector',
        paths: [
          polylineBez(shapes.gear(62, 70, 42, 34, 18), true),
          ellipseBez(62, 70, 12, 12),
        ],
      },
      ...base,
    },
    {
      id: 'obj-estrella',
      layer: SAMPLE_COLORS.mark,
      transform: translation(60, 70),
      kind: {
        type: 'vector',
        paths: [
          polylineBez(shapes.star(176, 98, 22, 9), true),
          polylineBez([[128, 28], [206, 28]], false),
          polylineBez([[128, 22], [166, 22]], false),
        ],
      },
      ...base,
    },
    {
      id: 'obj-foto',
      layer: SAMPLE_COLORS.photo,
      transform: translation(110, 240),
      kind: { type: 'image', source_path: 'ejemplos/retrato.png', width_px: 600, height_px: 400, size_mm: [90, 60] },
      ...base,
    },
  ];
}

export function sampleDocument(): Document {
  return { version: 2, objects: sampleObjects(), layers: sampleLayers(), device_id: 'falcon2-22w' };
}

export function sampleDevices(): DeviceProfile[] {
  return [
    // Falcon2 22 W según docs/hardware/falcon2.md, sección 7 ("Perfil recomendado"). Las
    // aceleraciones son las seguras hasta leer $120/$121 de la máquina; el rotativo es el mandril
    // del Creality Rotary (40 mm por vuelta), apagado.
    {
      id: 'falcon2-22w',
      name: 'Falcon2 22 W',
      kind: 'grbl',
      dialect: dialectForKind('grbl'),
      bed: [400, 415],
      origin: 'bottom_left',
      s_max: 1000,
      power_mode: 'dynamic',
      accel: [1000, 1000],
      max_rate: [25000, 25000],
      rapid_rate: 25000,
      baud: 115200,
      rx_buffer: 127,
      start_gcode: '',
      end_gcode: '',
      return_to_origin: true,
      z_axis: false,
      z_invert: false,
      rotary: { ...DEFAULT_ROTARY, kind: 'chuck', mm_per_rotation: 40 },
      scan_offsets: [],
      scan_initial_offset: 0,
    },
    {
      id: 'taller-co2',
      name: 'Banco pequeño 300 x 200',
      kind: 'grbl',
      dialect: dialectForKind('grbl'),
      bed: [300, 200],
      origin: 'top_left',
      s_max: 255,
      power_mode: 'constant',
      accel: [1500, 1500],
      max_rate: [24000, 24000],
      rapid_rate: 8000,
      baud: 115200,
      rx_buffer: 127,
      start_gcode: '',
      end_gcode: 'M5',
      return_to_origin: false,
      z_axis: true,
      z_invert: false,
      rotary: { ...DEFAULT_ROTARY },
      scan_offsets: [],
      scan_initial_offset: 0,
    },
    // Un grblHAL de 32 bits, para ver en el gestor un tipo distinto de GRBL. Su dialecto es el de
    // GRBL 1.1 (como en el motor); el rotativo de rodillos viene con las cifras de xTool.
    {
      id: 'grblhal-500',
      name: 'grblHAL 500 x 500 (diodo)',
      kind: 'grbl_hal',
      dialect: dialectForKind('grbl_hal'),
      bed: [500, 500],
      origin: 'bottom_left',
      s_max: 1000,
      power_mode: 'dynamic',
      accel: [2000, 2000],
      max_rate: [30000, 30000],
      rapid_rate: 30000,
      baud: 115200,
      rx_buffer: 255,
      start_gcode: '',
      end_gcode: '',
      return_to_origin: true,
      z_axis: true,
      z_invert: false,
      rotary: { ...DEFAULT_ROTARY },
      scan_offsets: [{ speed: 6000, offset: 0.2 }],
      scan_initial_offset: 0,
    },
  ];
}
