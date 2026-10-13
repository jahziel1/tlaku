import type { CodeKind, CodeSpec, QrEcc } from '@/api/types';
import type { I18nKey } from '@/i18n';

export interface KindInfo {
  kind: CodeKind;
  /** Código de barras de una dimensión: tiene alto propio y texto legible. */
  linear: boolean;
  label: I18nKey;
  hint: I18nKey;
}

/** Tipos de código en el orden en que se ofrecen. */
export const KINDS: KindInfo[] = [
  { kind: 'qr', linear: false, label: 'codes.kind.qr', hint: 'codes.hint.qr' },
  { kind: 'data_matrix', linear: false, label: 'codes.kind.data_matrix', hint: 'codes.hint.data_matrix' },
  { kind: 'code128', linear: true, label: 'codes.kind.code128', hint: 'codes.hint.code128' },
  { kind: 'code39', linear: true, label: 'codes.kind.code39', hint: 'codes.hint.code39' },
  { kind: 'ean13', linear: true, label: 'codes.kind.ean13', hint: 'codes.hint.ean13' },
  { kind: 'ean8', linear: true, label: 'codes.kind.ean8', hint: 'codes.hint.ean8' },
  { kind: 'upc_a', linear: true, label: 'codes.kind.upc_a', hint: 'codes.hint.upc_a' },
  { kind: 'itf', linear: true, label: 'codes.kind.itf', hint: 'codes.hint.itf' },
  { kind: 'codabar', linear: true, label: 'codes.kind.codabar', hint: 'codes.hint.codabar' },
];

export const kindInfo = (kind: CodeKind): KindInfo => KINDS.find((k) => k.kind === kind) ?? KINDS[0]!;

export const ECC_LEVELS: { value: QrEcc; label: I18nKey }[] = [
  { value: 'l', label: 'codes.ecc.l' },
  { value: 'm', label: 'codes.ecc.m' },
  { value: 'q', label: 'codes.ecc.q' },
  { value: 'h', label: 'codes.ecc.h' },
];

export const DEFAULT_CODE_SPEC: CodeSpec = {
  kind: 'qr',
  data: '',
  ecc: 'm',
  size_mm: 30,
  height_mm: 15,
  margin_mm: 2,
  human_readable: false,
  invert: false,
  font_family: '',
};

/** Límites de los campos numéricos (mm). */
export const CODE_LIMITS = { size: [2, 1000], height: [1, 1000], margin: [0, 100] } as const;
