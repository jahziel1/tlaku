import { describe, expect, it } from 'vitest';
import { createAppStore } from '@/store';
import { createSnapping } from './toolSnapping';

describe('ajuste a la cuadrícula con la preferencia gridMm', () => {
  const NONE = { shift: false, alt: false, ctrl: false };

  it('usa la separación elegida como paso del imán', () => {
    const store = createAppStore();
    store.getState().setPref('gridMm', 5);
    store.setState({ zoom: 10, snapEnabled: true, snapGrid: true, snapObjects: false });
    const { snapOpts } = createSnapping(() => store.getState(), () => ({ zoom: 10, panX: 0, panY: 0 }));
    expect(snapOpts(NONE)?.grid).toBe(5);
  });

  it('con 0 (automática) sigue al zoom', () => {
    const store = createAppStore();
    store.getState().setPref('gridMm', 0);
    store.setState({ zoom: 10, snapEnabled: true, snapGrid: true, snapObjects: false });
    const { snapOpts } = createSnapping(() => store.getState(), () => ({ zoom: 10, panX: 0, panY: 0 }));
    expect(snapOpts(NONE)?.grid).toBe(1);
  });
});
