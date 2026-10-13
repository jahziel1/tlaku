import { describe, expect, it, vi } from 'vitest';
import { setApi } from '@/api';
import { createMockApi, type MockApi } from '@/api/mock';
import type { DeviceProfile, Point } from '@/api/types';
import { docToWork, FALLBACK_DEVICE, workToDoc } from '@/lib/device';
import { bindMachineEvents, createAppStore } from '@/store';
import { selectHeadDoc } from '@/store/machine';
import { continuousDelta, stepDelta } from './jogMath';
import { computeJobPlacement } from './placement';

/**
 * Convención de coordenadas: `user_origin`, la posición del cabezal y "ir al origen" están en
 * coordenadas de la MÁQUINA (las que muestra GRBL); solo la interfaz las pasa al documento
 * (origen abajo a la izquierda, Y hacia arriba) con `workToDoc`, según `DeviceProfile.origin`.
 */
const BED: Point = [400, 300];
const PROFILES = {
  bottom_left: { origin: 'bottom_left' as const, corner: [0, 0] },
  top_left: { origin: 'top_left' as const, corner: [0, 300] },
  bottom_right: { origin: 'bottom_right' as const, corner: [400, 0] },
  top_right: { origin: 'top_right' as const, corner: [400, 300] },
};
const device = (origin: DeviceProfile['origin']): DeviceProfile => ({ ...FALLBACK_DEVICE, id: origin, name: origin, bed: BED, origin });

describe('el X0 Y0 de la máquina cae en la esquina del perfil', () => {
  for (const [name, { origin, corner }] of Object.entries(PROFILES)) {
    it(name, () => {
      expect(workToDoc(device(origin), [0, 0])).toEqual(corner);
      expect(docToWork(device(origin), corner as Point)).toEqual([0, 0]);
    });
  }
});

describe('colocación del trabajo con posiciones de máquina', () => {
  // Diseño de 100 x 50 mm; el punto 6 (abajo-izquierda) se ancla al cabezal / origen de usuario.
  const design: [Point, Point] = [[0, 0], [100, 50]];
  const input = { jobOrigin: 6, design, userOrigin: null, head: null } as const;

  it('top_left: la Y de la máquina crece hacia abajo, el trabajo queda dentro de la cama', () => {
    const d = device('top_left');
    // Cabezal 20 mm bajo el borde superior: en el documento está en Y = 280.
    const p = computeJobPlacement({ ...input, device: d, startFrom: 'current_position', head: [30, 20] });
    expect(p.target).toEqual([30, 280]);
    expect(p.rect).toEqual([[30, 280], [130, 330]]);
    expect(p.overflow.top).toBeCloseTo(30, 6); // la esquina inferior izquierda ancla: el trabajo sube y se sale por arriba
    expect(p.fits).toBe(false);
    // Anclando arriba-izquierda (0) cuelga hacia abajo y cabe.
    const q = computeJobPlacement({ ...input, jobOrigin: 0, device: d, startFrom: 'user_origin', userOrigin: [30, 20] });
    expect(q.rect).toEqual([[30, 230], [130, 280]]);
    expect(q.fits).toBe(true);
  });

  it('bottom_right: la X de la máquina crece hacia la izquierda', () => {
    const d = device('bottom_right');
    // Cabezal a 40 mm del borde derecho: en el documento está en X = 360.
    const p = computeJobPlacement({ ...input, device: d, startFrom: 'current_position', head: [40, 10] });
    expect(p.target).toEqual([360, 10]);
    expect(p.rect).toEqual([[360, 10], [460, 60]]);
    expect(p.overflow.right).toBeCloseTo(60, 6);
    // Anclando arriba-derecha (2) el trabajo cuelga hacia la izquierda y cabe.
    const q = computeJobPlacement({ ...input, jobOrigin: 2, device: d, startFrom: 'user_origin', userOrigin: [40, 100] });
    expect(q.rect).toEqual([[260, 50], [360, 100]]);
    expect(q.fits).toBe(true);
  });

  it('el origen de usuario y el cabezal dan el mismo resultado para la misma posición de máquina', () => {
    for (const { origin } of Object.values(PROFILES)) {
      const d = device(origin);
      const a = computeJobPlacement({ ...input, device: d, startFrom: 'current_position', head: [50, 60] });
      const b = computeJobPlacement({ ...input, device: d, startFrom: 'user_origin', userOrigin: [50, 60] });
      expect(a.rect, origin).toEqual(b.rect);
    }
  });
});

describe('mover el cabezal respeta el origen del perfil', () => {
  it('top_left: subir en pantalla es Y negativa en la máquina', () => {
    expect(stepDelta(device('top_left'), { x: 0, y: 1 }, 5)).toEqual([0, -5]);
  });
  it('bottom_right: ir a la derecha en pantalla es X negativa en la máquina', () => {
    expect(stepDelta(device('bottom_right'), { x: 1, y: 0 }, 5)).toEqual([-5, 0]);
  });
  it('el jog continuo llega al borde de la cama en pantalla, no en la máquina', () => {
    // top_left, cabezal en Y de máquina 20 = 280 en el documento: arriba quedan 20 mm, o sea Y de máquina -20.
    expect(continuousDelta(device('top_left'), { x: 0, y: 1 }, [30, 20])).toEqual([0, -20]);
    // bottom_right, X de máquina 40 = 360 en el documento: a la derecha quedan 40 mm, o sea X de máquina -40.
    expect(continuousDelta(device('bottom_right'), { x: 1, y: 0 }, [40, 10])).toEqual([-40, 0]);
  });
});

describe('acciones del slice con perfiles que no son abajo-izquierda', () => {
  let api: MockApi;
  let store: ReturnType<typeof createAppStore>;
  /** Mueve el cabezal simulado y espera a que el store reciba el reporte de estado. */
  const moveHeadTo = async (pos: Point) => {
    api.machine.pos = [...pos];
    api.machine.emitStatus();
    await vi.waitFor(() => expect(store.getState().status?.work_pos).toEqual(pos));
  };

  async function setup(origin: DeviceProfile['origin']) {
    api = createMockApi({ latencyMs: 0, statusIntervalMs: 20, minJobMs: 200, jobSpeedup: 1e6 });
    setApi(api);
    store = createAppStore();
    await store.getState().loadDevices();
    store.setState({ devices: [device(origin)], activeDeviceId: origin });
    await bindMachineEvents(api, store);
    await store.getState().refreshPorts();
    await store.getState().connect();
    await vi.waitFor(() => expect(store.getState().status).not.toBeNull());
  }

  for (const origin of ['top_left', 'bottom_right'] as const) {
    it(`${origin}: fijar el origen guarda la posición de la máquina y ir al origen vuelve a ella`, async () => {
      await setup(origin);
      await moveHeadTo([30, 20]);
      store.getState().setUserOriginHere();
      expect(store.getState().jobOptions.user_origin).toEqual([30, 20]); // sin convertir al documento
      await moveHeadTo([100, 90]);
      await store.getState().goToOrigin();
      expect(api.machine.pos).toEqual([30, 20]);
    });

    it(`${origin}: ir al origen sin origen de usuario va al X0 Y0 de la máquina`, async () => {
      await setup(origin);
      await moveHeadTo([30, 20]);
      await store.getState().goToOrigin();
      expect(api.machine.pos).toEqual([0, 0]);
    });

    it(`${origin}: el marcador del cabezal y la ilustración usan la misma conversión`, async () => {
      await setup(origin);
      await moveHeadTo([30, 20]);
      const head = selectHeadDoc(store.getState());
      expect(head).toEqual(workToDoc(device(origin), [30, 20]));
      // El cabezal sobre el punto de anclaje: la colocación apunta al mismo lugar del documento.
      const p = computeJobPlacement({
        startFrom: 'current_position',
        jobOrigin: 6,
        userOrigin: null,
        head: [30, 20],
        device: device(origin),
        design: [[0, 0], [10, 10]],
      });
      expect(p.target).toEqual(head);
    });
  }
});
