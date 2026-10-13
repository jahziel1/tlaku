import { describe, expect, it } from 'vitest';
import {
  connectionAddress,
  connectionIssue,
  connectionPortId,
  isNetworkConnection,
  isNetworkPortId,
  isValidHost,
  withConnectionType,
} from './DeviceConnectionModel';

describe('validación de la conexión por red', () => {
  it('acepta IP, nombres y IPv6', () => {
    for (const h of ['192.168.1.50', 'fluidnc.local', 'laser-1', 'mi_maquina.lan', '::1', 'fe80::1']) expect(isValidHost(h), h).toBe(true);
  });

  it('rechaza lo que no es una dirección', () => {
    for (const h of ['', '  ', 'tcp://10.0.0.1', '10.0.0.1/ruta', 'dos palabras', '-malo.local', 'a'.repeat(300), 'host:80']) expect(isValidHost(h), h).toBe(false);
  });

  it('TCP: host y puerto obligatorios y en rango', () => {
    expect(connectionIssue({ type: 'tcp', host: '10.0.0.5', port: 23 })).toBeNull();
    expect(connectionIssue({ type: 'tcp', host: '', port: 23 })).toBe('host');
    expect(connectionIssue({ type: 'tcp', host: '10.0.0.5', port: 0 })).toBe('port');
    expect(connectionIssue({ type: 'tcp', host: '10.0.0.5', port: 70000 })).toBe('port');
    expect(connectionIssue({ type: 'tcp', host: '10.0.0.5', port: 23.5 })).toBe('port');
  });

  it('WebSocket: solo ws://, con host y puerto válidos', () => {
    for (const url of ['ws://10.0.0.5', 'ws://10.0.0.5:81', 'ws://fluidnc.local:81/', 'WS://host/ws?x=1', 'ws://[::1]:81/']) {
      expect(connectionIssue({ type: 'web_socket', url }), url).toBeNull();
    }
    expect(connectionIssue({ type: 'web_socket', url: 'wss://10.0.0.5' })).toBe('wss');
    for (const url of ['', 'ws://', 'http://10.0.0.5', '10.0.0.5:81', 'ws://host:', 'ws://host:99999', 'ws://ho st']) {
      expect(connectionIssue({ type: 'web_socket', url }), url).toBe('url');
    }
  });

  it('serie, UDP y perfiles viejos sin el campo no tienen nada que validar aquí', () => {
    expect(connectionIssue({ type: 'serial' })).toBeNull();
    expect(connectionIssue({ type: 'udp', host: '192.168.1.9' })).toBeNull();
    expect(connectionIssue(undefined)).toBeNull();
  });
});

describe('dirección que se entrega al motor', () => {
  it('TCP sale como tcp://host:puerto y WebSocket como su URL', () => {
    expect(connectionPortId({ type: 'tcp', host: '192.168.1.50', port: 23 })).toBe('tcp://192.168.1.50:23');
    expect(connectionPortId({ type: 'tcp', host: '::1', port: 99 })).toBe('tcp://[::1]:99');
    expect(connectionPortId({ type: 'web_socket', url: ' ws://fluidnc.local:81/ ' })).toBe('ws://fluidnc.local:81/');
  });

  it('serie y UDP no la tienen: se usa el puerto elegido', () => {
    expect(connectionPortId({ type: 'serial' })).toBeNull();
    expect(connectionPortId({ type: 'udp', host: '10.0.0.9' })).toBeNull();
    expect(connectionPortId(undefined)).toBeNull();
  });

  it('se muestra sin el esquema tcp://', () => {
    expect(connectionAddress({ type: 'tcp', host: '10.0.0.5', port: 23 })).toBe('10.0.0.5:23');
    expect(connectionAddress({ type: 'serial' })).toBeNull();
  });

  it('reconoce las direcciones de red y no confunde los puertos serie', () => {
    expect(isNetworkPortId('tcp://10.0.0.5:23')).toBe(true);
    expect(isNetworkPortId('ws://h:81')).toBe(true);
    for (const p of ['/dev/cu.usbserial-1', 'COM3', 'virtual:falcon2', 'virtualnet:falcon2', '', null, undefined]) expect(isNetworkPortId(p)).toBe(false);
    expect(isNetworkConnection({ type: 'tcp', host: 'a', port: 1 })).toBe(true);
    expect(isNetworkConnection({ type: 'udp', host: 'a' })).toBe(false);
  });
});

describe('cambio de tipo', () => {
  it('conserva el host al pasar de TCP a WebSocket y al revés', () => {
    const ws = withConnectionType({ type: 'tcp', host: '10.0.0.5', port: 23 }, 'web_socket');
    expect(ws).toEqual({ type: 'web_socket', url: 'ws://10.0.0.5:81' });
    expect(withConnectionType(ws, 'tcp')).toEqual({ type: 'tcp', host: '10.0.0.5', port: 23 });
  });

  it('a serie no deja nada y el mismo tipo no cambia el objeto', () => {
    expect(withConnectionType({ type: 'tcp', host: 'a', port: 5 }, 'serial')).toEqual({ type: 'serial' });
    const c = { type: 'tcp', host: 'a', port: 5 } as const;
    expect(withConnectionType(c, 'tcp')).toBe(c);
    expect(withConnectionType(undefined, 'tcp')).toEqual({ type: 'tcp', host: '', port: 23 });
  });
});
