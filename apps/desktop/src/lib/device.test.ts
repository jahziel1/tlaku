import { describe, expect, it } from 'vitest';
import type { DeviceProfile } from '@/api/types';
import {
  applyKindChange,
  applyProposal,
  applyRotaryPreset,
  DEFAULT_DIALECT,
  DEVICE_KINDS,
  dialectForKind,
  dialectSummary,
  dialectWarnings,
  docDeltaToWork,
  docToWork,
  duplicateDeviceProfile,
  FALLBACK_DEVICE,
  kindDefaults,
  matchRotaryPreset,
  newDeviceProfile,
  normalizeDevice,
  parseGrblSettings,
  parseGrblVersion,
  planKindChange,
  proposeFromSettings,
  ROTARY_PRESETS,
  rotaryCircumference,
  sameDevice,
  sampleGcode,
  scanOffsetRowIssues,
  slugify,
  sortScanOffsets,
  uniqueDeviceId,
  validateDevice,
  validateDialect,
  workToDoc,
  type DeviceKind,
} from './device';

const device = (patch: Partial<DeviceProfile> = {}): DeviceProfile => ({ ...FALLBACK_DEVICE, ...patch });

const DOLLAR_DOLLAR = [
  '$0=10',
  '$22=1',
  '$30=1000',
  '$31=0',
  '$32=1',
  '$100=80.000',
  '$110=12000.000',
  '$111=9000.000',
  '$120=500.000',
  '$121=400.000',
  '$130=410.000',
  '$131=400.000',
  'ok',
];

describe('parseGrblSettings', () => {
  it('lee las líneas $N=valor y descarta lo demás', () => {
    const s = parseGrblSettings(['Grbl 1.1h', ...DOLLAR_DOLLAR, '[MSG:algo]', '$x=3']);
    expect(s[130]).toBe(410);
    expect(s[110]).toBe(12000);
    expect(s[30]).toBe(1000);
    expect(s[32]).toBe(1);
    expect(Object.keys(s)).toHaveLength(12);
  });

  it('acepta negativos, espacios y comentarios al final', () => {
    expect(parseGrblSettings(['$23=-1', ' $5 = 0 (invert)', '$$'])).toEqual({ 23: -1, 5: 0 });
  });
});

describe('proposeFromSettings', () => {
  it('propone área, velocidad, aceleración y S máx. desde $130/$131, $110/$111, $120/$121 y $30', () => {
    const p = proposeFromSettings(parseGrblSettings(DOLLAR_DOLLAR), device({ s_max: 255 }));
    const byField = Object.fromEntries(p.changes.map((c) => [c.field, c.to]));
    expect(byField).toEqual({ bed: [410, 400], max_rate: [12000, 9000], accel: [500, 400], s_max: 1000 });
    expect(p.laserMode).toBe(true);
    expect(p.nothingFound).toBe(false);
    expect(p.inSync).toBe(false);
  });

  it('no propone lo que ya coincide y lo marca como sincronizado', () => {
    const current = device({ bed: [410, 400], max_rate: [12000, 9000], accel: [500, 400], s_max: 1000 });
    const p = proposeFromSettings(parseGrblSettings(DOLLAR_DOLLAR), current);
    expect(p.changes).toEqual([]);
    expect(p.inSync).toBe(true);
  });

  it('avisa si el modo láser está desactivado ($32=0)', () => {
    const p = proposeFromSettings(parseGrblSettings(['$32=0', '$30=1000']), device());
    expect(p.laserMode).toBe(false);
  });

  it('con $32 ausente no afirma nada sobre el modo láser', () => {
    expect(proposeFromSettings({ 30: 255 }, device()).laserMode).toBeNull();
  });

  it('si solo viene un eje conserva el valor actual del otro', () => {
    const p = proposeFromSettings({ 130: 300 }, device({ bed: [400, 350] }));
    expect(p.changes).toEqual([{ field: 'bed', codes: '$130/$131', from: [400, 350], to: [300, 350] }]);
  });

  it('ignora valores no positivos y avisa si no hay nada que interpretar', () => {
    const p = proposeFromSettings({ 130: 0, 131: -5, 0: 10 }, device());
    expect(p.changes).toEqual([]);
    expect(p.nothingFound).toBe(true);
    expect(p.inSync).toBe(false);
  });

  it('applyProposal solo toca los campos elegidos', () => {
    const p = proposeFromSettings(parseGrblSettings(DOLLAR_DOLLAR), device({ s_max: 255 }));
    const only = applyProposal(device({ s_max: 255 }), p.changes.filter((c) => c.field === 'bed'));
    expect(only.bed).toEqual([410, 400]);
    expect(only.s_max).toBe(255);
    expect(only.max_rate).toEqual(FALLBACK_DEVICE.max_rate);
  });
});

describe('validateDevice', () => {
  it('acepta el perfil de fábrica', () => {
    expect(validateDevice(FALLBACK_DEVICE)).toEqual({});
  });

  it('marca cada campo inválido', () => {
    const bad = validateDevice(device({ name: '  ', bed: [0, 400], s_max: 0, accel: [100, -1], max_rate: [0, 0], rapid_rate: 0, baud: 0, rx_buffer: 8 }));
    expect(bad).toEqual({
      name: 'required',
      bed: 'positive',
      s_max: 'positive',
      accel: 'positive',
      max_rate: 'positive',
      rapid_rate: 'positive',
      baud: 'positive',
      rx_buffer: 'tooSmall',
    });
  });

  it('S máx. debe ser entera y la cama razonable', () => {
    expect(validateDevice(device({ s_max: 10.5 })).s_max).toBe('integer');
    expect(validateDevice(device({ bed: [20000, 10] })).bed).toBe('tooBig');
    expect(validateDevice(device({ rx_buffer: 127.5 })).rx_buffer).toBe('integer');
  });
});

describe('alta y copia', () => {
  it('slugify quita acentos y símbolos', () => {
    expect(slugify('Láser Diodo #2 (grande)')).toBe('laser-diodo-2-grande');
    expect(slugify('***')).toBe('maquina');
  });

  it('los ids no se repiten', () => {
    expect(uniqueDeviceId('Mi láser', ['mi-laser', 'mi-laser-2'])).toBe('mi-laser-3');
    expect(uniqueDeviceId('Otro', ['mi-laser'])).toBe('otro');
  });

  it('un perfil nuevo trae valores de fábrica y id único', () => {
    const d = newDeviceProfile('Mi láser', ['mi-laser']);
    expect(d.id).toBe('mi-laser-2');
    expect(d.name).toBe('Mi láser');
    expect(d.bed).toEqual(FALLBACK_DEVICE.bed);
  });

  it('duplicar copia todo menos id y nombre, sin compartir arreglos', () => {
    const src = device({ id: 'a', name: 'A', bed: [300, 200], end_gcode: 'M5\nG0 X0 Y0' });
    const copy = duplicateDeviceProfile(src, '(copia)', ['a']);
    expect(copy.name).toBe('A (copia)');
    expect(copy.id).toBe('a-copia');
    expect(copy.bed).toEqual([300, 200]);
    expect(copy.end_gcode).toBe(src.end_gcode);
    copy.bed[0] = 1;
    expect(src.bed[0]).toBe(300);
    expect(sameDevice(src, src)).toBe(true);
    expect(sameDevice(src, copy)).toBe(false);
  });
});

describe('coordenadas según el origen del perfil', () => {
  const bed = [400, 300] as [number, number];
  it('abajo a la izquierda no cambia nada', () => {
    expect(workToDoc({ bed, origin: 'bottom_left' }, [10, 20])).toEqual([10, 20]);
    expect(docDeltaToWork({ origin: 'bottom_left' }, 5, 5)).toEqual([5, 5]);
  });
  it('arriba a la izquierda invierte Y', () => {
    expect(workToDoc({ bed, origin: 'top_left' }, [10, 20])).toEqual([10, 280]);
    expect(docDeltaToWork({ origin: 'top_left' }, 5, 5)).toEqual([5, -5]);
  });
  it('a la derecha invierte X y la conversión es su propia inversa', () => {
    const d = { bed, origin: 'top_right' as const };
    expect(workToDoc(d, [10, 20])).toEqual([390, 280]);
    expect(docToWork(d, workToDoc(d, [10, 20]))).toEqual([10, 20]);
    expect(docDeltaToWork(d, 5, 5)).toEqual([-5, -5]);
  });
});

describe('parseGrblVersion', () => {
  it('distingue 1.1f o posterior de las anteriores', () => {
    expect(parseGrblVersion("Grbl 1.1h ['$' for help]")?.modernEnough).toBe(true);
    expect(parseGrblVersion('Grbl 1.1f')?.modernEnough).toBe(true);
    expect(parseGrblVersion('Grbl 1.1e')?.modernEnough).toBe(false);
    expect(parseGrblVersion('Grbl 0.9j')?.modernEnough).toBe(false);
    expect(parseGrblVersion('Grbl 1.2')?.modernEnough).toBe(true);
    expect(parseGrblVersion('Marlin 2.0')).toBeNull();
  });
});

describe('perfiles de la Fase 4', () => {
  const rotary = (patch = {}) => ({ ...FALLBACK_DEVICE.rotary, enabled: true, ...patch });

  it('un perfil viejo sin los campos nuevos se completa', () => {
    const viejo: Record<string, unknown> = { ...FALLBACK_DEVICE };
    for (const k of ['z_axis', 'rotary', 'scan_offsets']) delete viejo[k];
    const d = normalizeDevice(viejo as never);
    expect(d.z_axis).toBe(false);
    expect(d.rotary).toEqual(FALLBACK_DEVICE.rotary);
    expect(d.scan_offsets).toEqual([]);
  });

  it('completa un rotativo a medias y conserva lo guardado', () => {
    const d = normalizeDevice({ ...FALLBACK_DEVICE, rotary: { enabled: true, kind: 'chuck' } as never });
    expect(d.rotary).toMatchObject({ enabled: true, kind: 'chuck', mm_per_rotation: 32, object_diameter: 80 });
  });

  it('completa un perfil sin dialecto ni campos de Z y barrido como GRBL 1.1', () => {
    const { dialect: _d, z_invert: _z, scan_initial_offset: _s, ...old } = FALLBACK_DEVICE;
    const d = normalizeDevice(old as never);
    expect(d.dialect).toEqual(FALLBACK_DEVICE.dialect);
    expect(d.dialect.constant_on).toBe('M3 S0');
    expect([d.z_invert, d.scan_initial_offset]).toEqual([false, 0]);
  });

  it('un perfil completo vuelve como el mismo objeto', () => {
    expect(normalizeDevice(FALLBACK_DEVICE)).toBe(FALLBACK_DEVICE);
  });

  it('con el rotativo apagado no se validan sus diámetros', () => {
    expect(validateDevice(device({ rotary: { ...rotary({ enabled: false }), object_diameter: 0 } }))).toEqual({});
  });

  it('con el rotativo activo los diámetros y los mm por vuelta deben ser positivos', () => {
    expect(validateDevice(device({ rotary: rotary({ object_diameter: 0 }) })).object_diameter).toBe('positive');
    expect(validateDevice(device({ rotary: rotary({ roller_diameter: -1 }) })).roller_diameter).toBe('positive');
    expect(validateDevice(device({ rotary: rotary({ mm_per_rotation: 0 }) })).mm_per_rotation).toBe('positive');
  });

  it('el diámetro del rodillo solo importa con rodillos', () => {
    expect(validateDevice(device({ rotary: rotary({ kind: 'chuck', roller_diameter: 0 }) }))).toEqual({});
  });

  it('las velocidades de la tabla de barrido son positivas y únicas', () => {
    expect(validateDevice(device({ scan_offsets: [{ speed: 3000, offset: 0.1 }, { speed: 6000, offset: -0.2 }] }))).toEqual({});
    expect(validateDevice(device({ scan_offsets: [{ speed: 3000, offset: 0.1 }, { speed: 3000, offset: 0.2 }] })).scan_offsets).toBe('duplicate');
    expect(validateDevice(device({ scan_offsets: [{ speed: 0, offset: 0 }] })).scan_offsets).toBe('positive');
    expect(scanOffsetRowIssues([{ speed: 1000, offset: 0 }, { speed: 1000, offset: 1 }, { speed: 2000, offset: 0 }])).toEqual(['duplicate', 'duplicate', null]);
  });

  it('ordena por velocidad sin tocar el original', () => {
    const rows = [{ speed: 6000, offset: 1 }, { speed: 2000, offset: 0 }];
    expect(sortScanOffsets(rows).map((r) => r.speed)).toEqual([2000, 6000]);
    expect(rows[0]!.speed).toBe(6000);
  });
});

describe('tipo de controladora', () => {
  it('los valores por defecto siguen la tabla del motor (device.rs)', () => {
    // GRBL con modo láser y toda su familia: el dialecto de GRBL 1.1.
    for (const k of ['grbl', 'grbl_hal', 'fluid_nc', 'grbl_lpc', 'custom_gcode'] as const) expect(dialectForKind(k)).toEqual(DEFAULT_DIALECT);
    // GRBL antiguo: sin M4, G0 no apaga (S0 explícito) y S entera.
    const m3 = dialectForKind('grbl_m3');
    expect([m3.dynamic_on, m3.g0_laser_off, m3.travel_s0, m3.s_decimals]).toEqual([null, false, true, 0]);
    // Smoothie: S de 0 a 1, cierre con M400 y sin dato del largo de línea.
    expect(kindDefaults('smoothie')).toMatchObject({ s_max: 1, power_mode: 'constant', baud: 115200 });
    expect(dialectForKind('smoothie')).toMatchObject({ end_sync: 'M400', max_line_len: 0, dynamic_on: null, dwell: 'p_milliseconds' });
    // Marlin: inline, F en G0, sin M4, 255, 250000 baudios y líneas cortas.
    expect(kindDefaults('marlin')).toMatchObject({ s_max: 255, power_mode: 'constant', baud: 250000 });
    expect(dialectForKind('marlin')).toMatchObject({ constant_on: 'M3 S0 I', feed_on_g0: true, travel_s0: true, end_sync: 'M400', max_line_len: 96, s_decimals: 0, dwell: 'p_milliseconds' });
    // GRBL: el perfil de siempre (potencia dinámica).
    expect(kindDefaults('grbl')).toMatchObject({ s_max: 1000, power_mode: 'dynamic', baud: 115200 });
    expect(kindDefaults('grbl_lpc').s_max).toBe(1);
    expect(kindDefaults('grbl_m3').s_max).toBe(255);
    expect(DEVICE_KINDS).toHaveLength(8);
  });

  it('un perfil sin dialecto toma el de su tipo, no el de GRBL', () => {
    const { dialect: _omit, ...old } = device({ kind: 'marlin' });
    expect(normalizeDevice(old).dialect).toEqual(dialectForKind('marlin'));
    // Con un dialecto parcial guardado se respeta lo guardado y se completa con el del tipo.
    expect(normalizeDevice({ ...old, dialect: { laser_off: 'M999' } as DeviceProfile['dialect'] }).dialect).toEqual({ ...dialectForKind('marlin'), laser_off: 'M999' });
  });

  it('cambiar de tipo propone solo los campos que cambian, sin tocar el resto', () => {
    const grbl = device();
    const plan = planKindChange(grbl, kindDefaults('marlin'));
    expect(plan.map((c) => c.field).sort()).toEqual(['baud', 'dialect', 'power_mode', 's_max']);
    // grblHAL es igual a GRBL 1.1: no hay nada que confirmar.
    expect(planKindChange(grbl, kindDefaults('grbl_hal'))).toEqual([]);
    // Solo el tipo.
    const only = applyKindChange(grbl, kindDefaults('marlin'), false);
    expect(only).toEqual({ ...grbl, kind: 'marlin' });
    // Con los valores del tipo: pisa dialecto, S máxima, modo de potencia y baudios; la cama y el rotativo quedan.
    const full = applyKindChange({ ...grbl, bed: [123, 45] }, kindDefaults('marlin'), true);
    expect(full).toMatchObject({ kind: 'marlin', s_max: 255, power_mode: 'constant', baud: 250000, bed: [123, 45] });
    expect(full.dialect).toEqual(dialectForKind('marlin'));
    expect(full.dialect).not.toBe(dialectForKind('marlin')); // una copia, no compartida
  });

  it('el tipo personalizado conserva todo lo que tiene el perfil', () => {
    const marlin = device({ ...kindDefaults('marlin') });
    expect(planKindChange(marlin, kindDefaults('custom_gcode'))).toEqual([]);
    const next = applyKindChange(marlin, kindDefaults('custom_gcode'), true);
    expect(next).toEqual({ ...marlin, kind: 'custom_gcode' });
  });

  it('no hay dos tipos repetidos y todos tienen valores', () => {
    const kinds: DeviceKind[] = [...DEVICE_KINDS];
    expect(new Set(kinds).size).toBe(kinds.length);
    for (const k of kinds) expect(kindDefaults(k).kind).toBe(k);
  });
});

describe('dialecto de G-code', () => {
  const dialect = (patch: Partial<DeviceProfile['dialect']> = {}) => ({ ...DEFAULT_DIALECT, ...patch });

  it('los dialectos de fábrica son válidos', () => {
    for (const k of DEVICE_KINDS) expect(validateDialect(dialectForKind(k)), k).toEqual({});
  });

  it('power_cmd necesita {s} cuando la potencia va en línea aparte', () => {
    expect(validateDialect(dialect({ s_inline: false, power_cmd: 'M3 S100' })).power_cmd).toBe('missingS');
    expect(validateDialect(dialect({ s_inline: false, power_cmd: 'M3 S{s} P{p}' })).power_cmd).toBe('badPlaceholder');
    expect(validateDialect(dialect({ s_inline: false, power_cmd: 'M3 S{s}\nM5' })).power_cmd).toBe('multiline');
    expect(validateDialect(dialect({ s_inline: false, power_cmd: 'M106 P1 S{s}' }))).toEqual({});
    // Con S dentro del G1 la plantilla no se usa, así que no se exige.
    expect(validateDialect(dialect({ s_inline: true, power_cmd: '' }))).toEqual({});
  });

  it('exige encendido con S en línea, apagado y órdenes de una sola línea', () => {
    expect(validateDialect(dialect({ constant_on: '' })).constant_on).toBe('empty');
    // Sin encendido propio vale si el láser se enciende con la línea de potencia aparte.
    expect(validateDialect(dialect({ constant_on: '', s_inline: false }))).toEqual({});
    expect(validateDialect(dialect({ laser_off: ' ' })).laser_off).toBe('empty');
    expect(validateDialect(dialect({ dynamic_on: '' })).dynamic_on).toBe('empty');
    expect(validateDialect(dialect({ dynamic_on: null }))).toEqual({});
    expect(validateDialect(dialect({ constant_on: 'M3 S0\nM8' })).constant_on).toBe('multiline');
    expect(validateDialect(dialect({ end_sync: 'M400\r\nM5' })).end_sync).toBe('multiline');
  });

  it('el aire va completo o no va', () => {
    expect(validateDialect(dialect({ air_on: 'M8', air_off: null })).air_off).toBe('airPair');
    expect(validateDialect(dialect({ air_on: null, air_off: 'M9' })).air_on).toBe('airPair');
    expect(validateDialect(dialect({ air_on: null, air_off: null }))).toEqual({});
  });

  it('decimales y largo de línea en su rango', () => {
    expect(validateDialect(dialect({ s_decimals: 7 })).s_decimals).toBe('range');
    expect(validateDialect(dialect({ decimals: -1 })).decimals).toBe('range');
    expect(validateDialect(dialect({ decimals: 2.5 })).decimals).toBe('range');
    expect(validateDialect(dialect({ max_line_len: 0 }))).toEqual({});
    expect(validateDialect(dialect({ max_line_len: 10 })).max_line_len).toBe('lineTooShort');
    expect(validateDialect(dialect({ max_line_len: 5000 })).max_line_len).toBe('range');
  });

  it('avisa de combinaciones peligrosas sin bloquear', () => {
    expect(dialectWarnings({ dialect: dialect({ g0_laser_off: false, travel_s0: false }), power_mode: 'constant' })).toEqual(['g0StaysOn']);
    expect(dialectWarnings({ dialect: dialect({ g0_laser_off: false, travel_s0: true }), power_mode: 'constant' })).toEqual([]);
    expect(dialectWarnings({ dialect: dialect({ dynamic_on: null }), power_mode: 'dynamic' })).toEqual(['dynamicMissing']);
    expect(dialectWarnings({ dialect: dialect(), power_mode: 'dynamic' })).toEqual([]);
  });

  it('resume el dialecto en una línea', () => {
    expect(dialectSummary(device())).toBe('S 0-1000, M4 dinámico, S en cada G1, aire M8/M9');
    expect(dialectSummary(device({ ...kindDefaults('marlin') }))).toBe('S 0-255, sin modo dinámico, S en cada G1, aire M8/M9, cierre M400');
    const m3 = device({ ...kindDefaults('grbl_m3') });
    expect(dialectSummary(m3)).toContain('G0 no apaga el láser');
    expect(dialectSummary(device({ dialect: dialect({ air_on: null, air_off: null, s_inline: false }) }), false)).toBe('M4 dinámico, potencia en línea aparte, sin aire');
  });

  it('el ejemplo sigue el dialecto: GRBL con M4 y S dentro del G1', () => {
    const lines = sampleGcode(device({ end_gcode: '' }), { label: 'Cuadrado' });
    expect(lines).toEqual([
      '; Generado por Tlaku', 'G90', 'G21', 'M4 S0', '; Cuadrado', 'M8',
      'G0 X10 Y10', 'G1 X30 S400 F6000', 'G1 Y30', 'G1 X10', 'G1 Y10', 'M9', 'M5',
    ]);
  });

  it('el ejemplo de Marlin: M3 S0 I, F en el G0, S entera y cierre con M400', () => {
    const lines = sampleGcode(device({ end_gcode: '', return_to_origin: true, ...kindDefaults('marlin') }), { label: 'Cuadrado' });
    expect(lines).toContain('M3 S0 I');
    expect(lines).toContain('G0 X10 Y10 F10000');
    expect(lines).toContain('G1 X30 S102 F6000');
    expect(lines.slice(-3)).toEqual(['M5', 'G0 X0 Y0 F10000', 'M400']);
  });

  it('el ejemplo con potencia en línea aparte la escribe antes del movimiento y apaga antes de moverse', () => {
    const fan = dialect({ s_inline: false, constant_on: '', power_cmd: 'M106 P1 S{s}', laser_off: 'M107 P1', g0_laser_off: false, dynamic_on: null });
    const lines = sampleGcode(device({ dialect: fan, s_max: 255, power_mode: 'constant', end_gcode: '' }), { label: 'Cuadrado', power: 100 });
    const at = lines.indexOf('M106 P1 S255');
    expect(at).toBeGreaterThan(0);
    expect(lines[at + 1]).toBe('G1 X30 F6000');
    expect(lines.at(-1)).toBe('M107 P1');
  });

  it('el ejemplo respeta el estilo de comentario, el script de inicio y el aire ausente', () => {
    const lines = sampleGcode(device({ start_gcode: 'G54\n', end_gcode: '', dialect: dialect({ comment: 'parentheses', air_on: null, air_off: null }) }), { label: 'Cuad(r)ado' });
    expect(lines[0]).toBe('(Generado por Tlaku)');
    expect(lines).toContain('(Cuadrado)');
    expect(lines).toContain('G54');
    expect(lines).not.toContain('M8');
  });
});

describe('rotativo: modelos', () => {
  const rot = FALLBACK_DEVICE.rotary;

  it('trae las cifras de xTool y Creality de taller.md', () => {
    const byId = Object.fromEntries(ROTARY_PRESETS.map((p) => [p.id, p]));
    expect(byId.xtool_ra2_roller).toMatchObject({ kind: 'roller', mm_per_rotation: 32, roller_diameter: 17 });
    expect(byId.xtool_ra2_chuck).toMatchObject({ kind: 'chuck', mm_per_rotation: 128 });
    expect(byId.creality_chuck).toMatchObject({ kind: 'chuck', mm_per_rotation: 40 });
  });

  it('aplicar un modelo cambia tipo y mm por vuelta pero no el diámetro de la pieza, el espejo ni si está activo', () => {
    const base = { ...rot, enabled: true, mirror: true, object_diameter: 65 };
    const chuck = applyRotaryPreset(base, ROTARY_PRESETS.find((p) => p.id === 'xtool_ra2_chuck')!);
    expect(chuck).toMatchObject({ enabled: true, mirror: true, object_diameter: 65, kind: 'chuck', mm_per_rotation: 128, roller_diameter: 17 });
    const roller = applyRotaryPreset({ ...base, roller_diameter: 30 }, ROTARY_PRESETS[0]!);
    expect(roller).toMatchObject({ kind: 'roller', mm_per_rotation: 32, roller_diameter: 17 });
  });

  it('reconoce el modelo elegido y devuelve nulo con valores propios', () => {
    expect(matchRotaryPreset(rot)?.id).toBe('xtool_ra2_roller');
    expect(matchRotaryPreset({ ...rot, mm_per_rotation: 41 })).toBeNull();
    expect(matchRotaryPreset({ ...rot, roller_diameter: 20 })).toBeNull();
    // Con mandril no importa el diámetro del rodillo.
    expect(matchRotaryPreset({ ...rot, kind: 'chuck', mm_per_rotation: 128, roller_diameter: 99 })?.id).toBe('xtool_ra2_chuck');
  });

  it('la vuelta de la pieza es pi por el diámetro', () => {
    expect(rotaryCircumference({ object_diameter: 80 })).toBeCloseTo(251.327, 3);
    expect(rotaryCircumference({ object_diameter: 0 })).toBeNull();
  });

  it('el desfase inicial del barrido debe ser un número', () => {
    expect(validateDevice(device({ scan_initial_offset: -0.15 }))).toEqual({});
    expect(validateDevice(device({ scan_initial_offset: Number.NaN })).scan_initial_offset).toBe('required');
  });
});
