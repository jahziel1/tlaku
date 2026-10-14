import { afterEach, describe, expect, it } from 'vitest';
import { getLayerDefaults, makeLayer, newLayerFor } from '@/lib/layers';
import { useStore } from '@/store';
import { currentDefaultsLevel, hasColorDefaults, removeColorDefaults, resetLayerDefaults, saveColorDefaults, saveLayerAsDefaults } from './defaults';

const RED = '#ff3b4a';

afterEach(() => {
  useStore.getState().resetPrefs();
  useStore.getState().newDocument();
});

describe('valores por defecto de capas nuevas (C35)', () => {
  it('guardar toma los ajustes de la capa; las capas nuevas empiezan con ellos', () => {
    const s = useStore.getState();
    s.ensureLayer(RED);
    s.updateLayer(RED, { speed: 2222, power: 31, name: 'Mi corte', order: 2 });
    saveLayerAsDefaults(useStore.getState(), RED);

    const saved = useStore.getState().prefs.layerDefaults;
    expect(saved).toMatchObject({ speed: 2222, power: 31 });
    expect(saved && ['color', 'name', 'order'].some((k) => k in saved)).toBe(false);
    expect(getLayerDefaults()).toEqual(saved);

    const fresh = newLayerFor('#2f7bff');
    expect(fresh).toMatchObject({ speed: 2222, power: 31, color: '#2f7bff', order: null });
    expect(fresh.name).not.toBe('Mi corte');
    expect(makeLayer('#000', 'x').speed).toBe(2222);
    // Las capas ya creadas no cambian.
    expect(useStore.getState().doc.layers[0]!.speed).toBe(2222);
    expect(useStore.getState().notices.at(-1)?.level).toBe('success');
  });

  it('restablecer vuelve a los valores de fábrica', () => {
    const s = useStore.getState();
    s.ensureLayer(RED);
    s.updateLayer(RED, { speed: 2222 });
    saveLayerAsDefaults(useStore.getState(), RED);
    resetLayerDefaults(useStore.getState());
    expect(useStore.getState().prefs.layerDefaults).toBeNull();
    expect(getLayerDefaults()).toBeNull();
    expect(newLayerFor('#2f7bff').speed).toBe(1000);
  });

  it('un cambio en las preferencias (leer el disco, importar un paquete) se refleja en makeLayer', () => {
    useStore.getState().replacePrefs({ prefs: { layerDefaults: { speed: 1800, mode: 'fill' } } });
    expect(makeLayer('#000', 'x')).toMatchObject({ speed: 1800, mode: 'fill' });
    useStore.getState().resetPrefs();
    expect(makeLayer('#000', 'x')).toMatchObject({ speed: 1000, mode: 'line' });
  });
});

describe('valores por defecto por color y por máquina (C35)', () => {
  const BLUE = '#2f7bff';
  const setup = () => {
    useStore.setState({ activeDeviceId: 'laser-a' });
    const s = useStore.getState();
    s.ensureLayer(RED);
    s.updateLayer(RED, { speed: 700, power: 80 });
  };
  afterEach(() => {
    useStore.setState({ activeDeviceId: null });
  });

  it('guardar para color y máquina solo vale en esa máquina', () => {
    setup();
    saveColorDefaults(useStore.getState(), RED, 'machine');
    expect(useStore.getState().prefs.layerColorDefaults).toHaveLength(1);
    expect(useStore.getState().prefs.layerColorDefaults[0]).toMatchObject({ color: RED, device: 'laser-a' });
    expect(makeLayer(RED, 'x')).toMatchObject({ speed: 700, power: 80 });
    // Otro color: fábrica.
    expect(makeLayer(BLUE, 'x').speed).toBe(1000);
    // Otra máquina: fábrica.
    useStore.setState({ activeDeviceId: 'laser-b' });
    expect(makeLayer(RED, 'x').speed).toBe(1000);
  });

  it('el orden es máquina y color, luego color, luego general', () => {
    setup();
    // General: 1500.
    useStore.getState().updateLayer(RED, { speed: 1500 });
    saveLayerAsDefaults(useStore.getState(), RED);
    expect(makeLayer(RED, 'x').speed).toBe(1500);
    // Solo color: 900.
    useStore.getState().updateLayer(RED, { speed: 900 });
    saveColorDefaults(useStore.getState(), RED, 'color');
    expect(makeLayer(RED, 'x').speed).toBe(900);
    expect(makeLayer(BLUE, 'x').speed).toBe(1500);
    // Color y máquina: 300.
    useStore.getState().updateLayer(RED, { speed: 300 });
    saveColorDefaults(useStore.getState(), RED, 'machine');
    expect(makeLayer(RED, 'x').speed).toBe(300);
    useStore.setState({ activeDeviceId: 'laser-b' });
    expect(makeLayer(RED, 'x').speed).toBe(900);
    expect(currentDefaultsLevel(useStore.getState(), RED)).toBe('color');
    expect(currentDefaultsLevel(useStore.getState(), BLUE)).toBe('general');
    // Quitar el de máquina vuelve al de color.
    useStore.setState({ activeDeviceId: 'laser-a' });
    expect(currentDefaultsLevel(useStore.getState(), RED)).toBe('machine');
    removeColorDefaults(useStore.getState(), RED, 'machine');
    expect(makeLayer(RED, 'x').speed).toBe(900);
    removeColorDefaults(useStore.getState(), RED, 'color');
    expect(makeLayer(RED, 'x').speed).toBe(1500);
  });

  it('guardar de nuevo reemplaza la entrada y no se duplica', () => {
    setup();
    saveColorDefaults(useStore.getState(), RED, 'machine');
    useStore.getState().updateLayer(RED, { speed: 650 });
    saveColorDefaults(useStore.getState(), RED, 'machine');
    expect(useStore.getState().prefs.layerColorDefaults).toHaveLength(1);
    expect(makeLayer(RED, 'x').speed).toBe(650);
    expect(hasColorDefaults(useStore.getState(), RED, 'machine')).toBe(true);
    expect(hasColorDefaults(useStore.getState(), RED, 'color')).toBe(false);
  });

  it('sin máquina activa no se guarda para una máquina', () => {
    setup();
    useStore.setState({ activeDeviceId: null });
    saveColorDefaults(useStore.getState(), RED, 'machine');
    expect(useStore.getState().prefs.layerColorDefaults).toEqual([]);
  });

  it('las preferencias guardadas antes (solo layerDefaults) se conservan y la lista nueva viene vacía', () => {
    useStore.getState().replacePrefs({ prefs: { layerDefaults: { speed: 1800 } } });
    expect(useStore.getState().prefs.layerColorDefaults).toEqual([]);
    expect(makeLayer(RED, 'x').speed).toBe(1800);
  });

  it('leer el disco o un paquete sanea la lista: descarta lo inválido', () => {
    useStore.getState().replacePrefs({
      prefs: {
        layerColorDefaults: [
          { color: '#FF3B4A', device: 'laser-a', template: { speed: 400 } },
          { color: 'rojo', device: null, template: { speed: 1 } },
          { color: BLUE, device: 5, template: { speed: 1 } },
          { color: BLUE, device: null, template: null },
          { color: BLUE, device: null, template: { speed: 'rápido', power: 33 } },
        ],
      },
    });
    const list = useStore.getState().prefs.layerColorDefaults;
    expect(list.map((e) => [e.color, e.device])).toEqual([
      [RED, 'laser-a'],
      [BLUE, null],
    ]);
    expect(list[1]!.template).toMatchObject({ speed: 1000, power: 33 });
    useStore.setState({ activeDeviceId: 'laser-a' });
    expect(makeLayer(RED, 'x').speed).toBe(400);
  });

  it('cambiar de máquina activa cambia con qué valores nace la capa', () => {
    setup();
    saveColorDefaults(useStore.getState(), RED, 'machine');
    expect(newLayerFor(RED).speed).toBe(700);
    useStore.setState({ activeDeviceId: 'laser-b' });
    expect(newLayerFor(RED).speed).toBe(1000);
  });
});
