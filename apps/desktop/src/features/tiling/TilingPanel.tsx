import { Grid2x2, LoaderCircle, TriangleAlert, X } from 'lucide-react';
import { useShallow } from 'zustand/react/shallow';
import { Check, Choice } from '@/features/arrange/Dialog';
import { fmt } from '@/lib/platform';
import { t } from '@/i18n';
import { selectActiveDevice, useStore } from '@/store';
import { LengthField } from '@/ui/LengthField';
import { feedBetween, isSingleDirection, type TileOrder } from './plan';
import { feedLines } from './feeder';
import { beginSession, cancelCurrent, finishSession, resumeSession, restartSession, selectTile, sendCurrent, skipCurrent } from './run';
import { counts } from './session';
import { closeTiling, plannedWithMargin, useTilingUi } from './store';
import { TilePreview } from './TilePreview';
import './tiling.css';

const ORDERS: TileOrder[] = ['rows', 'serpentine'];

/** Por qué no se puede empezar, o `null` si se puede. */
export function startBlockedReason(args: { design: unknown; planError: string | null; fits: boolean }): string | null {
  if (!args.design) return t('tiling.reason.nothing');
  if (args.planError) return args.planError;
  if (!args.fits) return t('tiling.err.sizeBed');
  return null;
}

/** Panel flotante del asistente de tiling (M39): opciones, vista previa y envío tramo por tramo. */
export function TilingPanel() {
  const phase = useTilingUi((s) => s.phase);
  const close = () => closeTiling();
  return (
    <section className="til-panel" role="dialog" aria-label={t('tiling.title')} data-phase={phase}>
      <header className="til-head">
        <h2 className="panel-title"><Grid2x2 size={15} aria-hidden="true" />{t('tiling.title')}</h2>
        <button type="button" className="icon-btn" aria-label={t('tiling.close')} title={t('tiling.close')} onClick={close}>
          <X size={16} aria-hidden="true" />
        </button>
      </header>
      {phase === 'config' ? <ConfigStep /> : <RunStep />}
    </section>
  );
}

function ConfigStep() {
  const { config, design, plan, saved, patchConfig } = useTilingUi(useShallow((s) => ({ config: s.config, design: s.design, plan: s.plan, saved: s.saved, patchConfig: s.patchConfig })));
  const device = useStore(useShallow((s) => {
    const d = selectActiveDevice(s);
    return { bed: d.bed, feeder: d.feeder };
  }));
  const [bedW, bedH] = device.bed;
  const fits = config.size[0] + 2 * config.margin <= bedW + 1e-9 && config.size[1] + 2 * config.margin <= bedH + 1e-9;
  const planError = plan && !plan.ok ? t(plan.error) : null;
  const ready = plan?.ok ? plan.plan : null;
  const blocked = startBlockedReason({ design, planError, fits });
  const single = ready ? isSingleDirection(ready) : false;
  const feeder = device.feeder.enabled;
  const specs = ready ? plannedWithMargin(ready.tiles, config.margin).map((p) => p.spec) : [];
  const advance = single && specs.length > 1 ? (feedBetween(specs[0]!, specs[1]!) ?? device.feeder.feed_mm) : null;

  return (
    <div className="til-body">
      {saved && (
        <div className="til-resume" role="status">
          <p>{t('tiling.resume.text', { done: counts(saved).done + counts(saved).skipped, total: saved.specs.length })}</p>
          <div className="row">
            <button type="button" className="btn btn--accent" onClick={resumeSession}>{t('tiling.resume.go')}</button>
            <button type="button" className="btn" onClick={restartSession}>{t('tiling.resume.drop')}</button>
          </div>
        </div>
      )}
      <p className="hint">
        {design
          ? t('tiling.intro', { w: fmt(design[1][0] - design[0][0], 1), h: fmt(design[1][1] - design[0][1], 1), bw: fmt(bedW, 0), bh: fmt(bedH, 0) })
          : t('tiling.reason.nothing')}
      </p>
      <div className="til-grid">
        <LengthField label={t('tiling.size.w')} value={config.size[0]} min={1} max={10000} step={1} onCommit={(v) => patchConfig({ size: [v, config.size[1]] })} />
        <LengthField label={t('tiling.size.h')} value={config.size[1]} min={1} max={10000} step={1} onCommit={(v) => patchConfig({ size: [config.size[0], v] })} />
        <LengthField label={t('tiling.overlap')} value={config.overlap} min={0} max={1000} step={0.5} onCommit={(v) => patchConfig({ overlap: v })} />
        <LengthField label={t('tiling.margin')} value={config.margin} min={0} max={1000} step={1} onCommit={(v) => patchConfig({ margin: v })} />
      </div>
      <p className="hint">{t('tiling.size.hint', { w: fmt(bedW - 2 * config.margin, 0), h: fmt(bedH - 2 * config.margin, 0) })}</p>
      <Choice
        legend={t('tiling.order')}
        value={config.order}
        options={ORDERS.map((o) => ({ value: o, label: t(o === 'rows' ? 'tiling.order.rows' : 'tiling.order.serpentine') }))}
        onChange={(order) => patchConfig({ order })}
      />
      <Check label={t('tiling.marks')} checked={config.marks} onChange={(marks) => patchConfig({ marks })} />
      <p className="hint">{t('tiling.marks.hint')}</p>

      {planError && <p className="warn-line" role="alert"><TriangleAlert size={14} aria-hidden="true" /><span>{planError}</span></p>}
      {!planError && !fits && <p className="warn-line" role="alert"><TriangleAlert size={14} aria-hidden="true" /><span>{t('tiling.err.sizeBed')}</span></p>}
      {ready && design && (
        <>
          <p className="til-count num" role="status">{t('tiling.count', { n: ready.tiles.length, cols: ready.cols, rows: ready.rows })}</p>
          <TilePreview design={design} specs={ready.tiles.map((p) => p.spec)} />
          {feeder && single && (
            <p className="hint" role="status">{t('tiling.feeder.auto', { mm: fmt(advance ?? 0, 1) })}</p>
          )}
          {feeder && !single && <p className="hint" role="status">{t('tiling.feeder.manual')}</p>}
        </>
      )}
      <div className="til-actions">
        <button type="button" className="btn btn--accent" disabled={blocked !== null} title={blocked ?? undefined} onClick={beginSession}>{t('tiling.start')}</button>
        <button type="button" className="btn" onClick={closeTiling}>{t('tiling.close')}</button>
        {blocked && <span className="hint" role="status">{blocked}</span>}
      </div>
    </div>
  );
}

function RunStep() {
  const { phase, session, message, design, config } = useTilingUi(useShallow((s) => ({ phase: s.phase, session: s.session, message: s.message, design: s.design, config: s.config })));
  const feeder = useStore((s) => selectActiveDevice(s).feeder);
  if (!session || !design) return null;
  const n = session.specs.length;
  const cur = session.current;
  const spec = session.specs[cur]!;
  const c = counts(session);
  const first = c.done + c.skipped === 0;
  const busy = phase === 'sending' || phase === 'feeding';
  const prevDone = session.status.some((v, i) => v === 'done' && i !== cur);
  const lines = feedLines(feeder, session.feeder && cur + 1 < n ? (feedBetween(spec, session.specs[cur + 1]!) ?? feeder.feed_mm) : feeder.feed_mm);

  return (
    <div className="til-body">
      <p className="til-progress num" role="status">{t('tiling.progress', { done: c.done, skipped: c.skipped, total: n })}</p>
      <TilePreview design={design} specs={session.specs} status={session.status} current={cur} onPick={busy ? undefined : selectTile} />

      {phase === 'place' && (
        <div className="til-step">
          <h3 className="til-sub">{t('tiling.place.title', { n: cur + 1, total: n })}</h3>
          <p>{t('tiling.place.text', { x: fmt(spec.rect[0][0], 1), y: fmt(spec.rect[0][1], 1), x2: fmt(spec.rect[1][0], 1), y2: fmt(spec.rect[1][1], 1), m: fmt(config.margin, 1) })}</p>
          {config.marks && prevDone && <p className="hint">{t('tiling.place.marks')}</p>}
          {session.feeder && !first && <p className="hint">{t('tiling.place.feeder')}</p>}
          {session.feeder && first && <p className="hint">{t('tiling.place.feederFirst', { gcode: lines.join(' / ') })}</p>}
          {message && <p className="warn-line" role="alert"><TriangleAlert size={14} aria-hidden="true" /><span>{message}</span></p>}
          <div className="til-actions">
            <button type="button" className="btn btn--accent" data-autofocus onClick={() => void sendCurrent()}>{t('tiling.send', { n: cur + 1 })}</button>
            <button type="button" className="btn" onClick={skipCurrent}>{t('tiling.skip', { n: cur + 1 })}</button>
          </div>
          <p className="hint">{t('tiling.pick.hint')}</p>
        </div>
      )}

      {phase === 'sending' && (
        <div className="til-step">
          <h3 className="til-sub"><LoaderCircle size={14} className="til-spin" aria-hidden="true" />{t('tiling.sending', { n: cur + 1, total: n })}</h3>
          <p className="hint">{t('tiling.sending.hint')}</p>
          <div className="til-actions">
            <button type="button" className="btn btn--danger" onClick={() => void cancelCurrent()}>{t('tiling.cancel')}</button>
          </div>
        </div>
      )}

      {phase === 'feeding' && (
        <div className="til-step">
          <h3 className="til-sub"><LoaderCircle size={14} className="til-spin" aria-hidden="true" />{t('tiling.feeding')}</h3>
          <p className="hint">{t('tiling.feeding.hint')}</p>
        </div>
      )}

      {phase === 'failed' && (
        <div className="til-step">
          <h3 className="til-sub">{t('tiling.failed.title', { n: cur + 1 })}</h3>
          <p className="warn-line" role="alert"><TriangleAlert size={14} aria-hidden="true" /><span>{message}</span></p>
          <div className="til-actions">
            <button type="button" className="btn btn--accent" onClick={() => void sendCurrent()}>{t('tiling.retry', { n: cur + 1 })}</button>
            <button type="button" className="btn" onClick={skipCurrent}>{t('tiling.skip', { n: cur + 1 })}</button>
            <button type="button" className="btn" onClick={() => useTilingUi.getState().set({ phase: 'place', message: null })}>{t('tiling.back')}</button>
          </div>
        </div>
      )}

      {phase === 'finished' && (
        <div className="til-step">
          <h3 className="til-sub">{t('tiling.finished.title')}</h3>
          <p>{t('tiling.finished.text', { done: c.done, skipped: c.skipped })}</p>
          <div className="til-actions">
            <button type="button" className="btn btn--accent" onClick={finishSession}>{t('tiling.finish')}</button>
            <button type="button" className="btn" onClick={restartSession}>{t('tiling.again')}</button>
          </div>
        </div>
      )}

      {!busy && phase !== 'finished' && (
        <div className="til-actions">
          <button type="button" className="btn" onClick={restartSession}>{t('tiling.restart')}</button>
        </div>
      )}
    </div>
  );
}
