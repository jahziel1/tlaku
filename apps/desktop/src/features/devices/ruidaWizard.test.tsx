import { renderToString } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import { applyKindChange, DEVICE_KINDS, FALLBACK_DEVICE, kindDefaults } from '@/lib/device';
import { buildProfile } from './profile';
import { SaveStep } from './steps';

const noop = () => {};

const build = () =>
  buildProfile({ entry: null, kind: 'ruida', name: 'Mi CO2', takenIds: [], probedBaud: null, settings: null, useMachine: new Set() });

describe('Agregar máquina con una Ruida', () => {
  it('Ruida se ofrece entre los tipos', () => {
    expect(DEVICE_KINDS).toContain('ruida');
  });

  it('el perfil nuevo habla UDP, sin IP todavía, con la cama y el home de partida', () => {
    const { profile, provenance } = build();
    expect(profile.kind).toBe('ruida');
    expect(profile.connection).toEqual({ type: 'udp', host: '' });
    expect(profile.bed).toEqual([600, 400]);
    expect(profile.origin).toBe('top_left');
    // Son suposiciones: el asistente las marca como valores de partida.
    expect(provenance.bed).toBe('default');
    expect(provenance.origin).toBe('default');
    expect(profile.ruida).toEqual({ magic: null, run_after_upload: false });
  });

  it('el último paso pide la IP y no muestra baudios ni S máxima', () => {
    const h = renderToString(<SaveStep name="Mi CO2" onName={noop} built={build()} host="192.168.1.100" onHost={noop} />);
    expect(h).toContain('IP de la controladora Ruida');
    expect(h).toContain('192.168.1.100');
    expect(h).toContain('Experimental');
    expect(h).not.toContain('Baudios');
    expect(h).not.toContain('S máxima');
  });

  it('con otro tipo no hay campo de IP', () => {
    const grbl = buildProfile({ entry: null, kind: 'grbl', name: 'X', takenIds: [], probedBaud: null, settings: null, useMachine: new Set() });
    const h = renderToString(<SaveStep name="X" onName={noop} built={grbl} host="" onHost={noop} />);
    expect(h).not.toContain('IP de la controladora Ruida');
    // Y en cambio sí muestra los baudios y la S máxima, que con Ruida no aplican.
    expect(h).toContain('Baudios');
    expect(h).toContain('S máxima');
  });

  it('cambiar el tipo a Ruida y volver ajusta la conexión del perfil', () => {
    const toRuida = applyKindChange(FALLBACK_DEVICE, kindDefaults('ruida'), false);
    expect(toRuida.connection).toEqual({ type: 'udp', host: '' });
    const withIp = { ...toRuida, connection: { type: 'udp' as const, host: '10.0.0.7' } };
    // Dentro de Ruida la IP escrita se conserva.
    expect(applyKindChange(withIp, kindDefaults('ruida'), false).connection).toEqual({ type: 'udp', host: '10.0.0.7' });
    expect(applyKindChange(withIp, kindDefaults('grbl'), false).connection).toEqual({ type: 'serial' });
    // Una conexión TCP de otro tipo no se pisa.
    const tcp = { ...FALLBACK_DEVICE, connection: { type: 'tcp' as const, host: 'h', port: 23 } };
    expect(applyKindChange(tcp, kindDefaults('fluid_nc'), false).connection).toEqual(tcp.connection);
  });
});
