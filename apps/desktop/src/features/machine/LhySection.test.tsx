import { renderToString } from 'react-dom/server';
import { beforeEach, describe, expect, it } from 'vitest';
import { setApi } from '@/api';
import { createMockApi } from '@/api/mock';
import type { DeviceProfile, LhyPreview, LhyState, LhyWarning } from '@/api/types';
import { setLanguage, t } from '@/i18n';
import { FALLBACK_DEVICE } from '@/lib/device';
import { useStore } from '@/store';
import { boardOptions, LhyFields } from './LhyFields';
import {
  isUnconfirmedStop,
  lhyConfirmKey,
  LhyExperimentalBadge,
  LhyNotApplicable,
  LhyPreviewBody,
  LhyStartNotice,
  LhyStopNoticeView,
  warningLines,
  warningText,
} from './LhyNotice';
import { LhyPanel, LhySection, statusText } from './LhySection';

const noop = () => {};

const k40 = (over: Partial<DeviceProfile> = {}): DeviceProfile => ({
  ...FALLBACK_DEVICE,
  id: 'k40',
  name: 'K40',
  kind: 'lhy_micro',
  bed: [300, 200],
  origin: 'top_left',
  lhymicro: { board: 'M2' },
  ...over,
});

const STATE: LhyState = { board: 'M2', virtual_machine: true, position_known: true, status_code: 206 };

const power = (layer: string | null, value: number): LhyWarning => ({ kind: 'power_ignored', block: 0, layer, value, fires: false });
const PREVIEW: LhyPreview = {
  board: 'M2',
  warnings: [power('#ff0000', 60), power('#ff0000', 60), { kind: 'gray_thresholded', block: 1, layer: 'Foto', value: null, fires: false }],
  packets: 120,
  bytes: 4080,
  est_seconds: 95,
  burn_mm: 320,
  travel_mm: 80,
  homes_first: true,
};

beforeEach(() => {
  setLanguage('es');
  setApi(createMockApi({ latencyMs: 0 }));
  useStore.setState({ devices: [FALLBACK_DEVICE], activeDeviceId: FALLBACK_DEVICE.id, jobMessage: '', jobError: null });
});

describe('panel del K40', () => {
  it('con un perfil GRBL no aparece', () => {
    expect(renderToString(<LhySection />)).toBe('');
  });

  it('sin conexión: etiqueta experimental, la potencia por perilla, qué no aplica y pide conectarse', () => {
    const h = renderToString(<LhyPanel connected={false} state={null} />);
    expect(h).toContain('K40 con M2 Nano');
    expect(h).toContain('Experimental');
    expect(h).toContain('no controla la potencia por software');
    expect(h).toContain('perilla del K40');
    expect(h).toContain('no usa G-code');
    expect(h).toContain('Conéctate a la placa');
    expect(h).toContain('no informa su posición real');
    expect(h).toContain('Moshiboard');
  });

  it('conectado y con home: placa, estado y posición contada', () => {
    const h = renderToString(<LhyPanel connected state={STATE} />);
    expect(h).toContain('M2');
    expect(h).toContain('Lista');
    expect(h).toContain('Contada desde el último home');
    expect(h).not.toContain('No se sabe dónde está el cabezal');
  });

  it('sin home avisa que no se moverá nada', () => {
    const h = renderToString(<LhyPanel connected state={{ ...STATE, position_known: false }} />);
    expect(h).toContain('Desconocida: haz home');
    expect(h).toContain('no se moverá nada hasta que hagas home');
  });

  it('239 avisa de la falta de energía (paro de emergencia, tapa, fuente)', () => {
    const h = renderToString(<LhyPanel connected state={{ ...STATE, status_code: 239 }} />);
    expect(h).toContain('energía insuficiente (239)');
    expect(statusText(239)).toBe('Sin energía');
    expect(statusText(77)).toBe('Código 77');
  });

  it('la insignia y los motivos de lo que no aplica', () => {
    expect(renderToString(<LhyExperimentalBadge />)).toContain('no con una placa real');
    expect(renderToString(<LhyNotApplicable reason="lhymicro.console.reason" />)).toContain('no usa G-code');
    expect(renderToString(<LhyNotApplicable reason="lhymicro.override.reason" />)).toContain('perilla');
    expect(renderToString(<LhyNotApplicable reason="lhymicro.axes.reason" />)).toContain('eje Z');
    expect(renderToString(<LhyNotApplicable reason="lhymicro.fire.reason" />)).toContain('disparo de prueba');
    expect(renderToString(<LhyNotApplicable reason="lhymicro.mset.reason" />)).toContain('«$»');
  });
});

describe('perfil del K40', () => {
  it('modelo de placa (la M2 marcada como la de serie), origen y notas', () => {
    const h = renderToString(<LhyFields draft={k40()} edit={noop} />);
    for (const s of ['Modelo de placa', 'M2 (la de serie)', 'B2', 'Origen y home', 'Arriba a la izquierda', 'perilla del K40', 'USB', 'Experimental']) expect(h).toContain(s);
  });

  it('una placa que no está en la lista se conserva en el selector', () => {
    expect(boardOptions('M2')).toEqual(['M2', 'M1', 'M', 'M3', 'A', 'B', 'B1', 'B2']);
    expect(boardOptions('X9').at(-1)).toBe('X9');
  });
});

describe('confirmar el envío con un K40', () => {
  it('con otro tipo de máquina no hay aviso ni texto propio', () => {
    expect(renderToString(<LhyStartNotice device={FALLBACK_DEVICE} />)).toBe('');
    expect(lhyConfirmKey(FALLBACK_DEVICE)).toBeNull();
    expect(lhyConfirmKey(k40())).toBe('lhymicro.start.confirm');
  });

  it('el aviso dice que es experimental y que la potencia sale de la perilla, antes de enviar', () => {
    const h = renderToString(<LhyStartNotice device={k40()} />);
    expect(h).toContain('K40 con M2 Nano (experimental)');
    expect(h).toContain('La placa M2 Nano de serie no controla la potencia por software: se ajusta con la perilla del K40');
    expect(h).toContain('Preparando el trabajo para la placa');
  });

  it('los avisos del codificador salen en español, sin repetir, con el nombre de la capa', () => {
    const h = renderToString(<LhyPreviewBody state={{ status: 'ready', preview: PREVIEW }} />);
    expect(h).toContain('120 paquetes');
    expect(h).toContain('irá primero al home');
    expect(h).toContain('#ff0000: pide 60 % de potencia, pero la placa no la controla');
    expect(h).toContain('Foto: los niveles de gris se binarizan');
    expect(warningLines(PREVIEW.warnings)).toHaveLength(2);
  });

  it('sin avisos lo dice; sin capa conocida usa el número de bloque; un pulso no es una pausa', () => {
    const clean = renderToString(<LhyPreviewBody state={{ status: 'ready', preview: { ...PREVIEW, warnings: [], homes_first: false } }} />);
    expect(clean).toContain('no encontró nada que la placa ignore');
    expect(clean).not.toContain('irá primero al home');
    expect(warningText(power(null, 40))).toContain('bloque 1');
    const dwell: LhyWarning = { kind: 'dwell_skipped', block: 2, layer: 'A', value: 100, fires: false };
    expect(warningText(dwell)).toContain('pausa de 100 ms');
    expect(warningText({ ...dwell, fires: true })).toContain('pulso de 100 ms');
    for (const kind of ['air_assist_ignored', 'z_ignored'] as const) expect(warningText({ kind, block: 0, layer: 'A', value: null, fires: false })).toContain('A:');
  });

  it('un error al preparar el trabajo se muestra', () => {
    const h = renderToString(<LhyPreviewBody state={{ status: 'error', message: 'bloque 0: velocidad 200 mm/s supera el máximo' }} />);
    expect(h).toContain('No se pudo preparar el trabajo');
    expect(h).toContain('supera el máximo');
  });

  it('en inglés los avisos también salen en inglés', () => {
    setLanguage('en');
    try {
      expect(warningText(power('Red', 60))).toBe("Red: asks for 60 % power, but the board does not control it: it will burn at whatever the K40's knob says.");
      const h = renderToString(<LhyStartNotice device={k40()} />);
      expect(h).toContain('does not control power by software');
      expect(t('lhymicro.start.confirm')).toBe('Send to the machine');
    } finally {
      setLanguage('es');
    }
  });
});

describe('detener sin confirmación', () => {
  const UNCONFIRMED =
    'cancelado por el usuario (paquetes confirmados: 3 de 9). Se intentó detener la placa y apagar el láser pero NO hubo confirmación: puede seguir en marcha. Usa el paro de emergencia o abre la tapa';

  it('reconoce la marca que deja el motor', () => {
    expect(isUnconfirmedStop(UNCONFIRMED)).toBe(true);
    expect(isUnconfirmedStop('Trabajo detenido: la placa se detuvo y el láser se apagó.')).toBe(false);
    expect(isUnconfirmedStop(null, undefined, '')).toBe(false);
  });

  it('muestra el aviso destacado del paro de emergencia', () => {
    const h = renderToString(<LhyStopNoticeView message={UNCONFIRMED} error={null} />);
    expect(h).toContain('role="alert"');
    expect(h).toContain('La placa no confirmó la parada');
    expect(h).toContain('paro de emergencia del K40');
  });

  it('también cuando el envío se rechazó antes del primer paquete', () => {
    expect(renderToString(<LhyStopNoticeView message="" error={UNCONFIRMED} />)).toContain('La placa no confirmó la parada');
  });

  it('una parada confirmada no lo muestra', () => {
    expect(renderToString(<LhyStopNoticeView message="Trabajo detenido: la placa se detuvo y el láser se apagó." error={null} />)).toBe('');
  });
});
