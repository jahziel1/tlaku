import type { I18nKey } from '@/i18n';
import type { AppState } from '@/store';
import { selectSelectedObjects } from '@/store';

/** Una fila del menú contextual. */
export type MenuEntry =
  | { kind: 'command'; id: string }
  /** Acción propia del menú (no es un comando del registro). */
  | { kind: 'action'; id: string; label: I18nKey; run: () => void; enabled?: true | I18nKey }
  | { kind: 'separator' }
  | { kind: 'submenu'; id: string; label: I18nKey; items: MenuEntry[] };

const cmd = (id: string): MenuEntry => ({ kind: 'command', id });
const SEP: MenuEntry = { kind: 'separator' };

export interface MenuHooks {
  /** Pasa a editar el texto `id` (panel de texto). */
  editText: (id: string) => void;
  /** Convierte el texto `id` en trayectos. */
  textToPaths: (id: string) => void;
  has: (commandId: string) => boolean;
}

/**
 * Entradas del menú contextual. Con objetos seleccionados trae las acciones sobre ellos (cortar,
 * copiar, agrupar, bloquear, orden, voltear...); sobre el fondo, las del documento (pegar, seleccionar,
 * ajustar la vista). Solo se listan comandos que existen en el registro y las acciones propias de un
 * objeto (editar texto, convertir a trayectos) aparecen solo cuando hay uno solo seleccionado.
 */
export function buildContextMenu(s: AppState, onObject: boolean, hooks: MenuHooks): MenuEntry[] {
  const picked = onObject ? selectSelectedObjects(s) : [];
  const out: MenuEntry[] = [];
  const add = (...entries: MenuEntry[]) => out.push(...entries);

  if (picked.length > 0) {
    const only = picked.length === 1 ? picked[0]! : null;
    if (only?.kind.type === 'text') {
      add({ kind: 'action', id: 'ctx.editText', label: 'canvas.ctx.editText', run: () => hooks.editText(only.id) });
      if (only.kind.outline.length > 0) {
        add({ kind: 'action', id: 'ctx.textToPaths', label: 'canvas.ctx.textToPaths', run: () => hooks.textToPaths(only.id) });
      }
      add(SEP);
    }
    if (only?.kind.type === 'image' && hooks.has('tools.traceImage')) add(cmd('tools.traceImage'), SEP);
    add(cmd('edit.cut'), cmd('edit.copy'), cmd('edit.copySvg'), cmd('edit.paste'), cmd('edit.duplicate'), cmd('edit.delete'), SEP);
    add(cmd('tools.saveComponent'), SEP);
    add(cmd('edit.group'), cmd('edit.ungroup'), cmd('arrange.lock'), SEP);
    add(
      {
        kind: 'submenu', id: 'ctx.order', label: 'canvas.ctx.order',
        items: [cmd('arrange.order.front'), cmd('arrange.order.forward'), cmd('arrange.order.backward'), cmd('arrange.order.back')],
      },
      {
        kind: 'submenu', id: 'ctx.transform', label: 'canvas.ctx.transform',
        items: [
          cmd('arrange.flip.h'), cmd('arrange.flip.v'), SEP,
          cmd('arrange.rotate.ccw90'), cmd('arrange.rotate.cw90'), cmd('arrange.twoPoint'), SEP,
          cmd('arrange.mirrorLine'),
        ],
      },
      {
        kind: 'submenu', id: 'ctx.align', label: 'canvas.ctx.align',
        items: [
          cmd('arrange.align.left'), cmd('arrange.align.hcenter'), cmd('arrange.align.right'),
          cmd('arrange.align.top'), cmd('arrange.align.vcenter'), cmd('arrange.align.bottom'), SEP,
          cmd('arrange.centerOnBed'),
        ],
      },
      {
        kind: 'submenu', id: 'ctx.select', label: 'canvas.ctx.select',
        items: [cmd('edit.selectInvert'), cmd('edit.selectLayer'), cmd('edit.selectOpen'), cmd('edit.selectContained'), cmd('edit.selectSmaller')],
      },
    );
    if (only && hooks.has('arrange.offset')) add(SEP, cmd('arrange.offset'));
  } else {
    add(cmd('edit.paste'), cmd('edit.pasteInPlace'), SEP, cmd('edit.selectAll'), cmd('edit.selectInvert'), SEP);
    if (s.doc.objects.some((o) => o.locked)) add(cmd('arrange.unlockAll'), SEP);
    add(cmd('view.fitBed'), cmd('view.fitContent'));
  }
  // Solo comandos que existen y sin separadores sobrantes (al inicio, al final o dobles).
  return tidy(out.filter((e) => e.kind !== 'command' || hooks.has(e.id)).map((e) => (e.kind === 'submenu' ? { ...e, items: tidy(e.items.filter((i) => i.kind !== 'command' || hooks.has(i.id))) } : e)));
}

function tidy(entries: MenuEntry[]): MenuEntry[] {
  const out: MenuEntry[] = [];
  for (const e of entries) {
    if (e.kind === 'separator' && (out.length === 0 || out[out.length - 1]!.kind === 'separator')) continue;
    out.push(e);
  }
  while (out.length > 0 && out[out.length - 1]!.kind === 'separator') out.pop();
  return out;
}
