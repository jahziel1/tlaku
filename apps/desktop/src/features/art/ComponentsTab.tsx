import { Pencil, Save, Trash2 } from 'lucide-react';
import { useEffect, useId, useRef, useState } from 'react';
import { t, type I18nKey } from '@/i18n';
import { useStore } from '@/store';
import { svgDataUrl, thumbnailSvg } from './catalog';
import { insertComponent, removeUserComponent, renameUserComponent, requestSaveComponent } from './componentActions';
import { cleanName, sizeLabel, type Placement } from './components';
import { useArtPanel } from './artStore';
import { useDragInsert } from './dragInsert';
import type { UserComponent } from './types';

const PLACEMENTS: readonly Placement[] = ['auto', 'original', 'center'];

/** Texto para buscar un componente: su nombre sin acentos ni mayúsculas. */
const norm = (s: string) => s.normalize('NFD').replace(/[̀-ͯ]/g, '').toLowerCase();

function Row({ c }: { c: UserComponent }) {
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState(c.name);
  const input = useRef<HTMLInputElement>(null);
  const { handlers, ghost } = useDragInsert((at) => void insertComponent(c, at ? { at } : {}));
  const size = sizeLabel(c.data);
  const thumb = c.data.thumb ? svgDataUrl(thumbnailSvg(c.data.thumb)) : null;

  useEffect(() => {
    if (editing) input.current?.select();
  }, [editing]);

  const commit = () => {
    const clean = cleanName(draft);
    setEditing(false);
    if (clean !== '' && clean !== c.name) void renameUserComponent(c.name, clean);
    else setDraft(c.name);
  };

  return (
    <li className="art-comp">
      {editing ? (
        <div className="art-comp__main">
          <span className="art-comp__img art-comp__img--empty" />
          <input
            ref={input}
            className="input art-comp__rename"
            aria-label={t('art.component.rename.aria', { name: c.name })}
            value={draft}
            maxLength={80}
            onChange={(e) => setDraft(e.target.value)}
            onBlur={commit}
            onKeyDown={(e) => {
              if (e.key === 'Enter') commit();
              if (e.key === 'Escape') {
                e.stopPropagation();
                setDraft(c.name);
                setEditing(false);
              }
            }}
          />
        </div>
      ) : (
        <button type="button" className="art-comp__main" title={t('art.component.tile.hint', { name: c.name, size })} aria-label={t('art.component.tile.insert', { name: c.name })} {...handlers}>
          {thumb ? <img src={thumb} alt="" draggable={false} className="art-comp__img" /> : <span className="art-comp__img art-comp__img--empty" />}
          <span className="art-comp__text">
            <span className="art-comp__name">{c.name}</span>
            <span className="art-comp__meta">
              {size} · {t('art.component.tile.objects', { n: c.data.objects.length })}
            </span>
          </span>
        </button>
      )}
      <button type="button" className="icon-btn" title={t('art.component.rename.button', { name: c.name })} aria-label={t('art.component.rename.button', { name: c.name })} onClick={() => { setDraft(c.name); setEditing(true); }}>
        <Pencil size={14} aria-hidden="true" />
      </button>
      <button type="button" className="icon-btn" title={t('art.component.delete.button', { name: c.name })} aria-label={t('art.component.delete.button', { name: c.name })} onClick={() => void removeUserComponent(c.name)}>
        <Trash2 size={14} aria-hidden="true" />
      </button>
      {ghost && thumb && <img src={thumb} alt="" className="art-ghost" style={{ left: ghost.x, top: ghost.y }} />}
    </li>
  );
}

/** Pestaña «Mis componentes» de la biblioteca de arte: guardar la selección, insertar, renombrar y borrar. */
export function ComponentsTab({ query }: { query: string }) {
  const components = useArtPanel((s) => s.components);
  const placement = useArtPanel((s) => s.placement);
  const setPlacement = useArtPanel((s) => s.setPlacement);
  const hasSelection = useStore((s) => s.selectedIds.length > 0);
  const placementId = useId();
  const q = norm(query.trim());
  const shown = components.filter((c) => q === '' || norm(c.name).includes(q));
  const noSelection = t('art.component.save.needSelection');

  return (
    <div className="art-comps">
      <div className="art-comps__bar">
        <button type="button" className="btn" disabled={!hasSelection} title={hasSelection ? undefined : noSelection} onClick={() => requestSaveComponent()}>
          <Save size={14} aria-hidden="true" /> {t('art.component.save')}
        </button>
        <label className="art-comps__placement" htmlFor={placementId}>
          <span>{t('art.component.placement')}</span>
          <select id={placementId} className="input" value={placement} onChange={(e) => setPlacement(e.target.value as Placement)}>
            {PLACEMENTS.map((p) => (
              <option key={p} value={p}>
                {t(`art.component.placement.${p}` as I18nKey)}
              </option>
            ))}
          </select>
        </label>
      </div>
      {shown.length === 0 ? (
        <p className="hint art-empty">{components.length === 0 ? t('art.component.empty') : t('art.component.emptyFiltered')}</p>
      ) : (
        <ul className="art-comp-list">
          {shown.map((c) => (
            <Row key={c.name} c={c} />
          ))}
        </ul>
      )}
    </div>
  );
}
