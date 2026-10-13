import type { DeviceProfile } from '@/api/types';

/** Perfil provisional mientras no se ha cargado ninguno (cama de 400 x 400 mm). */
export const FALLBACK_DEVICE: DeviceProfile = {
  id: 'predeterminado',
  name: 'Máquina GRBL',
  kind: 'grbl',
  bed: [400, 400],
  origin: 'bottom_left',
  s_max: 1000,
  power_mode: 'dynamic',
  accel: [2000, 2000],
  max_rate: [20000, 20000],
  rapid_rate: 10000,
  baud: 115200,
  rx_buffer: 127,
  start_gcode: '',
  end_gcode: 'M5',
  return_to_origin: false,
};
