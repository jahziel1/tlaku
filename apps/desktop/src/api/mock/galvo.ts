import type { DeviceProfile, GalvoPreview, GalvoSafetyAck, GalvoState, JobArgs } from '../types';
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

  constructor(private readonly machine: MockMachine) {}

  connect(port: string, device: DeviceProfile | undefined, ack: GalvoSafetyAck | undefined): string {
    if (isMockGalvoUsb(port)) {
      if (!ack || !ack.eyewear || !ack.enclosure || !ack.emergency_stop) throw new Error(MOCK_GALVO_ACK_REQUIRED);
      this.acks.push({ ...ack });
    }
    this.redLight = false;
    this.laserMayBeOn = false;
    this.waiting = null;
    void device;
    return this.machine.connect(port, false, false, true);
  }

  state(): GalvoState {
    this.machine.requireConnection();
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
