/**
 * Textos de los datos de fábrica (material, operación, clase, confianza, avisos). Cada clave de
 * los datos se traduce con una tabla tipada, así que una clave nueva sin traducir no compila.
 */
import type { Confidence, MachineClass, MaterialPreset, MaterialRole, MaterialWarning } from '@/api/types';
import { t, type I18nKey } from '@/i18n';

const MATERIAL: Record<string, I18nKey> = {
  mdf: 'materials.factory.material.mdf',
  plywood_birch: 'materials.factory.material.plywood_birch',
  plywood_basswood: 'materials.factory.material.plywood_basswood',
  basswood: 'materials.factory.material.basswood',
  acrylic_black: 'materials.factory.material.acrylic_black',
  acrylic_clear: 'materials.factory.material.acrylic_clear',
  leather: 'materials.factory.material.leather',
  cardboard: 'materials.factory.material.cardboard',
  paper: 'materials.factory.material.paper',
  cork: 'materials.factory.material.cork',
  slate: 'materials.factory.material.slate',
  anodized_aluminum: 'materials.factory.material.anodized_aluminum',
  stainless_steel: 'materials.factory.material.stainless_steel',
  glass: 'materials.factory.material.glass',
};

const TITLE: Record<string, I18nKey> = {
  cut: 'materials.factory.title.cut',
  engrave: 'materials.factory.title.engrave',
  score: 'materials.factory.title.score',
  mark_black: 'materials.factory.title.mark_black',
  mark_white: 'materials.factory.title.mark_white',
  mark_deep: 'materials.factory.title.mark_deep',
};

const CLASS: Record<MachineClass, I18nKey> = {
  diode_5: 'materials.class.diode_5',
  diode_10: 'materials.class.diode_10',
  diode_20: 'materials.class.diode_20',
  diode_40: 'materials.class.diode_40',
  co2_40: 'materials.class.co2_40',
  co2_60: 'materials.class.co2_60',
  co2_80: 'materials.class.co2_80',
  co2_100: 'materials.class.co2_100',
  fiber_20: 'materials.class.fiber_20',
  fiber_30: 'materials.class.fiber_30',
  fiber_50: 'materials.class.fiber_50',
};

const CONFIDENCE: Record<Confidence, { label: I18nKey; why: I18nKey }> = {
  official: { label: 'materials.confidence.official', why: 'materials.confidence.official.why' },
  guide: { label: 'materials.confidence.guide', why: 'materials.confidence.guide.why' },
  forum: { label: 'materials.confidence.forum', why: 'materials.confidence.forum.why' },
};

const ROLE: Record<MaterialRole, I18nKey> = {
  cut: 'materials.role.cut',
  engrave: 'materials.role.engrave',
  score: 'materials.role.score',
};

const WARNING: Record<MaterialWarning, I18nKey> = {
  not_cut_with_diode: 'materials.warn.not_cut_with_diode',
  chrome_leather: 'materials.warn.chrome_leather',
  needs_coating: 'materials.warn.needs_coating',
  range_edge: 'materials.warn.range_edge',
  generic_row: 'materials.warn.generic_row',
};

export const classLabel = (c: MachineClass): string => t(CLASS[c]);
export const roleLabel = (r: MaterialRole): string => t(ROLE[r]);
export const warningText = (w: MaterialWarning): string => t(WARNING[w]);
export const confidenceLabel = (c: Confidence): string => t(CONFIDENCE[c].label);
export const confidenceWhy = (c: Confidence): string => t(CONFIDENCE[c].why);

/** Nombre del material: traducido si viene de la biblioteca de fábrica, tal cual si lo escribió el usuario. */
export function materialName(p: Pick<MaterialPreset, 'material' | 'material_key'>): string {
  const key = p.material_key ? MATERIAL[p.material_key] : undefined;
  return key ? t(key) : p.material;
}

/** Nombre de la operación, con la misma regla. */
export function titleName(p: Pick<MaterialPreset, 'title' | 'title_key'>): string {
  const key = p.title_key ? TITLE[p.title_key] : undefined;
  return key ? t(key) : p.title;
}

/** Nombre de un material de fábrica a partir de su clave (para las restricciones). */
export function materialKeyName(materialKey: string): string {
  const key = MATERIAL[materialKey];
  return key ? t(key) : materialKey;
}

/** Sitio de una dirección ("eu.crealityfalcon.com"), para mostrarlo en el enlace. */
export function siteOf(url: string): string {
  try {
    return new URL(url).hostname.replace(/^www\./, '');
  } catch {
    return url;
  }
}
