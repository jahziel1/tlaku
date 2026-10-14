import { renderToString } from 'react-dom/server';
import { beforeEach, describe, expect, it } from 'vitest';
import { setApi } from '@/api';
import { createMockApi } from '@/api/mock';
import type { DeviceProfile, GalvoPreview, GalvoState, GalvoStepInfo } from '@/api/types';
import { setLanguage } from '@/i18n';
import { FALLBACK_DEVICE } from '@/lib/device';
import { useStore } from '@/store';
import { galvoDevice } from './galvoFixtures';
import { GalvoFields } from './GalvoFields';
import {
  galvoConfirmKey,
  GalvoExperimentalBadge,
  GalvoMoveBody,
  GalvoNotApplicable,
  GalvoPreviewBody,
  GalvoStartNotice,
  GalvoStopNoticeView,
  GalvoStopSteps,
  moveText,
  stepLabel,
} from './GalvoNotice';
import { GalvoSafetyBody, NO_CHECKS, type AckChecks } from './GalvoSafetyDialog';
import { GalvoInputs, GalvoPanel, GalvoSection, type GalvoActions } from './GalvoSection';
import {
  completeAck,
  galvoBoardNumber,
  galvoPorts,
  galvoUsbBoards,
  inputBits,
  isGalvoPort,
  isGalvoUsbPort,
  isGalvoVirtualPort,
  isUnconfirmedStop,
  SOURCE_WAVELENGTH_NM,
  withoutGalvoPorts,
} from './galvoPorts';
import { portsForDevice, portForDevice, withoutRuidaPort } from './ruidaPorts';
import { portLabel } from './virtualPort';
import { formatPulseWidths, parsePulseWidths, requiresPulseWidth, usesPulseWidth, validateGalvoProfile } from './galvoProfile';

const noop = () => {};

const STATE: GalvoState = {
  board: 'LMCV4-FIBER-M',
  vendor_id: 0x9588,
  product_id: 0x9899,
  virtual_machine: false,
  serial: '0001-0002-0003-0004',
  version: '0000-0102-0000-0020',
  status_raw: 0x20,
  busy: false,
  ready: true,
  axis: false,
  inputs: 0x0005,
  inputs_known: true,
  working: false,
  red_light_on: false,
  laser_may_be_on: false,
  position: [55, 55],
  position_known: false,
  field_mm: 110,
  source: 'fiber_q',
  waiting_move: null,
  framing: false,
  interlock: null,
  pointer_offset: [0, 0],
  overscan_mm: 0,
};

const STEP: GalvoStepInfo = { step: 2, steps: 6, repeat: 1, repeats: 2, piece: 2, pieces: 3, dx: -100, dy: 0, rotary_deg: 0, pause_ms: 500 };

const ACTIONS: GalvoActions = { onFrame: noop, onAdjust: noop, onRedRun: noop, onRedLight: noop, onStop: noop, onContinue: noop };

const panel = (over: Partial<Parameters<typeof GalvoPanel>[0]> = {}) =>
  renderToString(<GalvoPanel connected state={STATE} step={null} loops={0} blocked={null} actions={ACTIONS} {...over} />);

beforeEach(() => {
  setLanguage('es');
  setApi(createMockApi({ latencyMs: 0 }));
  useStore.setState({ devices: [FALLBACK_DEVICE], activeDeviceId: FALLBACK_DEVICE.id, galvoLaserWarning: null, jobMessage: '', jobError: null });
});

describe('puertos del galvo', () => {
  it('reconoce la tarjeta USB y la virtual, y las separa de las demás', () => {
    expect(isGalvoUsbPort('usb:galvo:0')).toBe(true);
    expect(isGalvoUsbPort('usb:k40:0')).toBe(false);
    expect(isGalvoVirtualPort('virtual:galvo')).toBe(true);
    expect(isGalvoVirtualPort('virtual:galvo@5')).toBe(true);
    expect(isGalvoVirtualPort('virtual:k40')).toBe(false);
    expect(isGalvoPort('COM3')).toBe(false);
    const all = ['/dev/ttyUSB0', 'usb:k40:0', 'usb:galvo:0', 'usb:galvo:1', 'virtual:falcon2', 'virtual:galvo'];
    expect(galvoPorts(all)).toEqual(['usb:galvo:0', 'usb:galvo:1', 'virtual:galvo']);
    expect(galvoUsbBoards(all)).toEqual(['usb:galvo:0', 'usb:galvo:1']);
    expect(withoutGalvoPorts(all)).not.toContain('usb:galvo:0');
    expect(galvoBoardNumber('usb:galvo:1')).toBe(2);
  });

  it('un perfil galvo solo ve sus puertos; los demás perfiles, ninguno del galvo', () => {
    const all = ['/dev/ttyUSB0', 'usb:galvo:0', 'virtual:falcon2', 'virtual:galvo'];
    expect(portsForDevice(galvoDevice(), all)).toEqual(['usb:galvo:0', 'virtual:galvo']);
    expect(portsForDevice(FALLBACK_DEVICE, all)).toEqual(['/dev/ttyUSB0', 'virtual:falcon2']);
    expect(withoutRuidaPort(all)).toEqual(['/dev/ttyUSB0', 'virtual:falcon2']);
    // Un resto de otro tipo de máquina no sirve; la tarjeta que ya no está tampoco.
    expect(portForDevice(galvoDevice(), '/dev/ttyUSB0', all)).toBe('usb:galvo:0');
    expect(portForDevice(galvoDevice(), 'virtual:galvo', all)).toBe('virtual:galvo');
    expect(portForDevice(galvoDevice(), 'usb:galvo:7', all)).toBe('usb:galvo:0');
    expect(portForDevice(FALLBACK_DEVICE, 'usb:galvo:0', all)).toBe('/dev/ttyUSB0');
  });

  it('los nombres de los puertos', () => {
    expect(portLabel('usb:galvo:0')).toBe('Galvo · tarjeta USB 1');
    expect(portLabel('virtual:galvo')).toContain('Galvo (tarjeta simulada)');
  });

  it('las longitudes de onda y el aviso de apagado sin confirmar', () => {
    expect(SOURCE_WAVELENGTH_NM).toEqual({ fiber_q: 1064, fiber_mopa: 1064, co2: 10600, uv: 355 });
    // Se reconoce por el código estable, sin importar el idioma del texto.
    expect(isUnconfirmedStop('listo', '[galvo_stop_unconfirmed] x. Se intentó apagar la fuente pero NO hubo confirmación: puede seguir')).toBe(true);
    expect(isUnconfirmedStop('[galvo_stop_unconfirmed] x. The source may still be enabled; there was NO confirmation')).toBe(true);
    expect(isUnconfirmedStop('Trabajo terminado', null, undefined)).toBe(false);
    // El texto solo no basta: sin código no se reconoce (ni en español ni en inglés).
    expect(isUnconfirmedStop('x. Se intentó apagar la fuente pero NO hubo confirmación')).toBe(false);
    expect(isUnconfirmedStop('there was NO confirmation that the source turned off')).toBe(false);
  });

  it('las entradas se leen bit a bit', () => {
    const bits = inputBits(0b101);
    expect(bits).toHaveLength(16);
    expect(bits.filter((b) => b.on).map((b) => b.bit)).toEqual([0, 2]);
  });
});

describe('panel del galvo', () => {
  it('con un perfil GRBL no aparece', () => {
    expect(renderToString(<GalvoSection />)).toBe('');
  });

  it('sin conexión: etiqueta experimental, haz invisible, no hay pausa y pide conectarse', () => {
    const h = renderToString(<GalvoPanel connected={false} state={null} step={null} loops={0} blocked={null} actions={ACTIONS} />);
    expect(h).toContain('Galvo (JCZ/BJJCZ)');
    expect(h).toContain('Experimental');
    expect(h).toContain('El haz es invisible');
    expect(h).toContain('Conéctate a la tarjeta');
    expect(h).not.toContain('Entradas (solo lectura)');
  });

  it('oculta lo que no aplica y dice por qué: consola, overrides, «$» y pausa', () => {
    const h = panel();
    expect(h).toContain('Lo que no aplica al galvo');
    for (const text of ['no usa G-code', 'no tiene ajustes en vivo de velocidad ni de potencia', 'ajustes «$» de GRBL', 'no tiene pausa, a propósito']) {
      expect(h).toContain(text);
    }
  });

  it('conectado: tarjeta, serie, versión, estado, lente, puntero y fuente apagada', () => {
    const h = panel();
    for (const text of ['LMCV4-FIBER-M', '9588:9899', '0001-0002-0003-0004', 'Lista', '110 mm', '1064 nm', 'Apagado', 'Apagada (confirmado)', 'Posición desconocida']) {
      expect(h).toContain(text);
    }
    expect(h).not.toContain('PUEDE ESTAR ENCENDIDA');
  });

  it('la tarjeta simulada se dice simulada', () => {
    expect(panel({ state: { ...STATE, virtual_machine: true } })).toContain('Simulada (sin hardware)');
  });

  it('encuadre con el puntero rojo: rectángulo, contorno, recorrido en rojo, apagar puntero y detener', () => {
    const h = panel();
    for (const text of ['Encuadre con el puntero rojo', 'Rectángulo', 'Contorno', 'Recorrido en rojo', 'Encender puntero', 'Apagar puntero', 'Detener y apagar']) {
      expect(h).toContain(text);
    }
    expect(h).toContain('El láser nunca se habilita al encuadrar');
    // El puntero apagado: no hay nada que apagar.
    expect(h).toMatch(/<button[^>]*disabled=""[^>]*title="El puntero ya está apagado\."/);
    const on = panel({ state: { ...STATE, red_light_on: true } });
    expect(on).toContain('Encendido');
    expect(on).not.toContain('El puntero ya está apagado.');
  });

  it('sin poder encuadrar, los botones se deshabilitan con el motivo', () => {
    const h = panel({ blocked: 'No hay nada que enviar.' });
    expect(h).toContain('title="No hay nada que enviar."');
    expect(h.match(/title="No hay nada que enviar\."[^>]*>|disabled=""[^>]*title="No hay nada que enviar\."/g)?.length).toBeGreaterThanOrEqual(2);
  });

  it('encuadrando, la máquina «en marcha» no apaga el ajuste en vivo (frameBlocked)', () => {
    const framing = { ...STATE, framing: true };
    const adjustOff = (h: string) => /aria-label="Derecha"[^>]*disabled=""|disabled=""[^>]*aria-label="Derecha"/.test(h);
    // Sin `frameBlocked`, el motivo de `blocked` (la máquina figura en marcha por el propio encuadre) lo apagaba.
    expect(adjustOff(panel({ state: framing, blocked: 'La máquina no está en reposo.' }))).toBe(true);
    // El contenedor lo deja pasar mientras el único motivo sea ese.
    expect(adjustOff(panel({ state: framing, blocked: 'La máquina no está en reposo.', frameBlocked: null }))).toBe(false);
  });

  it('las entradas son de solo lectura y avisan que no hay enclavamiento por software', () => {
    const h = renderToString(<GalvoInputs inputs={0x0005} known />);
    expect(h).toContain('0x0005');
    expect(h).toContain('Entrada 0: alta');
    expect(h).toContain('Entrada 1: baja');
    expect(h).toContain('Solo se leen');
    expect(h).toContain('no puede comprobar el enclavamiento');
    expect(h).not.toContain('<button');
    expect(renderToString(<GalvoInputs inputs={0} known={false} />)).toContain('Todavía no se leyeron');
  });

  it('la fuente que pudo quedar encendida se muestra con alarma', () => {
    const h = panel({ state: { ...STATE, laser_may_be_on: true } });
    expect(h).toContain('PUEDE ESTAR ENCENDIDA');
    expect(h).toContain('CORTA LA ENERGÍA');
    expect(h).toContain('role="alert"');
  });

  it('con el trabajo esperando el material: el paso, cuánto mover y los botones de continuar o detener', () => {
    const h = panel({ state: { ...STATE, working: true, waiting_move: STEP } });
    expect(h).toContain('Mueve el material:');
    expect(h).toContain('Paso 2 de 6');
    expect(h).toContain('pieza 2 de 3');
    expect(h).toContain('repetición 1 de 2');
    expect(h).toContain('Mesa: X -100');
    expect(h).toContain('Ya moví el material: continuar');
  });

  it('marcando: dice el paso; encuadrando: cuenta las vueltas', () => {
    expect(panel({ state: { ...STATE, working: true }, step: STEP })).toContain('Marcando Paso 2 de 6');
    const framing = panel({ state: { ...STATE, working: true, framing: true }, loops: 7 });
    expect(framing).toContain('Vuelta 7');
  });

  it('en inglés', () => {
    setLanguage('en');
    const h = panel();
    expect(h).toContain('What does not apply to the galvo');
    expect(h).toContain('Framing with the red pointer');
    expect(h).toContain('Inputs (read-only)');
    setLanguage('es');
  });
});

describe('avisos del galvo', () => {
  it('la etiqueta experimental y la ayuda', () => {
    const h = renderToString(<GalvoExperimentalBadge />);
    expect(h).toContain('Experimental');
    expect(h).toContain('clase 4');
  });

  it('el motivo de lo que no aplica', () => {
    expect(renderToString(<GalvoNotApplicable reason="galvo.pause.reason" />)).toContain('no tiene pausa');
    expect(renderToString(<GalvoNotApplicable reason="galvo.home.reason" />)).toContain('no tiene home');
  });

  it('el aviso de cortar la energía es destacado, persistente y solo se descarta con un botón', () => {
    expect(renderToString(<GalvoStopNoticeView message={null} onDismiss={noop} />)).toBe('');
    const h = renderToString(<GalvoStopNoticeView message="NO hubo confirmación de que la fuente se apagara" onDismiss={noop} />);
    expect(h).toContain('role="alert"');
    expect(h).toContain('aria-live="assertive"');
    expect(h).toContain('La fuente pudo quedar ENCENDIDA');
    expect(h).toContain('CORTA LA ENERGÍA');
    expect(h).toContain('Ya corté la energía');
    expect(h).toContain('NO hubo confirmación');
    expect(renderToString(<GalvoStopSteps message="x" />)).toContain('paro de emergencia');
  });

  it('los pasos del marcado dividido se escriben con pieza y repetición solo si hay varias', () => {
    expect(stepLabel(STEP)).toBe('Paso 2 de 6 · pieza 2 de 3 · repetición 1 de 2');
    expect(stepLabel({ ...STEP, pieces: 1, repeats: 1 })).toBe('Paso 2 de 6');
    expect(moveText(STEP)).toContain('Mesa: X -100');
    expect(moveText({ ...STEP, dx: 0, dy: 0, rotary_deg: -45 })).toBe('Rotativo: -45,0°');
    expect(moveText({ ...STEP, dx: 12.5, dy: 3, rotary_deg: 90 })).toContain('X +12');
  });

  it('esperar el material: qué mover, la pausa, que el láser disparará y que cerrar detiene', () => {
    const h = renderToString(<GalvoMoveBody info={STEP} />);
    expect(h).toContain('La tarjeta no tiene ejes');
    expect(h).toContain('Mesa: X -100');
    expect(h).toContain('500 ms');
    expect(h).toContain('el láser disparará');
    expect(h).toContain('cancela el trabajo y apaga la fuente');
  });

  it('el diálogo de inicio: haz invisible, sin pausa, y solo con un perfil galvo', () => {
    expect(renderToString(<GalvoStartNotice device={FALLBACK_DEVICE} />)).toBe('');
    const h = renderToString(<GalvoStartNotice device={galvoDevice({ galvo: { ...galvoDevice().galvo, watts: 30 } })} />);
    expect(h).toContain('Galvo: el láser disparará');
    expect(h).toContain('INVISIBLE (1064 nm, 30 W)');
    expect(h).toContain('No hay pausa: solo Detener');
    expect(galvoConfirmKey(galvoDevice())).toBe('galvo.start.confirm');
    expect(galvoConfirmKey(FALLBACK_DEVICE)).toBeNull();
  });

  it('el resumen del trabajo: pasos del marcado dividido, movimientos a mano y avisos', () => {
    const preview: GalvoPreview = { steps: 6, pieces: 3, repeats: 2, manual_moves: 5, est_seconds: 125, warnings: ['capa #f00: ppi sin efecto', 'capa #f00: ppi sin efecto'], field_mm: 110, source: 'fiber_q', max_power_pct: 80 };
    const h = renderToString(<GalvoPreviewBody state={{ status: 'ready', preview }} />);
    expect(h).toContain('Campo 110 mm');
    expect(h).toContain('80 %');
    expect(h).toContain('6 pasos (3 piezas × 2 repeticiones)');
    expect(h).toContain('5 veces tendrás que mover el material');
    // Un aviso repetido sale una sola vez.
    expect(h.match(/ppi sin efecto/g)).toHaveLength(1);
    expect(renderToString(<GalvoPreviewBody state={{ status: 'ready', preview: { ...preview, steps: 1, warnings: [] } }} />)).toContain('Sin avisos');
    expect(renderToString(<GalvoPreviewBody state={{ status: 'loading' }} />)).toContain('Calculando');
    expect(renderToString(<GalvoPreviewBody state={{ status: 'error', message: 'no cabe' }} />)).toContain('no cabe');
  });
});

describe('diálogo de seguridad', () => {
  const body = (checked: AckChecks) => renderToString(<GalvoSafetyBody device={galvoDevice()} checked={checked} onCheck={noop} />);

  it('pide las tres confirmaciones, con la longitud de onda y los vatios, todas desmarcadas', () => {
    const h = body(NO_CHECKS);
    expect(h).toContain('clase 4');
    expect(h).toContain('INVISIBLE (aquí: 1064 nm, 20 W)');
    expect(h).toContain('densidad óptica correcta para 1064 nm y 20 W');
    expect(h).toContain('recinto cerrado');
    expect(h).toContain('paro de emergencia');
    expect(h).toContain('Esto se pide en cada conexión: Tlaku no lo recuerda');
    expect(h.match(/type="checkbox"/g)).toHaveLength(3);
    expect(h).not.toContain('checked=""');
  });

  it('con una fuente UV pide gafas para 355 nm', () => {
    const uv = galvoDevice({ galvo: { ...galvoDevice().galvo, source: 'uv', watts: 5 } });
    expect(renderToString(<GalvoSafetyBody device={uv} checked={NO_CHECKS} onCheck={noop} />)).toContain('355 nm');
  });

  it('solo con las tres casillas se crea el reconocimiento', () => {
    expect(completeAck(NO_CHECKS)).toBeNull();
    expect(completeAck({ eyewear: true, enclosure: true, emergencyStop: false })).toBeNull();
    expect(completeAck({ eyewear: false, enclosure: true, emergencyStop: true })).toBeNull();
    expect(completeAck({ eyewear: true, enclosure: true, emergencyStop: true })).toEqual({ eyewear: true, enclosure: true, emergency_stop: true });
  });
});

describe('perfil del galvo', () => {
  const html = (d: DeviceProfile, issues = validateGalvoProfile(d)) => renderToString(<GalvoFields draft={d} edit={noop} issues={issues} />);

  it('agrupa por secciones con ayudas cortas y la etiqueta Experimental', () => {
    const h = html(galvoDevice());
    for (const section of ['Lente y fuente', 'Retardos y saltos por defecto', 'Puntero rojo', 'Corrección del campo', 'Cilindro y cono', 'Marcado dividido y repetido']) {
      expect(h, section).toContain(section);
    }
    for (const label of ['Campo de la lente', 'Fuente láser', 'Potencia de la fuente', 'Rango de frecuencia', 'Potencia máxima permitida', 'Velocidad del encuadre', 'Escala', 'Intercambiar X e Y', 'Barril, trapecio y sesgo', 'Archivo .cor (solo informativo)', 'Repeticiones del marcado']) {
      expect(h, label).toContain(label);
    }
    expect(h).toContain('Experimental');
    expect(h).toContain('Longitud de onda típica de esta fuente: 1064 nm');
    expect(h).toContain('no usa G-code');
    // Una fibra Q-switched no tiene ancho de pulso.
    expect(h).toContain('no tiene ancho de pulso ajustable');
  });

  it('MOPA y UV piden la lista de anchos de pulso; MOPA la exige', () => {
    const mopa = galvoDevice({ galvo: { ...galvoDevice().galvo, source: 'fiber_mopa', pulse_widths_ns: [] } });
    const h = html(mopa);
    expect(h).toContain('Anchos de pulso');
    expect(h).toContain('Es obligatorio en MOPA');
    expect(h).toContain('Una fuente MOPA necesita la lista de anchos de pulso.');
    expect(html(galvoDevice({ galvo: { ...galvoDevice().galvo, source: 'uv' } }))).toContain('Opcional');
  });

  it('el archivo .cor es solo informativo y avisa que no se aplica', () => {
    const d = galvoDevice();
    d.galvo.correction.cor_file = 'lente.cor';
    const h = html(d);
    expect(h).toContain('lente.cor');
    expect(h).toContain('Tlaku no aplica el archivo .cor');
    expect(html(galvoDevice())).not.toContain('Tlaku no aplica el archivo .cor');
  });

  it('cilindro, cono y marcado dividido muestran sus campos solo cuando se activan', () => {
    const off = html(galvoDevice());
    expect(off).not.toContain('Diámetro del cilindro');
    expect(off).not.toContain('Lado de la pieza');
    expect(off).not.toContain('Avance entre repeticiones');
    const d = galvoDevice();
    d.galvo.cylinder.enabled = true;
    d.galvo.taper.enabled = true;
    d.galvo.split.enabled = true;
    d.galvo.repeat.count = 3;
    const on = html(d);
    for (const label of ['Diámetro del cilindro', 'Distancia del espejo', 'Extremo superior', 'Lado de la pieza', 'Solape', 'Pausa tras mover', 'Avance entre repeticiones', 'Pausa entre repeticiones']) {
      expect(on, label).toContain(label);
    }
    expect(on).toContain('no se ha verificado');
  });

  it('un origen que no es abajo a la izquierda se marca', () => {
    const h = html(galvoDevice({ origin: 'top_left' }));
    expect(h).toContain('debe ser «abajo a la izquierda»');
  });

  it('en inglés', () => {
    setLanguage('en');
    const h = html(galvoDevice());
    expect(h).toContain('Lens and source');
    expect(h).toContain('.cor file (informational only)');
    setLanguage('es');
  });
});

describe('validación del perfil galvo', () => {
  const issues = (patch: (g: DeviceProfile['galvo']) => void, over: Partial<DeviceProfile> = {}) => {
    const d = galvoDevice(over);
    patch(d.galvo);
    return validateGalvoProfile(d);
  };

  it('el perfil de partida es válido', () => {
    expect(validateGalvoProfile(galvoDevice())).toEqual({});
  });

  it('refleja lo que rechaza el motor', () => {
    expect(issues((g) => (g.field_mm = 0))).toHaveProperty('field_mm');
    expect(issues((g) => (g.watts = -1))).toHaveProperty('watts');
    expect(issues((g) => ((g.freq_min_khz = 90), (g.freq_max_khz = 80)))).toHaveProperty('freq');
    expect(issues((g) => (g.max_power_pct = 120))).toHaveProperty('max_power_pct');
    expect(issues((g) => (g.max_power_pct = 0))).toHaveProperty('max_power_pct');
    expect(issues((g) => (g.light_speed = 0))).toHaveProperty('light_speed');
    expect(issues((g) => (g.jump.speed = 0))).toHaveProperty('jump');
    expect(issues((g) => ((g.jump.delay_min_us = 500), (g.jump.delay_max_us = 100)))).toHaveProperty('jump');
    expect(issues((g) => (g.correction.scale = [0.05, 1]))).toHaveProperty('scale');
    expect(issues((g) => (g.correction.bulge = [0.5, 0]))).toHaveProperty('nonlinear');
    expect(issues((g) => (g.correction.skew = [0, -0.4]))).toHaveProperty('nonlinear');
    expect(issues((g) => (g.repeat.count = 0))).toHaveProperty('repeat_count');
    expect(issues((g) => (g.repeat.count = 1.5))).toHaveProperty('repeat_count');
    expect(issues((g) => (g.repeat.count = 100001))).toHaveProperty('repeat_count');
    expect(issues((g) => (g.split.piece_mm = -1))).toHaveProperty('piece_mm');
    expect(issues((g) => ((g.split.enabled = true), (g.split.overlap_mm = 110)))).toHaveProperty('overlap_mm');
    // El solape solo cuenta con el marcado dividido activo.
    expect(issues((g) => (g.split.overlap_mm = 110))).not.toHaveProperty('overlap_mm');
  });

  it('MOPA exige anchos de pulso; UV y fibra Q no', () => {
    expect(issues((g) => (g.source = 'fiber_mopa'))).toHaveProperty('pulse_widths', 'galvo.err.pulseRequired');
    expect(issues((g) => ((g.source = 'fiber_mopa'), (g.pulse_widths_ns = [4, 9])))).not.toHaveProperty('pulse_widths');
    expect(issues((g) => (g.source = 'uv'))).not.toHaveProperty('pulse_widths');
    expect(issues((g) => (g.pulse_widths_ns = [4, -2]))).toHaveProperty('pulse_widths', 'galvo.err.pulseList');
  });

  it('cilindro y cono: medidas positivas y el mismo eje', () => {
    expect(issues((g) => ((g.cylinder.enabled = true), (g.cylinder.diameter_mm = 0)))).toHaveProperty('cylinder_diameter');
    expect(issues((g) => ((g.cylinder.enabled = true), (g.cylinder.mirror_distance_mm = 0)))).toHaveProperty('cylinder_distance');
    expect(issues((g) => ((g.taper.enabled = true), (g.taper.length_mm = 0)))).toHaveProperty('taper');
    expect(issues((g) => ((g.cylinder.enabled = true), (g.taper.enabled = true), (g.cylinder.axis = 'x'), (g.taper.axis = 'y')))).toHaveProperty('taper', 'galvo.err.axis');
    // Apagados, sus medidas no cuentan.
    expect(issues((g) => (g.cylinder.diameter_mm = 0))).toEqual({});
  });

  it('el origen debe ser abajo a la izquierda', () => {
    expect(validateGalvoProfile(galvoDevice({ origin: 'top_right' }))).toHaveProperty('origin');
  });

  it('la lista de pulsos se lee y se escribe', () => {
    expect(parsePulseWidths('4, 6;9  13')).toEqual([4, 6, 9, 13]);
    expect(parsePulseWidths('9, 4, 9')).toEqual([4, 9]);
    expect(parsePulseWidths('4, x')).toBeNull();
    expect(parsePulseWidths('4, 0')).toBeNull();
    expect(parsePulseWidths('4.5')).toBeNull();
    expect(formatPulseWidths([4, 6])).toBe('4, 6');
    expect(usesPulseWidth('fiber_mopa') && usesPulseWidth('uv')).toBe(true);
    expect(usesPulseWidth('fiber_q') || usesPulseWidth('co2')).toBe(false);
    expect(requiresPulseWidth('fiber_mopa')).toBe(true);
    expect(requiresPulseWidth('uv')).toBe(false);
  });
});
