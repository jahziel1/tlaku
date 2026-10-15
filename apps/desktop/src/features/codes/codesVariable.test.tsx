// @vitest-environment jsdom
import { act } from 'react';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { setApi } from '@/api';
import { createMockApi } from '@/api/mock';
import type { CsvTable, Document } from '@/api/types';
import { EMPTY_CSV, useVariablesUi } from '@/features/variables/context';
import { DEFAULT_VARIABLE_TEXT } from '@/features/variables/defaults';
import { setLanguage } from '@/i18n';
import { polylineBez } from '@/lib/bezpath';
import { useStore } from '@/store';
import { audit, cleanup, click, describeViolations, mount } from '@/test/dom';
import { CodesContent } from './CodesDialog';
import { useCodesDialog } from './codesStore';
import { DEFAULT_CODE_SPEC } from './kinds';
import { applyCodeToDoc } from './placement';

const CSV: CsvTable = { headers: ['Id', 'Nombre'], rows: [['A-1', 'Ana'], ['B-2', 'Luis']], delimiter: ',' };

function setupDoc(): void {
  const doc: Document = {
    version: 2, objects: [], layers: [], device_id: null,
    variable_text: { ...DEFAULT_VARIABLE_TEXT, csv_path: '/d/p.csv', csv_row: 1, sources: [{ name: 'p', path: '/d/p.csv', on_short: 'repeat' }] },
  };
  useStore.getState().replaceDocument(doc, null);
  useVariablesUi.setState({ csv: { path: '/d/p.csv', table: CSV, error: null, loading: false }, extra: {} });
}

async function type(el: HTMLTextAreaElement | HTMLInputElement, value: string): Promise<void> {
  const proto = el instanceof HTMLTextAreaElement ? HTMLTextAreaElement.prototype : HTMLInputElement.prototype;
  await act(async () => {
    Object.getOwnPropertyDescriptor(proto, 'value')!.set!.call(el, value);
    el.dispatchEvent(new Event('input', { bubbles: true }));
  });
}

/** Espera (con act) a que `cond` se cumpla; falla a los 3 s. */
async function until(cond: () => boolean): Promise<void> {
  for (let i = 0; i < 60 && !cond(); i++) {
    await act(async () => {
      await new Promise((r) => setTimeout(r, 50));
    });
  }
  expect(cond()).toBe(true);
}

const insertButton = (host: HTMLElement) => [...host.querySelectorAll('button')].find((b) => b.textContent === 'Insertar')!;

beforeEach(() => {
  setLanguage('es');
  setApi(createMockApi());
  setupDoc();
  useCodesDialog.setState({ open: true, spec: { ...DEFAULT_CODE_SPEC, data: '' } });
});
afterEach(async () => {
  await cleanup();
  useVariablesUi.setState({ csv: EMPTY_CSV, extra: {} });
});

describe('placement de un código variable', () => {
  it('guarda los ajustes del código en el objeto solo si se piden', () => {
    const paths = [polylineBez([[0, 0], [5, 0], [5, 5]], true)];
    const code = { ...DEFAULT_CODE_SPEC, data: 'x/{csv:Id}' };
    const withCode = applyCodeToDoc({ version: 2, objects: [], layers: [], device_id: null }, { paths, size: [5, 5], center: [0, 0], layer: '#00ff00', code });
    const o = withCode.doc.objects[0]!;
    expect(o.kind.type === 'vector' && o.kind.code?.data).toBe('x/{csv:Id}');
    const plain = applyCodeToDoc({ version: 2, objects: [], layers: [], device_id: null }, { paths, size: [5, 5], center: [0, 0], layer: '#00ff00' });
    expect(plain.doc.objects[0]!.kind.type === 'vector' && plain.doc.objects[0]!.kind.code).toBeUndefined();
  });
});

describe('diálogo de códigos con contenido variable', () => {
  it('muestra el contenido con la fila actual y lo inserta como código variable', async () => {
    const m = await mount(<CodesContent titleId="t" />);
    const area = m.host.querySelector('textarea') as HTMLTextAreaElement;
    await type(area, 'https://tienda.example/{csv:Id}');
    expect(m.host.textContent).toContain('Con la fila actual: https://tienda.example/B-2');
    // Esperar a que termine el debounce y el motor simulado.
    await until(() => !(insertButton(m.host) as HTMLButtonElement).disabled);
    await click(insertButton(m.host));
    const objs = useStore.getState().doc.objects;
    expect(objs).toHaveLength(1);
    const k = objs[0]!.kind;
    expect(k.type).toBe('vector');
    if (k.type === 'vector') {
      expect(k.code).toMatchObject({ kind: 'qr', data: 'https://tienda.example/{csv:Id}' });
      expect(k.paths.length).toBeGreaterThan(0);
    }
  });

  it('una columna que no existe avisa y no deja insertar', async () => {
    const m = await mount(<CodesContent titleId="t" />);
    await type(m.host.querySelector('textarea') as HTMLTextAreaElement, '{csv:Nada}');
    expect(m.host.textContent).toContain('no tiene la columna');
    const insert = [...m.host.querySelectorAll('button')].find((b) => b.textContent === 'Insertar')!;
    await click(insert);
    expect(useStore.getState().doc.objects).toHaveLength(0);
  });

  it('unas llaves sueltas (un JSON) se codifican tal cual y no son variables', async () => {
    const m = await mount(<CodesContent titleId="t" />);
    await type(m.host.querySelector('textarea') as HTMLTextAreaElement, '{"id": 7}');
    expect(m.host.textContent).not.toContain('Con la fila actual');
    await until(() => !(insertButton(m.host) as HTMLButtonElement).disabled);
    await click(insertButton(m.host));
    const k = useStore.getState().doc.objects[0]!.kind;
    expect(k.type === 'vector' && k.code).toBeUndefined();
  });

  it('el selector de columnas agrega la variable al contenido y no rompe la accesibilidad', async () => {
    const m = await mount(<CodesContent titleId="t" />);
    expect(describeViolations(await audit(m.host))).toEqual([]);
    const picker = [...m.host.querySelectorAll('select')].find((s) => [...s.options].some((o) => o.textContent?.includes('{csv:Nombre}')))!;
    const opt = [...picker.options].find((o) => o.textContent?.includes('{csv:Nombre}'))!;
    await act(async () => {
      Object.getOwnPropertyDescriptor(HTMLSelectElement.prototype, 'value')!.set!.call(picker, opt.value);
      picker.dispatchEvent(new Event('change', { bubbles: true }));
    });
    expect(useCodesDialog.getState().spec.data).toBe('{csv:Nombre}');
  });
});
