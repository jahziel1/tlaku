import { getApiKind } from '@/api';
import { KOFI_URL, REPO_URL, SITE_URL } from './links';

/**
 * Abrir una dirección en el navegador del sistema. Solo se abren las del repositorio del proyecto y
 * su sitio: es lo que permite el plugin `opener` (alcance en `src-tauri/capabilities/default.json`) y
 * esta comprobación lo repite del lado de la interfaz. La app nunca envía nada por su cuenta: solo
 * abre la página y quien la usa decide si publica.
 */
const ALLOWED = [REPO_URL, SITE_URL];
/** Direcciones exactas fuera del proyecto: solo la página de donaciones, sin subrutas. */
const EXACT = [KOFI_URL];

/** ¿Es una dirección https dentro del repositorio o del sitio del proyecto? */
export function isAllowedUrl(url: string): boolean {
  let parsed: URL;
  try {
    parsed = new URL(url);
  } catch {
    return false;
  }
  if (parsed.protocol !== 'https:' || parsed.username || parsed.password) return false;
  const here = `${parsed.origin}${parsed.pathname}`;
  if (EXACT.includes(here)) return true;
  return ALLOWED.some((base) => here === base || here.startsWith(`${base}/`));
}

/** Cómo se abre de verdad; las pruebas lo reemplazan. */
export interface OpenEnv {
  open: (url: string) => Promise<void>;
}

export const openEnv: OpenEnv = {
  async open(url) {
    if (getApiKind() === 'tauri') {
      const { openUrl } = await import('@tauri-apps/plugin-opener');
      await openUrl(url);
      return;
    }
    // Fuera de Tauri (navegador y modo simulado) basta con una pestaña nueva.
    window.open(url, '_blank', 'noopener,noreferrer');
  },
};

/** Abre `url` en el navegador. Devuelve `false` si no está permitida o el sistema no pudo abrirla. */
export async function openExternal(url: string): Promise<boolean> {
  if (!isAllowedUrl(url)) return false;
  try {
    await openEnv.open(url);
    return true;
  } catch {
    return false;
  }
}
