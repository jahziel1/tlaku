import { renderToString } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import { FALLBACK_DEVICE } from '@/lib/device';
import { RotaryFields, ScanOffsetsTable, ZAxisField } from './DeviceExtras';

const noop = () => {};

describe('campos de máquina de la Fase 4', () => {
  it('casilla del eje Z motorizado', () => {
    expect(renderToString(<ZAxisField draft={FALLBACK_DEVICE} edit={noop} />)).toContain('Eje Z motorizado');
  });

  it('rotativo apagado: solo la casilla', () => {
    const h = renderToString(<RotaryFields draft={FALLBACK_DEVICE} edit={noop} issues={{}} />);
    expect(h).toContain('Usar un rotativo');
    expect(h).not.toContain('mm por vuelta');
  });

  it('rotativo con rodillos: mm por vuelta, diámetro del rodillo y de la pieza, con la ayuda de medición', () => {
    const draft = { ...FALLBACK_DEVICE, rotary: { ...FALLBACK_DEVICE.rotary, enabled: true } };
    const h = renderToString(<RotaryFields draft={draft} edit={noop} issues={{}} />);
    for (const s of ['Rodillos', 'Mandril', 'mm por vuelta', 'Diámetro del rodillo', 'Diámetro de la pieza', 'Y=360']) expect(h).toContain(s);
  });

  it('rotativo con mandril: sin diámetro del rodillo; los errores se muestran', () => {
    const draft = { ...FALLBACK_DEVICE, rotary: { ...FALLBACK_DEVICE.rotary, enabled: true, kind: 'chuck' as const } };
    const h = renderToString(<RotaryFields draft={draft} edit={noop} issues={{ object_diameter: 'positive' }} />);
    expect(h).not.toContain('Diámetro del rodillo');
    expect(h).toContain('Debe ser un número mayor que cero.');
  });

  it('tabla de barrido: vacía, con filas y con velocidad repetida', () => {
    expect(renderToString(<ScanOffsetsTable rows={[]} onChange={noop} />)).toContain('Sin corrección');
    const rows = [{ speed: 3000, offset: 0.1 }, { speed: 3000, offset: 0.2 }];
    const h = renderToString(<ScanOffsetsTable rows={rows} onChange={noop} error="duplicate" />);
    expect(h).toContain('Agregar fila');
    expect(h).toContain('Quitar la fila 2');
    expect(h).toContain('No puede haber dos filas con la misma velocidad.');
    expect(h).toContain('graba una prueba bidireccional');
  });
});
