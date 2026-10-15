import { forum, official, shop, type CatalogEntry } from './catalogBase';

/**
 * Diodos y CO2 de escritorio que hablan GRBL (o grblHAL) por USB serie, añadidos después del primer
 * catálogo. Cada cifra lleva su URL; lo que no tiene fuente se deja sin definir. Los descartes (xTool F1/P2,
 * LaserPecker, WeCreat, Gweike Cloud...) y por qué están en `docs/hardware/catalogo-fuentes.md`.
 */

const COMGROW_MANUAL = 'https://drucktipps3d.de/wp-content/uploads/2022/02/COMGO-Z1-manual-V1.1.pdf';
const LONGER_B1_WIKI = 'https://wiki.longer3d.com/en/b1-30w';
const LONGER_B1_REVIEW = 'https://the-gadgeteer.com/2023/06/23/longer-b1-30-watt-laser-engraver-review-with-great-power-comes-great-responsibility/';
const TS2_REVIEW = 'https://the-gadgeteer.com/2023/01/03/twotrees-ts2-10w-diode-laser-engraver-review-a-powerful-auto-focusing-laser-makes-this-unit-great-for-newbies/';
const ATOMSTACK_A20V2 = 'https://atomstack.com/it/products/atomstack-ace-pro-v2';
const ATOMSTACK_A20_DUMP = 'https://forum.lightburnsoftware.com/t/168593';
const ATOMSTACK_A40 = 'https://atomstack.com/fr/products/atomstack-a40-pro-laser-machine-with-f60-air-assist-kit-f3-matrix-cutting-panel-fb2-enclosure-and-r1-pro-rotary-roller';
const ATOMSTACK_A40_FORUM = 'https://forum.lightburnsoftware.com/t/163085';
const SCULPFUN_A9 = 'https://sculpfun.com/products/sculpfun-sf-a9-40w-laser-engraver-cutting-machine';
const SCULPFUN_A9_DUMP = 'https://forum.lightburnsoftware.com/t/157620';
const ALGOLASER_DELTA = 'https://algolaser.com/products/algolaser-delta-22w-diode-laser-engraver';
const ALGOLASER_DELTA_FORUM = 'https://forum.lightburnsoftware.com/t/123448';
const ALGOLASER_FAQ = 'https://algolaser.com/pages/faq';
const NEJE_MAX4_FORUM = 'https://forum.lightburnsoftware.com/t/117525';
const ATEZR_REVIEW = 'https://the-gadgeteer.com/2022/10/21/atezr-p20-plus-laser-engraving-machine-review-its-not-for-everyone-but-perfect-for-those-who-want-one';
const ATEZR_FORUM = 'https://forum.v1e.com/t/atezr-20w-and-36w-laser-machines-lightburn-miscellaneous-ops-and-observations/36785';
const MONPORT_40W = 'https://monportlaser.com/products/monport-40w-lightburn-laser-engraver';
const FALCON_A1_PRO = 'https://www.creality.com/products/falcon-a1-pro-20w';
const FALCON_A1_FORUM = 'https://forum.lightburnsoftware.com/t/177926';

/** Texto del saludo del SF-A9 (`$I`: «[MSG: Using machine:Sculpfun SF-A9]») y del Delta («AlgoLaser Delta Ready!»). */
const SF_A9_SIGNATURE = ['sculpfun sf-a9'];
const DELTA_SIGNATURE = ['algolaser delta'];

export const DIODE_ENTRIES: readonly CatalogEntry[] = [
  {
    id: 'comgrow-z1',
    brand: 'Comgrow',
    model: 'Z1 (1.6 / 5 / 10 W)',
    kind: 'grbl',
    aliases: ['COMGO Z1', 'Comgo'],
    controller: shop('GRBL (LaserGRBL / LightBurn)', COMGROW_MANUAL, 'catalog.note.comgrow.manual'),
    bed: shop([400, 400], COMGROW_MANUAL),
    sMax: shop(1000, COMGROW_MANUAL),
    m4: shop(true, COMGROW_MANUAL),
    baud: shop(115200, COMGROW_MANUAL),
    origin: shop('bottom_left', COMGROW_MANUAL),
    notes: ['catalog.note.comgrow.s1000'],
  },
  {
    id: 'longer-b1',
    brand: 'Longer',
    model: 'B1 (20 / 30 W)',
    kind: 'grbl',
    aliases: ['Laser B1', 'B1 30W', 'B1 20W'],
    controller: official('GRBL (LaserGRBL / LightBurn, M8 para el aire)', LONGER_B1_WIKI),
    bed: shop([450, 440], LONGER_B1_REVIEW, 'catalog.note.longer.b1Bed'),
    maxRate: shop(36000, LONGER_B1_REVIEW),
    sMax: official(1000, LONGER_B1_WIKI),
    air: official(true, LONGER_B1_WIKI, 'catalog.note.longer.b1Air'),
    baud: official(115200, LONGER_B1_WIKI),
    zAxis: shop(false, LONGER_B1_REVIEW, 'catalog.note.longer.b1Z'),
    notes: ['catalog.note.longer.b1Limits'],
  },
  {
    id: 'twotrees-ts2',
    brand: 'Two Trees',
    model: 'TS2 (10 W)',
    kind: 'grbl',
    aliases: ['TS2-10W', 'TS2 20W'],
    bed: shop([450, 450], TS2_REVIEW),
    maxRate: shop(10000, TS2_REVIEW, 'catalog.note.twotrees.ts2Speed'),
    notes: ['catalog.note.twotrees.firmware', 'catalog.note.twotrees.ts2Board'],
  },

  {
    id: 'atomstack-a20-pro-v2',
    brand: 'Atomstack',
    model: 'A20 Pro V2',
    kind: 'grbl',
    aliases: ['A20 Pro', 'Ace Pro V2'],
    bed: official([400, 365], ATOMSTACK_A20V2, 'catalog.note.atomstack.a20v2Bed'),
    maxRate: official(24000, ATOMSTACK_A20V2),
    notes: ['catalog.note.atomstack.a20v2Firmware'],
  },
  {
    id: 'atomstack-a20-pro',
    brand: 'Atomstack',
    model: 'A20 Pro (original)',
    kind: 'grbl',
    aliases: ['A20'],
    controller: forum('Grbl 1.1h', ATOMSTACK_A20_DUMP),
    bed: forum([400, 400], ATOMSTACK_A20_DUMP),
    sMax: forum(1000, ATOMSTACK_A20_DUMP),
    notes: ['catalog.note.atomstack.a20Dump'],
  },
  {
    id: 'atomstack-a40-pro',
    brand: 'Atomstack',
    model: 'A40 Pro / A40 Pro V2 (24 / 48 W)',
    kind: 'grbl',
    aliases: ['A40', 'A40 Pro V2'],
    controller: forum('Grbl C10-0132-V1002 (ESP32)', ATOMSTACK_A40_FORUM),
    bed: official([385, 400], ATOMSTACK_A40, 'catalog.note.atomstack.a40Bed'),
    notes: ['catalog.note.atomstack.a40Firmware'],
  },

  {
    // El saludo de `$I` trae «Using machine:Sculpfun SF-A9» (hilo del foro con la salida completa).
    id: 'sculpfun-sf-a9',
    brand: 'Sculpfun',
    model: 'SF-A9 (20 / 40 W)',
    kind: 'grbl',
    aliases: ['A9', 'SF A9'],
    detect: forum(SF_A9_SIGNATURE, SCULPFUN_A9_DUMP),
    signatures: [{ contains: SF_A9_SIGNATURE, source: SCULPFUN_A9_DUMP, kind: 'foro' }],
    controller: forum('Grbl 1.1h (32 bits)', SCULPFUN_A9_DUMP),
    bed: official([400, 400], SCULPFUN_A9),
    maxRate: official(36000, SCULPFUN_A9, 'catalog.note.sculpfun.a9Speed'),
    sMax: forum(1000, SCULPFUN_A9_DUMP),
    air: official(true, SCULPFUN_A9),
    notes: ['catalog.note.sculpfun.a9Dump'],
  },
  {
    id: 'sculpfun-icube-pro',
    brand: 'Sculpfun',
    model: 'iCube Pro (5 W)',
    kind: 'grbl',
    aliases: ['iCube'],
    bed: official([125, 125], 'https://sculpfun.com/products/sculpfun-icube-pro-5w-portable-laser-engraving-machine'),
    maxRate: official(10000, 'https://sculpfun.com/products/sculpfun-icube-pro-5w-portable-laser-engraving-machine'),
    notes: ['catalog.note.sculpfun.icube'],
  },
  {
    id: 'sculpfun-icube-pro-max',
    brand: 'Sculpfun',
    model: 'iCube Pro Max (10 W)',
    kind: 'grbl',
    aliases: ['iCube Max'],
    bed: official([120, 120], 'https://sculpfun.com/products/sculpfun-icube-pro-max-10w-portable-laser-engraving-machine'),
    maxRate: official(10000, 'https://sculpfun.com/products/sculpfun-icube-pro-max-10w-portable-laser-engraving-machine'),
    notes: ['catalog.note.sculpfun.icube'],
  },
  {
    id: 'sculpfun-icube-ultra',
    brand: 'Sculpfun',
    model: 'iCube Ultra (12 W)',
    kind: 'grbl',
    aliases: ['iCube Ultra Dual'],
    bed: official([150, 150], 'https://sculpfun.com/products/icube-ultra-portable-laser-engraving-machine', 'catalog.note.sculpfun.icubeUltraDual'),
    maxRate: official(10000, 'https://sculpfun.com/products/icube-ultra-portable-laser-engraving-machine'),
    notes: ['catalog.note.sculpfun.icube'],
  },

  {
    // El saludo de un Delta real dice «AlgoLaser Delta Ready! ... GrblHAL 1.1f» (misma familia de firmware que Ortur).
    id: 'algolaser-delta',
    brand: 'AlgoLaser',
    model: 'Delta (22 W)',
    kind: 'grbl_hal',
    alsoKinds: ['grbl'],
    aliases: ['Delta 22W'],
    detect: forum(DELTA_SIGNATURE, ALGOLASER_DELTA_FORUM),
    signatures: [{ contains: DELTA_SIGNATURE, show: ['grblhal'], source: ALGOLASER_DELTA_FORUM, kind: 'foro' }],
    controller: forum('GrblHAL 1.1f', ALGOLASER_DELTA_FORUM),
    bed: official([440, 415], ALGOLASER_DELTA),
    maxRate: official(40000, ALGOLASER_DELTA, 'catalog.note.algolaser.deltaSpeed'),
    notes: ['catalog.note.algolaser.deltaHoming'],
  },
  {
    id: 'algolaser-diy-kit',
    brand: 'AlgoLaser',
    model: 'DIY Kit (5 / 10 W)',
    kind: 'grbl',
    aliases: ['DIY Kit MK2', 'DIY Kit MK3'],
    bed: official([400, 400], ALGOLASER_FAQ),
    notes: ['catalog.note.algolaser.diyFirmware'],
  },

  {
    id: 'neje-max-4',
    brand: 'Neje',
    model: 'Max 4 (E80 / E40)',
    kind: 'grbl',
    aliases: ['Neje 4 Max', 'Neje Max4', 'Max 4 E80'],
    controller: forum('Grbl 1.1f', NEJE_MAX4_FORUM),
    notes: ['catalog.note.neje.max4', 'catalog.note.neje.max4Bed'],
  },
  {
    id: 'atezr-p20-plus',
    brand: 'Atezr',
    model: 'P20 Plus (20 W)',
    kind: 'grbl',
    aliases: ['P20', 'Atezr 20W'],
    controller: forum('Grbl 1.1h', ATEZR_FORUM),
    bed: shop([430, 430], ATEZR_REVIEW),
    notes: ['catalog.note.atezr.p20'],
  },
  {
    id: 'monport-40w-pro',
    brand: 'Monport',
    model: '40 W Pro CO2 (8 × 12 in)',
    kind: 'grbl',
    aliases: ['Monport 40W', 'Monport 40W 2.0'],
    controller: official('GRBL', MONPORT_40W),
    bed: official([300, 200], MONPORT_40W),
    maxRate: official(21000, MONPORT_40W),
    notes: ['catalog.note.monport.co2Grbl'],
  },
  {
    // Firmware propio con base grblHAL 1.1f; LightBurn lo describe como no estándar (hilo con el volcado de `$$`).
    id: 'creality-falcon-a1-pro',
    brand: 'Creality',
    model: 'Falcon A1 Pro',
    kind: 'grbl_hal',
    alsoKinds: ['grbl'],
    aliases: ['Falcon A1', 'A1 Pro'],
    controller: forum('GrblHAL 1.1f (firmware propio)', FALCON_A1_FORUM, 'catalog.note.falcon.a1Custom'),
    bed: official([358, 268], FALCON_A1_PRO, 'catalog.note.falcon.a1Bed'),
    maxRate: official(36000, FALCON_A1_PRO),
    accel: forum(1000, FALCON_A1_FORUM),
    sMax: forum(1000, FALCON_A1_FORUM),
    notes: ['catalog.note.falcon.a1Reports'],
  },
];
