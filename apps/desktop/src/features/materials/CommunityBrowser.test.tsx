// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { exampleCatalogIndex } from '@/api/mock/communityCatalog';
import type { CatalogState } from '@/api/types';
import { en, es, setLanguage } from '@/i18n';
import { audit, cleanup, click, describeViolations, mount } from '@/test/dom';
import { CommunityBrowserView, StatusLine, type BrowserViewProps } from './CommunityBrowser';
import { EMPTY_FILTER, type MyMachine } from './communityModel';

const falcon: MyMachine = { name: 'Creality Falcon2 22 W', model: 'creality-falcon2-22w', kind: 'diode', machineClass: 'diode_20' };
const fetched = Date.UTC(2026, 9, 12, 15, 30);
const network = (): CatalogState => ({ index: exampleCatalogIndex(), source: 'network', fetched_at: fetched, error: null });

function props(patch: Partial<BrowserViewProps> = {}): BrowserViewProps {
  return {
    catalog: network(),
    status: 'done',
    filter: EMPTY_FILTER,
    selectedId: null,
    me: falcon,
    canApply: true,
    onClose: vi.fn(),
    onRefresh: vi.fn(),
    onFilter: vi.fn(),
    onSelect: vi.fn(),
    onUse: vi.fn(),
    onApply: vi.fn(),
    onShare: vi.fn(),
    ...patch,
  };
}

const text = () => document.body.textContent ?? '';
const rows = () => [...document.querySelectorAll('.com-row')];
const button = (label: string) => [...document.querySelectorAll('button')].find((b) => b.textContent?.includes(label));

beforeEach(() => setLanguage('es'));
afterEach(async () => {
  await cleanup();
  setLanguage('es');
});

describe('navegador de la comunidad', () => {
  it('por omisión muestra solo mi modelo y mi clase, con el aviso de seguridad y de privacidad', async () => {
    await mount(<CommunityBrowserView {...props()} />);
    expect(rows()).toHaveLength(3);
    expect(text()).toContain('solo orientativos');
    expect(text()).toContain('jahziel1.github.io');
    expect(text()).toContain('no se envía nada tuyo');
    // El primero (mismo modelo, el más confirmado) es el que se ve en detalle.
    const detail = document.querySelector('.com__detail')!;
    expect(detail.textContent).toContain('Verificado por 2 personas');
    expect(detail.textContent).toContain('@ejemplo-dos');
    expect(detail.textContent).toContain('Mismo modelo');
    expect(detail.textContent).toContain('nadie la comprueba');
  });

  it('un ajuste de otra potencia muestra la advertencia de «solo orientativo» en su detalle', async () => {
    await mount(<CommunityBrowserView {...props({ filter: { ...EMPTY_FILTER, scope: 'all' }, selectedId: 'community.mdf.3.cut.diode_10.103' })} />);
    const detail = document.querySelector('.com__detail')!;
    expect(detail.textContent).toContain('Otra potencia');
    expect(detail.textContent).toContain('Solo orientativo');
    expect(detail.querySelector('.com-d__compat')?.getAttribute('data-tone')).toBe('warn');
  });

  it('los botones llaman a las acciones con el ajuste elegido', async () => {
    const p = props();
    await mount(<CommunityBrowserView {...p} />);
    await click(button('Usar (copiar')!);
    await click(button('Aplicar a la capa seleccionada')!);
    expect(p.onUse).toHaveBeenCalledOnce();
    expect(p.onApply).toHaveBeenCalledOnce();
    expect((p.onUse as ReturnType<typeof vi.fn>).mock.calls[0]![0].id).toBe('community.basswood.3.cut.diode_20.101');
    await click(rows()[1]!);
    expect(p.onSelect).toHaveBeenCalledWith('community.basswood.3.engrave.diode_20.102');
    await click(button('Actualizar')!);
    expect(p.onRefresh).toHaveBeenCalledOnce();
    await click(button('Compartir un ajuste mío')!);
    expect(p.onShare).toHaveBeenCalledOnce();
  });

  it('sin capa seleccionada, «Aplicar» queda sin efecto y explica por qué', async () => {
    const p = props({ canApply: false });
    await mount(<CommunityBrowserView {...p} />);
    const apply = button('Aplicar a la capa seleccionada')!;
    expect(apply.getAttribute('aria-disabled')).toBe('true');
    expect(apply.title).not.toBe('');
    await click(apply);
    expect(p.onApply).not.toHaveBeenCalled();
  });

  it('con el catálogo vacío invita a compartir el primero, sin ajustes inventados', async () => {
    const empty: CatalogState = { index: { ...exampleCatalogIndex(), entries: [] }, source: 'network', fetched_at: fetched, error: null };
    const p = props({ catalog: empty });
    await mount(<CommunityBrowserView {...p} />);
    expect(rows()).toHaveLength(0);
    expect(text()).toContain('Todavía no hay ajustes de la comunidad');
    expect(text()).toContain('0 ajustes en el catálogo');
    const share = [...document.querySelectorAll('.com__empty button')].find((b) => b.textContent?.includes('Compartir'))!;
    await click(share);
    expect(p.onShare).toHaveBeenCalledOnce();
  });

  it('con filtros que no coinciden dice que no hay resultados y sugiere ampliar', async () => {
    await mount(<CommunityBrowserView {...props({ filter: { ...EMPTY_FILTER, search: 'zzz' } })} />);
    expect(text()).toContain('Ningún ajuste coincide');
    expect(text()).toContain('Todas las máquinas');
  });

  it('si no sé la potencia de mi láser, avisa que muestra todo el tipo de láser', async () => {
    await mount(<CommunityBrowserView {...props({ me: { name: 'Mi láser', model: null, kind: 'diode', machineClass: null } })} />);
    expect(text()).toContain('No sé la potencia de tu láser');
    expect(rows().length).toBeGreaterThan(3);
  });

  it('axe no encuentra problemas, con datos, vacío y sin conexión', async () => {
    for (const catalog of [network(), { index: null, source: 'none', fetched_at: null, error: 'sin red' } satisfies CatalogState]) {
      const m = await mount(<CommunityBrowserView {...props({ catalog })} />);
      const result = await audit();
      expect(describeViolations(result)).toEqual([]);
      await m.unmount();
    }
  });
});

describe('línea de estado', () => {
  const render = async (catalog: CatalogState | null, status: 'idle' | 'loading' | 'done') => {
    await mount(<StatusLine catalog={catalog} status={status} />);
    return text();
  };

  it('descargando, al día, copia guardada, sin conexión y sin copia', async () => {
    expect(await render(null, 'loading')).toContain('Descargando el catálogo');
    await cleanup();
    expect(await render(network(), 'done')).toContain('Catálogo al día (publicado el 2026-10-15)');
    await cleanup();
    const cache: CatalogState = { ...network(), source: 'cache' };
    expect(await render(cache, 'loading')).toContain('Buscando novedades');
    await cleanup();
    const offline: CatalogState = { ...cache, error: 'No se pudo conectar con el sitio de Tlaku: sin red.' };
    const offlineText = await render(offline, 'done');
    expect(offlineText).toContain('No se pudo actualizar; se muestra la copia guardada el');
    expect(offlineText).toContain('2026-10-15');
    expect(offlineText).toContain('sin red');
    await cleanup();
    const none: CatalogState = { index: null, source: 'none', fetched_at: null, error: 'sin red' };
    expect(await render(none, 'done')).toContain('no hay ninguna copia guardada');
  });
});

describe('textos', () => {
  it('todas las claves del navegador existen en español e inglés y no son idénticas', () => {
    const keys = Object.keys(es).filter((k) => k.startsWith('materials.community.'));
    expect(keys.length).toBeGreaterThan(80);
    for (const k of keys) {
      expect(en[k as keyof typeof en], k).toBeTruthy();
    }
    const same = keys.filter((k) => en[k as keyof typeof en] === es[k as keyof typeof es] && !/^materials\.community\.(detail\.laser|notes\.fromAnon)$/.test(k));
    // Solo pueden coincidir textos cortos que se escriben igual en ambos idiomas.
    for (const k of same) expect((es[k as keyof typeof es] as string).length, k).toBeLessThan(14);
  });

  it('se ve en inglés', async () => {
    setLanguage('en');
    await mount(<CommunityBrowserView {...props()} />);
    expect(text()).toContain('Community settings');
    expect(text()).toContain('only a rough guide');
    expect(text()).toContain('Use (copy to my library)');
  });
});
