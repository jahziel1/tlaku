import type { DeviceProfile, LayerSettings } from '@/api/types';
import type { I18nKey } from '@/i18n';

type PowerMode = DeviceProfile['power_mode'];

/**
 * Reglas de compatibilidad entre los ajustes de una capa. Cada una devuelve el motivo (clave de
 * i18n) por el que un control no se puede usar, o `null` si está libre. Un control ya encendido
 * nunca se bloquea: así un documento que trae una combinación incompatible se puede arreglar.
 */
export interface LayerRules {
  /** Perforación y modo punto son dos formas de cortar a intervalos: no se combinan. */
  perforation: I18nKey | null;
  dotMode: I18nKey | null;
  /** La rampa necesita un trazo continuo: el modo punto dispara puntos sueltos. */
  ramp: I18nKey | null;
  /** Con relleno por contornos no hay ángulo de barrido que incrementar. */
  angleIncrement: I18nKey | null;
  /** Con pass-through la imagen se envía tal cual: no se trama ni se ajusta. */
  imageProcessing: I18nKey | null;
}

export function layerRules(layer: LayerSettings): LayerRules {
  return {
    perforation: layer.dot_mode.enabled && !layer.perforation.enabled ? 'layers.reason.perfDot' : null,
    dotMode: layer.perforation.enabled && !layer.dot_mode.enabled ? 'layers.reason.dotPerf' : null,
    ramp: layer.dot_mode.enabled && !layer.ramp.enabled ? 'layers.reason.rampDot' : null,
    angleIncrement: layer.mode === 'fill' && layer.fill_style === 'offset' ? 'layers.reason.offsetAngle' : null,
    imageProcessing: layer.mode === 'image' && layer.image.pass_through ? 'layers.reason.passThrough' : null,
  };
}

/** Combinaciones incompatibles que ya están encendidas a la vez (se avisan en el editor). */
export function activeConflicts(layer: LayerSettings): I18nKey[] {
  const out: I18nKey[] = [];
  if (layer.mode === 'line' && layer.dot_mode.enabled && layer.perforation.enabled) out.push('layers.warn.dotPerf');
  if (layer.mode === 'line' && layer.dot_mode.enabled && layer.ramp.enabled) out.push('layers.warn.dotRamp');
  return out;
}

/** La potencia mínima de una capa de línea solo cambia algo con potencia dinámica (M4). */
export function minPowerApplies(layer: Pick<LayerSettings, 'mode'>, powerMode: PowerMode): boolean {
  return layer.mode === 'image' || (layer.mode === 'line' && powerMode === 'dynamic');
}

export type AdvancedFeature =
  | 'perforation'
  | 'lead_in'
  | 'lead_out'
  | 'overcut'
  | 'delays'
  | 'cut_through'
  | 'dot_mode'
  | 'ramp'
  | 'min_power'
  | 'tabs'
  | 'fill_style'
  | 'fill_grouping'
  | 'angle_increment'
  | 'pass_through'
  | 'enhance'
  | 'denoise'
  | 'sub_layers';

/**
 * Ajustes avanzados que la capa ya usa y que el modo principiante oculta. Solo cuenta lo que el
 * modo de la capa muestra: un ajuste de línea en una capa de relleno no se aplica ni se oculta.
 */
export function advancedInUse(layer: LayerSettings, powerMode: PowerMode = 'dynamic'): AdvancedFeature[] {
  const out: AdvancedFeature[] = [];
  const mode = layer.mode;
  if (mode === 'line') {
    if (layer.perforation.enabled) out.push('perforation');
    if (layer.lead_in.enabled) out.push('lead_in');
    if (layer.lead_out.enabled) out.push('lead_out');
    if (layer.overcut > 0) out.push('overcut');
    if (layer.start_delay_ms > 0 || layer.end_delay_ms > 0) out.push('delays');
    if (layer.cut_through.enabled) out.push('cut_through');
    if (layer.dot_mode.enabled) out.push('dot_mode');
    if (layer.ramp.enabled) out.push('ramp');
    if (layer.tabs.enabled) out.push('tabs');
    if (minPowerApplies(layer, powerMode) && layer.min_power > 0) out.push('min_power');
  }
  if (mode === 'fill') {
    if (layer.fill_style !== 'scan') out.push('fill_style');
    if (layer.fill_grouping !== 'all') out.push('fill_grouping');
  }
  if (mode !== 'line' && layer.angle_increment !== 0) out.push('angle_increment');
  if (mode === 'image') {
    if (layer.image.pass_through) out.push('pass_through');
    if (layer.image.enhance_radius > 0 && layer.image.enhance_amount > 0) out.push('enhance');
    if (layer.image.denoise > 0) out.push('denoise');
  }
  if (layer.sub_layers.length > 0) out.push('sub_layers');
  return out;
}
