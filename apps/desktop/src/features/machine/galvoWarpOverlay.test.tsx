// @vitest-environment jsdom
import { act } from 'react';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { useStore } from '@/store';
import { cleanup, mount } from '@/test/dom';
import { galvoDevice } from './galvoFixtures';
import { GalvoWarpOverlay } from './GalvoWarpOverlay';
import { useWarpUi } from './galvoWarpState';

function setDevice(over: Parameters<typeof galvoDevice>[0] = {}) {
  const d = galvoDevice(over);
  useStore.setState({ devices: [d], activeDeviceId: d.id, zoom: 2, panX: 10, panY: 300 });
  return d;
}

describe('capa de la deformación sobre el lienzo', () => {
  let area: HTMLElement;
  beforeEach(() => {
    area = document.createElement('div');
    area.className = 'canvas-area';
    document.body.append(area);
    useWarpUi.setState({ show: false });
  });
  afterEach(async () => {
    await cleanup();
  });

  const paint = async () => {
    await mount(<GalvoWarpOverlay />);
    await act(async () => {
      await new Promise((r) => setTimeout(r, 150));
    });
  };

  it('no dibuja nada mientras no se pide o no hay deformación', async () => {
    const d = setDevice();
    d.galvo.cylinder.enabled = true;
    await paint();
    expect(area.querySelector('[data-testid="galvo-warp"]')).toBeNull();
    useWarpUi.setState({ show: true });
    useStore.setState({ devices: [galvoDevice()] });
    await act(async () => undefined);
    expect(area.querySelector('[data-testid="galvo-warp"]')).toBeNull();
  });

  it('con cilindro y la casilla activa dibuja el campo y la rejilla deformada, con el horizonte en rojo', async () => {
    const d = galvoDevice();
    d.galvo.cylinder = { enabled: true, diameter_mm: 60, mirror_distance_mm: 160, axis: 'x', concave: false };
    d.bed = [110, 110];
    useStore.setState({ devices: [d], activeDeviceId: d.id, zoom: 2, panX: 10, panY: 300 });
    useWarpUi.setState({ show: true });
    await paint();
    const svg = area.querySelector('[data-testid="galvo-warp"]');
    expect(svg).not.toBeNull();
    expect(svg!.getAttribute('aria-label')).toMatch(/Rejilla|Design grid/);
    const paths = [...svg!.querySelectorAll('path')];
    // El campo + 12 líneas de cada sentido (cada 10 mm en 110), más los tramos cortados en el horizonte.
    expect(paths.length).toBeGreaterThanOrEqual(1 + 24);
    expect(paths.some((p) => p.getAttribute('stroke') === 'var(--danger)')).toBe(true);
    expect(paths.some((p) => p.getAttribute('stroke') === 'var(--accent)')).toBe(true);
  });

  it('con una máquina que no es galvo no dibuja', async () => {
    const d = galvoDevice({ kind: 'grbl' });
    d.galvo.cylinder.enabled = true;
    useStore.setState({ devices: [d], activeDeviceId: d.id });
    useWarpUi.setState({ show: true });
    await paint();
    expect(area.querySelector('[data-testid="galvo-warp"]')).toBeNull();
  });
});
