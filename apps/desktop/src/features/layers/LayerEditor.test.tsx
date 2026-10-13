import { renderToString } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import { DEFAULT_TAB_SETTINGS, makeLayer } from '@/lib/layers';
import { GUIDE_COLOR } from '@/lib/palette';
import { LayerEditor } from './LayerEditor';
import { LayerFieldsView, type FieldsDevice } from './LayerFields';
import { SubLayers } from './SubLayers';

/** Humo: el editor se dibuja en cada modo y muestra solo los campos que corresponden. */
const html = (patch = {}) => renderToString(<LayerEditor layer={makeLayer('#ff0000', 'Corte', patch)} />);

describe('LayerEditor por modo', () => {
  it('line: kerf y sección de puentes cerrada por defecto', () => {
    const h = html();
    expect(h).toContain('Kerf');
    expect(h).toContain('Dejar puentes sin cortar');
    expect(h).not.toContain('Ancho del puente');
    expect(h).not.toContain('Tamaño de celda');
    expect(h).not.toContain('Relleno</label>');
  });

  it('line con puentes: ancho, cantidad, separación y formas interiores', () => {
    const h = html({ tabs: { ...DEFAULT_TAB_SETTINGS, enabled: true } });
    for (const label of ['Ancho del puente', 'Cantidad', 'Separación', 'Omitir formas interiores']) expect(h).toContain(label);
    expect(h).toContain('4 puentes por forma cerrada');
  });

  it('line con separación fija: la cantidad pasa a ser el mínimo por forma', () => {
    const h = html({ tabs: { ...DEFAULT_TAB_SETTINGS, enabled: true, spacing: 20 } });
    expect(h).toContain('Mínimo por forma');
    expect(h).toContain('Uno cada 20 mm');
  });

  it('fill con barrido: ángulo, trama cruzada, bidireccional y overscan', () => {
    const h = html({ mode: 'fill' });
    for (const label of ['Ángulo', 'Trama cruzada', 'Bidireccional', 'Overscan automático']) expect(h).toContain(label);
    expect(h).not.toContain('Con contornos no se usan');
    expect(h).not.toContain('Dejar puentes sin cortar');
  });

  it('fill con contornos: oculta los ajustes de barrido y dice por qué', () => {
    const h = html({ mode: 'fill', fill_style: 'offset' });
    expect(h).toContain('Con contornos no se usan');
    for (const label of ['Trama cruzada', 'Bidireccional', 'Overscan automático']) expect(h).not.toContain(label);
    expect(h).toContain('Intervalo');
  });

  it('image: muestra el ángulo de barrido y el tramado ofrece los tres modos nuevos', () => {
    const h = html({ mode: 'image' });
    expect(h).toContain('Ángulo de barrido');
    for (const label of ['Semitono', 'Líneas de periódico', 'Boceto']) expect(h).toContain(label);
    expect(h).not.toContain('Tamaño de celda');
    expect(h).not.toContain('Trama cruzada');
  });

  it('image con semitono o líneas de periódico: tamaño de celda y ángulo de rejilla', () => {
    for (const mode of ['halftone', 'newsprint'] as const) {
      const h = html({ mode: 'image', image: { ...makeLayer('#f00', 'x').image, mode } });
      expect(h).toContain('Tamaño de celda');
      expect(h).toContain('Ángulo de la rejilla');
    }
    const sketch = html({ mode: 'image', image: { ...makeLayer('#f00', 'x').image, mode: 'sketch' } });
    expect(sketch).not.toContain('Tamaño de celda');
    expect(sketch).toContain('Solo graba los bordes');
  });

  it('sin eje Z en la máquina no aparecen los campos de Z', () => {
    expect(html()).not.toContain('Desplazamiento Z');
  });

  it('con Z en la capa y sin eje Z en la máquina avisa que se ignora', () => {
    const h = html({ z_offset: 1.5 });
    expect(h).toContain('no tiene eje Z motorizado');
    expect(h).toContain('Poner Z en 0');
    expect(h).not.toContain('Bajada por pasada');
  });

  it('una capa de un documento viejo (sin campos nuevos) se dibuja igual', () => {
    const viejo: Record<string, unknown> = { ...makeLayer('#00f', 'Viejo', { mode: 'fill' }) };
    for (const k of ['fill_style', 'tabs', 'z_offset', 'z_step']) delete viejo[k];
    expect(renderToString(<LayerEditor layer={viejo as never} />)).toContain('Trama cruzada');
  });
});

/** Los campos con la máquina como dato: así se prueban el modo principiante y la potencia constante. */
const GRBL: FieldsDevice = { maxRate: 25000, powerMode: 'dynamic', hasZ: false };
const fields = (patch = {}, beginner = false, device: FieldsDevice = GRBL) => {
  const layer = makeLayer('#ff0000', 'Corte', patch);
  return renderToString(<LayerFieldsView layer={layer} target={{ color: layer.color, sub: null }} beginner={beginner} device={device} />);
};
const on = (o: object) => ({ ...o, enabled: true });

describe('secciones avanzadas del editor', () => {
  it('line: "Corte avanzado" plegada con todos sus controles disponibles', () => {
    const h = fields();
    expect(h).toContain('Corte avanzado');
    expect(h).toMatch(/<details class="lyr-fold">/);
    for (const label of ['Perforación', 'Entrada al contorno', 'Salida del contorno', 'Sobrecorte', 'Pausa al empezar', 'Pausa al terminar', 'Corte pasante', 'Modo punto', 'Rampa de potencia']) {
      expect(h, label).toContain(label);
    }
    expect(h).not.toContain('Tramo cortado');
    expect(h).not.toContain('Agrupación');
    expect(h).not.toContain('Imagen avanzada');
  });

  it('line con algo en uso: la sección arranca abierta, marcada, y muestra sus campos', () => {
    const base = makeLayer('#f00', 'x');
    const h = fields({ perforation: { ...base.perforation, enabled: true, cut_mm: 2, skip_mm: 3 } });
    expect(h).toMatch(/<details class="lyr-fold" open="">/);
    expect(h).toContain('en uso');
    expect(h).toContain('Tramo cortado');
    expect(h).toContain('Patrón de 5 mm: 2 cortados y 3 saltados');
  });

  it('entrada y salida muestran forma, longitud y ángulo; con arco, el barrido', () => {
    const base = makeLayer('#f00', 'x');
    const h = fields({ lead_in: on({ ...base.lead_in }), lead_out: on({ ...base.lead_out, kind: 'arc' }) });
    expect(h).toContain('Recta');
    expect(h).toContain('Barrido del arco');
    expect(h).toContain('Longitud');
  });

  it('el modo punto deshabilita la rampa y la perforación y dice por qué', () => {
    const base = makeLayer('#f00', 'x');
    const h = fields({ dot_mode: on({ ...base.dot_mode }) });
    expect(h).toContain('no hay un trazo continuo en el que hacer la rampa');
    expect(h).toContain('No se combina con el modo punto');
    expect(h).toContain('Duración del disparo');
  });

  it('la perforación deshabilita el modo punto', () => {
    const base = makeLayer('#f00', 'x');
    expect(fields({ perforation: on({ ...base.perforation }) })).toContain('No se combina con la perforación');
  });

  it('fill: sección "Relleno" con estilo (incluye por regiones) y agrupación', () => {
    const h = fields({ mode: 'fill' });
    expect(h).toContain('Por regiones');
    expect(h).toContain('Agrupación');
    expect(h).toContain('Forma por forma');
    expect(h).not.toContain('Corte avanzado');
    expect(h).not.toContain('Imagen avanzada');
  });

  it('image: sección "Imagen avanzada" con pass-through, realce y quitar motas', () => {
    const h = fields({ mode: 'image' });
    for (const label of ['Imagen avanzada', 'Enviar sin tramar', 'Realce de bordes: radio', 'Realce de bordes: fuerza', 'Quitar motas']) expect(h, label).toContain(label);
    expect(h).not.toContain('Corte avanzado');
  });

  it('pass-through deshabilita el tramado y los ajustes de imagen con el motivo', () => {
    const base = makeLayer('#f00', 'x');
    const h = fields({ mode: 'image', image: { ...base.image, pass_through: true } });
    expect(h).toContain('se envía tal cual');
    expect(h).toMatch(/<select[^>]*disabled/);
    expect((h.match(/<input[^>]*disabled/g) ?? []).length).toBeGreaterThanOrEqual(6);
    expect(h).toMatch(/<details class="lyr-fold" open="">/);
  });

  it('el incremento de ángulo aparece en fill e image, no en line, y se bloquea con contornos', () => {
    expect(fields({ mode: 'fill' })).toContain('Incremento de ángulo');
    expect(fields({ mode: 'image' })).toContain('Incremento de ángulo');
    expect(fields()).not.toContain('Incremento de ángulo');
    const offset = fields({ mode: 'fill', fill_style: 'offset' });
    expect(offset).toContain('Incremento de ángulo');
    expect(offset).toContain('no hay ángulo de barrido que incrementar');
  });

  it('el incremento de ángulo avisa que necesita varias pasadas', () => {
    expect(fields({ mode: 'fill', angle_increment: 10, passes: 1 })).toContain('Solo se nota con 2 o más pasadas');
    expect(fields({ mode: 'fill', angle_increment: 10, passes: 2 })).not.toContain('Solo se nota con 2 o más pasadas');
  });
});

describe('potencia mínima en line (C08)', () => {
  it('se muestra con potencia dinámica (M4), explicando que baja en las esquinas', () => {
    const h = fields();
    expect(h).toContain('Potencia mínima');
    expect(h).toContain('M4');
  });

  it('no se muestra con potencia constante (M3)', () => {
    expect(fields({}, false, { ...GRBL, powerMode: 'constant' })).not.toContain('Potencia mínima');
  });

  it('en fill no se muestra; en imagen se muestra siempre', () => {
    expect(fields({ mode: 'fill' })).not.toContain('Potencia mínima');
    expect(fields({ mode: 'image' }, false, { ...GRBL, powerMode: 'constant' })).toContain('Potencia mínima');
  });
});

describe('modo principiante (C60, D18)', () => {
  it('oculta los ajustes avanzados de cada modo y deja los básicos', () => {
    const line = fields({}, true);
    for (const label of ['Corte avanzado', 'Dejar puentes sin cortar', 'Potencia mínima']) expect(line, label).not.toContain(label);
    for (const label of ['Velocidad', 'Potencia', 'Pasadas', 'Kerf']) expect(line, label).toContain(label);

    const fill = fields({ mode: 'fill' }, true);
    for (const label of ['Agrupación', 'Incremento de ángulo', 'Por regiones']) expect(fill, label).not.toContain(label);
    expect(fill).toContain('Trama cruzada');

    const image = fields({ mode: 'image' }, true);
    for (const label of ['Imagen avanzada', 'Enviar sin tramar', 'Incremento de ángulo']) expect(image, label).not.toContain(label);
    expect(image).toContain('Tramado');
    expect(image).toContain('Brillo');
  });

  it('oculta el eje Z aunque la máquina lo tenga', () => {
    expect(fields({}, false, { ...GRBL, hasZ: true })).toContain('Desplazamiento Z');
    expect(fields({}, true, { ...GRBL, hasZ: true })).not.toContain('Desplazamiento Z');
  });

  it('si la capa ya usa algo avanzado, lo avisa en lugar de ocultarlo en silencio', () => {
    const base = makeLayer('#f00', 'x');
    expect(fields({ ramp: on({ ...base.ramp }) }, true)).toContain('modo principiante oculta');
    expect(fields({}, true)).not.toContain('modo principiante oculta');
    expect(fields({ ramp: on({ ...base.ramp }) }, false)).not.toContain('modo principiante oculta');
  });

  it('el editor muestra el interruptor del modo principiante', () => {
    expect(html()).toContain('Modo principiante');
  });
});

describe('editor completo con las secciones nuevas', () => {
  it('muestra las operaciones adicionales, las posiciones y los valores por defecto', () => {
    const h = html();
    for (const label of ['Operaciones adicionales', 'Agregar operación', 'Sin operaciones adicionales', 'Valores por defecto', 'Guardar como valores por defecto para capas nuevas', 'Restablecer']) {
      expect(h, label).toContain(label);
    }
  });

  it('una capa de guías no muestra ajustes de corte', () => {
    const h = renderToString(<LayerEditor layer={makeLayer(GUIDE_COLOR, 'Guías')} />);
    expect(h).toContain('Capa de guías');
    for (const label of ['Velocidad', 'Corte avanzado', 'Operaciones adicionales', 'Valores por defecto']) expect(h, label).not.toContain(label);
  });
});

describe('lista de operaciones adicionales', () => {
  it('lista cada operación con su modo y resumen, y permite reordenar y borrar', () => {
    const parent = makeLayer('#ff0000', 'Corte', {
      sub_layers: [makeLayer('#ff0000', '', { mode: 'fill', speed: 3000, power: 30, interval: 0.2 }), makeLayer('#ff0000', '', { speed: 500, power: 80, passes: 2 })],
    });
    const h = renderToString(<SubLayers layer={parent} />);
    expect(h).toContain('Operación 1: Relleno');
    expect(h).toContain('3000 mm/min · 30 % · 0.2 mm');
    expect(h).toContain('Operación 2: Línea');
    expect(h).toContain('500 mm/min · 80 % · ×2');
    for (const label of ['Editar la operación 1', 'Subir la operación 2', 'Bajar la operación 1', 'Borrar la operación 2']) expect(h, label).toContain(label);
  });

  it('al llegar al máximo deshabilita el botón y lo explica', () => {
    const sub = makeLayer('#ff0000', '');
    const parent = makeLayer('#ff0000', 'Corte', { sub_layers: Array.from({ length: 10 }, () => sub) });
    const h = renderToString(<SubLayers layer={parent} />);
    expect(h).toContain('máximo de 10 operaciones adicionales');
    expect(h).toMatch(/<button[^>]*disabled=""[^>]*>.*Agregar operación/);
  });
});
