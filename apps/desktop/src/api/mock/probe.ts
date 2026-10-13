import type { LaserApi, ProbeResult } from '../api';

type Lag = <T>(value: () => T) => Promise<T>;

/** Puertos que ofrece `listPorts` en el modo mock: el primero es una Falcon2 Pro, el segundo un GRBL cualquiera. */
export const MOCK_FALCON_PORT = '/dev/cu.usbserial-1420';
export const MOCK_GRBL_PORT = '/dev/cu.usbmodem1101';

/**
 * `$$` de una Falcon2 Pro 22 W (volcado B de docs/hardware/falcon2.md, hilo
 * https://forum.lightburnsoftware.com/t/falcon-2-pro-stuck-in-startup-loop/192185). Los números van
 * con tres decimales donde GRBL los imprime así.
 */
export function falconProSettings(): Record<string, string> {
  return {
    $0: '10', $1: '250', $2: '0', $3: '4', $10: '3', $13: '0', $20: '0', $21: '1', $22: '1', $23: '3',
    $24: '2000.000', $25: '4000.000', $26: '20', $27: '5.000', $30: '1000', $31: '0', $32: '1',
    $100: '80.000', $101: '80.000', $110: '50000.000', $111: '50000.000', $120: '5000.000', $121: '5000.000',
    $130: '400.000', $131: '400.000', $150: '36', $151: '100', $152: '30', $153: '0', $154: '1', $155: '1',
  };
}

/** `$$` de un diodo GRBL 1.1h típico (tabla de fábrica, con el modo láser activado). */
export function genericGrblSettings(): Record<string, string> {
  return {
    $0: '10', $1: '25', $2: '0', $3: '0', $4: '0', $5: '0', $6: '0', $10: '1', $11: '0.010', $12: '0.002',
    $13: '0', $20: '0', $21: '0', $22: '0', $23: '0', $24: '25.000', $25: '500.000', $26: '250', $27: '1.000',
    $30: '1000', $31: '0', $32: '1', $100: '80.000', $101: '80.000', $102: '250.000',
    $110: '6000.000', $111: '6000.000', $112: '500.000', $120: '1000.000', $121: '1000.000', $122: '10.000',
    $130: '400.000', $131: '400.000', $132: '200.000',
  };
}

export function falconProProbe(): ProbeResult {
  return {
    family: 'grbl',
    firmware: 'Grbl 1.1f (20220810)',
    banner: [
      'usb connect succeed',
      '[OK]system start-up CV50-Pro-MASTER-Release V1.0.1 20240325-1042',
      '[OK] ILmp_app_init OK',
      '[OK] gas_init OK',
      '[MSG:Using cpu_map…esp32-s3]',
      "Grbl 1.1f ['$' for help]",
      '[OK] ILmp Link connect ok',
    ].join('\n'),
    info: [
      '[VER:1.1f.20220810:]',
      '[OPT:VHL,127,65536]',
      '[software versions: CV50-Pro-MASTER-Release V1.0.0 20231129-1024]',
      'inquire_info->power:22W ,CV50-LASER-22W-Release V3.0.5 20231129-1001',
    ],
    settings: falconProSettings(),
    suggested_kind: 'grbl',
  };
}

export function genericGrblProbe(): ProbeResult {
  return {
    family: 'grbl',
    firmware: 'Grbl 1.1h (20190825)',
    banner: "Grbl 1.1h ['$' for help]",
    info: ['[VER:1.1h.20190825:]', '[OPT:V,15,128]'],
    settings: genericGrblSettings(),
    suggested_kind: 'grbl',
  };
}

/** `probePort` simulado: Falcon2 Pro en el primer puerto, GRBL genérico en el segundo y `virtual:*` como el motor. */
export function mockProbe(lag: Lag): Pick<LaserApi, 'probePort'> {
  return {
    probePort: ({ port, baud }) =>
      lag(() => {
        if (baud <= 0) throw 'Los baudios deben ser mayores que 0.';
        // Direcciones de red: responde un GRBL genérico, salvo `no-responde` (simula un equipo que no contesta).
        if (/^(tcp|wss?):\/\//i.test(port)) {
          if (port.includes('no-responde')) throw `${port.replace(/^tcp:\/\//i, '')} no contestó a tiempo. Revisa que la máquina esté encendida, conectada a la misma red que este equipo y que la dirección sea la correcta.`;
          return genericGrblProbe();
        }
        if (port === MOCK_FALCON_PORT || /^virtual:.*falcon/i.test(port)) return falconProProbe();
        if (port === MOCK_GRBL_PORT || port.startsWith('virtual:')) return genericGrblProbe();
        throw `No se pudo abrir ${port}: no existe. Revisa el cable, que la máquina esté encendida y que ningún otro programa use el puerto.`;
      }),
  };
}
