// @vitest-environment jsdom
import { act } from 'react';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { setApi } from '@/api';
import { createMockApi, type MockApi } from '@/api/mock';
import type { DeviceProfile } from '@/api/types';
import { setLanguage } from '@/i18n';
import { FALLBACK_DEVICE } from '@/lib/device';
import { useStore } from '@/store';
import { audit, cleanup, click, describeViolations, mount } from '@/test/dom';
import { RuidaDocDialog } from './RuidaDocDialog';
import { RuidaSettingsDialog } from './RuidaSettingsDialog';

const ruida = (): DeviceProfile => ({
  ...FALLBACK_DEVICE,
  id: 'ruida-1',
  name: 'CO2 6040',
  kind: 'ruida',
  bed: [600, 400],
  connection: { type: 'udp', host: '192.168.1.100' },
  ruida: { magic: null, run_after_upload: false },
});

let api: MockApi;

beforeEach(async () => {
  setLanguage('es');
  useStore.getState().setPref('ruidaExperimentalWrites', true);
  api = createMockApi({ latencyMs: 0 });
  setApi(api);
  useStore.setState({ devices: [ruida()], activeDeviceId: 'ruida-1' });
  await api.connect({ port: 'virtual:ruida', baud: 115200, device: ruida() });
});

afterEach(async () => {
  await cleanup();
  api.dispose();
});

const settle = () => act(async () => new Promise<void>((r) => setTimeout(r, 20)));

/** Escribe en un campo de React (hace falta el setter nativo para que se entere). */
async function type(el: HTMLInputElement, value: string) {
  await act(async () => {
    Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')!.set!.call(el, value);
    el.dispatchEvent(new Event('input', { bubbles: true }));
  });
}

const button = (text: RegExp | string): HTMLButtonElement => {
  const found = [...document.querySelectorAll('button')].find((b) => (typeof text === 'string' ? b.textContent?.trim() === text : text.test(b.textContent ?? '')));
  if (!found) throw new Error(`no hay un botón «${String(text)}»`);
  return found;
};

const editField = (name: RegExp): HTMLInputElement => {
  const el = [...document.querySelectorAll<HTMLInputElement>('input[aria-label]')].find((i) => name.test(i.getAttribute('aria-label') ?? ''));
  if (!el) throw new Error(`no hay un campo ${String(name)}`);
  return el;
};

async function openDialog() {
  await mount(<RuidaSettingsDialog onClose={() => undefined} />);
  await settle();
}

describe('ajustes de la controladora Ruida (experimental)', () => {
  it('muestra la marca experimental y los ajustes leídos, agrupados, con su rango', async () => {
    await openDialog();
    const text = document.body.textContent ?? '';
    expect(text).toContain('EXPERIMENTAL: sin probar con una controladora real');
    expect(text).toContain('Velocidad de movimiento rápido (G0)');
    expect(text).toContain('Eje X');
    expect(text).toContain('Rotativo');
    expect(text).toContain('MEM_MAX_VELOCITY_X');
    // 200000 µm/s se muestran como 200 mm/s y el rango como 1 – 600 mm/s.
    expect(text).toContain('200 mm/s');
    expect(text).toContain('1 – 600 mm/s');
    expect(text).toContain('RDC6442S');
  });

  it('no deja revisar sin cambios válidos y marca los valores fuera de rango o que no son números', async () => {
    await openDialog();
    expect(button('Revisar los cambios').disabled).toBe(true);
    const g0 = editField(/Velocidad de movimiento rápido/);
    await type(g0, '900');
    expect(document.body.textContent).toContain('Fuera del rango permitido');
    expect(button('Revisar los cambios').disabled).toBe(true);
    await type(g0, 'abc');
    expect(document.body.textContent).toContain('Escribe un número');
    await type(g0, '150');
    expect(document.body.textContent).not.toContain('Fuera del rango');
    expect(button('Revisar los cambios').disabled).toBe(false);
  });

  it('muestra cada valor actual y nuevo y no escribe sin la casilla de confirmación', async () => {
    await openDialog();
    await type(editField(/Velocidad de movimiento rápido/), '150');
    await type(editField(/Velocidad de home/), '25');
    await click(button('Revisar los cambios'));
    await settle();
    const text = document.body.textContent ?? '';
    expect(text).toContain('Revisar los cambios');
    expect(text).toContain('200 mm/s');
    expect(text).toContain('150 mm/s');
    expect(text).toContain('20 mm/s');
    expect(text).toContain('25 mm/s');
    const confirm = document.querySelector<HTMLInputElement>('.rset-confirm input')!;
    expect(document.body.textContent).toContain('Entiendo que esto cambia la configuración de mi controladora');
    const write = button('Escribir en la controladora');
    expect(write.disabled).toBe(true);
    await click(write);
    expect((await api.ruidaSettingsRead()).settings.find((s) => s.key === 'g0_speed')!.value).toBe(200000);
    await click(confirm);
    expect(button('Escribir en la controladora').disabled).toBe(false);
  });

  it('con la casilla marcada escribe, verifica y deja un respaldo que se puede restaurar con las mismas protecciones', async () => {
    await openDialog();
    await type(editField(/Velocidad de movimiento rápido/), '150');
    await click(button('Revisar los cambios'));
    await settle();
    await click(document.querySelector('.rset-confirm input')!);
    await click(button('Escribir en la controladora'));
    await settle();
    let text = document.body.textContent ?? '';
    expect(text).toContain('Listo: se escribieron 1 ajuste(s)');
    expect(text).toContain('coincide');
    expect(text).toContain('Respaldo previo guardado en');
    expect((await api.ruidaSettingsRead()).settings.find((s) => s.key === 'g0_speed')!.value).toBe(150000);

    // Cerrar el informe y restaurar el respaldo automático.
    await click(button('Cerrar'));
    await settle();
    expect(document.body.textContent).toContain('ruida-20261014-');
    await click(button('Restaurar…'));
    await settle();
    text = document.body.textContent ?? '';
    expect(text).toContain('Restaurar un respaldo');
    expect(text).toContain('150 mm/s');
    expect(text).toContain('200 mm/s');
    const restore = button('Restaurar en la controladora');
    expect(restore.disabled).toBe(true);
    await click(document.querySelector('.rset-confirm input')!);
    await click(button('Restaurar en la controladora'));
    await settle();
    expect(document.body.textContent).toContain('Restaurado: 1 ajuste(s)');
    expect((await api.ruidaSettingsRead()).settings.find((s) => s.key === 'g0_speed')!.value).toBe(200000);
  });

  it('un error del motor al escribir se muestra y no se da por hecho', async () => {
    await openDialog();
    await type(editField(/Velocidad de movimiento rápido/), '150');
    await click(button('Revisar los cambios'));
    await settle();
    await click(document.querySelector('.rset-confirm input')!);
    // La máquina se desconecta antes de escribir.
    await api.disconnect();
    await click(button('Escribir en la controladora'));
    await settle();
    expect(document.body.textContent).toContain('No se pudo escribir');
    expect(document.body.textContent).not.toContain('Listo: se escribieron');
  });

  it('calibra un eje: propone el paso nuevo y lo carga en la tabla para revisarlo', async () => {
    await openDialog();
    const inputs = [...document.querySelectorAll<HTMLInputElement>('.rset-cal input')];
    await type(inputs[1]!, '99');
    await click(button('Calcular'));
    await settle();
    expect(document.body.textContent).toMatch(/paso propuesto 6\.?287 µm/);
    await click(button('Usar este valor'));
    expect(editField(/Paso del eje X/).value).toBe('6287');
    await click(button('Revisar los cambios'));
    await settle();
    expect(document.body.textContent).toContain('6350 µm');
    expect(document.body.textContent).toContain('6287 µm');
    // Una medida absurda se rechaza con su motivo.
    await click(button('Volver'));
    const again = [...document.querySelectorAll<HTMLInputElement>('.rset-cal input')];
    await type(again[1]!, '50');
    await click(button('Calcular'));
    await settle();
    expect(document.body.textContent).toContain('error de medida');
  });

  it('el rotativo se lee y se escribe como número sin unidad documentada', async () => {
    await openDialog();
    expect(document.body.textContent).toContain('Rotativo: diámetro');
    expect(document.body.textContent).toContain('sin unidad documentada');
    await type(editField(/Rotativo: diámetro/), '60000');
    await click(button('Revisar los cambios'));
    await settle();
    await click(document.querySelector('.rset-confirm input')!);
    await click(button('Escribir en la controladora'));
    await settle();
    expect((await api.ruidaSettingsRead()).settings.find((s) => s.key === 'rotary_diameter')!.value).toBe(60000);
  });

  it('pasa la auditoría de accesibilidad', async () => {
    await openDialog();
    const r = await audit();
    expect(describeViolations(r)).toEqual([]);
  });
});

describe('borrar y ejecutar documentos de la controladora (experimental)', () => {
  const file = { index: 1, name: 'PIEZA' };

  it('borrar pide la casilla explícita, no borra "todos" y avisa que es experimental', async () => {
    api.machine.storedDocs.push('PIEZA');
    let done = '';
    let closed = false;
    await mount(<RuidaDocDialog action="delete" file={file} onClose={() => (closed = true)} onDone={(m) => (done = m)} />);
    const text = document.body.textContent ?? '';
    expect(text).toContain('sin probar con una controladora real');
    expect(text).toContain('Nunca se borran «todos»');
    const go = button('Borrar el documento');
    expect(go.disabled).toBe(true);
    await click(go);
    expect(api.machine.storedDocs).toEqual(['PIEZA']);
    await click(document.querySelector('.rset-confirm input')!);
    await click(button('Borrar el documento'));
    await settle();
    expect(api.machine.storedDocs).toEqual([]);
    expect(done).toContain('«PIEZA» borrado');
    expect(closed).toBe(true);
  });

  it('ejecutar pasa por la lista de comprobación: con casillas obligatorias no deja empezar hasta marcarlas', async () => {
    api.machine.storedDocs.push('PIEZA');
    useStore.getState().setPref('jobChecklistRequired', true);
    useStore.getState().setPref('jobChecklistItems', 3);
    await mount(<RuidaDocDialog action="run" file={file} onClose={() => undefined} onDone={() => undefined} />);
    expect(document.body.textContent).toContain('Ejecutar un documento de la controladora');
    const boxes = [...document.querySelectorAll<HTMLInputElement>('.mc-check input')];
    expect(boxes.length).toBe(2);
    await click(document.querySelector('.rset-confirm input')!);
    expect(button('Ejecutar el documento').disabled).toBe(true);
    for (const b of boxes) await click(b);
    expect(button('Ejecutar el documento').disabled).toBe(false);
    await click(button('Ejecutar el documento'));
    await settle();
  });

  it('un nombre que ya no coincide se rechaza y se muestra el error, sin cerrar', async () => {
    api.machine.storedDocs.push('OTRO');
    let closed = false;
    await mount(<RuidaDocDialog action="delete" file={file} onClose={() => (closed = true)} onDone={() => undefined} />);
    await click(document.querySelector('.rset-confirm input')!);
    await click(button('Borrar el documento'));
    await settle();
    expect(document.body.textContent).toContain('se llama «OTRO», no «PIEZA»');
    expect(api.machine.storedDocs).toEqual(['OTRO']);
    expect(closed).toBe(false);
  });
});

describe('escrituras experimentales apagadas (lo de fábrica)', () => {
  it('con la preferencia apagada se puede leer pero nada de lo que escribe está habilitado', async () => {
    useStore.getState().setPref('ruidaExperimentalWrites', false);
    await openDialog();
    const text = document.body.textContent ?? '';
    expect(text).toContain('200 mm/s');
    expect(text).toContain('Desactivado: enciende «Escrituras experimentales en Ruida»');
    // Los campos y los botones que escriben están deshabilitados y dicen por qué.
    const g0 = editField(/Velocidad de movimiento rápido/);
    expect(g0.disabled).toBe(true);
    expect(g0.title).toContain('Escrituras experimentales en Ruida');
    expect(button('Revisar los cambios').disabled).toBe(true);
    expect(button('Calcular').disabled).toBe(true);
    // Exportar un respaldo solo lee: sigue habilitado.
    expect(button('Exportar respaldo…').disabled).toBe(false);
  });

  it('borrar y ejecutar quedan deshabilitados y la API de mock también las rechaza sin la bandera', async () => {
    useStore.getState().setPref('ruidaExperimentalWrites', false);
    api.machine.storedDocs.push('PIEZA');
    await mount(<RuidaDocDialog action="delete" file={{ index: 1, name: 'PIEZA' }} onClose={() => undefined} onDone={() => undefined} />);
    expect(button('Borrar el documento').disabled).toBe(true);
    expect(button('Borrar el documento').title).toContain('Escrituras experimentales en Ruida');
    const off = { experimental_writes: false };
    await expect(api.ruidaFileDelete({ index: 1, name: 'PIEZA', confirmed: true, ...off })).rejects.toThrow('desactivadas');
    await expect(api.ruidaFileRun({ index: 1, name: 'PIEZA', confirmed: true, ...off })).rejects.toThrow('desactivadas');
    await expect(api.ruidaSettingsApply({ changes: [{ key: 'g0_speed', value: 150000 }], confirmed: true, ...off })).rejects.toThrow('desactivadas');
    await expect(api.ruidaPrepareStore({ name: 'X', overwrite: true, ...off })).rejects.toThrow('desactivadas');
    // Elegir un nombre sin reemplazar no es una escritura.
    await expect(api.ruidaPrepareStore({ name: 'X', overwrite: false, ...off })).resolves.toBeUndefined();
    expect(api.machine.storedDocs).toEqual(['PIEZA']);
  });
});
