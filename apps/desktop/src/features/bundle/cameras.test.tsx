import { renderToString } from 'react-dom/server';
import { beforeEach, describe, expect, it } from 'vitest';
import { getApi, setApi, type UserBundle } from '@/api';
import { createMockApi } from '@/api/mock';
import type { CameraPreset, CameraSetup, HeadCalibration, LensCalibration } from '@/api/types';
import { commands } from '@/commands';
import { useArtPanel } from '@/features/art/artStore';
import type { UserArt } from '@/features/art/types';
import { fitAlignment, parseSetup } from '@/features/camera/setup';
import { emptyLibrary } from '@/features/materials/library';
import { useMaterials } from '@/features/materials/state';
import { DEFAULT_PREFS, useStore } from '@/store';
import { emptyCameraSetup } from '@/store/camera';
import { sampleDevices } from '@/api/mock/sample';
import { applyImport, currentState } from './apply';
import { applicableSections, ExportBody, ImportPreview } from './BundleDialog';
import { buildArtSection, planArt } from './art';
import { buildCamerasSection, camerasChanges, planCameras } from './cameras';
import { exportUserBundle, readUserBundle } from './files';
import { allSections, buildUserBundle, planImport, sectionChanges, type CurrentState } from './model';

const LENS: LensCalibration = { width: 1280, height: 720, fx: 950, fy: 960, cx: 645, cy: 355, k1: -0.32, k2: 0.11, p1: 0, p2: 0, k3: 0, rms_error_px: 0.34, views_used: 8 };
const LENS_B: LensCalibration = { ...LENS, fx: 1000, k1: -0.2 };
const HEAD: HeadCalibration = { image_size: [1280, 720], ref_px: [640, 360], px_to_mm: [[0.1, 0], [0, -0.1]], offset_mm: [5, -30], rms_mm: 0.06, observations: 4 };
const align = (dx = 0) =>
  fitAlignment([1280, 720], [[100, 600], [1180, 610], [1170, 90], [110, 100]], [[dx, 0], [297 + dx, 0], [297 + dx, 210], [dx, 210]])!;

const preset = (name: string, over: Partial<CameraPreset> = {}): CameraPreset => ({
  name,
  source: 'network',
  device_id: 'dev-123',
  device_label: 'USB Cam',
  width: 1280,
  height: 720,
  network_url: 'http://ana:topsecret@192.168.1.20/snap.jpg',
  dpm: 8,
  offset_x_mm: 1,
  offset_y_mm: -1,
  grayscale: false,
  ...over,
});

const setup = (over: Partial<CameraSetup> = {}): CameraSetup => ({ ...emptyCameraSetup(), ...over });

const full = (): CameraSetup =>
  setup({ calibration: LENS, alignment: align(), head: { enabled: true, calibration: HEAD }, presets: [preset('Taller')] });

describe('sección de cámaras: armar', () => {
  it('lleva calibración, alineación, cabezal y configuraciones, sin claves ni ids de dispositivo', () => {
    const s = buildCamerasSection(full());
    expect(s.calibration).toEqual(LENS);
    expect(s.alignment?.image_size).toEqual([1280, 720]);
    expect(s.head).toEqual({ enabled: true, calibration: HEAD });
    expect(s.presets).toHaveLength(1);
    expect(s.presets![0]).toMatchObject({ name: 'Taller', device_id: null, device_label: 'USB Cam', network_url: 'http://192.168.1.20/snap.jpg' });
    expect(JSON.stringify(s)).not.toMatch(/topsecret|ana:|dev-123/);
  });

  it('una configuración sin nada guardado da una sección vacía pero válida', () => {
    const s = buildCamerasSection(emptyCameraSetup());
    expect(s).toEqual({ calibration: null, alignment: null, head: null, presets: [] });
  });

  it('el paquete completo no filtra la clave en ningún campo', () => {
    const b = buildUserBundle({ devices: [], activeDeviceId: null, library: emptyLibrary(), prefs: { ...DEFAULT_PREFS }, shortcuts: {}, cameras: full() }, { sections: allSections() });
    expect(JSON.stringify(b)).not.toContain('topsecret');
    expect(b.cameras?.presets?.[0]?.network_url).toBe('http://192.168.1.20/snap.jpg');
  });
});

describe('sección de cámaras: vista previa', () => {
  it('sobre una cámara vacía todo es nuevo', () => {
    const plan = planCameras(buildCamerasSection(full()), emptyCameraSetup())!;
    expect([plan.calibration, plan.alignment, plan.head]).toEqual(['new', 'new', 'new']);
    expect(plan.presets).toMatchObject({ skipped: 0, renamed: 0 });
    expect(plan.presets!.added.map((p) => p.name)).toEqual(['Taller']);
    expect(camerasChanges(plan)).toBe(4);
    expect(plan.setup.calibration).toEqual(LENS);
    expect(plan.setup.head?.calibration).toEqual(HEAD);
  });

  it('lo igual a lo local no es un cambio', () => {
    const local = setup({ calibration: LENS, alignment: align(), head: { enabled: true, calibration: HEAD }, presets: [preset('Taller', { network_url: 'http://192.168.1.20/snap.jpg', device_id: null })] });
    const plan = planCameras(buildCamerasSection(local), local)!;
    expect([plan.calibration, plan.alignment, plan.head]).toEqual(['same', 'same', 'same']);
    expect(plan.presets).toMatchObject({ skipped: 1 });
    expect(plan.presets!.added).toEqual([]);
    expect(camerasChanges(plan)).toBe(0);
  });

  it('una pieza distinta reemplaza a la local y se avisa; las configuraciones nunca pisan a las locales', () => {
    const local = setup({
      calibration: LENS,
      alignment: align(),
      presets: [preset('Taller', { dpm: 3, device_id: null }), preset('Garage', { device_id: null, network_url: 'http://192.168.1.20/snap.jpg' })],
    });
    const incoming = buildCamerasSection(setup({ calibration: LENS, alignment: align(10), presets: [preset('taller'), preset('Garage', { network_url: 'http://192.168.1.20/snap.jpg' }), preset('Nueva')] }));
    const plan = planCameras(incoming, local)!;
    expect(plan.calibration).toBe('same');
    expect(plan.alignment).toBe('replace');
    expect(plan.presets).toMatchObject({ renamed: 1, skipped: 1 });
    expect(plan.presets!.added.map((p) => p.name)).toEqual(['taller (2)', 'Nueva']);
    // Lo local sigue ahí, en su lugar, y lo nuevo se suma al final.
    expect(plan.setup.presets!.map((p) => p.name)).toEqual(['Taller', 'Garage', 'taller (2)', 'Nueva']);
    expect(plan.setup.presets![0]!.dpm).toBe(3);
  });

  it('si entra una lente distinta, la alineación y la del cabezal locales se quitan por no cuadrar', () => {
    const local = full();
    const plan = planCameras({ calibration: LENS_B }, local)!;
    expect(plan.calibration).toBe('replace');
    expect(plan.cleared).toEqual(['alignment', 'head']);
    expect(plan.setup.alignment).toBeNull();
    expect(plan.setup.head).toEqual({ enabled: true, calibration: null });
    expect(plan.setup.calibration).toEqual(LENS_B);
    // Si el paquete trae las suyas, se usan y no se quita nada.
    const both = planCameras({ calibration: LENS_B, alignment: buildCamerasSection(full()).alignment, head: { enabled: false, calibration: HEAD } }, local)!;
    expect(both.cleared).toEqual([]);
    expect(both.setup.head).toEqual({ enabled: false, calibration: HEAD });
    // Sin lente nueva, lo local se queda.
    const none = planCameras({ presets: [] }, local)!;
    expect(none.cleared).toEqual([]);
    expect(none.setup.alignment).toEqual(local.alignment);
  });

  it('quita las claves que traiga un archivo ajeno y descarta lo inválido', () => {
    const plan = planCameras(
      {
        calibration: { width: 'x' },
        alignment: { image_size: [1, 2] },
        head: { enabled: true, calibration: { image_size: [1] } },
        presets: [preset('Con clave'), { source: 'local' }, 'basura'],
      },
      emptyCameraSetup(),
    )!;
    expect(plan.invalid.sort()).toEqual(['alignment', 'calibration', 'head']);
    expect(plan.calibration).toBeNull();
    expect(plan.presets).toMatchObject({ invalid: 2, strippedUrls: 1 });
    expect(plan.presets!.added[0]).toMatchObject({ device_id: null, network_url: 'http://192.168.1.20/snap.jpg' });
    expect(JSON.stringify(plan.setup)).not.toContain('topsecret');
  });

  it('respeta el máximo de configuraciones', () => {
    const local = setup({ presets: Array.from({ length: 31 }, (_, i) => preset(`p${i}`)) });
    const plan = planCameras({ presets: [preset('a'), preset('b'), preset('c')] }, local)!;
    expect(plan.presets!.added).toHaveLength(1);
    expect(plan.presets!.dropped).toBe(2);
  });

  it('lo que no es una sección no es un plan', () => {
    expect(planCameras(undefined, emptyCameraSetup())).toBeNull();
    expect(planCameras(null, emptyCameraSetup())).toBeNull();
    expect(planCameras([], emptyCameraSetup())).toBeNull();
    expect(planCameras({}, emptyCameraSetup())).toMatchObject({ calibration: null, alignment: null, head: null, presets: null });
  });
});

const art = (name: string, extra = ''): UserArt => ({ name, svg: `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><rect width="${extra || 5}" height="5"/></svg>` });

describe('sección de arte', () => {
  it('lleva solo los dibujos propios', () => {
    expect(buildArtSection([art('a'), art('b')]).items.map((i) => i.name)).toEqual(['a', 'b']);
  });

  it('mezcla: nuevos entran, idénticos se saltan y los que chocan entran con otro nombre', () => {
    const local = [art('uno'), art('dos')];
    const plan = planArt({ items: [art('uno'), art('DOS', '9'), art('tres'), art('tres'), { name: '', svg: '<svg/>' }, { name: 'x', svg: 'no es svg' }, 5, art('tres', '7')] }, local)!;
    expect(plan.items.map((i) => i.name)).toEqual(['DOS', 'tres', 'tres']);
    expect(plan.skipped).toBe(2); // 'uno' idéntico y el segundo 'tres' idéntico al primero
    expect(plan.renamed).toBe(2);
    expect(plan.invalid).toBe(3);
  });

  it('un svg demasiado pesado se descarta', () => {
    const big = { name: 'enorme', svg: `<svg>${'x'.repeat(2 * 1024 * 1024)}</svg>` };
    expect(planArt({ items: [big] }, [])!.invalid).toBe(1);
    expect(planArt(null, [])).toBeNull();
    expect(planArt({}, [])).toEqual({ items: [], renamed: 0, skipped: 0, invalid: 0 });
  });
});

function current(over: Partial<CurrentState> = {}): CurrentState {
  return {
    devices: sampleDevices(),
    activeDeviceId: sampleDevices()[0]!.id,
    library: emptyLibrary(),
    prefs: { ...DEFAULT_PREFS },
    shortcuts: {},
    knownCommands: new Set(commands.list().map((c) => c.id)),
    ...over,
  };
}

async function reset() {
  setApi(createMockApi({ latencyMs: 0 }));
  useMaterials.setState({ library: emptyLibrary(), loaded: false, loadError: null });
  useStore.setState({ jobState: 'idle', notices: [] });
  useStore.getState().setCameraSetup(emptyCameraSetup(), false);
  useArtPanel.setState({ user: [], error: null });
  await useStore.getState().loadDevices();
}

beforeEach(reset);

describe('importar y exportar cámaras y arte', () => {
  it('ida y vuelta: exporta, cambia el estado y al importar vuelve (sin tocar la fuente ni la opacidad locales)', async () => {
    const s = useStore.getState();
    s.setCameraSetup({ ...full(), prefs: { ...full().prefs, opacity: 0.3, source: 'network', network_url: 'http://ana:topsecret@192.168.1.20/snap.jpg' } }, true);
    await getApi().artSaveUser({ items: [art('flor'), art('luna')] });
    expect(await exportUserBundle(allSections())).toBe(true);
    const file = await readUserBundle('tlaku.tlakubundle');
    expect(file.cameras?.calibration).toEqual(LENS);
    expect(file.art?.items.map((i) => i.name)).toEqual(['flor', 'luna']);
    expect(JSON.stringify(file)).not.toContain('topsecret');

    // Otro equipo: sin calibración, con otra opacidad y otros dibujos.
    s.setCameraSetup({ ...emptyCameraSetup(), prefs: { ...emptyCameraSetup().prefs, opacity: 0.9 } }, true);
    await getApi().artRemoveUser({ name: 'flor' });
    await getApi().artRemoveUser({ name: 'luna' });
    await getApi().artSaveUser({ items: [art('luna', '9')] });

    const plan = planImport(file, await currentState());
    expect(plan.cameras).toMatchObject({ calibration: 'new', alignment: 'new', head: 'new' });
    expect(plan.art).toMatchObject({ renamed: 1, skipped: 0 });
    expect(sectionChanges(plan)).toMatchObject({ cameras: 4, art: 2 });
    // La vista previa no escribió nada.
    expect(useStore.getState().cameraSetup.calibration).toBeNull();
    expect((await getApi().artListUser()).map((a) => a.name)).toEqual(['luna']);

    const result = await applyImport(plan, allSections(), 'tlaku.tlakubundle');
    expect(result.cameras).toEqual({ parts: 3, presets: 1 });
    expect(result.art).toEqual({ added: 2, failed: 0 });
    const cam = useStore.getState().cameraSetup;
    expect(cam.calibration).toEqual(LENS);
    expect(cam.head).toEqual({ enabled: true, calibration: HEAD });
    expect(cam.alignment?.image_size).toEqual([1280, 720]);
    expect(cam.presets?.map((p) => p.name)).toEqual(['Taller']);
    // Lo que es de este equipo no cambia.
    expect(cam.prefs.opacity).toBe(0.9);
    // Quedó en el respaldo (camera.json en la app real).
    const saved = parseSetup(await getApi().loadCameraSetup());
    expect(saved.calibration).toEqual(LENS);
    expect(saved.presets?.[0]?.network_url).toBe('http://192.168.1.20/snap.jpg');
    expect(JSON.stringify(await getApi().loadCameraSetup())).not.toContain('topsecret');
    // Los dibujos entraron sin pisar el que ya había con ese nombre.
    expect((await getApi().artListUser()).map((a) => a.name).sort()).toEqual(['flor', 'luna', 'luna-2']);
    expect(useArtPanel.getState().user).toHaveLength(3);
    expect(useStore.getState().notices.at(-1)?.level).toBe('success');
  });

  it('solo aplica las secciones marcadas: sin marcar cámaras no se toca la cámara', async () => {
    const plan = planImport(buildUserBundle({ ...current(), cameras: full(), userArt: [art('flor')] }, { sections: allSections() }), await currentState());
    const result = await applyImport(plan, { ...allSections(false), art: true });
    expect(result.cameras).toBeNull();
    expect(result.art).toEqual({ added: 1, failed: 0 });
    expect(useStore.getState().cameraSetup.calibration).toBeNull();
    const only = await applyImport(plan, { ...allSections(false), cameras: true });
    expect(only.art).toBeNull();
    expect(useStore.getState().cameraSetup.calibration).toEqual(LENS);
  });

  it('un guardado de arte que falla avisa y deja el resto aplicado', async () => {
    const plan = planImport(buildUserBundle({ ...current(), cameras: full(), userArt: [art('flor')] }, { sections: allSections() }), await currentState());
    const base = createMockApi({ latencyMs: 0 });
    setApi({ ...base, artSaveUser: () => Promise.reject(new Error('disco lleno')) });
    const result = await applyImport(plan, allSections());
    expect(result.art).toBeNull();
    expect(result.cameras).not.toBeNull();
    expect(useStore.getState().notices.some((n) => n.level === 'error' && n.text.includes('disco lleno'))).toBe(true);
  });

  it('un paquete de antes (sin cámaras ni arte) sigue leyéndose igual', async () => {
    const old = { ...buildUserBundle({ ...current() }, { sections: { devices: true, materials: true, prefs: true } }) } as UserBundle;
    delete old.cameras;
    delete old.art;
    const plan = planImport(old, current());
    expect(plan.cameras).toBeNull();
    expect(plan.art).toBeNull();
    expect(applicableSections(plan, allSections())).toMatchObject({ cameras: false, art: false });
  });
});

describe('diálogo: cámaras y arte', () => {
  it('la exportación muestra qué lleva cada sección nueva', () => {
    const html = renderToString(
      <ExportBody choice={allSections()} onChange={() => {}} counts={{ devices: 1, materials: 2, art: 4, presets: 3, lens: true, alignment: true, head: false }} />,
    );
    expect(html).toContain('Cámaras');
    expect(html).toContain('Biblioteca de arte propia');
    expect(html).toContain('lente, alineación con la cama');
    expect(html).toContain('3 configuraciones de cámara');
    expect(html).toContain('sin usuario ni clave');
    expect(html).toContain('4 dibujos');
    expect(html).not.toMatch(/bundle\.[a-z]/);
    expect((html.match(/type="checkbox"/g) ?? []).length).toBe(9);
  });

  it('la vista previa avisa de lo que reemplaza, de lo que se quita y de las claves descartadas', () => {
    const local = setup({ calibration: LENS, alignment: align(), presets: [preset('Taller', { dpm: 1, device_id: null, network_url: null })] });
    const bundle = {
      ...buildUserBundle({ ...current(), cameras: emptyCameraSetup(), userArt: [art('flor'), art('luna')] }, { sections: allSections() }),
      cameras: { calibration: LENS_B, presets: [preset('Taller'), preset('Otra')] },
    } as UserBundle;
    const plan = planImport(bundle, current({ cameras: local, userArt: [art('flor')] }));
    const html = renderToString(<ImportPreview plan={plan} choice={allSections()} onChange={() => {}} />);
    expect(html).toContain('reemplaza a la que tienes');
    expect(html).toContain('Tu alineación actual se quita');
    expect(html).toContain('2 configuraciones nuevas');
    expect(html).toContain('se quitaron');
    expect(html).toContain('1 dibujos nuevos');
    expect(html).toContain('luna');
    expect(html).not.toMatch(/bundle\.[a-z]/);
    expect(html).not.toContain('topsecret');
  });
});
