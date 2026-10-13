import { ChevronRight } from 'lucide-react';
import { useEffect, useLayoutEffect, useMemo, useRef, useState, type KeyboardEvent } from 'react';
import { createPortal } from 'react-dom';
import { commands } from '@/commands';
import { convertTextToPaths } from '@/features/text/convert';
import { t, type I18nKey } from '@/i18n';
import { isMac } from '@/lib/platform';
import { useStore } from '@/store';
import './contextmenu.css';
import { buildContextMenu, type MenuEntry } from './model';
import { closeContextMenu, useContextMenu } from './uiStore';

const WIDTH = 232;

function Panel({ entries, onRun, label, submenu }: { entries: MenuEntry[]; onRun: (e: MenuEntry) => void; label: string; submenu?: boolean }) {
  const ref = useRef<HTMLDivElement>(null);
  const [open, setOpen] = useState<string | null>(null);
  const mac = isMac();
  const state = useStore.getState();

  useEffect(() => {
    if (!submenu) ref.current?.querySelector<HTMLElement>('[role="menuitem"]')?.focus();
  }, [submenu]);

  const items = () => [...(ref.current?.querySelectorAll<HTMLElement>(':scope > [role="menuitem"]') ?? [])];
  const onKeyDown = (e: KeyboardEvent) => {
    const els = items();
    const i = els.indexOf(document.activeElement as HTMLElement);
    if (e.key === 'ArrowDown' || e.key === 'ArrowUp' || e.key === 'Home' || e.key === 'End') {
      e.preventDefault();
      e.stopPropagation();
      const n = e.key === 'Home' ? 0 : e.key === 'End' ? els.length - 1 : (i + (e.key === 'ArrowDown' ? 1 : -1) + els.length) % els.length;
      els[n]?.focus();
    } else if (e.key === 'ArrowRight' && els[i]?.getAttribute('aria-haspopup') === 'menu') {
      e.preventDefault();
      e.stopPropagation();
      els[i]!.click();
    } else if (e.key === 'ArrowLeft' && submenu) {
      e.preventDefault();
      e.stopPropagation();
      (ref.current?.parentElement?.querySelector(':scope > [aria-haspopup="menu"][aria-expanded="true"]') as HTMLElement | null)?.focus();
    }
  };

  return (
    <div ref={ref} className={submenu ? 'ctxmenu ctxmenu--sub' : 'ctxmenu'} role="menu" aria-label={label} onKeyDown={onKeyDown}>
      {entries.map((e, k) => {
        if (e.kind === 'separator') return <div key={`sep-${k}`} className="ctxmenu__sep" role="separator" />;
        if (e.kind === 'submenu') {
          const expanded = open === e.id;
          return (
            <div key={e.id} className="ctxmenu__group" onMouseEnter={() => setOpen(e.id)} onMouseLeave={() => setOpen((cur) => (cur === e.id ? null : cur))}>
              <button type="button" role="menuitem" className="ctxmenu__item" aria-haspopup="menu" aria-expanded={expanded} onClick={() => setOpen(e.id)}>
                <span className="ctxmenu__text">{t(e.label)}</span>
                <ChevronRight size={14} aria-hidden="true" />
              </button>
              {expanded && <Panel entries={e.items} onRun={onRun} label={t(e.label)} submenu />}
            </div>
          );
        }
        const reason: true | I18nKey = e.kind === 'command' ? commands.availability(e.id, state) : (e.enabled ?? true);
        const checked = e.kind === 'command' ? commands.checked(e.id, state) : false;
        const text = e.kind === 'command' ? t(commands.label(e.id, state)) : t(e.label);
        const hint = e.kind === 'command' ? commands.shortcutLabel(e.id, mac) : '';
        return (
          <button
            key={e.kind === 'command' ? e.id : e.id}
            type="button"
            role={checked ? 'menuitemcheckbox' : 'menuitem'}
            aria-checked={checked || undefined}
            className="ctxmenu__item"
            aria-disabled={reason !== true || undefined}
            title={reason === true ? undefined : t(reason)}
            onClick={() => reason === true && onRun(e)}
          >
            <span className="ctxmenu__text">{text}</span>
            {hint && <span className="kbd">{hint}</span>}
          </button>
        );
      })}
    </div>
  );
}

/**
 * Menú contextual del lienzo (clic derecho). Se cierra con Esc, con un clic fuera, al cambiar el
 * tamaño de la ventana o al ejecutar una entrada. Las acciones salen del registro de comandos, así
 * que etiquetas, atajos y motivos de "no disponible" son los mismos que los de los menús.
 */
export function ContextMenuHost() {
  const menu = useContextMenu((s) => s.menu);
  const ref = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState({ left: 0, top: 0 });

  const entries = useMemo(
    () =>
      menu
        ? buildContextMenu(useStore.getState(), menu.onObject, {
            editText: (id) => {
              const s = useStore.getState();
              s.select([id]);
              s.requestTextFocus();
            },
            textToPaths: (id) => void convertTextToPaths(id),
            has: (id) => commands.has(id),
          })
        : [],
    [menu],
  );

  // Se coloca en el clic y, si no cabe, se corre hacia adentro de la ventana.
  useLayoutEffect(() => {
    if (!menu) return;
    const h = ref.current?.offsetHeight ?? 0;
    setPos({
      left: Math.max(4, Math.min(menu.x, window.innerWidth - WIDTH - 4)),
      top: Math.max(4, Math.min(menu.y, window.innerHeight - h - 4)),
    });
  }, [menu, entries]);

  useEffect(() => {
    if (!menu) return;
    const down = (e: PointerEvent) => {
      if (!ref.current?.contains(e.target as Node)) closeContextMenu();
    };
    const key = (e: globalThis.KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.preventDefault();
        e.stopPropagation();
        closeContextMenu();
      }
    };
    document.addEventListener('pointerdown', down, true);
    window.addEventListener('keydown', key, true);
    window.addEventListener('resize', closeContextMenu);
    window.addEventListener('blur', closeContextMenu);
    window.addEventListener('wheel', closeContextMenu, { passive: true });
    return () => {
      document.removeEventListener('pointerdown', down, true);
      window.removeEventListener('keydown', key, true);
      window.removeEventListener('resize', closeContextMenu);
      window.removeEventListener('blur', closeContextMenu);
      window.removeEventListener('wheel', closeContextMenu);
    };
  }, [menu]);

  if (!menu || entries.length === 0) return null;
  const run = (e: MenuEntry) => {
    closeContextMenu();
    if (e.kind === 'command') void commands.run(e.id);
    else if (e.kind === 'action') e.run();
  };
  return createPortal(
    <div ref={ref} className="ctxmenu-host" data-flip={pos.left + WIDTH * 2 > window.innerWidth || undefined} style={{ left: pos.left, top: pos.top, width: WIDTH }}>
      <Panel entries={entries} onRun={run} label={t('canvas.ctx.label')} />
    </div>,
    document.body,
  );
}
