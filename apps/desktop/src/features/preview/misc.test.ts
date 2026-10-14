import { describe, expect, it } from 'vitest';
import { commands } from '@/commands';
import { createAppStore } from '@/store';
import { errorCode } from '@/lib/errors';
import { cleanErrorMessage, previewErrorHint } from './errors';
import { fmtDistance, fmtSeconds } from './format';

describe('errores legibles', () => {
  it('reconoce por código los trabajos que se salen de la cama y los vacíos', () => {
    expect(previewErrorHint(errorCode('[bed_overflow] la capa 1 se sale del área'))).toBe('preview.error.hintBed');
    expect(previewErrorHint(errorCode('[nothing_to_send] nothing to produce: the design is empty'))).toBe('preview.error.hintEmpty');
    expect(previewErrorHint(errorCode('[bed_overflow] layer 1 goes outside the bed'))).toBe('preview.error.hintBed');
    expect(previewErrorHint(errorCode('fallo raro'))).toBeNull();
    // El texto ya no cuenta: sin código no hay pista, en ningún idioma.
    expect(previewErrorHint(errorCode('se sale de la cama'))).toBeNull();
    expect(previewErrorHint(null)).toBeNull();
  });

  it('quita el prefijo técnico "Error:"', () => {
    expect(cleanErrorMessage('Error: algo falló ')).toBe('algo falló');
    expect(cleanErrorMessage(null)).toBe('');
  });
});

describe('formato', () => {
  it('distancias en mm o m y tiempos cortos con decimales', () => {
    expect(fmtDistance(42)).toMatch(/mm/);
    expect(fmtDistance(12345)).toMatch(/m$/);
    expect(fmtSeconds(3.44)).toMatch(/s$/);
    expect(fmtSeconds(125)).toBe('2:05');
  });
});

describe('comandos de la vista previa', () => {
  it('K, coma y punto solo actúan con la vista previa abierta y calculada', () => {
    const store = createAppStore();
    const s = store.getState();
    for (const id of ['preview.play', 'preview.slower', 'preview.faster']) {
      expect(commands.availability(id, s)).toBe('preview.reason.unavailable');
    }
    store.setState({
      previewVisible: true,
      preview: { layers: [], moves: [0, 1, 1, 0], move_times: [1], total_s: 1, bounds: [[0, 0], [1, 1]], order_note: null, warnings: [] },
    });
    for (const id of ['preview.play', 'preview.slower', 'preview.faster']) {
      expect(commands.availability(id, store.getState())).toBe(true);
    }
  });

  it('no usa Espacio (paneo del lienzo) y respeta una tecla, un significado', () => {
    expect(commands.match('Space')).toBeUndefined();
    expect(commands.match('K')?.id).toBe('preview.play');
    expect(commands.match(',')?.id).toBe('preview.slower');
    expect(commands.match('.')?.id).toBe('preview.faster');
  });
});
