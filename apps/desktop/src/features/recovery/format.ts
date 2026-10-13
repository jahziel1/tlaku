import { locale } from '@/i18n';

/** Fecha y hora de una copia, en el idioma de la interfaz y en la zona horaria local. */
export function formatCopyDate(ms: number): string {
  return new Intl.DateTimeFormat(locale(), { dateStyle: 'medium', timeStyle: 'short' }).format(new Date(ms));
}

/** Tamaño legible: «820 KB», «3,4 MB». */
export function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  const kb = bytes / 1024;
  if (kb < 1024) return `${Math.round(kb)} KB`;
  return `${new Intl.NumberFormat(locale(), { maximumFractionDigits: 1 }).format(kb / 1024)} MB`;
}
