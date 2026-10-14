/**
 * Dirección de una cámara de red (M35): validar lo que escribe el usuario, ocultar la clave al
 * mostrarla y quitarla al exportar. La URL puede llevar `usuario:clave@`: se guarda solo en el
 * perfil local (`camera.json`) y nunca entra en un paquete `.tlakubundle`. El motor (el proxy
 * `fetch_camera_frame`) vuelve a comprobar todo esto y solo habla con la red local.
 */

export type UrlProblem = 'empty' | 'https' | 'scheme' | 'invalid';

export type ParsedUrl = { ok: true; url: string } | { ok: false; reason: UrlProblem };

/** Comprueba una URL `http://` y la devuelve sin espacios. */
export function parseNetworkUrl(raw: string): ParsedUrl {
  const text = raw.trim();
  if (text === '') return { ok: false, reason: 'empty' };
  if (/^https:\/\//i.test(text)) return { ok: false, reason: 'https' };
  if (!/^http:\/\//i.test(text)) return { ok: false, reason: 'scheme' };
  if (text.length > 2048 || /\s/.test(text) || [...text].some((ch) => ch.charCodeAt(0) < 32 || ch.charCodeAt(0) === 127)) return { ok: false, reason: 'invalid' };
  try {
    const u = new URL(text);
    if (u.hostname === '') return { ok: false, reason: 'invalid' };
  } catch {
    return { ok: false, reason: 'invalid' };
  }
  return { ok: true, url: text };
}

export function hasCredentials(url: string): boolean {
  return /^https?:\/\/[^/?#]*@/i.test(url);
}

/** Usuario y clave de la URL (todo lo que va antes de la última `@` de la dirección). */
const USERINFO = /^(https?:\/\/)([^/?#]*)@/i;

/** La misma URL con la clave como `***` (para mostrarla). Sin clave queda igual. */
export function maskCredentials(url: string): string {
  return url.replace(USERINFO, (all, scheme: string, info: string) => {
    const colon = info.indexOf(':');
    return colon < 0 ? all : `${scheme}${info.slice(0, colon)}:***@`;
  });
}

/** La URL sin usuario ni clave (para exportar). Una URL vacía o que no es `http(s)://` se descarta (`null`). */
export function stripCredentials(url: string | null | undefined): string | null {
  if (!url || !/^https?:\/\//i.test(url)) return null;
  return url.replace(USERINFO, '$1');
}

/**
 * ¿Parece una dirección de la red local? Privadas (10.x, 172.16-31.x, 192.168.x), de enlace local,
 * `localhost`, nombres `.local` o sin punto. Solo para avisar: el motor impone la regla de verdad.
 */
export function looksLocal(url: string): boolean {
  let host: string;
  try {
    host = new URL(url).hostname.toLowerCase();
  } catch {
    return false;
  }
  if (host === 'localhost' || host.endsWith('.local') || host.endsWith('.lan') || host.endsWith('.home')) return true;
  if (host.startsWith('[')) {
    const v6 = host.slice(1, -1);
    return v6 === '::1' || /^f[cd]/.test(v6) || /^fe[89ab]/.test(v6);
  }
  const m = /^(\d{1,3})\.(\d{1,3})\.(\d{1,3})\.(\d{1,3})$/.exec(host);
  if (!m) return !host.includes('.');
  const [a, b] = [Number(m[1]), Number(m[2])];
  return a === 10 || a === 127 || (a === 172 && b >= 16 && b <= 31) || (a === 192 && b === 168) || (a === 169 && b === 254);
}
