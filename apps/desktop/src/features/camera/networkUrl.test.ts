import { describe, expect, it } from 'vitest';
import { hasCredentials, looksLocal, maskCredentials, parseNetworkUrl, stripCredentials } from './networkUrl';

describe('dirección de la cámara de red', () => {
  it('acepta http y explica por qué rechaza lo demás', () => {
    expect(parseNetworkUrl('  http://192.168.1.20/snapshot.jpg ')).toEqual({ ok: true, url: 'http://192.168.1.20/snapshot.jpg' });
    expect(parseNetworkUrl('HTTP://cam.local:8080/video.mjpg').ok).toBe(true);
    expect(parseNetworkUrl('')).toEqual({ ok: false, reason: 'empty' });
    expect(parseNetworkUrl('https://192.168.1.20/')).toEqual({ ok: false, reason: 'https' });
    expect(parseNetworkUrl('rtsp://192.168.1.20/')).toEqual({ ok: false, reason: 'scheme' });
    expect(parseNetworkUrl('192.168.1.20')).toEqual({ ok: false, reason: 'scheme' });
    expect(parseNetworkUrl('http://')).toEqual({ ok: false, reason: 'invalid' });
    expect(parseNetworkUrl('http://cam /x')).toEqual({ ok: false, reason: 'invalid' });
  });

  it('oculta la clave al mostrarla y la quita al exportar', () => {
    const url = 'http://ana:s3cr3t@192.168.1.20:8080/snap.jpg?ch=1';
    expect(hasCredentials(url)).toBe(true);
    expect(hasCredentials('http://192.168.1.20/snap.jpg')).toBe(false);
    expect(hasCredentials('http://192.168.1.20/ruta@rara')).toBe(false);
    expect(maskCredentials(url)).toBe('http://ana:***@192.168.1.20:8080/snap.jpg?ch=1');
    expect(maskCredentials(url)).not.toContain('s3cr3t');
    expect(stripCredentials(url)).toBe('http://192.168.1.20:8080/snap.jpg?ch=1');
    // Sin clave, sin cambios; sin usuario el texto se queda igual.
    expect(maskCredentials('http://192.168.1.20/a')).toBe('http://192.168.1.20/a');
    expect(maskCredentials('http://soloUsuario@192.168.1.20/a')).toBe('http://soloUsuario@192.168.1.20/a');
    expect(stripCredentials('http://192.168.1.20/a')).toBe('http://192.168.1.20/a');
    expect(stripCredentials('http://a:p%40ss@host/a')).toBe('http://host/a');
    expect(stripCredentials(null)).toBeNull();
    expect(stripCredentials('no es una url')).toBeNull();
  });

  it('avisa cuando la dirección no parece de la red local', () => {
    for (const u of ['http://192.168.0.5/', 'http://10.1.2.3/', 'http://172.20.0.9/', 'http://169.254.1.1/', 'http://localhost:8080/', 'http://camara.local/', 'http://camara/', 'http://[::1]/', 'http://[fd00::5]/']) {
      expect(looksLocal(u), u).toBe(true);
    }
    for (const u of ['http://8.8.8.8/', 'http://172.32.0.1/', 'http://example.com/snap.jpg', 'http://[2606:4700::1111]/']) {
      expect(looksLocal(u), u).toBe(false);
    }
  });
});
