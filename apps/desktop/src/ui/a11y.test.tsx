// @vitest-environment jsdom
import { useState } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { audit, cleanup, click, describeViolations, mount, press } from '@/test/dom';
import { Modal } from './Modal';
import { NumberField } from './NumberField';
import { Toolbar } from './Toolbar';
import { useReturnFocus } from './useReturnFocus';

afterEach(cleanup);

function Opener({ portal = false, onClose = () => {} }: { portal?: boolean; onClose?: () => void }) {
  const [open, setOpen] = useState(false);
  return (
    <>
      <button type="button" id="opener" onClick={() => setOpen(true)}>Abrir</button>
      {open && (
        <Modal
          labelId="t1"
          portal={portal}
          onClose={() => {
            onClose();
            setOpen(false);
          }}
          backdropClassName="bk"
          cardClassName="card"
        >
          <h2 id="t1">Título</h2>
          <input id="first" aria-label="Nombre" />
          <button type="button" id="last">Aceptar</button>
        </Modal>
      )}
    </>
  );
}

describe('Modal: auditoría, foco y teclado', () => {
  it('axe no encuentra problemas', async () => {
    const m = await mount(<Opener />);
    await click(document.getElementById('opener')!);
    expect(describeViolations(await audit(m.host))).toEqual([]);
    await m.unmount();
  });

  for (const portal of [false, true]) {
    describe(portal ? 'en un portal' : 'en línea', () => {
      it('mueve el foco al abrir, lo atrapa con Tab y Mayús+Tab', async () => {
        const m = await mount(<Opener portal={portal} />);
        const opener = document.getElementById('opener')!;
        opener.focus();
        await click(opener);
        const first = document.getElementById('first')!;
        const last = document.getElementById('last')!;
        expect(document.activeElement).toBe(first);
        last.focus();
        await press('Tab');
        expect(document.activeElement).toBe(first);
        await press('Tab', { shiftKey: true });
        expect(document.activeElement).toBe(last);
        await m.unmount();
      });

      it('Esc cierra y el foco vuelve al control que lo abrió', async () => {
        const onClose = vi.fn();
        const m = await mount(<Opener portal={portal} onClose={onClose} />);
        const opener = document.getElementById('opener')!;
        opener.focus();
        await click(opener);
        expect(document.querySelector('[role="dialog"]')).not.toBeNull();
        await press('Escape');
        expect(onClose).toHaveBeenCalledTimes(1);
        expect(document.querySelector('[role="dialog"]')).toBeNull();
        expect(document.activeElement).toBe(opener);
        await m.unmount();
      });
    });
  }
});

describe('Toolbar: una parada de Tab y flechas', () => {
  it('solo el elemento activo está en el orden de tabulación y las flechas recorren la barra', async () => {
    const m = await mount(
      <Toolbar label="Herramientas" orientation="vertical">
        <button type="button">Uno</button>
        <button type="button">Dos</button>
        <button type="button">Tres</button>
      </Toolbar>,
    );
    const [a, b, c] = [...m.host.querySelectorAll('button')];
    expect([a!.tabIndex, b!.tabIndex, c!.tabIndex]).toEqual([0, -1, -1]);
    a!.focus();
    await press('ArrowDown');
    expect(document.activeElement).toBe(b);
    expect([a!.tabIndex, b!.tabIndex, c!.tabIndex]).toEqual([-1, 0, -1]);
    await press('End');
    expect(document.activeElement).toBe(c);
    await press('ArrowDown');
    expect(document.activeElement).toBe(a);
    await press('ArrowUp');
    expect(document.activeElement).toBe(c);
    expect(describeViolations(await audit(m.host))).toEqual([]);
    await m.unmount();
  });
});

describe('NumberField: teclado y lector de pantalla', () => {
  it('las flechas cambian el valor, Mayús lo multiplica, Esc revierte, Enter confirma', async () => {
    const onCommit = vi.fn();
    const m = await mount(<NumberField label="Ancho" value={10} step={1} min={0} max={100} onCommit={onCommit} />);
    const input = m.host.querySelector('input')!;
    expect(input.getAttribute('role')).toBe('spinbutton');
    expect(input.getAttribute('aria-valuenow')).toBe('10');
    input.focus();
    await press('ArrowUp');
    expect(input.value).toBe('11');
    await press('ArrowUp', { shiftKey: true });
    expect(input.value).toBe('21');
    await press('ArrowDown');
    await press('ArrowDown');
    expect(input.value).toBe('19');
    await press('Escape');
    expect(input.value).toBe('10');
    await press('ArrowUp');
    await press('Enter');
    expect(onCommit).toHaveBeenCalledWith(11);
    expect(describeViolations(await audit(m.host))).toEqual([]);
    await m.unmount();
  });
});

describe('useReturnFocus', () => {
  function Panel() {
    useReturnFocus();
    return <section aria-label="Panel"><button type="button" id="inside">Dentro</button></section>;
  }
  function Host() {
    const [open, setOpen] = useState(false);
    return (
      <>
        <button type="button" id="opener" onClick={() => setOpen(!open)}>Abrir</button>
        {open && <Panel />}
      </>
    );
  }
  it('devuelve el foco al abridor cuando el panel se cierra con el foco dentro', async () => {
    const m = await mount(<Host />);
    const opener = document.getElementById('opener')!;
    opener.focus();
    await click(opener);
    document.getElementById('inside')!.focus();
    await click(opener);
    expect(document.activeElement).toBe(opener);
    await m.unmount();
  });
});
