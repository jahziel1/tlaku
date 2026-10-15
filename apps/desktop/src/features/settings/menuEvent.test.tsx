// @vitest-environment jsdom
import { act } from 'react';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { setApi } from '@/api';
import { createMockApi } from '@/api/mock';
import type { EventName } from '@/api/types';
import { cleanup, mount } from '@/test/dom';
import { SettingsHost } from './index';
import { useSettingsUi } from './uiStore';

type Handler = (payload: never) => void;
const handlers = new Map<EventName, Handler>();
let unlistened = 0;

beforeEach(() => {
  handlers.clear();
  unlistened = 0;
  const mock = createMockApi({ latencyMs: 0 });
  setApi({
    ...mock,
    on: (event, handler) => {
      handlers.set(event, handler as Handler);
      return Promise.resolve(() => void unlistened++);
    },
  });
  useSettingsUi.getState().closeSettings();
});
afterEach(async () => {
  await cleanup();
  useSettingsUi.getState().closeSettings();
});

describe('menú nativo de macOS: Settings… (Cmd+,)', () => {
  it('el evento menu://preferences abre las Preferencias', async () => {
    await mount(<SettingsHost />);
    await act(async () => {
      await Promise.resolve();
    });
    const h = handlers.get('menu://preferences');
    expect(h).toBeDefined();
    expect(useSettingsUi.getState().open).toBe(false);
    await act(async () => {
      h!(null as never);
    });
    expect(useSettingsUi.getState().open).toBe(true);
  });

  it('al desmontar se deja de escuchar', async () => {
    await mount(<SettingsHost />);
    await act(async () => {
      await Promise.resolve();
    });
    await cleanup();
    expect(unlistened).toBeGreaterThan(0);
  });
});
