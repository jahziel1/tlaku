import { Check } from 'lucide-react';
import { useEffect, useMemo, useRef, useState, type KeyboardEvent } from 'react';
import { commands } from '@/commands';
import { useCommand } from '@/commands/useCommand';
import { baseName, openRecent, useRecents } from '@/features/history';
import { t, locale } from '@/i18n';
import { buildMenus, type ShellMenu } from './menuModel';

const ITEM_SELECTOR = '[role="menuitem"], [role="menuitemcheckbox"]';

function CommandItem({ id, onDone }: { id: string; onDone: () => void }) {
  const cmd = useCommand(id);
  const toggles = commands.get(id).checked !== undefined;
  return (
    <button
      type="button"
      role={toggles ? 'menuitemcheckbox' : 'menuitem'}
      aria-checked={toggles ? cmd.checked : undefined}
      className="menu__item"
      aria-disabled={!cmd.enabled}
      data-checked={cmd.checked}
      title={cmd.reason ?? undefined}
      tabIndex={-1}
      onPointerEnter={(e) => e.currentTarget.focus()}
      onClick={() => {
        if (!cmd.enabled) return;
        onDone();
        cmd.run();
      }}
    >
      <span className="menu__check" aria-hidden="true">{cmd.checked && <Check size={13} />}</span>
      <span className="menu__label">{cmd.label}</span>
      {cmd.shortcut && <kbd className="kbd">{cmd.shortcut}</kbd>}
      {cmd.reason && <span className="sr-only">. {cmd.reason}</span>}
    </button>
  );
}

/** Archivos recientes (Archivo > Abrir reciente). */
function RecentItems({ onDone }: { onDone: () => void }) {
  const paths = useRecents((s) => s.paths);
  const clear = useRecents((s) => s.clear);
  return (
    <div role="group" aria-label={t('history.menu.recent')}>
      <div className="menu__heading" aria-hidden="true">{t('history.menu.recent')}</div>
      {paths.length === 0 && (
        <button type="button" role="menuitem" className="menu__item" aria-disabled="true" tabIndex={-1}>
          <span className="menu__check" aria-hidden="true" />
          <span className="menu__label">{t('history.menu.noRecent')}</span>
        </button>
      )}
      {paths.map((path) => (
        <button
          key={path}
          type="button"
          role="menuitem"
          className="menu__item"
          title={path}
          tabIndex={-1}
          onPointerEnter={(e) => e.currentTarget.focus()}
          onClick={() => {
            onDone();
            void openRecent(path);
          }}
        >
          <span className="menu__check" aria-hidden="true" />
          <span className="menu__label menu__label--path">{baseName(path)}</span>
        </button>
      ))}
      {paths.length > 0 && (
        <button
          type="button"
          role="menuitem"
          className="menu__item"
          tabIndex={-1}
          onPointerEnter={(e) => e.currentTarget.focus()}
          onClick={() => {
            clear();
            onDone();
          }}
        >
          <span className="menu__check" aria-hidden="true" />
          <span className="menu__label">{t('history.menu.clearRecent')}</span>
        </button>
      )}
    </div>
  );
}

/** Barra de menús (Archivo, Editar...). Todo sale del registro de comandos. */
export function MenuBar() {
  const [openId, setOpenId] = useState<string | null>(null);
  const rootRef = useRef<HTMLElement>(null);
  const triggers = useRef(new Map<string, HTMLButtonElement>());
  const menus = useMemo<ShellMenu[]>(() => buildMenus(commands), []);

  useEffect(() => {
    if (!openId) return;
    const close = (e: PointerEvent) => {
      if (!rootRef.current?.contains(e.target as Node)) setOpenId(null);
    };
    window.addEventListener('pointerdown', close);
    return () => window.removeEventListener('pointerdown', close);
  }, [openId]);

  // Al abrir un menú el foco pasa a su primer elemento.
  useEffect(() => {
    if (openId) rootRef.current?.querySelector<HTMLElement>(`[role="menu"] ${ITEM_SELECTOR}`)?.focus();
  }, [openId]);

  // F10 lleva el foco a la barra de menús, como en las aplicaciones de escritorio.
  useEffect(() => {
    const onKey = (e: globalThis.KeyboardEvent) => {
      if (e.key !== 'F10' || e.defaultPrevented || e.ctrlKey || e.metaKey || e.altKey || e.shiftKey) return;
      e.preventDefault();
      rootRef.current?.querySelector<HTMLElement>('.menubar__trigger')?.focus();
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, []);

  const closeAndRestore = (id: string) => {
    setOpenId(null);
    triggers.current.get(id)?.focus();
  };

  const neighbor = (id: string, step: number) => menus[(menus.findIndex((m) => m.id === id) + step + menus.length) % menus.length]!.id;

  const onMenuKey = (e: KeyboardEvent<HTMLDivElement>, menu: ShellMenu) => {
    const items = [...e.currentTarget.querySelectorAll<HTMLElement>(ITEM_SELECTOR)];
    const i = items.indexOf(document.activeElement as HTMLElement);
    const move = (n: number) => items[(n + items.length) % items.length]?.focus();
    switch (e.key) {
      case 'ArrowDown': e.preventDefault(); move(i + 1); break;
      case 'ArrowUp': e.preventDefault(); move(i - 1); break;
      case 'Home': e.preventDefault(); move(0); break;
      case 'End': e.preventDefault(); move(items.length - 1); break;
      case 'ArrowRight': e.preventDefault(); setOpenId(neighbor(menu.id, 1)); break;
      case 'ArrowLeft': e.preventDefault(); setOpenId(neighbor(menu.id, -1)); break;
      case 'Escape': e.preventDefault(); e.stopPropagation(); closeAndRestore(menu.id); break;
      case 'Tab': setOpenId(null); break;
      default:
        // Escribir una letra salta al siguiente elemento que empiece con ella.
        if (e.key.length === 1 && !e.ctrlKey && !e.metaKey && !e.altKey) {
          const ch = e.key.toLocaleLowerCase(locale());
          const order = [...items.slice(i + 1), ...items.slice(0, i + 1)];
          order.find((el) => el.textContent?.trim().toLocaleLowerCase(locale()).startsWith(ch))?.focus();
        }
    }
  };

  const onTriggerKey = (e: KeyboardEvent<HTMLButtonElement>, menu: ShellMenu) => {
    const focusTrigger = (id: string) => triggers.current.get(id)?.focus();
    switch (e.key) {
      case 'ArrowDown':
      case 'ArrowUp':
        e.preventDefault();
        setOpenId(menu.id);
        break;
      case 'ArrowRight': e.preventDefault(); focusTrigger(neighbor(menu.id, 1)); break;
      case 'ArrowLeft': e.preventDefault(); focusTrigger(neighbor(menu.id, -1)); break;
      default: break;
    }
  };

  return (
    <nav ref={rootRef} className="menubar" aria-label={t('menu.label')}>
      {menus.map((menu) => {
        const open = openId === menu.id;
        return (
          <div key={menu.id} className="menubar__slot">
            <button
              ref={(el) => {
                if (el) triggers.current.set(menu.id, el);
              }}
              type="button"
              className="menubar__trigger"
              aria-haspopup="menu"
              aria-expanded={open}
              onClick={() => setOpenId(open ? null : menu.id)}
              onPointerEnter={() => openId && setOpenId(menu.id)}
              onKeyDown={(e) => onTriggerKey(e, menu)}
            >
              {t(menu.label)}
            </button>
            {open && (
              <div role="menu" aria-label={t(menu.label)} className="menu" onKeyDown={(e) => onMenuKey(e, menu)}>
                {menu.items.map((item, i) => {
                  if (item.kind === 'separator') return <div key={`sep-${i}`} role="separator" className="menu__sep" />;
                  if (item.kind === 'recent') return <RecentItems key="recent" onDone={() => closeAndRestore(menu.id)} />;
                  return <CommandItem key={item.id} id={item.id} onDone={() => closeAndRestore(menu.id)} />;
                })}
              </div>
            )}
          </div>
        );
      })}
    </nav>
  );
}
