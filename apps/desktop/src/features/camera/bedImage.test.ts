import { beforeEach, describe, expect, it, vi } from 'vitest';
import { setApi } from '@/api';
import { createMockApi } from '@/api/mock';
import type { LaserApi } from '@/api';
import { commands } from '@/commands';
import { selectActiveDevice, useStore } from '@/store';
import { emptyCameraSetup } from '@/store/camera';
import { useTraceDialog } from '@/features/trace/traceStore';
import { captureBedImage, captureBlockedReason, overlayBlockedReason, requestAction, vectorizeFromCamera } from './bedImage';
import { fitAlignment } from './setup';
import { useHeadUi } from './headState';

let grabbed: { png: string; width: number; height: number } | null = null;
let space: [number, number] | null = [1280, 720];

vi.mock('./session', () => ({
  grabCorrectedPng: async () => grabbed,
  grabCorrectedColorPng: async () => grabbed,
  correctedSpace: () => space,
  setMosaicBackdrop: () => undefined,
  refreshSnapshot: async () => false,
}));

/** PNG con la cabecera IHDR al tamaño pedido (el mock solo la lee). */
function fakePng(w: number, h: number): string {
  const bytes = new Uint8Array(33);
  bytes.set([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0, 0, 0, 13, 0x49, 0x48, 0x44, 0x52]);
  new DataView(bytes.buffer).setUint32(16, w);
  new DataView(bytes.buffer).setUint32(20, h);
  return btoa(String.fromCharCode(...bytes));
}

const calls: Parameters<LaserApi['composeBedImage']>[0][] = [];

beforeEach(() => {
  const base = createMockApi({ latencyMs: 0 });
  const api: LaserApi = Object.assign(Object.create(base) as LaserApi, {
    composeBedImage: (args: Parameters<LaserApi['composeBedImage']>[0]) => {
      calls.push(args);
      return base.composeBedImage(args);
    },
  });
  calls.length = 0;
  setApi(api);
  const s = useStore.getState();
  s.setCameraSetup(emptyCameraSetup(), true);
  s.replaceDocument({ version: 2, objects: [], layers: [], device_id: null }, null);
  s.setCameraState({ cameraStatus: 'live', cameraFrozen: false, cameraStreamSize: [1280, 720] });
  useHeadUi.setState({ confirm: null, progress: null, error: null, cancelRequested: false });
  useTraceDialog.getState().close();
  grabbed = { png: fakePng(1280, 720), width: 1280, height: 720 };
  space = [1280, 720];
});

/** Alineación manual: la imagen de 1280x720 cubre toda una cama de 400x300 mm (con un margen). */
function align() {
  const a = fitAlignment([1280, 720], [[100, 650], [1180, 650], [1180, 80], [100, 80]], [[20, 20], [380, 20], [380, 280], [20, 280]])!;
  expect(a).not.toBeNull();
  const s = useStore.getState();
  s.setCameraSetup({ ...s.cameraSetup, alignment: a });
}

describe('capturar la imagen de la cama', () => {
  it('guarda la foto alineada como imagen del documento, donde está en la cama', async () => {
    align();
    const id = await captureBedImage();
    expect(id).toBeTruthy();
    const s = useStore.getState();
    const obj = s.doc.objects.find((o) => o.id === id)!;
    expect(obj.kind.type).toBe('image');
    if (obj.kind.type !== 'image') return;
    const call = calls[0]!;
    // La foto viaja con la homografía de la alineación (píxel -> mm) y a la resolución de la foto.
    expect(call.tiles).toHaveLength(1);
    expect(call.save).toBe(true);
    const m = call.tiles[0]!.to_bed;
    // Los 4 puntos marcados caen en sus mm.
    const apply = (x: number, y: number) => {
      const w = m[2][0] * x + m[2][1] * y + m[2][2];
      return [(m[0][0] * x + m[0][1] * y + m[0][2]) / w, (m[1][0] * x + m[1][1] * y + m[1][2]) / w];
    };
    expect(apply(100, 650)[0]).toBeCloseTo(20, 6);
    expect(apply(1180, 80)[1]).toBeCloseTo(280, 6);
    // La imagen del documento queda en la esquina inferior izquierda de lo que ve la cámara.
    // La imagen cuelga de lo que ve la cámara dentro de la cama (puede asomar menos de 1 píxel de salida por abajo).
    expect(obj.transform[4]).toBeGreaterThanOrEqual(-1);
    expect(obj.transform[5]).toBeGreaterThanOrEqual(-1);
    const bed = selectActiveDevice(s).bed;
    expect(obj.transform[4] + obj.kind.size_mm[0]).toBeLessThanOrEqual(bed[0] + 1);
    expect(obj.transform[5] + obj.kind.size_mm[1]).toBeLessThanOrEqual(bed[1] + 1);
    expect(obj.kind.size_mm[0]).toBeGreaterThan(300);
    expect(obj.kind.source_path).toContain('camara-1.png');
    // Queda seleccionada, en la capa de imágenes, y deshacer la quita en un solo paso.
    expect(s.selectedIds).toEqual([id]);
    expect(s.doc.layers.some((l) => l.mode === 'image' && l.color === obj.layer)).toBe(true);
    s.undo();
    expect(useStore.getState().doc.objects).toHaveLength(0);
  });

  it('con el cuadro reducido (1280 de ancho) la matriz se reescala al tamaño de la foto', async () => {
    align();
    space = [1920, 1080];
    // La alineación está en 1280x720: no coincide con el espacio actual.
    expect(await captureBedImage()).toBeNull();
    expect(useStore.getState().doc.objects).toHaveLength(0);
    // Con la alineación hecha en 1920x1080 y una foto de 1280x720, la matriz se ajusta a la foto.
    const a = fitAlignment([1920, 1080], [[150, 975], [1770, 975], [1770, 120], [150, 120]], [[20, 20], [380, 20], [380, 280], [20, 280]])!;
    const s = useStore.getState();
    s.setCameraSetup({ ...s.cameraSetup, alignment: a });
    expect(await captureBedImage()).toBeTruthy();
    const m = calls.at(-1)!.tiles[0]!.to_bed;
    const w = m[2][0] * 100 + m[2][1] * 650 + m[2][2];
    // El píxel (150,975) del espacio de 1920 es el (100,650) de la foto de 1280.
    expect((m[0][0] * 100 + m[0][1] * 650 + m[0][2]) / w).toBeCloseTo(20, 6);
  });

  it('en gris pide la imagen en gris y sin alineación explica qué falta', async () => {
    const s = useStore.getState();
    expect(captureBlockedReason(s)).toBe('camera.mapping.noAlignment');
    expect(await captureBedImage()).toBeNull();
    expect(useStore.getState().notices.at(-1)?.level).toBe('error');
    align();
    expect(captureBlockedReason(useStore.getState())).toBeNull();
    useStore.getState().setCameraPrefs({ grayscale: true });
    await captureBedImage();
    expect(calls.at(-1)!.grayscale).toBe(true);
    useStore.getState().setCameraState({ cameraStatus: 'off' });
    expect(captureBlockedReason(useStore.getState())).toBe('camera.reason.notLive');
  });

  it('un proyecto guardado recibe el PNG en su carpeta', async () => {
    align();
    const s = useStore.getState();
    s.replaceDocument({ version: 2, objects: [], layers: [], device_id: null }, '/trabajos/mesa/proyecto.tlaku');
    await captureBedImage();
    expect(calls.at(-1)!.dir).toBe('/trabajos/mesa');
    expect(calls.at(-1)!.stem).toBe('proyecto');
  });
});

describe('vectorizar desde la cámara', () => {
  it('captura y abre el diálogo de vectorizar sobre la imagen nueva', async () => {
    align();
    expect(await vectorizeFromCamera()).toBe(true);
    const id = useTraceDialog.getState().imageId;
    expect(id).toBeTruthy();
    expect(useStore.getState().doc.objects.find((o) => o.id === id)?.kind.type).toBe('image');
  });

  it('si la captura falla no abre el diálogo', async () => {
    expect(await vectorizeFromCamera()).toBe(false);
    expect(useTraceDialog.getState().imageId).toBeNull();
  });
});

describe('comandos de cámara', () => {
  it('con la cámara fija el comando captura de una vez; con el cabezal pide confirmación', async () => {
    align();
    await commands.run('camera.capture');
    await vi.waitFor(() => expect(useStore.getState().doc.objects).toHaveLength(1));
    expect(useHeadUi.getState().confirm).toBeNull();
    // Cámara en el cabezal: no se mueve nada hasta confirmar.
    const s = useStore.getState();
    s.setCameraSetup({ ...s.cameraSetup, head: { enabled: true, calibration: null } });
    requestAction('capture');
    expect(useHeadUi.getState().confirm).toBe('capture');
    expect(useStore.getState().dockTab).toBe('camera');
    expect(calls).toHaveLength(1);
  });

  it('actualizar la superposición solo tiene sentido con el cuadro congelado o con el cabezal', () => {
    const s = useStore.getState();
    expect(overlayBlockedReason(s)).toBe('camera.reason.overlayLive');
    s.setCameraState({ cameraFrozen: true });
    expect(overlayBlockedReason(useStore.getState())).toBeNull();
    expect(commands.availability('camera.updateOverlay', useStore.getState())).toBe(true);
    useStore.getState().setCameraState({ cameraFrozen: false });
    expect(commands.availability('camera.vectorize', useStore.getState())).toBe('camera.mapping.noAlignment');
  });
});
