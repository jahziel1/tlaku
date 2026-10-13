import { fmt, fmtClock } from '@/lib/platform';

/** 842 mm -> "842 mm"; 12 345 mm -> "12,35 m". */
export function fmtDistance(mm: number): string {
  if (mm >= 10000) return `${fmt(mm / 1000, 2)} m`;
  if (mm >= 1000) return `${fmt(mm / 1000, 3)} m`;
  return `${fmt(mm, mm >= 100 ? 0 : 1)} mm`;
}

/** Tiempos cortos con decimales ("3,4 s"); desde 10 s, reloj ("1:05"). */
export function fmtSeconds(s: number): string {
  return s < 10 ? `${fmt(s, 1)} s` : fmtClock(s);
}
