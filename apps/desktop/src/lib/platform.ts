export function isMac(): boolean {
  if (typeof navigator === 'undefined') return false;
  return /Mac|iPhone|iPad/.test(navigator.platform || navigator.userAgent);
}

/** Formato de número con la configuración regional (coma decimal en español). */
export function fmt(value: number, digits = 1): string {
  return new Intl.NumberFormat('es', {
    minimumFractionDigits: digits,
    maximumFractionDigits: digits,
  }).format(value);
}

/** 125 s -> "2:05". Con horas: "1:02:05". */
export function fmtClock(totalSeconds: number): string {
  const s = Math.max(0, Math.round(totalSeconds));
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const sec = s % 60;
  const two = (n: number) => String(n).padStart(2, '0');
  return h > 0 ? `${h}:${two(m)}:${two(sec)}` : `${m}:${two(sec)}`;
}

/** Lee una preferencia sin romperse si el almacenamiento no está disponible. */
export function readPref(key: string): string | null {
  try {
    return localStorage.getItem(`lasersoft.${key}`);
  } catch {
    return null;
  }
}

export function writePref(key: string, value: string): void {
  try {
    localStorage.setItem(`lasersoft.${key}`, value);
  } catch {
    /* sin almacenamiento: la preferencia no se recuerda */
  }
}
