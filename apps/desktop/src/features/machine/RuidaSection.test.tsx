import { renderToString } from 'react-dom/server';
import { beforeEach, describe, expect, it } from 'vitest';
import { setApi } from '@/api';
import { createMockApi } from '@/api/mock';
import type { DeviceProfile, RuidaInfo } from '@/api/types';
import { FALLBACK_DEVICE } from '@/lib/device';
import { useStore } from '@/store';
import { RuidaFields } from './RuidaFields';
import { selectIsRuida } from './ruidaPorts';
import { ruidaConfirmKey, RuidaStartNotice } from './RuidaNotice';
import { bedMismatch, infoRows, RuidaExperimentalBadge, RuidaNotApplicable, RuidaPanel, RuidaSection } from './RuidaSection';

const noop = () => {};

const ruida = (over: Partial<DeviceProfile> = {}): DeviceProfile => ({
  ...FALLBACK_DEVICE,
  id: 'ruida-1',
  name: 'CO2 6040',
  kind: 'ruida',
  bed: [600, 400],
  connection: { type: 'udp', host: '192.168.1.100' },
  ruida: { magic: null, run_after_upload: false },
  ...over,
});

const INFO: RuidaInfo = {
  model: 'RDC6442S',
  card_id: '0x65106510',
  magic: 0x88,
  bed: [600, 400],
  rapid_mm_s: 200,
  home_mm_s: null,
  axis_max_mm_s: [400, 400],
  axis_accel_mm_s2: [3000, null],
  total_on_s: 7500,
  total_work_s: 3600,
  total_laser_s: null,
  total_jobs: 42,
  travel_m: [120, 95],
  documents: 2,
  board_version: 'RD-6442',
  previous_work: 90,
  control_mode: 0x4000,
  focus: true,
  z_return_docking: false,
  air_assist_mode: 2,
};

beforeEach(() => {
  setApi(createMockApi({ latencyMs: 0 }));
  useStore.setState({ devices: [FALLBACK_DEVICE], activeDeviceId: FALLBACK_DEVICE.id });
});

describe('panel de la controladora Ruida', () => {
  it('con un perfil GRBL no aparece', () => {
    expect(renderToString(<RuidaSection />)).toBe('');
  });

  it('el perfil activo decide si se muestra', () => {
    const s = { devices: [FALLBACK_DEVICE, ruida()], activeDeviceId: 'ruida-1' };
    expect(selectIsRuida(s)).toBe(true);
    expect(selectIsRuida({ ...s, activeDeviceId: FALLBACK_DEVICE.id })).toBe(false);
  });

  it('con un perfil Ruida muestra la etiqueta experimental, el motivo del G-code y pide conectarse', () => {
    const h = renderToString(<RuidaPanel device={ruida()} connected={false} busy={false} />);
    expect(h).toContain('Controladora Ruida');
    expect(h).toContain('Experimental');
    expect(h).toContain('no usa G-code');
    expect(h).toContain('Conéctate a la controladora');
    // Sin conexión no se puede leer.
    expect(h.match(/disabled/g)?.length).toBeGreaterThanOrEqual(2);
    expect(h).toContain('Leer información');
    expect(h).toContain('Leer lista');
  });

  it('ofrece los ajustes de la controladora marcados como experimentales y deshabilitados sin conexión', () => {
    const h = renderToString(<RuidaPanel device={ruida()} connected={false} busy={false} />);
    expect(h).toContain('Ajustes de la controladora…');
    expect(h).toContain('EXPERIMENTAL: sin probar con una controladora real');
    expect(h).not.toContain('Escribir los ajustes de la controladora no está disponible');
    // La lista de documentos ya no es solo de lectura: ejecutar y borrar piden confirmación.
    expect(h).toContain('Experimental: la numeración');
  });

  it('la insignia dice que es experimental y por qué', () => {
    const h = renderToString(<RuidaExperimentalBadge />);
    expect(h).toContain('Experimental');
    expect(h).toContain('no con una máquina real');
  });

  it('la tabla de información no inventa lo que la máquina no informó', () => {
    const rows = Object.fromEntries(infoRows(INFO));
    expect(rows['Modelo']).toBe('RDC6442S');
    expect(rows['ID de tarjeta']).toBe('0x65106510');
    expect(rows['Magic']).toBe('0x88');
    expect(rows['Cama']).toMatch(/600 × 400 mm/);
    expect(rows['Velocidad de home']).toBe('no informado');
    expect(rows['Aceleración máxima (X / Y)']).toContain('—');
    expect(rows['Tiempo encendida']).toBe('2 h 05 min');
    expect(rows['Tiempo de láser']).toBe('no informado');
    expect(rows['Documentos guardados']).toBe('2');
    expect(rows['Versión de la placa']).toBe('RD-6442');
    expect(rows['Modo de control del sistema']).toBe('sin retorno');
    expect(rows['Funciones']).toBe('foco, aire: modo 2');
    const blind = Object.fromEntries(infoRows({ ...INFO, board_version: null, control_mode: null, focus: null, z_return_docking: null, air_assist_mode: null }));
    expect(blind['Funciones']).toBe('no informado');
    expect(blind['Modo de control del sistema']).toBe('no informado');
    // Un modelo desconocido se dice así.
    expect(Object.fromEntries(infoRows({ ...INFO, model: null }))['Modelo']).toBe('modelo desconocido');
  });

  it('avisa si la cama del perfil no es la de la máquina (más de 1 mm)', () => {
    expect(bedMismatch([600, 400], [600, 400])).toBe(false);
    expect(bedMismatch([600, 400], [600.5, 400])).toBe(false);
    expect(bedMismatch([400, 300], [600, 400])).toBe(true);
    expect(bedMismatch([600, 400], null)).toBe(false);
  });
});

describe('lo que no aplica a Ruida', () => {
  it('los ajustes en vivo y la consola dicen por qué no aplican', () => {
    const o = renderToString(<RuidaNotApplicable reason="ruida.override.reason" />);
    expect(o).toContain('son de GRBL');
    const c = renderToString(<RuidaNotApplicable reason="ruida.console.reason" />);
    expect(c).toContain('no usa G-code');
  });
});

describe('campos del perfil Ruida', () => {
  it('IP, magic automático, ejecutar al subir y las notas de home y rotativo', () => {
    const h = renderToString(<RuidaFields draft={ruida()} edit={noop} />);
    for (const s of ['IP de la controladora', '192.168.1.100', 'Automático', 'Ejecutar al subir', 'Dónde está el home', 'rotativo de Ruida', 'Experimental']) expect(h).toContain(s);
    expect(h).not.toContain('Valor del magic');
    expect(h).not.toContain('Sin IP solo podrás');
  });

  it('con el magic a mano aparece el valor en hexadecimal', () => {
    const h = renderToString(<RuidaFields draft={ruida({ ruida: { magic: 0x11, run_after_upload: false } })} edit={noop} />);
    expect(h).toContain('Valor del magic');
    expect(h).toContain('0x11');
  });

  it('sin IP avisa que solo funcionará la máquina virtual; ejecutar al subir advierte del láser', () => {
    const h = renderToString(<RuidaFields draft={ruida({ connection: { type: 'udp', host: '' }, ruida: { magic: null, run_after_upload: true } })} edit={noop} />);
    expect(h).toContain('Sin IP solo podrás usar la máquina virtual');
    expect(h).toContain('EMPEZARÁ a ejecutarse');
  });
});

describe('confirmar el envío con Ruida', () => {
  it('dice si el trabajo se ejecuta o solo se guarda, y que es experimental', () => {
    const store = renderToString(<RuidaStartNotice device={ruida()} />);
    expect(store).toContain('Controladora Ruida (experimental)');
    expect(store).toContain('NO arrancará');
    const run = renderToString(<RuidaStartNotice device={ruida({ ruida: { magic: null, run_after_upload: true } })} />);
    expect(run).toContain('EMPEZARÁ a ejecutarse');
    expect(renderToString(<RuidaStartNotice device={FALLBACK_DEVICE} />)).toBe('');
  });

  it('el botón de confirmar cambia con el modo', () => {
    expect(ruidaConfirmKey(ruida())).toBe('ruida.start.confirmStore');
    expect(ruidaConfirmKey(ruida({ ruida: { magic: null, run_after_upload: true } }))).toBe('ruida.start.confirmRun');
    expect(ruidaConfirmKey(FALLBACK_DEVICE)).toBeNull();
  });
});
