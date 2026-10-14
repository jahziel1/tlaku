import { Plus, Search, Trash2, X } from 'lucide-react';
import { useEffect, useId, useMemo, useRef, useState } from 'react';
import type { Point } from '@/api/types';
import { t, type I18nKey } from '@/i18n';
import { useReturnFocus } from '@/ui/useReturnFocus';
import { screenToWorld } from '@/lib/viewport';
import { useStore } from '@/store';
import { addUserArt, insertArt, loadUserArt, removeUserArt } from './actions';
import { useArtPanel } from './artStore';
import { ART_CATEGORIES, BUILTIN_ART, artName, filterArt, svgDataUrl, thumbnailSvg, userItem, type ArtItem } from './catalog';
import './art.css';

/** Cuánto hay que mover el puntero para que un clic pase a ser un arrastre. */
const DRAG_PX = 6;

/** El lienzo bajo el puntero, y el punto del documento (mm) sobre el que está. */
export function dropPoint(clientX: number, clientY: number): Point | null {
  const el = document.elementFromPoint(clientX, clientY);
  const canvas = el instanceof HTMLCanvasElement && el.classList.contains('canvas') ? el : el?.closest('.canvas-area')?.querySelector('canvas.canvas');
  if (!(canvas instanceof HTMLCanvasElement)) return null;
  const rect = canvas.getBoundingClientRect();
  const { zoom, panX, panY } = useStore.getState();
  return screenToWorld({ zoom, panX, panY }, [clientX - rect.left, clientY - rect.top]);
}

function Tile({ item }: { item: ArtItem }) {
  const [ghost, setGhost] = useState<{ x: number; y: number } | null>(null);
  const start = useRef<{ x: number; y: number; id: number } | null>(null);
  const dragging = useRef(false);
  const name = artName(item);

  const onPointerDown = (e: React.PointerEvent<HTMLButtonElement>) => {
    if (e.button !== 0) return;
    start.current = { x: e.clientX, y: e.clientY, id: e.pointerId };
    dragging.current = false;
    e.currentTarget.setPointerCapture(e.pointerId);
  };
  const onPointerMove = (e: React.PointerEvent<HTMLButtonElement>) => {
    const s = start.current;
    if (!s) return;
    if (!dragging.current && Math.hypot(e.clientX - s.x, e.clientY - s.y) > DRAG_PX) dragging.current = true;
    if (dragging.current) setGhost({ x: e.clientX, y: e.clientY });
  };
  const finish = (e: React.PointerEvent<HTMLButtonElement>, cancelled: boolean) => {
    const s = start.current;
    start.current = null;
    setGhost(null);
    if (!s) return;
    if (e.currentTarget.hasPointerCapture(s.id)) e.currentTarget.releasePointerCapture(s.id);
    if (cancelled) return;
    if (!dragging.current) {
      void insertArt(item);
      return;
    }
    dragging.current = false;
    const at = dropPoint(e.clientX, e.clientY);
    if (at) void insertArt(item, at);
  };

  return (
    <li>
      <button
        type="button"
        className="art-tile"
        title={t('art.tile.hint', { name })}
        aria-label={t('art.tile.insert', { name })}
        onPointerDown={onPointerDown}
        onPointerMove={onPointerMove}
        onPointerUp={(e) => finish(e, false)}
        onPointerCancel={(e) => finish(e, true)}
        onKeyDown={(e) => {
          if (e.key === 'Enter' || e.key === ' ') {
            e.preventDefault();
            void insertArt(item);
          }
        }}
      >
        <img src={svgDataUrl(thumbnailSvg(item.svg))} alt="" draggable={false} className="art-tile__img" />
        <span className="art-tile__name">{name}</span>
      </button>
      {ghost && <img src={svgDataUrl(thumbnailSvg(item.svg))} alt="" className="art-ghost" style={{ left: ghost.x, top: ghost.y }} />}
    </li>
  );
}

/** Panel flotante "Biblioteca de arte": formas listas para insertar con un clic o arrastrándolas al lienzo. */
export function ArtPanel() {
  useReturnFocus();
  const titleId = useId();
  const { category, query, user, error, setCategory, setQuery, setOpen } = useArtPanel();

  useEffect(() => {
    void loadUserArt();
  }, []);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape' && !(e.target instanceof HTMLInputElement)) setOpen(false);
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [setOpen]);

  const items = useMemo(() => [...BUILTIN_ART, ...user.map((u) => userItem(u.name, u.svg))], [user]);
  const shown = filterArt(items, category, query);
  const cats: { id: typeof category; label: string }[] = [
    { id: 'todo', label: t('art.cat.todo') },
    ...ART_CATEGORIES.map((c) => ({ id: c, label: t(`art.cat.${c}` as I18nKey) })),
    { id: 'usuario', label: t('art.cat.usuario') },
  ];

  return (
    <section className="art-panel" role="dialog" aria-labelledby={titleId}>
      <header className="art-head">
        <h2 id={titleId} className="art-title">{t('art.title')}</h2>
        <button type="button" className="icon-btn" title={t('art.close')} aria-label={t('art.close')} onClick={() => setOpen(false)}>
          <X size={16} aria-hidden="true" />
        </button>
      </header>
      <p className="hint art-intro">{t('art.intro')}</p>
      <label className="art-search">
        <Search size={14} aria-hidden="true" />
        <input type="search" value={query} placeholder={t('art.search')} aria-label={t('art.search')} onChange={(e) => setQuery(e.target.value)} />
      </label>
      <div className="art-cats" role="group" aria-label={t('art.categories')}>
        {cats.map((c) => (
          <button key={c.id} type="button" className="art-cat" aria-pressed={category === c.id} onClick={() => setCategory(c.id)}>
            {c.label}
          </button>
        ))}
      </div>
      <div className="art-scroll">
        {shown.length === 0 ? (
          <p className="hint art-empty">{category === 'usuario' && query === '' ? t('art.user.empty') : t('art.empty')}</p>
        ) : (
          <ul className="art-grid">
            {shown.map((i) => (
              <Tile key={i.id} item={i} />
            ))}
          </ul>
        )}
      </div>
      {error && <p className="warn-line" role="alert">{error}</p>}
      <footer className="art-foot">
        <button type="button" className="btn" onClick={() => void addUserArt()}>
          <Plus size={14} aria-hidden="true" /> {t('art.user.add')}
        </button>
        {category === 'usuario' && user.length > 0 && (
          <details className="art-manage">
            <summary>{t('art.user.manage')}</summary>
            <ul>
              {user.map((u) => (
                <li key={u.name}>
                  <span>{u.name}</span>
                  <button type="button" className="icon-btn" title={t('art.user.remove', { name: u.name })} aria-label={t('art.user.remove', { name: u.name })} onClick={() => void removeUserArt(u.name)}>
                    <Trash2 size={14} aria-hidden="true" />
                  </button>
                </li>
              ))}
            </ul>
          </details>
        )}
      </footer>
      <p className="hint art-folder">{t('art.user.folder')}</p>
    </section>
  );
}

/** Monta el panel cuando está abierto. Lo coloca el shell una sola vez. */
export function ArtPanelHost() {
  const open = useArtPanel((s) => s.open);
  return open ? <ArtPanel /> : null;
}
