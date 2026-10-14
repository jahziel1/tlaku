import { renderToString } from 'react-dom/server';
import { afterEach, describe, expect, it } from 'vitest';
import type { RemoteClient, RemoteStatus } from '@/api/api';
import { en, es, setLanguage } from '@/i18n';
import { RemoteView, svgDataUrl, type RemoteViewProps } from './RemotePanel';

const client = (over: Partial<RemoteClient> = {}): RemoteClient => ({ id: 'a1b2c3d4', ip: '192.168.1.9', agent: 'iPhone · Safari', since_ms: 1, idle_s: 0, connected: true, ...over });

const running: RemoteStatus = {
  running: true,
  port: 47802,
  ip: '192.168.1.20',
  url: 'http://192.168.1.20:47802/',
  full_url: 'http://192.168.1.20:47802/#0123456789abcdef0123456789abcdef',
  qr_svg: '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><rect width="10" height="10"/></svg>',
  error: null,
  addrs: [
    { ip: '192.168.1.20', name: 'en0', recommended: true },
    { ip: '10.0.0.7', name: 'en1', recommended: false },
  ],
  clients: [],
};

const noop = () => undefined;
const props = (over: Partial<RemoteViewProps> = {}): RemoteViewProps => ({
  enabled: true,
  status: running,
  clients: [],
  framed: false,
  ip: null,
  onIp: noop,
  onCopy: noop,
  onRegenerate: noop,
  onKick: noop,
  ...over,
});

afterEach(() => setLanguage('es'));

describe('sección Control remoto de Preferencias', () => {
  it('apagado solo muestra el interruptor y el aviso, sin QR ni puerto', () => {
    const html = renderToString(<RemoteView {...props({ enabled: false, status: null })} />);
    expect(html).toContain(es['remote.enabled']);
    expect(html).toContain(es['remote.warn.http']);
    expect(html).not.toContain('<img');
    expect(html).not.toContain(es['remote.clients.title']);
  });

  it('encendido muestra el QR como imagen (no como HTML), el enlace sin la clave y los botones', () => {
    const html = renderToString(<RemoteView {...props()} />);
    expect(html).toContain('<img');
    expect(html).toContain('data:image/svg+xml;utf8,%3Csvg');
    expect(html).toContain('http://192.168.1.20:47802/');
    expect(html).not.toContain('0123456789abcdef0123456789abcdef');
    expect(html).toContain(es['remote.copy']);
    expect(html).toContain(es['remote.regenerate']);
    expect(html).toContain('Escuchando en 192.168.1.20:47802');
    expect(html).not.toMatch(/remote\.[a-z]/);
  });

  it('lista las direcciones de red disponibles con la automática primero', () => {
    const html = renderToString(<RemoteView {...props({ ip: '10.0.0.7' })} />);
    expect(html).toContain('Automática (192.168.1.20)');
    expect(html).toContain('192.168.1.20 (en0)');
    expect(html).toContain('10.0.0.7 (en1)');
  });

  it('lista quién está conectado, con su estado y un botón para desconectarlo', () => {
    const html = renderToString(<RemoteView {...props({ clients: [client(), client({ id: 'ffff0000', ip: '192.168.1.30', agent: 'Android · Chrome', connected: false, idle_s: 42 })] })} />);
    expect(html).toContain('iPhone · Safari · 192.168.1.9');
    expect(html).toContain('Android · Chrome · 192.168.1.30');
    expect(html).toContain('Conectado');
    expect(html).toContain('Sin actividad hace 42 s');
    expect((html.match(/Desconectar/g) ?? []).length).toBeGreaterThanOrEqual(2);
    expect(html).toContain('aria-label="Desconectar a iPhone · Safari · 192.168.1.9"');
    expect(renderToString(<RemoteView {...props()} />)).toContain(es['remote.clients.none']);
  });

  it('avisa si el trabajo no está encuadrado, y lo dice cuando sí', () => {
    expect(renderToString(<RemoteView {...props({ framed: false })} />)).toContain('encuadra');
    expect(renderToString(<RemoteView {...props({ framed: true })} />)).toContain(es['remote.framed.yes']);
  });

  it('muestra el error cuando no pudo arrancar', () => {
    const html = renderToString(<RemoteView {...props({ status: { ...running, running: false, error: 'El puerto 47802 ya lo usa otro programa; elige otro.', qr_svg: null, url: null, full_url: null } })} />);
    expect(html).toContain('No se pudo activar: El puerto 47802 ya lo usa otro programa');
    expect(html).not.toContain('<img');
  });

  it('también sale en inglés', () => {
    setLanguage('en');
    const html = renderToString(<RemoteView {...props({ clients: [client()] })} />);
    expect(html).toContain(en['remote.copy']);
    expect(html).toContain('Listening on 192.168.1.20:47802');
    expect(html).toContain('Disconnect');
    expect(html).not.toContain(es['remote.copy']);
  });

  it('el QR se convierte en una imagen de datos', () => {
    expect(svgDataUrl('<svg/>')).toBe('data:image/svg+xml;utf8,%3Csvg%2F%3E');
  });
});
