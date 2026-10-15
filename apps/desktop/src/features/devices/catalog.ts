import { official, forum, shop, measured, type CatalogEntry } from './catalogBase';
import { DIODE_ENTRIES } from './catalog.diode';
import { RUIDA_ENTRIES } from './catalog.ruida';

export * from './catalogBase';

/**
 * Catálogo de máquinas por marca y modelo. Las reglas (sala limpia, fuente por dato, niveles de
 * confianza) están en `catalogBase.ts`.
 *
 * - Datos de Creality Falcon2: `docs/hardware/falcon2.md`. Firmware de Atomstack, Ortur, Longer,
 *   Neje y xTool: `docs/hardware/controladoras.md` sección 1.4. Dialectos: `docs/hardware/dialectos-gcode.md`.
 * - Modelos añadidos después (CO2 Ruida, diodos GRBL): un archivo por familia (`catalog.ruida.ts`,
 *   `catalog.diode.ts`) y su documentación en `docs/hardware/catalogo-fuentes.md`.
 */

// ---------------------------------------------------------------------------
// Fuentes que se repiten
// ---------------------------------------------------------------------------

const FALCON_COMPARE = 'https://crealityfalcon.com/blogs/product-tutorials-and-support/the-creality-falcon2-series-comparison';
const FALCON_PROFILE = 'https://forum.creality.com/t/profile-settings/36308';
const FALCON_DUMP_A = 'https://forum.lightburnsoftware.com/t/creality-falcon2-s-value-variable-power-issue/137714';
const FALCON_DUMP_B = 'https://forum.lightburnsoftware.com/t/falcon-2-pro-stuck-in-startup-loop/192185';
const FALCON_M4 = 'https://forum.lightburnsoftware.com/t/creality-falcon-2-40w/137856';
/** Sesión con una Falcon2 Pro 22 W real (firmware CV50-Pro-MASTER 2023-11-29). */
const FALCON_MEASURED = 'https://github.com/jahziel1/tlaku/blob/main/docs/hardware/falcon2.md#10-primera-sesión-con-una-falcon2-real-2026-10-14-macos';
const FALCON_AIR = 'https://au.crealityfalcon.com/products/falcon2-pro-40w-and-22w-enclosed-laser-engraver-cutter';
const FALCON_HOMING = 'https://forum.lightburnsoftware.com/t/changing-default-home-location-for-creality-falcon-2-pro/184864';
const LM3_FORUM = 'https://forum.lightburnsoftware.com/t/ortur-lm3-not-connecting-with-either-wifi-or-usb/76960';
const ATOMSTACK_FORUM = 'https://forum.lightburnsoftware.com/t/adding-atomstack-x7-a10-s10-in-lightburn/59803';
const LASER_MODE_DOC = 'https://github.com/gnea/grbl/wiki/Grbl-v1.1-Laser-Mode';
const GRBL_ISSUES = 'https://github.com/gnea/grbl/wiki/Known-Issues';
const LB_DEVICE_SETTINGS = 'https://docs.lightburnsoftware.com/UI/DeviceSettings.html';

/** Lo que comparten las Falcon2: la 22 W y la 40 W son la misma plataforma; la Pro cambia cifras. */
function falcon(model: string, id: string, extra: Partial<CatalogEntry>): CatalogEntry {
  return {
    id,
    brand: 'Creality',
    model,
    kind: 'grbl',
    aliases: ['Falcon 2', 'Falcon2'],
    detect: forum(['cv50', 'usb connect succeed', 'ilmp'], FALCON_DUMP_B),
    sMax: forum(1000, FALCON_DUMP_A),
    m4: forum(true, FALCON_M4),
    air: official(true, FALCON_AIR),
    baud: official(115200, FALCON_PROFILE),
    origin: forum('bottom_left', FALCON_PROFILE),
    homing: forum(true, FALCON_DUMP_B, 'catalog.note.falcon.homing'),
    zAxis: official(false, FALCON_PROFILE),
    notes: ['catalog.note.falcon.readSpeeds', 'catalog.note.falcon.cable', 'catalog.note.falcon.workOffsets'],
    ...extra,
  };
}

// ---------------------------------------------------------------------------
// Catálogo
// ---------------------------------------------------------------------------

export const CATALOG: readonly CatalogEntry[] = [
  falcon('Falcon2 22 W', 'creality-falcon2-22w', {
    bed: official([400, 415], FALCON_COMPARE),
    maxRate: shop(25000, 'https://www.adorama.com/cr1005010047.html'),
    accel: forum(1000, FALCON_DUMP_A, 'catalog.note.falcon.accelDiffers'),
  }),
  falcon('Falcon2 40 W', 'creality-falcon2-40w', {
    bed: official([400, 415], FALCON_COMPARE),
    maxRate: shop(25000, 'https://www.adorama.com/cr1005010047.html'),
    accel: forum(1000, FALCON_DUMP_A, 'catalog.note.falcon.accelDiffers'),
  }),
  falcon('Falcon2 Pro (22 / 40 W)', 'creality-falcon2-pro', {
    bed: shop(
      [400, 415],
      'https://slickdeals.net/f/18970864-creality-falcon-2-pro-22w-laser-engraver-and-cutter-fully-enclosed-diode-laser-15-7-x-16-3-in-work-area-camera-alignment-air-assist-with-code-cffam-719-1',
      'catalog.note.falcon.proBedY',
    ),
    maxRate: measured(25000, FALCON_MEASURED, 'catalog.note.falcon.proSpeed'),
    accel: measured(1000, FALCON_MEASURED, 'catalog.note.falcon.accelDiffers'),
    homing: forum(true, FALCON_HOMING, 'catalog.note.falcon.homing'),
    // Cadena leída de `$I` en una Pro 22 W real. La clásica no tiene salida publicada: no se firma.
    signatures: [{ contains: ['cv50-pro-master'], show: ['inquire_info->power'], bed: [400, 415], source: FALCON_MEASURED, kind: 'medido' }],
  }),

  {
    id: 'sculpfun-s9',
    brand: 'Sculpfun',
    model: 'S9',
    kind: 'grbl',
    bed: official([410, 415], 'https://sculpfun.com/products/sculpfun-s9-laser-engraver', 'catalog.note.sculpfun.s9Bed'),
    maxRate: shop(10000, 'https://3ddevice.com.ua/en/product/sculpfun-s9-5.5w-laser-engraver-deluxe-bundle/'),
    notes: ['catalog.note.sculpfun.firmware', 'catalog.note.sculpfun.homing'],
  },
  {
    id: 'sculpfun-s30',
    brand: 'Sculpfun',
    model: 'S30 / S30 Pro / S30 Pro Max',
    kind: 'grbl',
    aliases: ['S30 Pro', 'S30 Pro Max'],
    maxRate: forum(6000, 'https://forum.lightburnsoftware.com/t/highest-speed-for-s30-pro-max/115242', 'catalog.note.sculpfun.s30Speed'),
    notes: ['catalog.note.sculpfun.s30Bed', 'catalog.note.sculpfun.firmware'],
  },

  {
    id: 'atomstack-a5-pro',
    brand: 'Atomstack',
    model: 'A5 Pro',
    kind: 'grbl',
    aliases: ['A5'],
    bed: shop([410, 400], 'https://atomstack.com/products/atomstack-a5-pro'),
    sMax: forum(1000, ATOMSTACK_FORUM),
    baud: forum(115200, ATOMSTACK_FORUM),
    notes: ['catalog.note.atomstack.laserMode'],
  },
  {
    id: 'atomstack-a10-pro',
    brand: 'Atomstack',
    model: 'A10 Pro',
    kind: 'grbl',
    aliases: ['A10'],
    bed: shop([410, 400], 'https://www.newegg.com/p/288-00CG-00058'),
    sMax: forum(1000, ATOMSTACK_FORUM),
    baud: forum(115200, ATOMSTACK_FORUM),
    notes: ['catalog.note.atomstack.laserMode'],
  },
  {
    id: 'atomstack-x20-pro',
    brand: 'Atomstack',
    model: 'X20 Pro',
    kind: 'grbl',
    aliases: ['X20'],
    bed: shop([400, 400], 'https://www.ozbargain.com.au/node/810066', 'catalog.note.atomstack.x20Bed'),
    notes: ['catalog.note.atomstack.x20Source'],
  },

  {
    id: 'ortur-lm2-pro',
    brand: 'Ortur',
    model: 'Laser Master 2 Pro',
    kind: 'grbl',
    aliases: ['LM2', 'LM2 Pro', 'Laser Master 2'],
    bed: shop([400, 400], 'https://www.cncsourced.com/reviews/ortur-laser-master-2-pro-review-specs-test/', 'catalog.note.ortur.lm2Bed'),
    notes: ['catalog.note.ortur.lm2Firmware'],
  },
  {
    id: 'ortur-lm3',
    brand: 'Ortur',
    model: 'Laser Master 3',
    kind: 'grbl_hal',
    // grblHAL con nivel de compatibilidad 1 o más dice `Grbl 1.1` en el saludo (docs/hardware/grbl.md).
    alsoKinds: ['grbl'],
    aliases: ['LM3'],
    detect: forum(['ortur laser master 3', 'grblhal'], LM3_FORUM),
    // El saludo y `[MODEL: ...]` del hilo traen el nombre completo.
    signatures: [{ contains: ['ortur laser master 3'], show: ['grblhal'], source: LM3_FORUM, kind: 'foro' }],
    notes: ['catalog.note.ortur.lm3Hal', 'catalog.note.ortur.lm3Bed'],
  },

  {
    id: 'xtool-d1',
    brand: 'xTool',
    model: 'D1',
    kind: 'grbl',
    notes: ['catalog.note.xtool.dialect', 'catalog.note.xtool.d1Bed', 'catalog.note.xtool.network'],
  },
  {
    id: 'xtool-d1-pro',
    brand: 'xTool',
    model: 'D1 Pro',
    kind: 'grbl',
    bed: official(
      [430, 390],
      'https://ca.xtool.com/products/portable-diode-laserbox-d1-for-laser-engraving-laser-cutting',
      'catalog.note.xtool.d1ProBed',
    ),
    // 400 mm/s según la tabla de especificaciones de xTool.
    maxRate: official(24000, 'https://ca.xtool.com/products/portable-diode-laserbox-d1-for-laser-engraving-laser-cutting'),
    notes: ['catalog.note.xtool.dialect', 'catalog.note.xtool.network'],
  },
  {
    id: 'xtool-s1',
    brand: 'xTool',
    model: 'S1',
    kind: 'grbl',
    bed: shop([498, 319], 'https://microcenter.com/quickView/697941/xtool-s1-enclosed-diode-laser-cutter', 'catalog.note.xtool.s1Bed'),
    // 600 mm/s en la ficha de la tienda.
    maxRate: shop(36000, 'https://microcenter.com/quickView/697941/xtool-s1-enclosed-diode-laser-cutter'),
    notes: ['catalog.note.xtool.dialect', 'catalog.note.xtool.network'],
  },

  {
    id: 'longer-ray5',
    brand: 'Longer',
    model: 'Ray5 (5 / 10 / 20 W)',
    kind: 'grbl',
    aliases: ['Ray 5'],
    bed: official([400, 400], 'https://www.longer.net/products/longer-ray5-10w-laser-engraver', 'catalog.note.longer.bed20w'),
    maxRate: official(10000, 'https://www.longer.net/products/longer-ray5-10w-laser-engraver', 'catalog.note.longer.speedConflict'),
    baud: official(115200, 'https://www.longer.net/products/longer-ray5-10w-laser-engraver'),
    notes: ['catalog.note.longer.esp32'],
  },

  {
    id: 'neje-master-2s',
    brand: 'Neje',
    model: 'Master 2S / 2S Plus',
    kind: 'grbl',
    aliases: ['Master 2S Plus', 'A40640'],
    notes: ['catalog.note.neje.bed', 'catalog.note.neje.baud'],
  },

  {
    id: 'twotrees-tts10-pro',
    brand: 'Two Trees',
    model: 'TTS-10 Pro',
    kind: 'grbl',
    bed: shop([300, 300], 'https://www.thingbits.in/products/twotrees-tts-10-pro-diode-laser-cnc-engraver'),
    notes: ['catalog.note.twotrees.firmware'],
  },
  {
    id: 'twotrees-tts20-pro',
    brand: 'Two Trees',
    model: 'TTS-20 Pro',
    kind: 'grbl',
    bed: shop([418, 418], 'https://www.cnx-software.com/2023/12/27/tts-20-pro-laser-engraver-review/', 'catalog.note.twotrees.tts20Bed'),
    maxRate: shop(30000, 'https://www.cnx-software.com/2023/12/27/tts-20-pro-laser-engraver-review/'),
    notes: ['catalog.note.twotrees.firmware'],
  },

  {
    id: 'k40-cohesion3d',
    brand: 'K40',
    model: 'K40 con Cohesion3D (GRBL-LPC)',
    kind: 'grbl_lpc',
    aliases: ['Cohesion3D', 'Cohesion3D Mini', 'LaserBoard', 'K40'],
    bed: forum([300, 200], 'https://forum.lightburnsoftware.com/t/cant-get-the-software-to-line-up-with-the-workable-area/17532', 'catalog.note.k40.bedOrder'),
    sMax: official(1, 'https://github.com/cprezzi/grbl-LPC'),
    notes: ['catalog.note.k40.smoothie', 'catalog.note.k40.origin'],
  },

  {
    // La placa M2 Nano de serie (Lihuiyu) no es GRBL ni usa puerto serie: habla LHYMICRO-GL por USB.
    // Solo se escribe la cama que tiene fuente; el home arriba a la izquierda y la velocidad máxima
    // son valores de partida que `buildProfile` marca como tales.
    id: 'k40-m2nano',
    brand: 'K40',
    model: 'K40 con M2 Nano (Lihuiyu)',
    kind: 'lhy_micro',
    aliases: ['M2 Nano', 'Lihuiyu', 'LHYMICRO', 'LaserDRW', 'K40 Whisperer', 'K40'],
    bed: forum([300, 200], 'https://forum.lightburnsoftware.com/t/cant-get-the-software-to-line-up-with-the-workable-area/17532', 'catalog.note.k40.bedOrder'),
    notes: ['lhymicro.note.power', 'lhymicro.note.position', 'lhymicro.note.moshiboard'],
  },

  {
    // Galvo con tarjeta JCZ/BJJCZ (EZCAD2, USB 9588:9899): fibra, UV o CO2 galvo. No es GRBL ni usa puerto
    // serie. No hay datos con fuente de la cama (es el campo de la lente): se piden en el asistente.
    id: 'galvo-ezcad2',
    brand: 'Galvo',
    model: 'Galvo (fibra o UV, EZCAD2)',
    kind: 'galvo',
    aliases: ['EZCAD2', 'EZCAD', 'JCZ', 'BJJCZ', 'LMCV4', 'LMC', 'fibra', 'fiber', 'UV', 'MOPA', 'Raycus', 'JPT', 'F-theta'],
    notes: ['galvo.note.experimental', 'galvo.note.field', 'galvo.note.origin'],
  },

  {
    id: 'grbl-generic',
    brand: 'GRBL',
    model: 'GRBL genérico (1.1f o posterior)',
    kind: 'grbl',
    generic: true,
    sMax: official(1000, LB_DEVICE_SETTINGS),
    m4: official(true, LASER_MODE_DOC),
    baud: official(115200, GRBL_ISSUES),
    notes: ['catalog.note.generic.grbl'],
  },
  {
    id: 'marlin-generic',
    brand: 'Marlin',
    model: 'Marlin genérico (láser)',
    kind: 'marlin',
    generic: true,
    sMax: official(255, 'https://marlinfw.org/docs/gcode/M003.html', 'catalog.note.marlin.powerUnit'),
    baud: official(250000, 'https://marlinfw.org/docs/setting/serial.html'),
    notes: ['catalog.note.marlin.slowStart', 'catalog.note.generic.noSettings'],
  },
  {
    id: 'smoothie-generic',
    brand: 'Smoothieware',
    model: 'Smoothie genérico (láser)',
    kind: 'smoothie',
    generic: true,
    sMax: official(1, 'https://smoothieware.org/laser', 'catalog.note.smoothie.maxS'),
    notes: ['catalog.note.smoothie.baud', 'catalog.note.generic.noSettings'],
  },

  // Modelos añadidos después, un archivo por familia.
  ...RUIDA_ENTRIES,
  ...DIODE_ENTRIES,
];

export function findEntry(id: string): CatalogEntry | undefined {
  return CATALOG.find((e) => e.id === id);
}

/** Todas las URLs de fuente de una entrada (para mostrarlas y para las pruebas). */
export function entrySources(entry: CatalogEntry): string[] {
  const facts = [entry.detect, entry.controller, entry.bed, entry.maxRate, entry.accel, entry.sMax, entry.m4, entry.air, entry.baud, entry.origin, entry.homing, entry.zAxis];
  const urls = [...facts.flatMap((f) => (f ? [f.source] : [])), ...(entry.signatures ?? []).map((s) => s.source)];
  return [...new Set(urls)];
}
