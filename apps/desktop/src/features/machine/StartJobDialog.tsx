import { Clock, Layers, TriangleAlert, Zap } from 'lucide-react';
import { useState } from 'react';
import { useShallow } from 'zustand/react/shallow';
import { t } from '@/i18n';
import { speedToDisplay, speedUnitLabel } from '@/lib/device';
import { fmt, fmtClock } from '@/lib/platform';
import { activeTabLabel, selectActiveDevice, usePref, useStore } from '@/store';
import { CHECKLIST_ALL, checklistPending, checklistSatisfied, visibleChecklist, withChecklistItem, CHECKLIST_ITEMS, type ChecklistItem } from './checklist';
import { summarizeJob } from './jobSummary';
import { useJobPlacement } from './hooks';
import { useLinkWarning } from './useLinkWarning';
import { Modal } from './Modal';
import { OriginIllustration } from './OriginIllustration';
import { galvoConfirmKey, GalvoStartNotice } from './GalvoNotice';
import { lhyConfirmKey, LhyStartNotice } from './LhyNotice';
import { ruidaConfirmKey, RuidaStartNotice } from './RuidaNotice';
import { useRuidaStartGate } from './ruidaStart';
import { SelectionOptions } from './SelectionOptions';
import { ReviewNotice } from '@/features/review';
import { launchRepeat } from '@/features/feeder/RepeatPanel';
import { repeatBlockedReason, useRepeat } from '@/features/feeder/repeat';
import { RepeatSection } from '@/features/feeder/RepeatSection';
import { TilingOffer } from '@/features/tiling/TilingOffer';
import { placementSummary } from './StartFromSection';

/**
 * "¿Láser listo y área despejada?": última parada antes de que el láser se encienda. Resume tiempo,
 * capas, potencia máxima y dónde quedará el trabajo; "Cancelar" es el foco inicial.
 */
export function StartJobDialog() {
  const close = useStore((s) => s.closeDialog);
  const confirm = useStore((s) => s.confirmStart);
  const doc = useStore((s) => s.doc);
  // El trabajo es el de la pestaña activa: el diálogo dice cuál.
  const docName = useStore((s) => activeTabLabel(s));
  const preview = useStore((s) => s.preview);
  const previewStatus = useStore((s) => s.previewStatus);
  const previewError = useStore((s) => s.previewError);
  const options = useStore((s) => s.jobOptions);
  const device = useStore(selectActiveDevice);
  const head = useStore(useShallow((s) => (s.connection.connected && s.status ? s.status.work_pos : null)));
  const placement = useJobPlacement();

  const ready = previewStatus === 'ready' && preview !== null;
  // Solo avisa: si el enlace serie no da abasto, la máquina hará pausas, pero el inicio no se bloquea.
  const linkWarning = useLinkWarning(ready);
  const summary = ready ? summarizeJob(doc, preview) : null;
  const where = placementSummary(placement, options.start_from, options.job_origin);
  // La lista de comprobación: qué casillas se muestran y si hay que marcarlas todas antes de enviar.
  const mask = usePref('jobChecklistItems');
  const required = usePref('jobChecklistRequired');
  const setPref = useStore((s) => s.setPref);
  const [ticked, setTicked] = useState<ReadonlySet<ChecklistItem['id']>>(new Set());
  const [configuring, setConfiguring] = useState(false);
  const toggle = (id: ChecklistItem['id'], on: boolean) =>
    setTicked((prev) => {
      const next = new Set(prev);
      if (on) next.add(id);
      else next.delete(id);
      return next;
    });
  const checklistOk = checklistSatisfied(mask, required, ticked);
  const pending = checklistPending(mask, ticked);
  // Con «Repetir con el alimentador» el botón de inicio manda la tanda entera.
  const repeat = useRepeat((s) => s.config);
  const repeating = repeat.enabled && repeatBlockedReason(device, repeat) === null;
  // Ruida guarda el trabajo con un nombre: uno repetido pide confirmar el reemplazo.
  const connected = useStore((s) => s.connection.connected);
  const ruidaGate = useRuidaStartGate(device, connected);
  const canStart = ready && placement.status === 'ok' && placement.fits && checklistOk && ruidaGate.ok && (!repeat.enabled || repeating);
  const blockedWhy =
    !ready || placement.status !== 'ok' || !placement.fits
      ? t('machine.start.blocked')
      : !checklistOk
        ? t('machine.checklist.blocked', { n: pending.length })
        : !ruidaGate.ok
          ? t('ruida.name.overwriteNeeded')
          : repeatBlockedReason(device, repeat) ?? t('machine.start.blocked');

  return (
    <Modal
      title={t('machine.start.title')}
      subtitle={t('machine.start.subtitle', { doc: docName, machine: device.name })}
      tone="warn"
      size="md"
      onClose={close}
      footer={
        <>
          <button type="button" className="btn" data-autofocus onClick={close}>{t('machine.dlg.cancel')}</button>
          <button type="button" className="btn btn--accent" disabled={!canStart} title={canStart ? undefined : blockedWhy} onClick={() => void (repeating ? launchRepeat() : ruidaGate.commit().then((ok) => (ok ? confirm() : undefined)))}>
            {t(repeating ? 'tiling.repeat.start' : (ruidaConfirmKey(device) ?? lhyConfirmKey(device) ?? galvoConfirmKey(device) ?? 'machine.start.confirm'))}
          </button>
        </>
      }
    >
      <ul className="mc-list mc-list--check">
        {(['machine.start.check2', 'machine.start.check3', 'machine.start.check4'] as const).map((k) => (
          <li key={k}>{t(k)}</li>
        ))}
      </ul>

      <section className="mc-checklist" aria-labelledby="start-checklist">
        <div className="mc-checklist__head">
          <h3 id="start-checklist" className="mc-sub">{t('machine.checklist.title')}</h3>
          <button type="button" className="btn" aria-expanded={configuring} onClick={() => setConfiguring((v) => !v)}>
            {t(configuring ? 'machine.checklist.done' : 'machine.checklist.configure')}
          </button>
        </div>
        {configuring ? (
          <div className="mc-checklist__config">
            {CHECKLIST_ITEMS.map((i) => (
              <label key={i.id} className="mc-check">
                <input type="checkbox" checked={(mask & i.bit) !== 0} onChange={(e) => setPref('jobChecklistItems', withChecklistItem(mask, i.bit, e.target.checked))} />
                {t(i.label)}
              </label>
            ))}
            <label className="mc-check">
              <input type="checkbox" checked={required} onChange={(e) => setPref('jobChecklistRequired', e.target.checked)} />
              {t('machine.checklist.required')}
            </label>
            <div className="row">
              <button type="button" className="btn" disabled={mask === CHECKLIST_ALL} onClick={() => setPref('jobChecklistItems', CHECKLIST_ALL)}>{t('machine.checklist.all')}</button>
            </div>
            <p className="hint">{t('machine.checklist.config.help')}</p>
          </div>
        ) : visibleChecklist(mask).length === 0 ? (
          <p className="hint">{t('machine.checklist.empty')}</p>
        ) : (
          <>
            <div className="mc-checklist__items">
              {visibleChecklist(mask).map((i) => (
                <label key={i.id} className="mc-check">
                  <input type="checkbox" checked={ticked.has(i.id)} onChange={(e) => toggle(i.id, e.target.checked)} />
                  {t(i.label)}
                </label>
              ))}
            </div>
            <p className={required && !checklistOk ? 'hint hint--danger' : 'hint'} role="status">
              {required ? (checklistOk ? t('machine.checklist.ready') : t('machine.checklist.blocked', { n: pending.length })) : t('machine.checklist.optional')}
            </p>
          </>
        )}
      </section>

      <RuidaStartNotice device={device} gate={ruidaGate} />
      <LhyStartNotice device={device} />
      <GalvoStartNotice device={device} />

      <SelectionOptions compact />
      <TilingOffer />
      <RepeatSection />
      <ReviewNotice />

      {previewStatus === 'computing' && <p className="hint" role="status">{t('machine.start.computing')}</p>}
      {previewStatus === 'error' && (
        <p className="warn-line" role="alert">
          <TriangleAlert size={14} aria-hidden="true" />
          <span>{previewError}</span>
        </p>
      )}

      {summary && (
        <dl className="mc-summary">
          <div>
            <dt><Clock size={13} aria-hidden="true" />{t('machine.start.time')}</dt>
            <dd className="num">{fmtClock(summary.totalSeconds)}</dd>
          </div>
          <div>
            <dt><Layers size={13} aria-hidden="true" />{t('machine.start.layers', { n: summary.layers.length })}</dt>
            <dd>
              <ul className="mc-layers">
                {summary.layers.map((l) => (
                  <li key={l.color}>
                    <span className="mc-swatch" style={{ background: l.color }} aria-hidden="true" />
                    <span className="mc-layers__name">{l.name}</span>
                    <span className="num mc-layers__meta">{fmt(l.power, 0)} % · {fmt(speedToDisplay(l.speed, device.speed_unit), device.speed_unit === 'mm_per_sec' ? 1 : 0)} {speedUnitLabel(device.speed_unit)}{l.passes > 1 ? ` · ${l.passes}×` : ''}</span>
                  </li>
                ))}
              </ul>
            </dd>
          </div>
          <div>
            <dt><Zap size={13} aria-hidden="true" />{t('machine.start.maxPower')}</dt>
            <dd className="num">{fmt(summary.maxPower, 0)} %</dd>
          </div>
        </dl>
      )}

      <div className="mc-start-where">
        <OriginIllustration device={device} placement={placement} jobOrigin={options.job_origin} head={head} userOrigin={options.user_origin} legend={false} />
        <p className={where.problem ? 'hint hint--danger' : 'hint'}>{where.text}</p>
      </div>

      {((summary && summary.warnings.length > 0) || linkWarning) && (
        <ul className="mc-warnings">
          {linkWarning && (
            <li className="warn-line" data-testid="link-warning">
              <TriangleAlert size={14} aria-hidden="true" />
              <span>{linkWarning}</span>
            </li>
          )}
          {summary?.warnings.map((w, i) => (
            <li key={i} className="warn-line">
              <TriangleAlert size={14} aria-hidden="true" />
              <span>{w}</span>
            </li>
          ))}
        </ul>
      )}
    </Modal>
  );
}
