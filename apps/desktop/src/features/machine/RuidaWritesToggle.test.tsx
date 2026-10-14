// @vitest-environment jsdom
import { act } from 'react';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { setLanguage } from '@/i18n';
import { getPref, useStore } from '@/store';
import { cleanup, click, mount } from '@/test/dom';
import { phraseMatches, RuidaWritesToggle } from './RuidaWritesToggle';

beforeEach(() => {
  setLanguage('es');
  useStore.getState().setPref('ruidaExperimentalWrites', false);
});
afterEach(cleanup);

const enable = () => [...document.querySelectorAll('button')].find((b) => b.textContent === 'Activar')!;

async function type(el: HTMLInputElement, value: string) {
  await act(async () => {
    Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')!.set!.call(el, value);
    el.dispatchEvent(new Event('input', { bubbles: true }));
  });
}

describe('preferencia «Escrituras experimentales en Ruida»', () => {
  it('está apagada de fábrica', () => {
    expect(getPref('ruidaExperimentalWrites')).toBe(false);
  });

  it('la frase se reconoce sin importar mayúsculas ni acentos', () => {
    for (const ok of ['ENTIENDO', ' entiendo ', 'Entiendo']) expect(phraseMatches(ok)).toBe(true);
    for (const bad of ['', 'si', 'ENTIENDOS']) expect(phraseMatches(bad)).toBe(false);
  });

  it('encenderla muestra el aviso y no se activa hasta escribir ENTIENDO o marcar la casilla', async () => {
    await mount(<RuidaWritesToggle />);
    await click(document.querySelector('.ruida-writes input')!);
    expect(document.body.textContent).toContain('solo se probaron contra un emulador');
    expect(getPref('ruidaExperimentalWrites')).toBe(false);
    expect(enable().disabled).toBe(true);
    await type(document.querySelector<HTMLInputElement>('.field input[type="text"]')!, 'entiendo');
    expect(enable().disabled).toBe(false);
    await click(enable());
    expect(getPref('ruidaExperimentalWrites')).toBe(true);
    // Apagarla es inmediato.
    await click(document.querySelector('.ruida-writes input')!);
    expect(getPref('ruidaExperimentalWrites')).toBe(false);
  });

  it('también se activa con la casilla y cancelar la deja apagada', async () => {
    await mount(<RuidaWritesToggle />);
    await click(document.querySelector('.ruida-writes input')!);
    await click([...document.querySelectorAll('button')].find((b) => b.textContent === 'Cancelar')!);
    expect(getPref('ruidaExperimentalWrites')).toBe(false);
    await click(document.querySelector('.ruida-writes input')!);
    await click(document.querySelector('.rset-confirm input')!);
    await click(enable());
    expect(getPref('ruidaExperimentalWrites')).toBe(true);
  });
});
