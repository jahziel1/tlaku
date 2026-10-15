import { beforeEach, describe, expect, it } from 'vitest';
import { setApi } from '@/api';
import { createMockApi } from '@/api/mock';
import { sampleDevices } from '@/api/mock/sample';
import { usePrompts } from '@/features/history/prompt';
import { makeLayer } from '@/lib/layers';
import { useStore } from '@/store';
import { isBadRotaryTest, openRotaryTestAsNewDocument } from './DeviceActions';

const device = () => {
  const d = sampleDevices()[0]!;
  return { ...d, rotary: { ...d.rotary, enabled: true, object_diameter: 80 } };
};
const lastNotice = () => useStore.getState().notices.at(-1);

beforeEach(() => {
  setApi(createMockApi({ latencyMs: 0 }));
  useStore.getState().replaceDocument({ version: 2, objects: [], layers: [makeLayer('#ff3b4a', 'Corte')], device_id: 'falcon2-22w' }, null);
});

describe('probar una vuelta', () => {
  it('abre la línea de una vuelta como documento nuevo, con la máquina del documento', async () => {
    expect(await openRotaryTestAsNewDocument(device(), 1500, 10)).toBe(true);
    const s = useStore.getState();
    expect(s.doc.objects.map((o) => o.id)).toEqual(['prueba-una-vuelta']);
    expect(s.doc.device_id).toBe('falcon2-22w');
    expect(s.filePath).toBeNull();
    expect(lastNotice()?.level).toBe('success');
  });

  it('con cambios sin guardar abre la prueba en otra pestaña y no toca el documento', async () => {
    useStore.getState().updateLayer('#ff3b4a', { power: 10 });
    const doc = useStore.getState().doc;
    const first = useStore.getState().activeTabId;
    const tabs = useStore.getState().tabs.length;
    expect(await openRotaryTestAsNewDocument(device(), 1500, 10)).toBe(true);
    expect(usePrompts.getState().current).toBeNull();
    expect(useStore.getState().tabs).toHaveLength(tabs + 1);
    useStore.getState().activateTab(first);
    expect(useStore.getState().doc).toBe(doc);
  });

  it('un error del motor avisa y no toca el documento', async () => {
    const doc = useStore.getState().doc;
    const off = { ...device(), rotary: { ...device().rotary, enabled: false } };
    expect(await openRotaryTestAsNewDocument(off, 1500, 10)).toBe(false);
    expect(lastNotice()?.level).toBe('error');
    expect(useStore.getState().doc).toBe(doc);
  });

  it('valida la velocidad y la potencia de la prueba', () => {
    expect(isBadRotaryTest(1500, 10)).toBe(false);
    expect(isBadRotaryTest(0, 10)).toBe(true);
    expect(isBadRotaryTest(1500, 0)).toBe(true);
    expect(isBadRotaryTest(1500, 101)).toBe(true);
    expect(isBadRotaryTest(Number.NaN, 10)).toBe(true);
  });
});
