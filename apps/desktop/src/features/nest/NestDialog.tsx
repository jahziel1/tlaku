import { TriangleAlert } from 'lucide-react';
import { useEffect, useId, useMemo, useState } from 'react';
import { Check, Choice, Dialog } from '@/features/arrange/Dialog';
import { selectionUnits } from '@/features/arrange/units';
import { UnitThumb } from '@/features/arrange/UnitThumb';
import { unitLabel } from '@/features/arrange/unitLabel';
import { t } from '@/i18n';
import { colorName } from '@/lib/palette';
import { fmt } from '@/lib/platform';
import { formatSize } from '@/lib/units';
import { selectActiveDevice, usePref, useStore } from '@/store';
import { NumberField } from '@/ui/NumberField';
import { LengthField } from '@/ui/LengthField';
import { applyNest } from './apply';
import { pieceNames } from './names';
import { useNestUi } from './nestStore';
import { NestPreview } from './NestPreview';
import { flatPolylines, unplacedSummary } from './preview';
import {
  buildNestRequest, materialCandidates, MAX_NEST_INSTANCES, NEST_STEP_RANGE, NEST_TIME_RANGE, resolveContainer, sanitizeCopies, totalInstances,
  type NestOptions,
} from './request';
import type { NestGravity, NestRotationMode } from './types';
import { useNestRun } from './useNestRun';
import './nest.css';

const MAX_UNPLACED_SHOWN = 8;

/** Diálogo "Acomodar piezas (nesting)": piezas y copias, contenedor, opciones, avance y vista previa. */
export function NestDialog() {
  const doc = useStore((s) => s.doc);
  const selectedIds = useStore((s) => s.selectedIds);
  const bed = useStore((s) => selectActiveDevice(s).bed);
  const lengthUnit = usePref('units');
  const options = useNestUi((s) => s.options);
  const patch = useNestUi((s) => s.patch);
  const close = useNestUi((s) => s.close);
  const run = useNestRun();
  const pickId = useId();
  const [tabId] = useState(() => useStore.getState().activeTabId);

  const units = useMemo(() => selectionUnits(doc, selectedIds), [doc, selectedIds]);
  const [copies, setCopies] = useState<Record<string, number>>({});
  const total = totalInstances(units, copies);
  const flat = useMemo(() => new Map(units.map((u) => [u.id, flatPolylines(u.objects)])), [units]);
  // Nombre legible de cada pieza ("Rectángulo 40×20 mm"); si dos se llaman igual, se numeran.
  const names = useMemo(() => pieceNames(units), [units]);
  const nameOf = (id: string): string => names.get(id) ?? id;

  // Material: objetos vectoriales cerrados que no son piezas.
  const candidates = useMemo(() => {
    const taken = new Set([...selectedIds, ...units.flatMap((u) => u.ids)]);
    return materialCandidates(doc, taken);
  }, [doc, selectedIds, units]);
  const material = candidates.find((o) => o.id === options.materialId) ?? candidates[0] ?? null;
  const container = resolveContainer(options.containerKind, bed, material);
  const layerName = (color: string) => doc.layers.find((l) => l.color.toLowerCase() === color.toLowerCase())?.name ?? colorName(color);

  // Un resultado de otras piezas, copias o contenedor ya no sirve.
  const inputKey = JSON.stringify([units.map((u) => [u.id, sanitizeCopies(copies[u.id] ?? 1)]), options.containerKind, material?.id ?? null, bed]);
  const { reset } = run;
  useEffect(() => reset(), [inputKey, reset]);

  const running = run.phase === 'running';
  const stopping = run.phase === 'stopping';
  const busy = running || stopping;
  const status = run.status;
  const placements = status?.placements ?? [];

  const startBlocked =
    units.length === 0 ? t('nest.reason.noSelection')
    : total > MAX_NEST_INSTANCES ? t('nest.pieces.tooMany', { max: MAX_NEST_INSTANCES })
    : !container ? t('nest.start.noContainer')
    : null;
  const applyBlocked =
    stopping ? t('nest.apply.stopping')
    : !status ? t('nest.apply.noResult')
    : placements.length === 0 ? (running ? t('nest.apply.noResult') : t('nest.apply.nonePlaced'))
    : null;

  const toggleRun = () => {
    if (busy) {
      void run.stop();
      return;
    }
    if (startBlocked || !container) return;
    void run.start(buildNestRequest({ units, copies, container, options }));
  };

  const accept = async () => {
    if (applyBlocked) return;
    const final = running ? await run.stop() : status;
    const done = final?.placements ?? [];
    if (done.length === 0) return;
    const s = useStore.getState();
    let ids: string[] = [];
    // El resultado va a la pestaña donde se abrió el diálogo (no a la que esté activa al aceptar).
    s.runInTab(tabId, () => {
      s.applyDocChange('nest.history', (d) => {
        const r = applyNest(d, units, done);
        ids = r.placed;
        return r.doc;
      });
      if (ids.length > 0) s.select(ids);
    });
    if (ids.length > 0) s.notify('success', t('nest.done', { n: done.length }));
    close();
  };

  const setAll = (n: number) => setCopies(Object.fromEntries(units.map((u) => [u.id, sanitizeCopies(n)])));
  const reason = startBlocked ?? (applyBlocked && !busy ? applyBlocked : null);
  const pct = status ? Math.round(status.utilization * 10) / 10 : 0;
  const unplaced = status && !status.running ? unplacedSummary(status.unplaced) : [];
  const unplacedCount = status && !status.running ? status.unplaced.length : 0;
  const error = run.error;

  return (
    <Dialog
      title={t('nest.title')}
      onClose={close}
      wide
      tall
      initialFocus=".field__input"
      footer={
        <>
          {reason && <p className="ar-reason" role="status">{reason}</p>}
          <button type="button" className="btn" onClick={close}>{t('nest.cancel')}</button>
          <button
            type="button"
            className="btn"
            aria-disabled={stopping || (!busy && startBlocked !== null)}
            title={!busy && startBlocked ? startBlocked : undefined}
            onClick={() => !stopping && toggleRun()}
          >
            {stopping ? t('nest.stopping') : running ? t('nest.stop') : t('nest.start')}
          </button>
          <button
            type="button"
            className="btn btn--accent"
            aria-disabled={applyBlocked !== null}
            title={applyBlocked ?? undefined}
            onClick={() => void accept()}
          >
            {t('nest.apply')}
          </button>
        </>
      }
    >
      <div className="ne-cols">
        <fieldset className="ne-form" disabled={busy}>
          <section className="ne-section" aria-labelledby={`${pickId}-pieces`}>
            <h3 id={`${pickId}-pieces`} className="ne-heading">{t('nest.pieces.title')}</h3>
            <NumberField
              label={t('nest.pieces.all')}
              value={units.length > 0 ? sanitizeCopies(copies[units[0]!.id] ?? 1) : 1}
              min={1}
              max={MAX_NEST_INSTANCES}
              step={1}
              onCommit={(v) => setAll(v)}
            />
            <ul className="ne-list" aria-label={t('nest.pieces.list')}>
              {units.map((u) => (
                <li key={u.id} className="ne-row">
                  <UnitThumb unit={u} />
                  <NumberField
                    label={t('nest.piece.copies', { name: nameOf(u.id) })}
                    value={sanitizeCopies(copies[u.id] ?? 1)}
                    min={1}
                    max={MAX_NEST_INSTANCES}
                    step={1}
                    onCommit={(v) => setCopies((c) => ({ ...c, [u.id]: sanitizeCopies(v) }))}
                  />
                </li>
              ))}
            </ul>
            <p className={`hint num${total > MAX_NEST_INSTANCES ? ' hint--danger' : ''}`}>
              {t('nest.pieces.total', { total, max: MAX_NEST_INSTANCES })}
            </p>
          </section>

          <section className="ne-section" aria-labelledby={`${pickId}-container`}>
            <h3 id={`${pickId}-container`} className="ne-heading">{t('nest.container.title')}</h3>
            <Choice<NestOptions['containerKind']>
              legend={t('nest.container.title')}
              value={options.containerKind}
              options={[
                { value: 'bed', label: t('nest.container.bed', { size: formatSize(bed[0], bed[1], lengthUnit, 0) }) },
                { value: 'object', label: t('nest.container.object') },
              ]}
              onChange={(containerKind) => patch({ containerKind })}
            />
            {options.containerKind === 'object' &&
              (candidates.length === 0 ? (
                <p className="hint">{t('nest.container.none')}</p>
              ) : (
                <div className="field">
                  <label htmlFor={`${pickId}-material`} className="field__label">{t('nest.container.pick')}</label>
                  <select
                    id={`${pickId}-material`}
                    className="select"
                    value={material?.id ?? ''}
                    onChange={(e) => patch({ materialId: e.target.value })}
                  >
                    {candidates.map((o) => (
                      <option key={o.id} value={o.id}>{t('nest.container.option', { name: unitLabel({ objects: [o] }), layer: layerName(o.layer) })}</option>
                    ))}
                  </select>
                  <p className="hint">{t('nest.container.hint')}</p>
                </div>
              ))}
          </section>

          <section className="ne-section" aria-labelledby={`${pickId}-options`}>
            <h3 id={`${pickId}-options`} className="ne-heading">{t('nest.opt.title')}</h3>
            <div className="field-grid">
              <LengthField label={t('nest.opt.spacing')} value={options.spacing} min={0} max={1000} step={0.5} onCommit={(spacing) => patch({ spacing })} />
              <LengthField label={t('nest.opt.margin')} value={options.margin} min={0} max={1000} step={0.5} onCommit={(margin) => patch({ margin })} />
            </div>
            <Choice<NestRotationMode>
              legend={t('nest.opt.rotation')}
              value={options.rotationMode}
              options={[
                { value: 'none', label: t('nest.rot.none') },
                { value: 'quarter', label: t('nest.rot.quarter') },
                { value: 'step', label: t('nest.rot.step') },
                { value: 'free', label: t('nest.rot.free') },
              ]}
              onChange={(rotationMode) => patch({ rotationMode })}
            />
            {options.rotationMode === 'step' && (
              <NumberField
                label={t('nest.opt.step')}
                value={options.stepDeg}
                min={NEST_STEP_RANGE[0]}
                max={NEST_STEP_RANGE[1]}
                step={5}
                unit={t('nest.unit.deg')}
                onCommit={(stepDeg) => patch({ stepDeg })}
              />
            )}
            <Check label={t('nest.opt.mirror')} checked={options.allowMirror} onChange={(allowMirror) => patch({ allowMirror })} />
            <Check label={t('nest.opt.holes')} checked={options.useHoles} onChange={(useHoles) => patch({ useHoles })} />
            <Choice<NestGravity>
              legend={t('nest.opt.gravity')}
              value={options.gravity}
              options={[
                { value: 'gravity', label: t('nest.gravity.gravity') },
                { value: 'bottom_left', label: t('nest.gravity.bottom_left') },
              ]}
              onChange={(gravity) => patch({ gravity })}
            />
            <div className="field-grid">
              <NumberField
                label={t('nest.opt.time')}
                value={options.timeLimitS}
                min={NEST_TIME_RANGE[0]}
                max={NEST_TIME_RANGE[1]}
                step={5}
                unit={t('nest.unit.s')}
                onCommit={(timeLimitS) => patch({ timeLimitS: Math.round(timeLimitS) })}
              />
              <NumberField label={t('nest.opt.seed')} value={options.seed} min={0} max={999999999} step={1} onCommit={(seed) => patch({ seed: Math.round(seed) })} />
            </div>
          </section>
        </fieldset>

        <div className="ne-side">
          <span className="field__label">{t('nest.preview.title')}</span>
          <NestPreview container={container} flat={flat} placements={status ? placements : null} />
          <div className="ne-legend">
            <span><i className="ne-swatch" aria-hidden="true" />{t('nest.legend.original')}</span>
            <span><i className="ne-swatch ne-swatch--copy" aria-hidden="true" />{t('nest.legend.copy')}</span>
          </div>
          {!status && <p className="hint">{t('nest.preview.hint')}</p>}

          {status && (
            <div className="ne-progress">
              <div
                className="ne-bar"
                role="progressbar"
                aria-label={t('nest.progress.label')}
                aria-valuemin={0}
                aria-valuemax={100}
                aria-valuenow={pct}
                aria-valuetext={t('nest.progress.pct', { pct: fmt(pct) })}
              >
                <div className="ne-bar__fill" style={{ width: `${Math.min(100, Math.max(0, pct))}%` }} />
              </div>
              <p className="ne-stats num">
                <strong>{t('nest.progress.utilization')}: {t('nest.progress.pct', { pct: fmt(pct) })}</strong>
                <span>{t('nest.progress.placed', { placed: status.placed, total: status.total })}</span>
                <span>{t('nest.progress.elapsed', { s: fmt(status.elapsed_ms / 1000) })}</span>
              </p>
            </div>
          )}
          <p className="hint" role="status" aria-live="polite">
            {busy ? t('nest.progress.running') : status ? (status.timed_out ? t('nest.progress.timeout') : t('nest.progress.done')) : ''}
          </p>
          {unplacedCount > 0 && (
            <div className="warn-line" role="status">
              <TriangleAlert size={14} aria-hidden="true" />
              <div>
                <p>{t('nest.progress.unplaced', { n: unplacedCount })}</p>
                <p className="hint">
                  {t('nest.progress.unplacedList', {
                    list: unplaced
                      .slice(0, MAX_UNPLACED_SHOWN)
                      .map((u) => `${nameOf(u.piece)}${u.count > 1 ? ` ×${u.count}` : ''}`)
                      .join(', ') + (unplaced.length > MAX_UNPLACED_SHOWN ? '…' : ''),
                  })}
                </p>
              </div>
            </div>
          )}
          {error && (
            <p className="warn-line" role="alert">
              <TriangleAlert size={14} aria-hidden="true" />
              <span>{t('nest.error', { msg: error })}</span>
            </p>
          )}
        </div>
      </div>
    </Dialog>
  );
}
