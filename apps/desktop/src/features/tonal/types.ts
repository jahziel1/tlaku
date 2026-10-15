import type { ToneCurve } from '@/bindings/ToneCurve';

/** Rectángulo `[x0, y0, x1, y1]` en mm del documento (Y hacia arriba). */
export type Box = [number, number, number, number];

/** Qué simula el motor (`tonal_preview`, ver `docs/app-api/trabajo.md`). */
export interface TonalRequest {
  curve: ToneCurve;
  /** Ancho del punto del láser, mm. */
  spot_mm: number;
  /** Zona a dibujar; `null` = toda la hoja de material. */
  region_mm: Box | null;
  /** Lado mayor de la imagen, en píxeles. */
  max_px: number;
}

/** La imagen simulada y dónde va. */
export interface TonalPreview {
  /** PNG RGBA en base64 (sin prefijo `data:`); transparente fuera de la hoja. */
  png_base64: string;
  width_px: number;
  height_px: number;
  /** Rectángulo que cubre el PNG. */
  covered_mm: Box;
  /** Hoja de material supuesta (la caja de lo grabado más un margen). */
  sheet_mm: Box;
  /** Lado de un píxel, mm. */
  px_mm: number;
  /** Dosis máxima de la imagen. */
  max_dose: number;
}
