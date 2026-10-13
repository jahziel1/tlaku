import { renderToString } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import { FALLBACK_DEVICE } from '@/lib/device';
import { ExtraAxesFields } from './ExtraAxesFields';
import { FrameOptions } from './FrameOptions';
import { MacroEditor } from './MacroEditor';
import { MoveWindowExtras } from './MoveWindowExtras';

const noop = () => {};


describe('ventana Mover completa', () => {
  it('sin Z ni U solo hay posiciones guardadas y disparo de prueba', () => {
    const h = renderToString(<MoveWindowExtras />);
    expect(h).not.toContain('Eje Z');
    expect(h).toContain('Posiciones guardadas');
    expect(h).toContain('Disparo de prueba');
  });
});

describe('editor de macros y opciones del perfil', () => {
  it('muestra el aviso de que pedirá confirmación y el error de una macro inválida', () => {
    const draft = { ...FALLBACK_DEVICE, macros: [{ name: 'Home', gcode: 'G0 X0 Y0', confirm: false }, { name: '', gcode: '', confirm: true }] };
    const h = renderToString(<MacroEditor draft={draft} edit={noop} />);
    expect(h).toContain('siempre pedirá confirmación');
    expect(h).toContain('La macro necesita un nombre.');
    expect(h).toContain('Agregar macro');
  });

  it('casillas de eje U y segundo láser', () => {
    const h = renderToString(<ExtraAxesFields draft={FALLBACK_DEVICE} edit={noop} />);
    expect(h).toContain('eje U');
    expect(h).toContain('dos láseres');
  });

  it('el encuadre con el láser apagado solo ofrece las casillas', () => {
    const off = renderToString(<FrameOptions />);
    expect(off).toContain('Repetir el encuadre');
    expect(off).toContain('láser encendido a baja potencia');
    expect(off).not.toContain('Potencia del encuadre');
  });
});
