import { renderToString } from 'react-dom/server';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { setApi } from '@/api';
import { createMockApi } from '@/api/mock';
import type { LaserApi } from '@/api';
import { commands } from '@/commands';
import { selectActiveDevice, useStore } from '@/store';
import { emptyCameraSetup } from '@/store/camera';
import { persistCameraSetup } from './actions';
import { acceptMarks, captureAndDetect, openMarksAsNewDocument } from './marks';
import { Photo, Result } from './MarksDialog';
import { useMarks } from './marksState';
import { createApiBackend, setSetupBackend } from './setup';

let grabbed: { png: string; width: number; height: number } | null = null;
let space: [number, number] | null = [1280, 720];

// La sesión de cámara usa canvas y video: aquí solo hace falta lo que consume `marks.ts`.
vi.mock('./session', () => ({
  grabCorrectedPng: async () => grabbed,
  correctedSpace: () => space,
  startCamera: async () => undefined,
  getCorrectedCanvas: () => ({ width: 0, height: 0 }),
}));

/** PNG con la cabecera IHDR al tamaño pedido (el mock solo la lee). */
function fakePng(w: number, h: number): string {
  const bytes = new Uint8Array(33);
  bytes.set([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0, 0, 0, 13, 0x49, 0x48, 0x44, 0x52]);
  new DataView(bytes.buffer).setUint32(16, w);
  new DataView(bytes.buffer).setUint32(20, h);
  return btoa(String.fromCharCode(...bytes));
}

const calls: Parameters<LaserApi['detectCameraMarks']>[0][] = [];

beforeEach(() => {
  const base = createMockApi({ latencyMs: 0 });
  const api: LaserApi = Object.assign(Object.create(base) as LaserApi, {
    detectCameraMarks: (args: Parameters<LaserApi['detectCameraMarks']>[0]) => {
      calls.push(args);
      return base.detectCameraMarks(args);
    },
  });
  calls.length = 0;
  setApi(api);
  setSetupBackend(createApiBackend(() => api));
  useStore.getState().setCameraSetup(emptyCameraSetup(), true);
  grabbed = { png: fakePng(1280, 720), width: 1280, height: 720 };
  space = [1280, 720];
  useMarks.setState({ open: false, count: 9 });
});

describe('capturar y detectar', () => {
  it('manda la foto corregida, la cama y la alineación provisional si existe', async () => {
    const first = await captureAndDetect(9);
    expect(first.size).toEqual([1280, 720]);
    expect(first.report.used).toBe(9);
    expect(calls[0]!.count).toBe(9);
    expect(calls[0]!.bed).toEqual(selectActiveDevice(useStore.getState()).bed);
    expect(calls[0]!.prior_to_bed).toBeNull();
    // Con una alineación guardada, esa homografía viaja como alineación provisional.
    expect(acceptMarks(first.report, first.size, 0)).toBeNull();
    await persistCameraSetup();
    await captureAndDetect(4);
    expect(calls[1]!.count).toBe(4);
    expect(calls[1]!.prior_to_bed).toEqual(useStore.getState().cameraSetup.alignment!.to_bed);
  });

  it('si la foto está reducida, pasa el reporte y la alineación provisional al espacio de la alineación', async () => {
    // Calibración de 1920x1080 mostrada en un cuadro de 1280x720.
    space = [1920, 1080];
    const small = await captureAndDetect(9);
    expect(small.size).toEqual([1920, 1080]);
    expect(acceptMarks(small.report, small.size, 0)).toBeNull();
    const aligned = useStore.getState().cameraSetup.alignment!;
    expect(aligned.image_size).toEqual([1920, 1080]);
    await persistCameraSetup();
    // La alineación guardada (px de 1920) se manda en px de la foto de 1280: escala k = 1.5.
    await captureAndDetect(9);
    const sent = calls.at(-1)!.prior_to_bed!;
    const saved = useStore.getState().cameraSetup.alignment!.to_bed;
    expect(sent[0]![0]).toBeCloseTo(saved[0]![0] * 1.5, 12);
    expect(sent[1]![2]).toBe(saved[1]![2]);
  });

  it('sin imagen de la cámara, explica qué falta', async () => {
    grabbed = null;
    await expect(captureAndDetect(9)).rejects.toThrow(/imagen de la cámara/);
  });
});

describe('aceptar la alineación', () => {
  it('guarda las marcas a una altura y, al sumar una segunda, el motor entrega el modelo de altura', async () => {
    const { report, size } = await captureAndDetect(9);
    expect(acceptMarks(report, size, 0)).toBeNull();
    await persistCameraSetup();
    let a = useStore.getState().cameraSetup.alignment!;
    expect(a.method).toBe('marks');
    expect(a.samples).toHaveLength(1);
    expect(a.image_size).toEqual([1280, 720]);

    expect(acceptMarks(report, size, 20)).toBeNull();
    await persistCameraSetup();
    a = useStore.getState().cameraSetup.alignment!;
    expect(a.samples!.map((s) => s.height_mm)).toEqual([0, 20]);
    expect(a.height?.source).toBe('measured');
    expect(a.reference_height_mm).toBe(0);
  });

  it('rechaza alturas demasiado cercanas, imágenes de otro tamaño y pocas marcas', async () => {
    const { report, size } = await captureAndDetect(9);
    expect(acceptMarks(report, size, 0)).toBeNull();
    expect(acceptMarks(report, size, 1)).toBe('tooClose');
    space = [640, 480];
    expect(acceptMarks(report, size, 10)).toBe('noSpace');
    space = [1280, 720];
    const few = { ...report, marks: report.marks.map((m, i) => (i < 6 ? { ...m, used: false } : m)) };
    expect(acceptMarks(few, size, 10)).toBe('tooFew');
    await persistCameraSetup();
  });
});

describe('documento de marcas', () => {
  it('se abre como documento nuevo con las cruces de la cama activa', async () => {
    expect(await openMarksAsNewDocument(9)).toBe(true);
    const doc = useStore.getState().doc;
    expect(doc.objects).toHaveLength(9);
    expect(doc.layers).toHaveLength(1);
    expect(doc.layers[0]!.mode).toBe('line');
    expect(await openMarksAsNewDocument(4)).toBe(true);
    expect(useStore.getState().doc.objects).toHaveLength(4);
  });

  it('el comando del menú abre el asistente', async () => {
    expect(useMarks.getState().open).toBe(false);
    await commands.run('camera.alignMarks');
    expect(useMarks.getState().open).toBe(true);
  });
});

describe('vista de la detección', () => {
  it('dibuja una marca por cada resultado, con su error, y las que faltan en punteado', async () => {
    const { report, size, png } = await captureAndDetect(9);
    report.marks[4]!.image = null;
    report.marks[4]!.used = false;
    report.marks[5]!.used = false;
    report.marks[6]!.residual_mm = 0.9;
    const html = renderToString(<Photo shot={{ report, png, size }} />);
    expect(html.match(/data-tone="ok"/g)).toHaveLength(6);
    expect(html.match(/data-tone="warn"/g)).toHaveLength(1);
    expect(html.match(/data-tone="bad"/g)).toHaveLength(1);
    expect(html.match(/data-tone="missing"/g)).toHaveLength(1);
    expect(html).toContain('0,90');
  });

  it('el resultado muestra la validación cruzada con 9 marcas y avisa que con 4 no se puede medir', async () => {
    const nine = (await captureAndDetect(9)).report;
    const html9 = renderToString(<Result report={nine} onAccept={() => undefined} />);
    expect(html9).toContain('dejando una marca fuera');
    expect(html9).not.toContain('disabled=""');
    const four = (await captureAndDetect(4)).report;
    const html4 = renderToString(<Result report={four} onAccept={() => undefined} />);
    expect(html4).toContain('Usa 9 marcas');
    const none = { ...nine, used: 3 };
    expect(renderToString(<Result report={none} onAccept={() => undefined} />)).toContain('disabled=""');
  });
});
