import { forum, official, type CatalogEntry } from './catalogBase';

/**
 * CO2 con controladora Ruida (conexión de Tlaku: UDP a la IP de la controladora).
 *
 * El tipo `ruida` de cada entrada sale de fuentes públicas que nombran la controladora (panel Ruida en
 * la ficha, hilos del foro de LightBurn); donde la ficha solo menciona RDWorks se dice en la nota. La
 * cama y la velocidad son cifras de fabricante (pulgadas convertidas a mm). Detalle y descartes en
 * `docs/hardware/catalogo-fuentes.md`.
 */

const OMTECH_MAKER = 'https://omtech.com/products/90w-co2-laser-engraver-cutting-machine-with-20-x-28-working-area-with-autofocus';
const OMTECH_1624 = 'https://omtech.com/products/60w-co2-laser-engraver-with-16-x-24-working-area';
const OMTECH_PRONTO = 'https://omtech.com/products/pronto-40-90w-co2-laser-engraver-and-cutter';
const OMTECH_POLAR_LITE = 'https://omtech.com/products/omtech-polar-lite-55w-desktop-co2-laser-engraver-and-cutter';
const MONPORT_FORUM = 'https://forum.lightburnsoftware.com/t/monport-60w-20-x-28-co2-with-autofocus-lightburn-profile/115154';
const THUNDER_FORUM = 'https://forum.lightburnsoftware.com/t/184980';
const MONPORT_EFFI7S = 'https://monportlaser.com/products/monport-90w-co2-laser-engraver-cutter-28-x-20-with-autofocus';
const THUNDER_NOVA35 = 'https://eu.thunderlaser.com/?p=508';
const THUNDER_NOVA35_SHEET = 'https://nz.thunderlaser.com/wp-content/uploads/2026/01/Nova-35-80w-Dec25.pdf';

export const RUIDA_ENTRIES: readonly CatalogEntry[] = [
  {
    // Ficha oficial: «Ruida control panel», 20 x 28 in (500 x 700 mm), 600 mm/s. Los modelos de 60 y 100 W de
    // la misma tabla comparten área y velocidad.
    id: 'omtech-maker-2028',
    brand: 'OMTech',
    model: 'Maker 20 × 28 in (60 / 90 / 100 W)',
    kind: 'ruida',
    aliases: ['AF2028', 'MF2028', 'AF2028-80', 'MF2028-80', 'Maker 2028'],
    controller: official('Ruida', OMTECH_MAKER),
    bed: official([700, 500], OMTECH_MAKER, 'catalog.note.omtech.makerBed'),
    maxRate: official(36000, OMTECH_MAKER),
    notes: ['catalog.note.ruida.network', 'catalog.note.omtech.makerPanel', 'catalog.note.ruida.bedFromController'],
  },
  {
    id: 'omtech-maker-1624',
    brand: 'OMTech',
    model: 'Maker 16 × 24 in (60 W)',
    kind: 'ruida',
    aliases: ['MF1624', 'MF1624-60', 'Maker 1624'],
    controller: official('Ruida', OMTECH_1624),
    bed: official([600, 400], OMTECH_1624, 'catalog.note.omtech.makerBed'),
    maxRate: official(36000, OMTECH_1624),
    notes: ['catalog.note.ruida.network', 'catalog.note.omtech.bed1624', 'catalog.note.ruida.bedFromController'],
  },
  {
    // La ficha no nombra la controladora: trae RDWorks, el software de Ruida (inferencia, dicha en la nota).
    id: 'omtech-pronto-40',
    brand: 'OMTech',
    model: 'Pronto 40 (90 W, 900 × 600 mm)',
    kind: 'ruida',
    aliases: ['Pronto', 'Pronto 35', 'Pronto 40'],
    controller: official('RDWorks (Ruida)', OMTECH_PRONTO, 'catalog.note.omtech.rdworks'),
    bed: official([900, 600], OMTECH_PRONTO),
    maxRate: official(60000, OMTECH_PRONTO),
    notes: ['catalog.note.ruida.network', 'catalog.note.ruida.bedFromController'],
  },
  {
    id: 'omtech-polar-lite-55w',
    brand: 'OMTech',
    model: 'Polar Lite (55 W, escritorio)',
    kind: 'ruida',
    aliases: ['Polar Lite', 'Polar 55W'],
    controller: official('RDWorks (Ruida)', OMTECH_POLAR_LITE, 'catalog.note.omtech.rdworks'),
    bed: official([510, 300], OMTECH_POLAR_LITE),
    maxRate: official(30000, OMTECH_POLAR_LITE),
    notes: ['catalog.note.ruida.network', 'catalog.note.ruida.bedFromController'],
  },
  {
    // El área de la ficha es la del modelo de 90 W; el de 60 W salió en un hilo del foro con DSP, FTDI y RDWorks.
    id: 'monport-co2-20x28',
    brand: 'Monport',
    model: 'CO2 20 × 28 in con autoenfoque (60 / 90 W)',
    kind: 'ruida',
    aliases: ['Effi7S', 'Monport 60W', 'Monport 80W', 'Monport 90W'],
    controller: forum('Ruida (DSP, RDWorks)', MONPORT_FORUM, 'catalog.note.monport.ruida'),
    bed: official([700, 500], MONPORT_EFFI7S, 'catalog.note.monport.ruidaBed'),
    notes: ['catalog.note.ruida.network', 'catalog.note.ruida.bedFromController'],
  },
  {
    id: 'thunder-nova-35',
    brand: 'Thunder Laser',
    model: 'Nova 35 (80 / 100 W)',
    kind: 'ruida',
    aliases: ['Nova35', 'Nova'],
    controller: forum('Ruida', THUNDER_FORUM, 'catalog.note.thunder.ruida'),
    bed: official([900, 600], THUNDER_NOVA35),
    maxRate: official(60000, THUNDER_NOVA35_SHEET),
    notes: ['catalog.note.ruida.network', 'catalog.note.ruida.bedFromController'],
  },
  {
    // Familia: sin cifras (cada máquina trae su cama y su origen), pero con las notas de cómo conectar.
    id: 'ruida-generic',
    brand: 'Ruida',
    model: 'Ruida genérico (RDC644x)',
    kind: 'ruida',
    generic: true,
    aliases: ['RDC6442', 'RDC6442G', 'RDC6445', 'RDC6445G', 'RDC6445S', 'RDC6444', 'RDWorks', 'CO2 DSP'],
    notes: ['catalog.note.ruida.network', 'catalog.note.ruida.bedFromController', 'catalog.note.ruida.generic'],
  },
];
