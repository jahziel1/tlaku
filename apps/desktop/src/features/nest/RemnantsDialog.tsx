import { Trash2 } from 'lucide-react';
import { useEffect, useId, useMemo, useState } from 'react';
import { askChoice } from '@/features/history/prompt';
import { t } from '@/i18n';
import { Modal } from '@/ui/Modal';
import { loadRemnants, removeRemnants, renameRemnant, retagRemnant, useRemnants } from './remnantStore';
import { normalizeMaterial, type Remnant, type RemnantLibrary } from './remnants';
import './nest.css';

/** Contorno del retazo en miniatura (eje Y hacia arriba). */
function Thumb({ r }: { r: Remnant }) {
  const d = r.paths.map((ring) => `${ring.map(([x, y], i) => `${i === 0 ? 'M' : 'L'}${x} ${y}`).join('')}Z`).join('');
  return (
    <svg className="ne-rem__thumb" viewBox={`0 ${-r.height_mm} ${r.width_mm} ${r.height_mm}`} aria-hidden="true" preserveAspectRatio="xMidYMid meet">
      <path d={d} transform="scale(1 -1)" fill="var(--cv-bed)" fillRule="evenodd" stroke="var(--cv-bed-edge)" strokeWidth={1.4} vectorEffect="non-scaling-stroke" />
    </svg>
  );
}

const sizeText = (r: Remnant): string =>
  r.rect
    ? t('nest.remnants.sizeRect', { w: Math.round(r.width_mm), h: Math.round(r.height_mm) })
    : t('nest.remnants.sizeShape', { w: Math.round(r.width_mm), h: Math.round(r.height_mm), area: Math.round(r.area_mm2 / 100) });

function Row({ r, materials }: { r: Remnant; materials: string }) {
  const [name, setName] = useState(r.name);
  const [material, setMaterial] = useState(r.material);
  const [thickness, setThickness] = useState(String(r.thickness_mm));
  useEffect(() => setName(r.name), [r.name]);
  useEffect(() => setMaterial(r.material), [r.material]);
  useEffect(() => setThickness(String(r.thickness_mm)), [r.thickness_mm]);

  const commitTags = () => {
    const n = Number(thickness.replace(',', '.'));
    const th = Number.isFinite(n) && n >= 0 ? n : r.thickness_mm;
    setThickness(String(th));
    if (material.trim() !== r.material || th !== r.thickness_mm) void retagRemnant(r.id, material, th);
  };
  const remove = async () => {
    const answer = await askChoice({
      title: t('nest.remnants.delete.title'),
      message: t('nest.remnants.delete.ask', { name: r.name }),
      choices: [
        { id: 'delete', label: t('nest.remnants.delete.confirm'), tone: 'danger' },
        { id: 'cancel', label: t('nest.remnants.delete.cancel') },
      ],
      cancelId: 'cancel',
    });
    if (answer === 'delete') void removeRemnants([r.id]);
  };

  return (
    <li className="ne-rem">
      <Thumb r={r} />
      <div className="ne-rem__body">
        <input
          className="input ne-rem__name"
          aria-label={t('nest.remnants.name')}
          value={name}
          maxLength={80}
          onChange={(e) => setName(e.target.value)}
          onBlur={() => name.trim() !== r.name && void renameRemnant(r.id, name)}
          onKeyDown={(e) => e.key === 'Enter' && e.currentTarget.blur()}
        />
        <div className="ne-rem__tags">
          <input
            className="input"
            aria-label={t('nest.remnants.material')}
            list={materials}
            value={material}
            placeholder={t('nest.remnants.material')}
            onChange={(e) => setMaterial(e.target.value)}
            onBlur={commitTags}
            onKeyDown={(e) => e.key === 'Enter' && e.currentTarget.blur()}
          />
          <input
            className="input num ne-rem__thick"
            aria-label={t('nest.remnants.thickness')}
            inputMode="decimal"
            value={thickness}
            onChange={(e) => setThickness(e.target.value)}
            onBlur={commitTags}
            onKeyDown={(e) => e.key === 'Enter' && e.currentTarget.blur()}
          />
          <span className="hint">{t('nest.remnants.mm')}</span>
        </div>
        <p className="hint num">{sizeText(r)}</p>
      </div>
      <button type="button" className="btn" aria-label={t('nest.remnants.delete.aria', { name: r.name })} title={t('nest.remnants.delete.confirm')} onClick={() => void remove()}>
        <Trash2 size={15} aria-hidden="true" />
      </button>
    </li>
  );
}

/** Inventario de retazos conectado al estado: lee la lista al abrirse. */
export function RemnantsDialog() {
  const library = useRemnants((s) => s.library);
  const loaded = useRemnants((s) => s.loaded);
  const loadError = useRemnants((s) => s.loadError);
  const setOpen = useRemnants((s) => s.setPanelOpen);
  useEffect(() => {
    void loadRemnants();
  }, []);
  return <RemnantsDialogView library={library} loaded={loaded} loadError={loadError} onClose={() => setOpen(false)} />;
}

interface ViewProps {
  library: RemnantLibrary;
  loaded: boolean;
  loadError: string | null;
  onClose: () => void;
}

/** Inventario de retazos: ver, filtrar por material, renombrar, corregir material y grosor y borrar. */
export function RemnantsDialogView({ library, loaded, loadError, onClose }: ViewProps) {
  const labelId = useId();
  const listId = useId();
  const [filter, setFilter] = useState('');

  const materials = useMemo(() => [...new Set(library.remnants.map((r) => r.material).filter(Boolean))], [library.remnants]);
  const shown = useMemo(() => {
    const q = normalizeMaterial(filter);
    return library.remnants
      .filter((r) => q === '' || normalizeMaterial(`${r.material} ${r.name}`).includes(q))
      .sort((a, b) => b.created_ms - a.created_ms);
  }, [library.remnants, filter]);

  const clearAll = async () => {
    if (shown.length === 0) return;
    const answer = await askChoice({
      title: t('nest.remnants.clear.title'),
      message: t('nest.remnants.clear.ask', { n: shown.length }),
      choices: [
        { id: 'delete', label: t('nest.remnants.clear.confirm', { n: shown.length }), tone: 'danger' },
        { id: 'cancel', label: t('nest.remnants.delete.cancel') },
      ],
      cancelId: 'cancel',
    });
    if (answer === 'delete') void removeRemnants(shown.map((r) => r.id));
  };

  return (
    <Modal labelId={labelId} onClose={onClose} backdropClassName="modal" cardClassName="modal__card modal__card--wide" guardOutside>
      <div className="modal__head">
        <h2 id={labelId} className="modal__title">{t('nest.remnants.title')}</h2>
      </div>
      <div className="modal__body ar-body">
        <p className="hint">{t('nest.remnants.intro')}</p>
        {loadError && <p className="warn-line" role="alert">{t('nest.remnants.err.load', { error: loadError })}</p>}
        <datalist id={listId}>{materials.map((m) => <option key={m} value={m} />)}</datalist>
        <div className="field">
          <label htmlFor={`${listId}-filter`} className="field__label">{t('nest.remnants.filter')}</label>
          <input id={`${listId}-filter`} className="input" value={filter} placeholder={t('nest.remnants.filter.placeholder')} onChange={(e) => setFilter(e.target.value)} />
        </div>
        {!loaded && !loadError ? (
          <p className="hint" role="status">{t('nest.remnants.loading')}</p>
        ) : shown.length === 0 ? (
          <p className="hint" role="status">{library.remnants.length === 0 ? t('nest.remnants.empty') : t('nest.remnants.noMatch')}</p>
        ) : (
          <ul className="ne-rem__list" aria-label={t('nest.remnants.list')}>
            {shown.map((r) => <Row key={r.id} r={r} materials={listId} />)}
          </ul>
        )}
        <p className="hint num">{t('nest.remnants.count', { n: library.remnants.length })}</p>
      </div>
      <div className="modal__foot">
        <button type="button" className="btn" aria-disabled={shown.length === 0} onClick={() => void clearAll()}>
          {filter.trim() ? t('nest.remnants.clearShown') : t('nest.remnants.clearAll')}
        </button>
        <button type="button" className="btn btn--accent" data-autofocus onClick={onClose}>{t('nest.remnants.close')}</button>
      </div>
    </Modal>
  );
}
