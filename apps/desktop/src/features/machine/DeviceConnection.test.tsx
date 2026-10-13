import { renderToString } from 'react-dom/server';
import { beforeEach, describe, expect, it } from 'vitest';
import { setApi } from '@/api';
import { createMockApi } from '@/api/mock';
import { FALLBACK_DEVICE } from '@/lib/device';
import { DeviceConnection } from './DeviceConnection';
import { testConnection } from './DeviceConnectionProbe';

const noop = () => {};
const tcp = { ...FALLBACK_DEVICE, connection: { type: 'tcp' as const, host: '192.168.1.50', port: 23 } };

describe('selector de conexión del perfil', () => {
  it('serie: solo el selector de tipo, sin campos de red ni botón de prueba', () => {
    const h = renderToString(<DeviceConnection draft={FALLBACK_DEVICE} edit={noop} />);
    for (const s of ['Tipo de conexión', 'Puerto serie (USB)', 'Red · TCP / Telnet', 'Red · WebSocket']) expect(h).toContain(s);
    expect(h).not.toContain('Probar conexión');
    expect(h).not.toContain('Dirección de la máquina');
  });

  it('TCP: dirección, puerto y «Probar conexión»', () => {
    const h = renderToString(<DeviceConnection draft={tcp} edit={noop} />);
    for (const s of ['Dirección de la máquina', 'value="192.168.1.50"', 'Puerto', 'value="23"', 'Probar conexión', 'grblHAL (Ethernet) y FluidNC usan el 23']) expect(h).toContain(s);
    expect(h).not.toContain('role="alert"');
  });

  it('WebSocket: una sola dirección ws://', () => {
    const draft = { ...FALLBACK_DEVICE, connection: { type: 'web_socket' as const, url: 'ws://fluidnc.local:81/' } };
    const h = renderToString(<DeviceConnection draft={draft} edit={noop} />);
    expect(h).toContain('Dirección WebSocket');
    expect(h).toContain('value="ws://fluidnc.local:81/"');
    expect(h).toContain('Probar conexión');
  });

  it('una dirección mal escrita se marca, explica el error y bloquea la prueba', () => {
    const bad = { ...tcp, connection: { type: 'tcp' as const, host: 'tcp://x', port: 23 } };
    const h = renderToString(<DeviceConnection draft={bad} edit={noop} />);
    expect(h).toContain('Escribe la dirección de la máquina');
    expect(h).toContain('aria-invalid="true"');
    expect(h).toContain('Corrige la dirección antes de probar.');
    expect(h).toMatch(/<button[^>]*disabled=""[^>]*>.*Probar conexión/);
    const wss = { ...FALLBACK_DEVICE, connection: { type: 'web_socket' as const, url: 'wss://x' } };
    expect(renderToString(<DeviceConnection draft={wss} edit={noop} />)).toContain('no están soportadas');
  });

  it('Ruida (UDP) y las placas Lihuiyu tienen su propia conexión: aquí no se muestra nada', () => {
    const ruida = { ...FALLBACK_DEVICE, kind: 'ruida' as const, connection: { type: 'udp' as const, host: '10.0.0.9' } };
    expect(renderToString(<DeviceConnection draft={ruida} edit={noop} />)).toBe('');
    expect(renderToString(<DeviceConnection draft={{ ...FALLBACK_DEVICE, kind: 'lhy_micro' }} edit={noop} />)).toBe('');
  });

  it('un perfil viejo sin el campo se trata como puerto serie', () => {
    const old = { ...FALLBACK_DEVICE } as Partial<typeof FALLBACK_DEVICE>;
    delete old.connection;
    const h = renderToString(<DeviceConnection draft={old as typeof FALLBACK_DEVICE} edit={noop} />);
    expect(h).toContain('Puerto serie (USB)');
  });
});

describe('Probar conexión', () => {
  beforeEach(() => setApi(createMockApi({ latencyMs: 0 })));

  it('muestra el firmware que respondió', async () => {
    const r = await testConnection('tcp://192.168.1.50:23', 115200);
    expect(r).toEqual({ ok: true, text: expect.stringContaining('Grbl 1.1h') });
  });

  it('un equipo que no contesta da un mensaje claro, sin excepción', async () => {
    const r = await testConnection('tcp://no-responde.local:23', 115200);
    expect(r.ok).toBe(false);
    expect(r.text).toMatch(/^No se pudo conectar: .*no contestó a tiempo/);
  });
});
