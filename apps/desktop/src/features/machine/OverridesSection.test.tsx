import { renderToString } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import { setApi } from '@/api';
import { createMockApi } from '@/api/mock';
import { useStore } from '@/store';
import { OverridesSection } from './OverridesSection';

describe('ajustes en vivo', () => {
  it('sin máquina conectada los controles están deshabilitados y dicen por qué', () => {
    setApi(createMockApi({ latencyMs: 0 }));
    const h = renderToString(<OverridesSection />);
    expect(h).toContain('Velocidad');
    expect(h).toContain('Potencia');
    expect(h).toContain('disabled');
    expect(h).toContain(useStore.getState().connection.connected ? '' : 'Conecta');
  });
});
