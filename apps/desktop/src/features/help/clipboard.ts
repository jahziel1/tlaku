/** Copia texto al portapapeles. Devuelve `false` si el sistema no lo permite (la app no se rompe). */
export async function copyText(text: string): Promise<boolean> {
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    /* sigue con el método antiguo */
  }
  try {
    const area = document.createElement('textarea');
    area.value = text;
    area.setAttribute('readonly', '');
    area.style.position = 'fixed';
    area.style.opacity = '0';
    document.body.appendChild(area);
    area.select();
    const ok = document.execCommand('copy');
    area.remove();
    return ok;
  } catch {
    return false;
  }
}

/** Datos de la instalación para pegar en un reporte. */
export function systemInfo(version: string | null, nav: { platform?: string; userAgent?: string } = {}): string {
  return [
    `Tlaku ${version ?? 'versión desconocida'}`,
    `Sistema: ${nav.platform || 'desconocido'}`,
    `Webview: ${nav.userAgent || 'desconocido'}`,
  ].join('\n');
}
