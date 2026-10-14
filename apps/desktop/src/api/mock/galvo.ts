import type { DeviceProfile, GalvoFrameKind, GalvoPreview, GalvoSafetyAck, GalvoState, GalvoVerifyReport, JobArgs } from '../types';
import { GALVO_FIRMWARE, type MockMachine } from './machine';
import { mockComputeJob } from './job';

/** El mensaje que el motor da al abrir una tarjeta real sin las tres confirmaciones de seguridad. */
export const MOCK_GALVO_ACK_REQUIRED =
  'Para conectar con una tarjeta galvo real hay que confirmar la seguridad láser: gafas con la densidad óptica correcta para la longitud de onda, recinto cerrado y saber dónde está el paro de emergencia.';

/** ¿Es una tarjeta USB real (y no la simulada)? */
export const isMockGalvoUsb = (port: string) => port.startsWith('usb:galvo:');

/** Estado del galvo simulado: lo mínimo para ensayar el panel sin Rust. */
export class MockGalvo {
  redLight = false;
  laserMayBeOn = false;
  waiting: GalvoState['waiting_move'] = null;
  /** Conexiones USB reales que pasaron por el reconocimiento (para las pruebas). */
  acks: GalvoSafetyAck[] = [];
  /** Perfil con el que se conectó (enclavamiento, desfase del puntero y overscan salen de él). */
  device: DeviceProfile | undefined;
  /** Encuadre en curso: forma, vueltas pedidas y último ajuste en vivo. */
  framing: { shape: GalvoFrameKind; loops: number | null; gap_mm: number | null } | null = null;
  adjust = { dx: 0, dy: 0, scale: 1 };

  constructor(private readonly machine: MockMachine) {}

  connect(port: string, device: DeviceProfile | undefined, ack: GalvoSafetyAck | undefined): string {
    if (isMockGalvoUsb(port)) {
      if (!ack || !ack.eyewear || !ack.enclosure || !ack.emergency_stop) throw new Error(MOCK_GALVO_ACK_REQUIRED);
      this.acks.push({ ...ack });
    }
    this.redLight = false;
    this.laserMayBeOn = false;
    this.waiting = null;
    this.framing = null;
    this.adjust = { dx: 0, dy: 0, scale: 1 };
    this.device = device;
    return this.machine.connect(port, false, false, true);
  }

  state(): GalvoState {
    this.machine.requireConnection();
    const req = this.device?.galvo.require_input ?? null;
    if (!this.machine.galvo) throw new Error('Esta función es solo de las máquinas galvo (tarjeta JCZ/BJJCZ).');
    const [x, y] = this.machine.pos;
    return {
      board: 'LMCV4-FIBER-M',
      vendor_id: 0x9588,
      product_id: 0x9899,
      virtual_machine: !isMockGalvoUsb(this.machine.port ?? ''),
      serial: '0001-0002-0003-0004',
      version: '0000-0102-0000-0020',
      status_raw: 0x20,
      busy: this.machine.state === 'run',
      ready: true,
      axis: false,
      inputs: 0,
      inputs_known: true,
      working: this.machine.state === 'run',
      red_light_on: this.redLight,
      laser_may_be_on: this.laserMayBeOn,
      position: [x, y],
      position_known: false,
      field_mm: 110,
      source: 'fiber_q',
      waiting_move: this.waiting,
      framing: this.framing !== null && this.machine.state === 'run',
      // La tarjeta simulada tiene las entradas en 0: un enclavamiento que exija un bit no se cumple.
      interlock: req ? { mask: req.mask, value: req.value, met: (0 & req.mask) === req.value } : null,
      pointer_offset: this.device?.pointer_offset ?? [0, 0],
      overscan_mm: this.device?.galvo.overscan_mm ?? 0,
    };
  }

  /** Encuadre con el puntero: empieza el bucle de la máquina simulada y recuerda lo pedido. */
  frame({ doc, device, options }: JobArgs, shape: GalvoFrameKind, loops: number | null, gapMm: number | null): void {
    this.state();
    if (loops !== null && loops < 1) throw new Error('El número de vueltas del encuadre debe ser al menos 1.');
    if (gapMm !== null && (!Number.isFinite(gapMm) || gapMm < 0)) throw new Error('La holgura entre objetos debe ser un número no negativo.');
    this.machine.frame(mockComputeJob(doc, device, options).bounds);
    this.framing = { shape, loops, gap_mm: gapMm };
    this.adjust = { dx: 0, dy: 0, scale: 1 };
  }

  /** Mueve o escala el encuadre en curso; falla si no hay uno o el valor no es válido. */
  adjustFrame(dx: number, dy: number, scale: number): void {
    this.state();
    if (!this.framing || this.machine.state !== 'run') throw new Error('No hay un encuadre en curso que mover o escalar.');
    if (![dx, dy, scale].every(Number.isFinite) || scale < 0.05 || scale > 20 || Math.abs(dx) > 1000 || Math.abs(dy) > 1000) {
      throw new Error('el ajuste del encuadre no es válido');
    }
    this.adjust = { dx, dy, scale };
  }

  /** Verificación sin láser: lee la tarjeta simulada y comprueba el perfil. La USB real exige el reconocimiento. */
  verify(device: DeviceProfile, port: string, ack: GalvoSafetyAck | null | undefined): GalvoVerifyReport {
    if (isMockGalvoUsb(port)) {
      if (!ack || !ack.eyewear || !ack.enclosure || !ack.emergency_stop) throw new Error(MOCK_GALVO_ACK_REQUIRED);
      this.acks.push({ ...ack });
    } else if (port !== 'virtual:galvo') {
      throw new Error(`«${port}» no es un puerto galvo. Elige una tarjeta USB de la lista o la máquina virtual.`);
    }
    const g = device.galvo;
    const req = g.require_input;
    const interlock = req ? { mask: req.mask, value: req.value, met: (0 & req.mask) === req.value } : null;
    const side = g.field_mm * 0.95;
    const fits = g.field_mm > 0 && Number.isFinite(g.field_mm);
    return {
      board: 'LMCV4-FIBER-M',
      vendor_id: 0x9588,
      product_id: 0x9899,
      virtual_machine: !isMockGalvoUsb(port),
      serial: '0001-0002-0003-0004',
      version: '0000-0102-0000-0020',
      status_raw: 0x20,
      busy: false,
      ready: true,
      inputs: 0,
      interlock,
      checks: [
        { id: 'board', ok: true, detail: 'LMCV4-FIBER-M (USB 9588:9899)' },
        { id: 'idle', ok: true, detail: 'la tarjeta está en reposo' },
        { id: 'frame', ok: fits, detail: fits ? `el encuadre del campo (${Math.round(side)} mm) compila` : 'el campo no es válido' },
        ...(interlock ? [{ id: 'interlock', ok: interlock.met, detail: `entradas 0x0000: se exige (entradas & ${interlock.mask}) == ${interlock.value}` }] : []),
      ],
    };
  }

  preview({ doc, device, options }: JobArgs): GalvoPreview {
    const p = mockComputeJob(doc, device, options);
    const g = device.galvo;
    const repeats = Math.max(1, g.repeat.count);
    const pieces = g.split.enabled ? 2 : 1;
    return {
      steps: pieces * repeats,
      pieces,
      repeats,
      manual_moves: pieces * repeats - 1,
      est_seconds: p.total_s,
      warnings: [],
      field_mm: g.field_mm,
      source: g.source,
      max_power_pct: g.max_power_pct,
    };
  }
}

export { GALVO_FIRMWARE };
