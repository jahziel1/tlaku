// @vitest-environment jsdom
import { act } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setApi } from '@/api';
import { createMockApi } from '@/api/mock';
import type { Document } from '@/api/types';
import { ALL_COMMANDS as commands } from '@/commands';
import { saveProject } from '@/features/history/files';
import { PromptHost } from '@/features/history/PromptHost';
import { usePrompts } from '@/features/history/prompt';
import { setLanguage } from '@/i18n';
import { useStore } from '@/store';
import { emptyDocument } from '@/store/document';
import { polylineBez } from '@/lib/bezpath';
import { audit, cleanup, click, describeViolations, mount } from '@/test/dom';
import { resetTabs } from '@/test/tabs';
import { resetCaptureWarning } from './actions';
import { VersionsHost } from './VersionsHost';
import { useVersionsUi } from './versionsUi';

vi.setConfig({ testTimeout: 20000 });

const s = () => useStore.getState();
const settle = (ms = 0) =>
  act(async () => {
    await new Promise((r) => setTimeout(r, ms));
  });

function obj(id: string): Document['objects'][number] {
  return { id, layer: '#ff0000', transform: [1, 0, 0, 1, 0, 0], kind: { type: 'vector', paths: [polylineBez([[0, 0], [10, 0], [10, 10], [0, 10]], true)] }, locked: false, hidden: false, group: null };
}

const dialog = () => document.querySelector<HTMLElement>('.ver-card');
const button = (text: string) => {
  const el = [...document.querySelectorAll<HTMLButtonElement>('button')].find((b) => b.textContent?.trim() === text);
  if (!el) throw new Error(`No hay un botón «${text}»`);
  return el;
};

let api: ReturnType<typeof createMockApi>;
let path: string;

/** Un proyecto guardado con dos versiones anteriores y un estado actual distinto. */
async function seed() {
  const setObjects = (...ids: string[]) => s().applyDocChange('history.import', (d) => ({ ...d, objects: ids.map(obj) }));
  setObjects('a');
  await saveProject(true);
  path = s().filePath!;
  await api.versionsSnapshot({ path, doc: s().doc, kind: 'manual', label: 'Primer corte' });
  setObjects('a', 'b');
  await api.versionsSnapshot({ path, doc: s().doc, kind: 'auto' });
  setObjects('c', 'd');
}

async function openDialog(forPath: string | null = path) {
  await mount(
    <>
      <VersionsHost />
      <PromptHost />
    </>,
  );
  await act(async () => useVersionsUi.getState().openFor(forPath));
  await settle(20);
}

beforeEach(() => {
  setLanguage('es');
  resetCaptureWarning();
  api = createMockApi({ latencyMs: 0 });
  setApi(api);
  resetTabs();
  useVersionsUi.setState({ open: false, path: null, nameOpen: false, changes: 0 });
});
afterEach(async () => {
  await cleanup();
  usePrompts.setState({ current: null, queue: [] });
});

describe('diálogo «Historial de versiones»', () => {
  it('lista las versiones de la más nueva a la más vieja, con tipo y resumen', async () => {
    await seed();
    await openDialog();
    expect(dialog()).not.toBeNull();
    const rows = [...document.querySelectorAll('.ver-row')];
    expect(rows).toHaveLength(2);
    expect(rows[0]!.textContent).toContain('Automática');
    expect(rows[1]!.textContent).toContain('Primer corte');
    expect(rows[1]!.textContent).toContain('Con nombre');
    expect(rows[1]!.textContent).toContain('1 objeto');
    expect(document.querySelector('.ver-foot')!.textContent).toContain('2 versiones');
  });

  it('elegir una versión muestra la vista previa y lo que cambiaría al restaurarla', async () => {
    await seed();
    await openDialog();
    await click(document.querySelectorAll('.ver-row')[1]!);
    await settle(20);
    expect(document.querySelector('.ver-detail img.ver-thumb--large')).not.toBeNull();
    const compare = document.querySelector('.ver-compare')!.textContent!;
    expect(compare).toContain('Se perderían 2 objetos agregados después.');
    expect(compare).toContain('Volvería 1 objeto que se quitó después.');
    // Mirar no toca el documento abierto.
    expect(s().doc.objects.map((o) => o.id)).toEqual(['c', 'd']);
  });

  it('«Restaurar como pestaña nueva» abre la copia sin tocar el documento y cierra el diálogo', async () => {
    await seed();
    const tab = s().activeTabId;
    await openDialog();
    await click(document.querySelectorAll('.ver-row')[1]!);
    await settle(20);
    await click(button('Restaurar como pestaña nueva'));
    await settle(20);
    expect(s().tabs).toHaveLength(2);
    expect(s().doc.objects.map((o) => o.id)).toEqual(['a']);
    expect(s().filePath).toBeNull();
    expect(dialog()).toBeNull();
    s().activateTab(tab);
    expect(s().doc.objects.map((o) => o.id)).toEqual(['c', 'd']);
  });

  it('«Reemplazar el documento abierto» pide confirmar y deja el estado anterior como versión', async () => {
    await seed();
    await openDialog();
    await click(document.querySelectorAll('.ver-row')[1]!);
    await settle(20);
    await click(button('Reemplazar el documento abierto…'));
    await settle(20);
    // La confirmación se ve por encima del diálogo.
    expect(usePrompts.getState().current?.title).toBe('Reemplazar el documento');
    expect(s().doc.objects.map((o) => o.id)).toEqual(['c', 'd']);
    await click(button('Restaurar'));
    await settle(30);
    expect(s().doc.objects.map((o) => o.id)).toEqual(['a']);
    expect((await api.versionsList({ path })).versions.some((v) => v.kind === 'restore')).toBe(true);
    s().undo();
    expect(s().doc.objects.map((o) => o.id)).toEqual(['c', 'd']);
  });

  it('con el archivo cerrado no ofrece reemplazar, y la comparación pide abrirlo', async () => {
    await seed();
    const fileTab = s().activeTabId;
    s().newTab();
    s().removeTab(fileTab); // el archivo ya no está abierto en ninguna pestaña
    await openDialog();
    await click(document.querySelectorAll('.ver-row')[0]!);
    await settle(20);
    expect(button('Reemplazar el documento abierto…').disabled).toBe(true);
    expect(document.querySelector('.ver-compare')!.textContent).toContain('Abre el archivo en una pestaña');
    expect(button('Restaurar como pestaña nueva').disabled).toBe(false);
  });

  it('borra una versión tras confirmar', async () => {
    await seed();
    await openDialog();
    await click(document.querySelectorAll('.ver-row')[0]!);
    await settle(20);
    await click(button('Borrar esta versión'));
    await settle(20);
    await click(button('Borrar'));
    await settle(30);
    expect(document.querySelectorAll('.ver-row')).toHaveLength(1);
    expect((await api.versionsList({ path })).versions).toHaveLength(1);
  });

  it('abrir el historial de un archivo que no está abierto, desde la lista de otros proyectos', async () => {
    await seed();
    const other = '/proyectos/ajeno.tlaku';
    await api.versionsSnapshot({ path: other, doc: { ...emptyDocument(), objects: [obj('z')] }, kind: 'manual', label: 'Ajeno v1' });
    await openDialog();
    const select = document.querySelector<HTMLSelectElement>('.ver-toolbar__projects select')!;
    expect(select.textContent).toContain('ajeno.tlaku');
    await act(async () => {
      select.value = other;
      select.dispatchEvent(new Event('change', { bubbles: true }));
    });
    await settle(30);
    expect(useVersionsUi.getState().path).toBe(other);
    expect(document.querySelector('.ver-row')!.textContent).toContain('Ajeno v1');
  });

  it('un documento sin archivo explica que lo cubre la recuperación', async () => {
    await openDialog(null);
    expect(dialog()!.textContent).toContain('Este documento aún no tiene archivo');
    expect(button('Guardar versión…').disabled).toBe(true);
  });

  it('aplica los ajustes y los guarda en el motor', async () => {
    await seed();
    await openDialog();
    const details = document.querySelector<HTMLDetailsElement>('.ver-settings')!;
    details.open = true;
    const box = document.querySelector<HTMLInputElement>('.ver-settings input[type="checkbox"]')!;
    await click(box);
    expect(button('Aplicar').disabled).toBe(false);
    await click(button('Aplicar'));
    await settle(20);
    expect((await api.versionsConfigGet()).enabled).toBe(false);
    expect(useVersionsUi.getState().config.enabled).toBe(false);
  });

  it('es accesible (axe)', async () => {
    await seed();
    await openDialog();
    await click(document.querySelectorAll('.ver-row')[1]!);
    await settle(20);
    const result = await audit();
    expect(describeViolations(result)).toEqual([]);
  });
});

describe('«Guardar versión…»', () => {
  it('el comando pide el nombre y guarda el documento actual', async () => {
    await seed();
    await mount(<VersionsHost />);
    const cmd = commands.find((c) => c.id === 'versions.saveNamed')!;
    expect(cmd.enabled?.(s())).toBe(true);
    await act(async () => void cmd.run(s()));
    await settle();
    const input = document.querySelector<HTMLInputElement>('.ver-name input')!;
    expect(button('Guardar versión').disabled).toBe(true);
    await act(async () => {
      const set = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')!.set!;
      set.call(input, 'Antes de grabar');
      input.dispatchEvent(new Event('input', { bubbles: true }));
    });
    await click(button('Guardar versión'));
    await settle(30);
    expect(document.querySelector('.ver-name')).toBeNull();
    const labels = (await api.versionsList({ path })).versions.map((v) => v.label);
    expect(labels).toContain('Antes de grabar');
  });

  it('sin archivo el comando está desactivado y dice por qué', () => {
    const cmd = commands.find((c) => c.id === 'versions.saveNamed')!;
    expect(cmd.enabled?.(s())).toBe('versions.reason.noFile');
  });

  it('«Historial de versiones…» está en el menú Archivo', async () => {
    const { MENUS } = await import('@/commands/menus');
    const file = MENUS.find((m) => m.id === 'file')!;
    expect(file.items).toContain('versions.open');
    expect(file.items).toContain('versions.saveNamed');
  });
});
