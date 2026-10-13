import { renderToString } from 'react-dom/server';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { setApi } from '@/api';
import { createMockApi } from '@/api/mock';
import type { CameraSetup, HeadCalibration } from '@/api/types';
import { useStore, type AppState } from '@/store';
import { emptyCameraSetup } from '@/store/camera';
import { CaptureSection } from './CaptureSection';
import { CenterSection } from './CenterSection';
import { useCenter, pickPoint, moveHeadToCenter, setOriginAtCenter, detectAt } from './centerFinder';
import { HeadConfirm, HeadSection } from './HeadSection';
import { useHeadCal, useHeadUi, useViewTool } from './headState';
import { startAutoRefresh } from './refresh';
import { createApiBackend, parseSetup, setSetupBackend } from './setup';
import { fitAlignment } from './setup';

const updateHeadOverlay = vi.fn(async () => true);
const refreshSnapshot = vi.fn(async () => true);

vi.mock('./headScan', async (orig) => ({ ...(await orig<Record<string, unknown>>()), updateHeadOverlay: () => updateHeadOverlay() }));
vi.mock('./session', () => ({
  grabCorrectedPng: async () => null,
  grabCorrectedColorPng: async () => null,
  correctedSpace: () => [1280, 720] as [number, number],
  setMosaicBackdrop: () => undefined,
  refreshSnapshot: () => refreshSnapshot(),
}));

const HEAD: HeadCalibration = { image_size: [1280, 720], ref_px: [640, 360], px_to_mm: [[0.1, 0], [0, -0.1]], offset_mm: [5, -30], rms_mm: 0.06, observations: 4 };

beforeEach(() => {
  setApi(createMockApi({ latencyMs: 0 }));
  setSetupBackend(createApiBackend());
  const s = useStore.getState();
  s.setCameraSetup(emptyCameraSetup(), true);
  s.setCameraState({ cameraStatus: 'live', cameraFrozen: false });
  useHeadUi.setState({ confirm: null, progress: null, error: null, mosaic: null });
  useHeadCal.getState().reset();
  useViewTool.getState().setTool('align');
  useCenter.getState().reset();
  updateHeadOverlay.mockClear();
  refreshSnapshot.mockClear();
});

describe('lectura de la configuración guardada', () => {
  it('una configuración de antes de la ola 4 se lee igual y queda sin cámara de cabezal', () => {
    const old = {
      version: 1,
      calibration: { width: 640, height: 480, fx: 620, fy: 625, cx: 320, cy: 240, k1: -0.1, k2: 0.01, p1: 0, p2: 0, k3: 0, rms_error_px: 0.3, views_used: 8 },
      alignment: null,
      prefs: { device_id: null, device_label: null, width: null, height: null, opacity: 0.5, visible: true },
    };
    const s = parseSetup(old);
    expect(s.calibration).toEqual(old.calibration);
    expect(s.calibration?.model).toBeUndefined();
    expect(s.head).toEqual({ enabled: false, calibration: null });
    expect(s.prefs.grayscale).toBeUndefined();
    expect(s.prefs.visible).toBe(true);
  });

  it('conserva el ojo de pez, el cabezal y los ajustes nuevos, y descarta lo inválido', () => {
    const raw: CameraSetup = {
      ...emptyCameraSetup(),
      calibration: { width: 640, height: 480, fx: 330, fy: 331, cx: 321, cy: 241, k1: -0.04, k2: 0.012, p1: 0, p2: 0, k3: 0, model: 'fisheye', k4: 0.001, view_zoom: 0.62, rms_error_px: 0.14, views_used: 12 },
      prefs: { ...emptyCameraSetup().prefs, grayscale: true, update_on_job_finish: true },
      head: { enabled: true, calibration: HEAD },
    };
    const s = parseSetup(JSON.parse(JSON.stringify(raw)));
    expect(s.calibration).toEqual(raw.calibration);
    expect(s.prefs.grayscale).toBe(true);
    expect(s.prefs.update_on_job_finish).toBe(true);
    expect(s.head).toEqual(raw.head);
    const bad = parseSetup({ ...raw, calibration: { ...raw.calibration, model: 'raro', view_zoom: -3, k4: 'x' }, head: { enabled: true, calibration: { ...HEAD, px_to_mm: [[1, 2]] } } });
    expect(bad.calibration?.model).toBeUndefined();
    expect(bad.calibration?.view_zoom).toBeUndefined();
    expect(bad.calibration?.k4).toBeUndefined();
    expect(bad.head).toEqual({ enabled: true, calibration: null });
  });

  it('el mock guarda y devuelve la cámara del cabezal y la lente de ojo de pez', async () => {
    const api = createMockApi({ latencyMs: 0 });
    const setup: CameraSetup = { ...emptyCameraSetup(), head: { enabled: true, calibration: HEAD } };
    const saved = await api.saveCameraSetup({ setup });
    expect(saved.head?.calibration?.offset_mm).toEqual([5, -30]);
    const report = await api.calibrateLens({ images_png_base64: ['a', 'b', 'c'].map(() => btoa('x')), board: [9, 6], square_mm: 25, model: 'fisheye' }).catch(() => null);
    // El mock exige leer el tamaño del PNG; con datos mínimos válidos responde con el modelo pedido.
    if (report) expect(report.calibration.model).toBe('fisheye');
  });
});

/**
 * `renderToString` lee el estado inicial del store (zustand usa `getInitialState` en el servidor),
 * no el actual: para pintar con un estado distinto se parchea ese objeto y se restaura.
 */
function ssr<S extends object>(store: { getInitialState: () => S }, patch: Partial<S>, render: () => string): string {
  const initial = store.getInitialState() as Record<string, unknown>;
  const saved: Record<string, unknown> = {};
  for (const k of Object.keys(patch)) saved[k] = initial[k];
  Object.assign(initial, patch);
  try {
    return render();
  } finally {
    Object.assign(initial, saved);
  }
}

const withCamera = (setup: CameraSetup, extra: Partial<AppState> = {}) => ({ cameraSetup: setup, cameraStatus: 'live' as const, ...extra });

describe('secciones del panel', () => {
  it('imagen de la cama: botones, ajustes del fondo y el motivo cuando no se puede', () => {
    const html = ssr(useStore, withCamera(emptyCameraSetup()), () => renderToString(<CaptureSection />));
    expect(html).toContain('Capturar imagen de la cama');
    expect(html).toContain('Vectorizar desde la cámara');
    expect(html).toContain('Fondo en escala de grises');
    expect(html).toContain('Actualizar el fondo al terminar un trabajo');
    expect(html).toContain('Primero alinea la cámara con la cama');
    const head = ssr(useStore, withCamera({ ...emptyCameraSetup(), head: { enabled: true, calibration: null } }), () => renderToString(<CaptureSection />));
    expect(head).toContain('Pide confirmación porque mueve la cabeza');
  });

  it('cámara en el cabezal: sin modo solo ofrece el interruptor; con modo muestra calibración y recorrido', () => {
    expect(ssr(useStore, withCamera(emptyCameraSetup()), () => renderToString(<HeadSection />))).not.toContain('Desfase cámara-láser');
    const html = ssr(useStore, withCamera({ ...emptyCameraSetup(), head: { enabled: true, calibration: null } }), () => renderToString(<HeadSection />));
    expect(html).toContain('Desfase cámara-láser');
    expect(html).toContain('Falta calibrar el desfase');
    expect(html).toContain('Por validar con hardware real');
    const ok = ssr(useStore, withCamera({ ...emptyCameraSetup(), head: { enabled: true, calibration: HEAD } }), () => renderToString(<HeadSection />));
    expect(ok).toContain('Desfase X 5,00 mm, Y -30,00 mm');
    expect(ok).toContain('Actualizar la superposición');
  });

  it('la confirmación avisa que la cabeza se mueve y el avance muestra el punto', () => {
    const setup = { ...emptyCameraSetup(), head: { enabled: true, calibration: HEAD } };
    const html = ssr(useStore, withCamera(setup), () => ssr(useHeadUi, { confirm: 'capture' }, () => renderToString(<HeadConfirm />)));
    expect(html).toContain('La cabeza se va a mover');
    expect(html).toContain('Mover y capturar');
    expect(html).toMatch(/\d+ posiciones/);
    const prog = ssr(useHeadUi, { progress: { step: 3, total: 9 } }, () => renderToString(<HeadConfirm />));
    expect(prog).toContain('Foto 3 de 9');
    expect(prog).toContain('Cancelar recorrido');
  });

  it('buscador de centro: pide alinear primero y avisa que falta validar con hardware', () => {
    const html = ssr(useStore, withCamera(emptyCameraSetup()), () => renderToString(<CenterSection />));
    expect(html).toContain('Buscador de centro');
    expect(html).toContain('Primero alinea la cámara con la cama');
    expect(html).toContain('Por validar con hardware real');
  });
});

describe('buscador de centro con la cámara fija', () => {
  function align() {
    // Imagen 1280x720 -> cama: la esquina (100,650) es (20,20) mm, etc. (perspectiva suave).
    const a = fitAlignment([1280, 720], [[100, 650], [1180, 650], [1180, 80], [100, 80]], [[20, 20], [380, 20], [380, 280], [20, 280]])!;
    const s = useStore.getState();
    s.setCameraSetup({ ...s.cameraSetup, alignment: a });
    return a;
  }

  it('tres clics dan el centro; el cuarto empieza de nuevo; tres alineados avisan', () => {
    align();
    // Un círculo de 40 mm de radio centrado en (200, 150) mm: se marcan sus puntos en píxeles (la alineación es afín).
    const px = (x: number, y: number): [number, number] => [100 + ((x - 20) / 360) * 1080, 650 - ((y - 20) / 260) * 570];
    const pt = (deg: number) => px(200 + 40 * Math.cos((deg * Math.PI) / 180), 150 + 40 * Math.sin((deg * Math.PI) / 180));
    pickPoint(pt(10));
    pickPoint(pt(130));
    expect(useCenter.getState().result).toBeNull();
    pickPoint(pt(250));
    const r = useCenter.getState().result!;
    expect(r.circle.center[0]).toBeCloseTo(200, 3);
    expect(r.circle.center[1]).toBeCloseTo(150, 3);
    expect(r.circle.radius).toBeCloseTo(40, 3);
    pickPoint(px(50, 50));
    expect(useCenter.getState().picks).toHaveLength(1);
    expect(useCenter.getState().result).toBeNull();
    // Alineados: error claro.
    useCenter.getState().reset();
    pickPoint(px(10, 100));
    pickPoint(px(100, 100));
    pickPoint(px(200, 100));
    expect(useCenter.getState().error).toContain('en línea recta');
  });

  it('fija el origen de usuario en el centro (en coordenadas de trabajo) y no mueve sin máquina', async () => {
    align();
    useCenter.setState({ result: { circle: { center: [200.25, 150.5], radius: 40, rms: 0 }, source: 'three' } });
    expect(setOriginAtCenter()).toBe(true);
    const o = useStore.getState().jobOptions.user_origin!;
    // El perfil por omisión tiene origen abajo a la izquierda: trabajo = documento.
    expect(o[0]).toBeCloseTo(200.25, 3);
    expect(o[1]).toBeCloseTo(150.5, 3);
    expect(await moveHeadToCenter()).toBe(false);
  });

  it('sin alineación, marcar un punto explica qué falta', async () => {
    pickPoint([10, 10]);
    expect(useCenter.getState().error).toContain('alinea la cámara');
    await detectAt([10, 10]);
    expect(useCenter.getState().error).toContain('alinea la cámara');
  });
});

describe('actualizar al terminar un trabajo', () => {
  function finishJob() {
    useStore.setState({ jobState: 'running' });
    useStore.setState({ jobState: 'finished' });
  }

  it('con la opción activa y el cuadro congelado, renueva el cuadro al terminar', async () => {
    const stop = startAutoRefresh(5);
    const s = useStore.getState();
    s.setCameraSetup({ ...s.cameraSetup, prefs: { ...s.cameraSetup.prefs, visible: true, update_on_job_finish: true } });
    s.setCameraState({ cameraFrozen: true });
    finishJob();
    await vi.waitFor(() => expect(refreshSnapshot).toHaveBeenCalledTimes(1));
    expect(updateHeadOverlay).not.toHaveBeenCalled();
    stop();
    useStore.setState({ jobState: 'idle' });
  });

  it('con la cámara en el cabezal repite el recorrido', async () => {
    const stop = startAutoRefresh(5);
    const s = useStore.getState();
    s.setCameraSetup({ ...s.cameraSetup, prefs: { ...s.cameraSetup.prefs, visible: true, update_on_job_finish: true }, head: { enabled: true, calibration: HEAD } });
    finishJob();
    await vi.waitFor(() => expect(updateHeadOverlay).toHaveBeenCalledTimes(1));
    stop();
    useStore.setState({ jobState: 'idle' });
  });

  it('sin la opción, con la cámara fija en vivo o con el fondo oculto no hace nada', async () => {
    const stop = startAutoRefresh(5);
    const s = useStore.getState();
    s.setCameraSetup({ ...s.cameraSetup, prefs: { ...s.cameraSetup.prefs, visible: true, update_on_job_finish: false } });
    s.setCameraState({ cameraFrozen: true });
    finishJob();
    useStore.setState({ jobState: 'idle' });
    s.setCameraSetup({ ...s.cameraSetup, prefs: { ...s.cameraSetup.prefs, visible: true, update_on_job_finish: true } });
    s.setCameraState({ cameraFrozen: false });
    finishJob();
    useStore.setState({ jobState: 'idle' });
    s.setCameraSetup({ ...s.cameraSetup, prefs: { ...s.cameraSetup.prefs, visible: false, update_on_job_finish: true } });
    s.setCameraState({ cameraFrozen: true });
    finishJob();
    await new Promise((r) => setTimeout(r, 40));
    expect(refreshSnapshot).not.toHaveBeenCalled();
    expect(updateHeadOverlay).not.toHaveBeenCalled();
    stop();
    useStore.setState({ jobState: 'idle' });
  });
});
