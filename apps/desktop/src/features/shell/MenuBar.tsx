import { Check } from 'lucide-react';
import { useEffect, useRef, useState, type KeyboardEvent } from 'react';
import { MENUS, type MenuDef } from '@/commands';
import { useCommand } from '@/commands/useCommand';
import { t } from '@/i18n';

function MenuItem({ id, onDone }: { id: string; onDone: () => void }) {
  const cmd = useCommand(id);
  const hasCheck = cmd.checked;
  return (
    <button
      type="button"
      role="menuitem"
      className="menu__item"
      aria-disabled={!cmd.enabled}
      data-checked={hasCheck}
      title={cmd.reason ?? undefined}
      onClick={() => {
        if (!cmd.enabled) return;
        onDone();
        cmd.run();
      }}
    >
      <span className="menu__check" aria-hidden="true">{hasCheck && <Check size={13} />}</span>
      <span className="menu__label">{cmd.label}</span>
      {cmd.shortcut && <kbd className="kbd">{cmd.shortcut}</kbd>}
    </button>
  );
}

/** Barra de menús (Archivo, Editar...). Todo sale del registro de comandos. */
export function MenuBar() {
  const [openId, setOpenId] = useState<string | null>(null);
  const rootRef = useRef<HTMLElement>(null);
  const triggers = useRef(new Map<string, HTMLButtonElement>());

  useEffect(() => {
    if (!openId) return;
    const close = (e: PointerEvent) => {
      if (!rootRef.current?.contains(e.target as Node)) setOpenId(null);
    };
    window.addEventListener('pointerdown', close);
    return () => window.removeEventListener('pointerdown', close);
  }, [openId]);

  useEffect(() => {
    if (openId) rootRef.current?.querySelector<HTMLElement>('[role="menu"] [role="menuitem"]')?.focus();
  }, [openId]);

  const closeAndRestore = (id: string) => {
    setOpenId(null);
    triggers.current.get(id)?.focus();
  };

  const onMenuKey = (e: KeyboardEvent<HTMLDivElement>, menu: MenuDef) => {
    const items = [...e.currentTarget.querySelectorAll<HTMLElement>('[role="menuitem"]')];
    const i = items.indexOf(document.activeElement as HTMLElement);
    const move = (n: number) => items[(n + items.length) % items.length]?.focus();
    const idx = MENUS.findIndex((m) => m.id === menu.id);
    switch (e.key) {
      case 'ArrowDown': e.preventDefault(); move(i + 1); break;
      case 'ArrowUp': e.preventDefault(); move(i - 1); break;
      case 'Home': e.preventDefault(); move(0); break;
      case 'End': e.preventDefault(); move(items.length - 1); break;
      case 'ArrowRight': e.preventDefault(); setOpenId(MENUS[(idx + 1) % MENUS.length]!.id); break;
      case 'ArrowLeft': e.preventDefault(); setOpenId(MENUS[(idx - 1 + MENUS.length) % MENUS.length]!.id); break;
      case 'Escape': e.preventDefault(); e.stopPropagation(); closeAndRestore(menu.id); break;
      case 'Tab': setOpenId(null); break;
      default: break;
    }
  };

  return (
    <nav ref={rootRef} className="menubar" aria-label={t('menu.label')}>
      {MENUS.map((menu) => {
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
              onKeyDown={(e) => {
                if (e.key === 'ArrowDown') {
                  e.preventDefault();
                  setOpenId(menu.id);
                }
              }}
            >
              {t(menu.label)}
            </button>
            {open && (
              <div role="menu" aria-label={t(menu.label)} className="menu" onKeyDown={(e) => onMenuKey(e, menu)}>
                {menu.items.map((item, i) =>
                  item === '-' ? <div key={`sep-${i}`} role="separator" className="menu__sep" /> : <MenuItem key={item} id={item} onDone={() => closeAndRestore(menu.id)} />,
                )}
              </div>
            )}
          </div>
        );
      })}
    </nav>
  );
}
