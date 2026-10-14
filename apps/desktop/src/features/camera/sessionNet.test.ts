import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setApi } from '@/api';
import { createMockApi, type MockApi } from '@/api/mock';
import { useStore } from '@/store';
import { emptyCameraSetup } from '@/store/camera';
import { createApiBackend, setSetupBackend } from './setup';
import { getCorrectedCanvas, getRawView, parkRawView, startCamera, stopCamera, grabRawPng } from './session';

/** Un canvas de mentira: sin WebGL (la sesión cae en la copia 2D) y con un contexto 2D que no dibuja. */
function fakeCanvas() {
  const ctx = { drawImage: vi.fn(), getImageData: vi.fn(), putImageData: vi.fn() };
  return {
    width: 0,
    height: 0,
    ctx,
    getContext: (kind: string) => (kind === '2d' ? ctx : null),
    toBlob: (cb: (b: Blob | null) => void) => cb(new Blob(['x'], { type: 'image/png' })),
    remove: vi.fn(),
  };
}

let api: MockApi;
let created: ReturnType<typeof fakeCanvas>[] = [];

beforeEach(() => {
  created = [];
  vi.stubGlobal('document', {
    createElement: (tag: string) => {
      if (tag !== 'canvas') return { style: {}, setAttribute: () => undefined, appendChild: () => undefined };
      const c = fakeCanvas();
      created.push(c);
      return c;
    },
    body: { appendChild: () => undefined },
  });
  vi.stubGlobal('cancelAnimationFrame', () => undefined);
  vi.stubGlobal(
    'FileReader',
    class {
      result: string | null = null;
      onload: (() => void) | null = null;
      readAsDataURL() {
        this.result = 'data:image/png;base64,AAAA';
        queueMicrotask(() => this.onload?.());
      }
    },
  );
  vi.stubGlobal('createImageBitmap', async () => ({ width: 640, height: 480, close: () => undefined }));
  api = createMockApi({ latencyMs: 0 });
  setApi(api);
  setSetupBackend(createApiBackend(() => api));
  const s = useStore.getState();
  s.setCameraSetup({ ...emptyCameraSetup(), prefs: { ...emptyCameraSetup().prefs, source: 'network', network_url: 'http://ana:clave@192.168.1.20/snap.jpg' } }, true);
  s.setCameraState({ cameraStatus: 'off', cameraProblem: null, cameraProblemDetail: null, cameraStreamSize: null, cameraFrozen: false });
});

afterEach(() => {
  stopCamera();
  vi.unstubAllGlobals();
});

describe('cámara de red en la sesión', () => {
  it('enciende con la URL guardada, pide cuadros al motor y los usa como imagen de la cámara', async () => {
    const fetchFrame = vi.fn(async () => ({ mime: 'image/jpeg', data_base64: 'AAAA' }));
    api.fetchCameraFrame = fetchFrame;
    await startCamera();
    await vi.waitFor(() => expect(useStore.getState().cameraStatus).toBe('live'));
    // El motor recibe la URL tal cual (con sus credenciales); la interfaz nunca la pide por su cuenta.
    expect(fetchFrame).toHaveBeenCalledWith({ url: 'http://ana:clave@192.168.1.20/snap.jpg' });
    expect(useStore.getState().cameraStreamSize).toEqual([640, 480]);
    // El resultado pasa por el corrector como cualquier cámara: ya hay imagen corregida.
    expect(getCorrectedCanvas().width).toBe(640);
    expect(getCorrectedCanvas().height).toBe(480);
    // La vista cruda del asistente de calibración es el canvas de los cuadros, no el video.
    const raw = getRawView() as unknown as ReturnType<typeof fakeCanvas>;
    expect(raw.width).toBe(640);
    parkRawView();
    expect(raw.remove).toHaveBeenCalled();
    // También se puede tomar un cuadro crudo para detectar el tablero.
    const frame = await grabRawPng(320);
    expect(frame).toMatchObject({ scale: 0.5, width: 640, height: 480 });

    stopCamera();
    expect(useStore.getState().cameraStatus).toBe('off');
    const calls = fetchFrame.mock.calls.length;
    await new Promise((r) => setTimeout(r, 300));
    expect(fetchFrame.mock.calls.length).toBeLessThanOrEqual(calls + 1); // a lo sumo el que iba en vuelo
  });

  it('si el motor rechaza la cámara, la sesión queda apagada y muestra el motivo', async () => {
    api.fetchCameraFrame = async () => {
      throw new Error('«8.8.8.8» no está en la red local: la cámara debe ser de tu red');
    };
    await startCamera();
    await vi.waitFor(() => expect(useStore.getState().cameraProblem).toBe('network'));
    const s = useStore.getState();
    expect(s.cameraStatus).toBe('off');
    expect(s.cameraProblemDetail).toContain('no está en la red local');
  });

  it('sin dirección o con una dirección mala no pide nada', async () => {
    const fetchFrame = vi.fn(async () => ({ mime: 'image/jpeg', data_base64: 'AAAA' }));
    api.fetchCameraFrame = fetchFrame;
    const s = useStore.getState();
    s.setCameraSetup({ ...s.cameraSetup, prefs: { ...s.cameraSetup.prefs, network_url: null } }, true);
    await startCamera();
    expect(useStore.getState().cameraProblem).toBe('noUrl');
    s.setCameraSetup({ ...s.cameraSetup, prefs: { ...s.cameraSetup.prefs, network_url: 'https://192.168.1.2/x' } }, true);
    await startCamera();
    expect(useStore.getState().cameraProblem).toBe('noUrl');
    expect(fetchFrame).not.toHaveBeenCalled();
  });

  it('apagar y volver a encender no deja dos bucles', async () => {
    const fetchFrame = vi.fn(async () => ({ mime: 'image/jpeg', data_base64: 'AAAA' }));
    api.fetchCameraFrame = fetchFrame;
    await startCamera();
    await startCamera();
    await vi.waitFor(() => expect(useStore.getState().cameraStatus).toBe('live'));
    stopCamera();
    await new Promise((r) => setTimeout(r, 50));
    const n = fetchFrame.mock.calls.length;
    await new Promise((r) => setTimeout(r, 400));
    expect(fetchFrame.mock.calls.length).toBeLessThanOrEqual(n + 1);
  });
});
