import { renderToString } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import { es } from '@/i18n';
import { dialectForKind, FALLBACK_DEVICE, kindDefaults, planKindChange, validateDialect } from '@/lib/device';
import type { DeviceProfile } from '@/api/types';
import { DialectEditor } from './DeviceDialect';
import { KindChangeBody, KindField } from './DeviceKindField';
import { RotaryTest } from './DeviceRotaryTest';

const noop = () => {};
const device = (patch: Partial<DeviceProfile> = {}): DeviceProfile => ({ ...FALLBACK_DEVICE, ...patch });
const custom = (dialect: Partial<DeviceProfile['dialect']> = {}) => device({ kind: 'custom_gcode', dialect: { ...FALLBACK_DEVICE.dialect, ...dialect } });
const html = (d: DeviceProfile) => renderToString(<DialectEditor draft={d} edit={noop} issues={d.kind === 'custom_gcode' ? validateDialect(d.dialect) : {}} />);

describe('selector del tipo de controladora', () => {
  it('ofrece los ocho tipos con nombres claros', () => {
    const h = renderToString(<KindField kind="grbl" onChange={noop} />);
    for (const s of ['GRBL 1.1 (la mayoría de láseres de diodo)', 'grblHAL', 'FluidNC', 'GRBL-LPC / Cohesion3D', 'GRBL antiguo (sin M4)', 'Smoothieware', 'Marlin', 'G-code personalizado']) {
      expect(h).toContain(s);
    }
    expect(h.match(/<option/g)).toHaveLength(8);
  });

  it('Marlin y Smoothieware ya se pueden enviar: sin el aviso de solo exportar', () => {
    for (const kind of ['marlin', 'smoothie', 'grbl_hal'] as const) {
      expect(renderToString(<KindField kind={kind} onChange={noop} />)).not.toContain(es['devices.kind.exportOnly.hint']);
    }
  });

  it('el tipo personalizado explica que parte del dialecto actual', () => {
    expect(renderToString(<KindField kind="custom_gcode" onChange={noop} />)).toContain('Parte del dialecto que ya tiene el perfil');
  });

  it('la confirmación lista lo que pisa: dialecto, S máxima, modo de potencia y baudios', () => {
    const changes = planKindChange(device(), kindDefaults('marlin'));
    const h = renderToString(<KindChangeBody draft={device()} changes={changes} />);
    for (const s of ['Dialecto de G-code', 'S máxima', 'Modo de potencia', 'Velocidad', '250000', '255', 'Constante (M3)', 'Con el tipo']) expect(h.toLowerCase()).toContain(s.toLowerCase());
  });
});

describe('editor del dialecto', () => {
  it('en un tipo de fábrica es de solo lectura: resumen, órdenes y vista previa', () => {
    const h = html(device());
    expect(h).toContain('S 0-1000, M4 dinámico, S en cada G1, aire M8/M9');
    expect(h).toContain('solo se consulta');
    expect(h).toContain('M4 S0'); // en las órdenes y en el ejemplo
    expect(h).toContain('G1 X30 S400 F6000');
    expect(h).toContain('readOnly');
    expect(h).toContain('disabled');
    expect(h).not.toContain('open=');
  });

  it('con G-code personalizado se edita: grupos abiertos y campos activos', () => {
    const h = html(custom());
    expect(h).toContain('open=');
    for (const g of ['Potencia', 'Encendido', 'Movimientos', 'Aire asistido', 'Final', 'Formato']) expect(h).toContain(`<legend>${g}</legend>`);
    expect(h).toContain('Estás editando el dialecto');
    expect(h).not.toContain('readOnly');
    expect(h).not.toContain('disabled');
  });

  it('muestra los errores de las plantillas junto al campo', () => {
    const h = html(custom({ s_inline: false, power_cmd: 'M3 S100', laser_off: '', air_on: 'M8', air_off: null }));
    expect(h).toContain('La plantilla debe incluir {s}');
    expect(h).toContain('Este campo no puede quedar vacío.');
    expect(h).toContain('Indica las dos órdenes del aire');
  });

  it('la vista previa sigue el dialecto: Marlin lleva M3 S0 I y cierra con M400', () => {
    const d = device({ ...kindDefaults('marlin'), dialect: dialectForKind('marlin') });
    const h = html(d);
    expect(h).toContain('M3 S0 I');
    expect(h).toContain('M400');
    expect(h).toContain('sin modo dinámico');
  });

  it('muestra la unidad de la pausa G4: segundos en GRBL, milisegundos en Marlin, editable solo en personalizado', () => {
    const grbl = html(device());
    expect(grbl).toContain('Tiempo de pausa (G4)');
    expect(grbl).toMatch(/<option value="p_seconds" selected="">/);
    const marlin = html(device({ ...kindDefaults('marlin'), dialect: dialectForKind('marlin') }));
    expect(marlin).toMatch(/<option value="p_milliseconds" selected="">/);
    expect(html(custom({ dwell: 's_seconds' }))).toMatch(/<option value="s_seconds" selected="">/);
    // Solo se edita con el tipo personalizado.
    expect(grbl).toMatch(/<select[^>]*disabled[^>]*>(?:(?!<\/select>).)*p_seconds/s);
  });

  it('avisa si el láser puede quedar encendido en los desplazamientos', () => {
    expect(html(custom({ g0_laser_off: false, travel_s0: false }))).toContain('puede quedar encendido');
  });
});

describe('probar una vuelta', () => {
  it('muestra el botón, la ayuda de qué revisar y el motivo cuando no se puede', () => {
    const ok = renderToString(<RotaryTest draft={device()} blocked={null} />);
    expect(ok).toContain('Probar una vuelta');
    expect(ok).toContain('Qué revisar');
    expect(ok).toContain('ALARM:2');
    expect(ok).not.toContain('disabled=""');
    const blocked = renderToString(<RotaryTest draft={device()} blocked="devices.rotary.test.saveFirst" />);
    expect(blocked).toContain('disabled=""');
    expect(blocked).toContain('Guarda los cambios del perfil primero');
  });
});
