import type { LaserApi } from '../api';
import type { CatalogEntry, CatalogIndex, CatalogState } from '../types';

type Lag = <T>(value: () => T) => Promise<T>;

/**
 * Catálogo de la comunidad del modo mock (maqueta del navegador de ajustes). **Son ejemplos
 * inventados para ver la pantalla, no ajustes reales ni verificados**: cada nota lo dice. El
 * catálogo real sale del sitio del proyecto (`catalog/v1/index.json`) y hoy está vacío.
 */
const base = (patch: Partial<CatalogEntry> & Pick<CatalogEntry, 'id' | 'material' | 'role' | 'operation' | 'mode' | 'machine_class' | 'laser_watts' | 'machine' | 'speed' | 'power'>): CatalogEntry => {
  const issue = patch.id.split('.').at(-1) ?? '0';
  return {
    air: false,
    notes: 'Ejemplo de la maqueta: no es un ajuste real.',
    source: `https://github.com/jahziel1/tlaku/issues/${issue}`,
    license: 'CC0-1.0',
    verified_by: 0,
    ...patch,
  };
};

export function exampleCatalogIndex(): CatalogIndex {
  return {
    schema: 'tlaku.community-materials',
    version: 1,
    generated_at: '2026-10-15',
    entries: [
      base({
        id: 'community.basswood.3.cut.diode_20.101',
        material: 'basswood', thickness: 3, role: 'cut', operation: 'cut', mode: 'line',
        machine_class: 'diode_20', laser_watts: 22, machine: 'Creality Falcon2 22 W', machine_model: 'creality-falcon2-22w',
        speed: 500, power: 100, passes: 2, air: true, contributor: 'ejemplo-uno', date: '2026-09-02', tlaku_version: '0.1.0',
        verified_by: 2,
        confirmations: [
          { by: 'ejemplo-dos', machine: 'Creality Falcon2 22 W', machine_class: 'diode_20', date: '2026-09-10' },
          { by: 'ejemplo-tres', machine: 'Atomstack A20 Pro', machine_class: 'diode_20', date: '2026-09-18' },
        ],
      }),
      base({
        id: 'community.basswood.3.engrave.diode_20.102',
        material: 'basswood', thickness: 3, role: 'engrave', operation: 'fill', mode: 'fill',
        machine_class: 'diode_20', laser_watts: 22, machine: 'Creality Falcon2 22 W', machine_model: 'creality-falcon2-22w',
        speed: 6000, power: 40, passes: 1, interval: 0.1, contributor: 'ejemplo-uno', date: '2026-09-02', tlaku_version: '0.1.0',
      }),
      base({
        id: 'community.mdf.3.cut.diode_10.103',
        material: 'mdf', thickness: 3, role: 'cut', operation: 'cut', mode: 'line',
        machine_class: 'diode_10', laser_watts: 10, machine: 'Ortur Laser Master 3 10 W',
        speed: 150, power: 100, passes: 4, air: true, contributor: 'ejemplo-cuatro', date: '2026-08-21',
      }),
      base({
        id: 'community.leather.x.engrave.co2_60.104',
        material: 'leather', role: 'engrave', operation: 'fill', mode: 'fill',
        machine_class: 'co2_60', laser_watts: 60, machine: 'CO2 genérico 60 W (Ruida)',
        speed: 18000, power: 18, interval: 0.08, contributor: 'ejemplo-cinco', date: '2026-07-30', verified_by: 1,
        confirmations: [{ by: 'ejemplo-seis', machine: 'CO2 50 W (Ruida)', machine_class: 'co2_40', date: '2026-08-04' }],
      }),
      base({
        id: 'community.felt.2.cut.diode_20.105',
        material: 'felt', material_name: 'Fieltro', thickness: 2, role: 'cut', operation: 'cut', mode: 'line',
        machine_class: 'diode_20', laser_watts: 20, machine: 'Atomstack A20 Pro',
        speed: 900, power: 80, passes: 1, contributor: 'ejemplo-siete', date: '2026-10-01',
      }),
      base({
        id: 'community.slate.x.image.fiber_20.106',
        material: 'slate', role: 'engrave', operation: 'image', mode: 'image',
        machine_class: 'fiber_20', laser_watts: 20, machine: 'Fibra 20 W (galvo)',
        speed: 3000, power: 60, interval: 0.05, frequency_khz: 40, contributor: 'ejemplo-ocho', date: '2026-10-09',
      }),
    ],
  };
}

type Mode = 'ok' | 'offline' | 'empty' | 'invalid';

/** `?community=offline|empty|invalid` en la dirección del modo mock: para ver los demás estados. */
function modeFromUrl(): Mode {
  try {
    const m = new URLSearchParams(globalThis.location?.search ?? '').get('community');
    return m === 'offline' || m === 'empty' || m === 'invalid' ? m : 'ok';
  } catch {
    return 'ok';
  }
}

const DAY = 86_400_000;

/**
 * Catálogo simulado: `cached` no usa «red» y `refresh` sí (como el comando real). La caché vive lo que
 * dura la página; con `?community=offline` arranca con una copia de hace tres días y la «red» falla.
 */
export function mockCommunityCatalog(lag: Lag, mode: () => Mode = modeFromUrl): Pick<LaserApi, 'communityCatalogCached' | 'communityCatalogRefresh'> {
  let cache: { index: CatalogIndex; at: number } | null = mode() === 'offline' ? { index: exampleCatalogIndex(), at: Date.now() - 3 * DAY } : null;
  const fromCache = (error: string | null): CatalogState => ({
    index: cache ? structuredClone(cache.index) : null,
    source: cache ? 'cache' : 'none',
    fetched_at: cache?.at ?? null,
    error,
  });
  return {
    communityCatalogCached: () => lag(() => fromCache(null)),
    communityCatalogRefresh: () =>
      lag((): CatalogState => {
        switch (mode()) {
          case 'offline':
            return fromCache('No se pudo conectar con el sitio de Tlaku: sin conexión.');
          case 'invalid':
            return fromCache('El catálogo descargado no es válido y se descartó: "community.basswood.3.cut.diode_20.101": "power" fuera de rango o inválido.');
          case 'empty': {
            cache = { index: { ...exampleCatalogIndex(), entries: [] }, at: Date.now() };
            return { ...fromCache(null), source: 'network' };
          }
          default:
            cache = { index: exampleCatalogIndex(), at: Date.now() };
            return { ...fromCache(null), source: 'network' };
        }
      }),
  };
}
