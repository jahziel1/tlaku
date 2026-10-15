import { Home, Minus, Plus, ScanLine, TriangleAlert } from 'lucide-react';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { getApi } from '@/api';
import type { ResumeReadiness, ResumeView } from '@/api/types';
import { drawResume, unproject, type Fit, type ResumePalette } from '@/features/preview/resumeDraw';
import { t } from '@/i18n';
import { errorMessage } from '@/lib/errors';
import { fmt, readPref, writePref } from '@/lib/platform';
import { selectActiveDevice, usePref, useStore } from '@/store';
import { isJobBusy } from '@/store/selectors';
import { checklistPending, checklistSatisfied, visibleChecklist, type ChecklistItem } from './checklist';
import { Modal } from './Modal';
import { stateLabel } from './StatusSection';
import { choiceAt, homeMode, moveCount, nearestMove, resumeBlockedReason, suggestedChoice, type RestartChoice } from './resumeModel';

/** Retroceso por omisión: lo que pase de las dos reglas (líneas, mm) manda lo que quede más atrás. */
const DEFAULT_BACK = { lines: 3, mm: 1.5 };

function readBack(): { lines: number; mm: number } {
  const lines = Number(readPref('resumeBackLines'));
  const mm = Number(readPref('resumeBackMm'));
  return {
    lines: Number.isFinite(lines) && lines >= 0 ? Math.round(lines) : DEFAULT_BACK.lines,
    mm: Number.isFinite(mm) && mm >= 0 ? mm : DEFAULT_BACK.mm,
  };
}

function paletteOf(el: HTMLElement): ResumePalette {
  const css = getComputedStyle(el);
  const v = (name: string, fallback: string) => css.getPropertyValue(name).trim() || fallback;
  return { done: v('--rs-done', '#2e9d5b'), todo: v('--rs-todo', '#d9822b'), travel: v('--rs-travel', '#8884'), marker: v('--rs-marker', '#d33'), bed: v('--rs-bed', '#0000') };
}

/** Vista previa del trabajo guardado: lo hecho en trazo continuo, lo que falta discontinuo, el punto de reinicio marcado. */
function ResumeCanvas({ view, choice, onPick }: { view: ResumeView; choice: RestartChoice; onPick: (move: number) => void }) {
  const ref = useRef<HTMLCanvasElement>(null);
  const fit = useRef<Fit | null>(null);
  useEffect(() => {
    const canvas = ref.current;
    const ctx = canvas?.getContext('2d');
    if (!canvas || !ctx) return;
    const ratio = window.devicePixelRatio || 1;
    const w = canvas.clientWidth || 480;
    const h = canvas.clientHeight || 280;
    canvas.width = Math.round(w * ratio);
    canvas.height = Math.round(h * ratio);
    ctx.setTransform(ratio, 0, 0, ratio, 0, 0);
    fit.current = drawResume(ctx, w, h, view, choice.moveIndex, choice.at, paletteOf(canvas));
  }, [view, choice.moveIndex, choice.at]);
  const click = (e: React.MouseEvent<HTMLCanvasElement>) => {
    if (!fit.current) return;
    const r = e.currentTarget.getBoundingClientRect();
    const [x, y] = unproject(fit.current, e.clientX - r.left, e.clientY - r.top);
    onPick(nearestMove(view, x, y));
  };
  return <canvas ref={ref} className="mc-resume__canvas" role="img" aria-label={t('machine.resume.preview.alt')} onClick={click} data-testid="resume-canvas" />;
}

/**
 * Continuar el trabajo interrumpido: vista previa de lo hecho y lo que falta, control del punto de reinicio y
 * los pasos obligatorios (home o confirmación, encuadre, lista de comprobación). El motor vuelve a exigirlos.
 */
export function ResumeDialog({ choose }: { choose: boolean }) {
  const close = useStore((s) => s.closeDialog);
  const job = useStore((s) => s.interrupted);
  const connected = useStore((s) => s.connection.connected);
  const device = useStore(selectActiveDevice);
  const status = useStore((s) => s.status);
  const busy = useStore(isJobBusy);
  const jogSpeed = useStore((s) => s.jogSpeed);
  const sendLine = useStore((s) => s.sendLine);
  const notify = useStore((s) => s.notify);
  const setDockTab = useStore((s) => s.setDockTab);
  const discard = useStore((s) => s.discardInterrupted);
  const mask = usePref('jobChecklistItems');
  const required = usePref('jobChecklistRequired');

  const [back, setBack] = useState(readBack);
  const [view, setView] = useState<ResumeView | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [picked, setPicked] = useState<number | null>(null);
  const [ready, setReady] = useState<ResumeReadiness | null>(null);
  const [unmoved, setUnmoved] = useState(false);
  const [framedRun, setFramedRun] = useState(false);
  const [framing, setFraming] = useState(false);
  const [framedOk, setFramedOk] = useState(false);
  const [ticked, setTicked] = useState<ReadonlySet<ChecklistItem['id']>>(new Set());
  const [sending, setSending] = useState(false);
  const sliderRef = useRef<HTMLInputElement>(null);

  // El recorrido guardado y el punto propuesto (otra vez si cambia el retroceso o la máquina conectada: con ella el
  // motor compara su posición de ahora con la guardada, por si siguió sola después de que la app se cerró).
  const machineIdle = status?.state === 'idle';
  useEffect(() => {
    let alive = true;
    const timer = setTimeout(() => {
      getApi()
        .interruptedJobView({ back_lines: back.lines, back_mm: back.mm })
        .then((v) => {
          if (!alive) return;
          setView(v);
          setError(null);
        })
        .catch((e) => alive && setError(errorMessage(e)));
    }, 250);
    return () => {
      alive = false;
      clearTimeout(timer);
    };
  }, [back, connected, machineIdle]);

  useEffect(() => {
    if (choose) sliderRef.current?.focus();
  }, [choose, view]);

  // Lo que le falta a la máquina conectada (homing y si ya se hizo), al día mientras el diálogo esté abierto.
  const refreshReady = useCallback(async () => {
    try {
      setReady(await getApi().continueCheck());
    } catch {
      setReady(null);
    }
  }, []);
  const idle = status?.state === 'idle';
  useEffect(() => {
    if (!connected) {
      setReady(null);
      return;
    }
    void refreshReady();
    const id = setInterval(() => void refreshReady(), 2000);
    return () => clearInterval(id);
  }, [connected, status?.state, refreshReady]);

  const choice: RestartChoice | null = useMemo(() => (view ? (picked === null ? suggestedChoice(view) : choiceAt(view, picked)) : null), [view, picked]);

  if (!job) return null;
  const sameMachine = device.id === job.device_id;
  const checklistOk = checklistSatisfied(mask, required, ticked);
  const reason = resumeBlockedReason({
    connected,
    sameMachine,
    jobBusy: busy,
    state: status?.state,
    ready,
    hasPoint: choice !== null,
    unmoved,
    framedRun,
    framedOk,
    checklistOk,
  });
  const mode = homeMode(ready);
  const n = view ? moveCount(view) : 0;
  const toggle = (id: ChecklistItem['id'], on: boolean) =>
    setTicked((prev) => {
      const next = new Set(prev);
      if (on) next.add(id);
      else next.delete(id);
      return next;
    });
  const setBackOpt = (patch: Partial<typeof back>) => {
    const next = { ...back, ...patch };
    writePref('resumeBackLines', String(next.lines));
    writePref('resumeBackMm', String(next.mm));
    setBack(next);
    setPicked(null);
  };

  const home = async () => {
    await sendLine('$H', { confirmed: true });
    await refreshReady();
  };
  const frame = async () => {
    setFraming(true);
    try {
      await getApi().continueFrame({ speed: jogSpeed });
      setFramedRun(true);
    } catch (e) {
      notify('error', errorMessage(e));
    } finally {
      setFraming(false);
    }
  };
  const go = async () => {
    if (!choice || reason) return;
    setSending(true);
    try {
      await getApi().continueJob({ args: { id: job.id, line: choice.line, at: choice.at, confirm_unmoved: unmoved, framed_ok: framedOk, checklist_ok: checklistOk } });
      notify('success', t('machine.resume.started', { line: choice.line + 1 }));
      close();
    } catch (e) {
      notify('error', errorMessage(e));
    } finally {
      setSending(false);
    }
  };

  const when = new Date(job.updated_at_ms).toLocaleString(undefined, { dateStyle: 'short', timeStyle: 'short' });
  return (
    <Modal
      title={t('machine.resume.title')}
      subtitle={t('machine.resume.summary', { percent: fmt((view?.job ?? job).percent, 0), when })}
      tone="warn"
      size="lg"
      onClose={close}
      footer={
        <>
          <button type="button" className="btn btn--danger" onClick={() => void discard()}>{t('machine.resume.discardJob')}</button>
          <button type="button" className="btn" data-autofocus onClick={close}>{t('machine.resume.cancel')}</button>
          <button type="button" className="btn btn--accent" disabled={reason !== null || sending} title={reason ? t(reason) : undefined} onClick={() => void go()}>
            {t('machine.resume.go', { line: choice ? choice.line + 1 : '…' })}
          </button>
        </>
      }
    >
      {!connected ? (
        <p className="warn-line" role="status">
          <TriangleAlert size={14} aria-hidden="true" />
          <span>{t('machine.resume.machine.notConnected', { name: job.device_name })}</span>
          <button type="button" className="btn" onClick={() => { close(); setDockTab('machine'); }}>{t('machine.resume.machine.open')}</button>
        </p>
      ) : !sameMachine ? (
        <p className="warn-line" role="status">
          <TriangleAlert size={14} aria-hidden="true" />
          <span>{t('machine.resume.machine.wrong', { current: device.name, name: job.device_name })}</span>
        </p>
      ) : busy || (status && status.state !== 'idle') ? (
        <p className="warn-line" role="status">
          <TriangleAlert size={14} aria-hidden="true" />
          <span>{busy ? t('machine.resume.machine.job') : t('machine.resume.machine.busy', { state: status?.state ?? '' })}</span>
        </p>
      ) : null}

      <section className="mc-resume__preview" aria-labelledby="rs-preview">
        <h3 id="rs-preview" className="mc-sub">{t('machine.resume.preview.title')}</h3>
        {error ? (
          <p className="warn-line" role="alert">
            <TriangleAlert size={14} aria-hidden="true" />
            <span>{t('machine.resume.preview.failed', { error })}</span>
          </p>
        ) : !view || !choice ? (
          <p className="hint" role="status">{t('machine.resume.preview.loading')}</p>
        ) : (
          <>
            <ResumeCanvas view={view} choice={choice} onPick={setPicked} />
            <ul className="mc-resume__legend" aria-hidden="true">
              <li><span className="mc-resume__key mc-resume__key--done" />{t('machine.resume.legend.done')}</li>
              <li><span className="mc-resume__key mc-resume__key--todo" />{t('machine.resume.legend.todo')}</li>
              <li><span className="mc-resume__key mc-resume__key--marker" />{t('machine.resume.legend.restart')}</li>
            </ul>
            <div className="mc-resume__point">
              <h3 className="mc-sub">{t('machine.resume.point.title')}</h3>
              <div className="row">
                <button type="button" className="icon-btn" title={t('machine.resume.point.prev')} aria-label={t('machine.resume.point.prev')} onClick={() => setPicked(Math.max(1, choice.moveIndex - 1))}>
                  <Minus size={14} aria-hidden="true" />
                </button>
                <input
                  ref={sliderRef}
                  type="range"
                  className="mc-resume__slider"
                  min={1}
                  max={Math.max(1, n - 1)}
                  step={1}
                  value={choice.moveIndex}
                  aria-label={t('machine.resume.point.slider')}
                  aria-valuetext={t('machine.resume.point.at', { line: choice.line + 1, percent: fmt(choice.percent, 0) })}
                  onChange={(e) => setPicked(Number(e.target.value))}
                />
                <button type="button" className="icon-btn" title={t('machine.resume.point.next')} aria-label={t('machine.resume.point.next')} onClick={() => setPicked(Math.min(n - 1, choice.moveIndex + 1))}>
                  <Plus size={14} aria-hidden="true" />
                </button>
                <button type="button" className="btn" disabled={picked === null} onClick={() => setPicked(null)}>{t('machine.resume.point.reset')}</button>
              </div>
              <p className="hint" role="status">
                {t('machine.resume.point.at', { line: choice.line + 1, percent: fmt(choice.percent, 0) })}{' '}
                {choice.custom
                  ? t('machine.resume.point.custom')
                  : view.suggested.row_restart
                    ? t('machine.resume.point.row')
                    : t('machine.resume.point.suggested', { exec: view.suggested.executing_line + 1 })}
              </p>
              <div className="row mc-resume__back">
                <span className="field__label">{t('machine.resume.point.back')}</span>
                <input type="number" className="input num" min={0} max={1000} step={1} value={back.lines} aria-label={t('machine.resume.point.backLines')} onChange={(e) => setBackOpt({ lines: Math.max(0, Math.round(Number(e.target.value) || 0)) })} />
                <span>{t('machine.resume.point.backLines')}</span>
                <input type="number" className="input num" min={0} max={1000} step={0.5} value={back.mm} aria-label={t('machine.resume.point.backMm')} onChange={(e) => setBackOpt({ mm: Math.max(0, Number(e.target.value) || 0) })} />
                <span>{t('machine.resume.point.backMm')}</span>
              </div>
              <p className="hint">{t('machine.resume.point.backHint')}</p>
            </div>
            {view.warnings.length > 0 && (
              <ul className="mc-warnings">
                {view.warnings.map((w, i) => (
                  <li key={i} className="warn-line">
                    <TriangleAlert size={14} aria-hidden="true" />
                    <span>{w}</span>
                  </li>
                ))}
              </ul>
            )}
          </>
        )}
      </section>

      <section className="mc-resume__steps" aria-labelledby="rs-steps">
        <h3 id="rs-steps" className="mc-sub">{t('machine.resume.steps.title')}</h3>
        {mode === 'checking' ? (
          <p className="hint" role="status">{connected ? t('machine.resume.checking') : t('machine.resume.blocked.connect')}</p>
        ) : mode === 'busy' ? (
          <p className="warn-line" role="status">
            <TriangleAlert size={14} aria-hidden="true" />
            <span>{t('machine.resume.steps.busy', { state: ready ? stateLabel(ready.state) : '' })}</span>
          </p>
        ) : mode === 'noReply' ? (
          <p className="warn-line" role="status">
            <TriangleAlert size={14} aria-hidden="true" />
            <span>{t('machine.resume.steps.noReply')}</span>
          </p>
        ) : mode === 'home' ? (
          <div className="mc-resume__step" data-done={ready?.homed ? 'true' : 'false'}>
            <h4>{t('machine.resume.home.title')}</h4>
            <p>{t('machine.resume.home.text')}</p>
            <div className="row">
              <button type="button" className="btn" disabled={!connected || !sameMachine || busy} onClick={() => void home()}>
                <Home size={14} aria-hidden="true" />
                {t('machine.resume.home.run')}
              </button>
              <span role="status" className={ready?.homed ? 'hint' : 'hint hint--danger'}>{ready?.homed ? t('machine.resume.home.done') : t('machine.resume.home.pending')}</span>
            </div>
          </div>
        ) : (
          <div className="mc-resume__step" data-done={unmoved ? 'true' : 'false'}>
            <h4>{t('machine.resume.unmoved.title')}</h4>
            <p>{t('machine.resume.unmoved.text')}</p>
            <p className="warn-line">
              <TriangleAlert size={14} aria-hidden="true" />
              <span>{t('machine.resume.unmoved.warn')}</span>
            </p>
            <label className="mc-check">
              <input type="checkbox" checked={unmoved} onChange={(e) => setUnmoved(e.target.checked)} />
              {t('machine.resume.unmoved.check')}
            </label>
          </div>
        )}

        <div className="mc-resume__step" data-done={framedOk ? 'true' : 'false'}>
          <h4>{t('machine.resume.frame.title')}</h4>
          <p>{t('machine.resume.frame.text')}</p>
          <div className="row">
            <button type="button" className="btn" disabled={!connected || !sameMachine || busy || framing || !idle} onClick={() => void frame()}>
              <ScanLine size={14} aria-hidden="true" />
              {t(framedRun ? 'machine.resume.frame.again' : 'machine.resume.frame.run')}
            </button>
          </div>
          <label className="mc-check">
            <input type="checkbox" checked={framedOk} disabled={!framedRun} onChange={(e) => setFramedOk(e.target.checked)} />
            {t('machine.resume.frame.check')}
          </label>
        </div>

        <div className="mc-resume__step" data-done={checklistOk ? 'true' : 'false'}>
          <h4>{t('machine.resume.checklist.title')}</h4>
          {visibleChecklist(mask).length === 0 ? (
            <p className="hint">{t('machine.checklist.empty')}</p>
          ) : (
            <div className="mc-checklist__items">
              {visibleChecklist(mask).map((i) => (
                <label key={i.id} className="mc-check">
                  <input type="checkbox" checked={ticked.has(i.id)} onChange={(e) => toggle(i.id, e.target.checked)} />
                  {t(i.label)}
                </label>
              ))}
            </div>
          )}
          <p className={required && !checklistOk ? 'hint hint--danger' : 'hint'} role="status">
            {required ? (checklistOk ? t('machine.checklist.ready') : t('machine.checklist.blocked', { n: checklistPending(mask, ticked).length })) : t('machine.checklist.optional')}
          </p>
        </div>

        <div className="mc-resume__step">
          <h4>{t('machine.resume.preamble.title')}</h4>
          <p>{t('machine.resume.preamble.text', { line: choice ? choice.line + 1 : '…' })}</p>
        </div>
        {reason && <p className="hint hint--danger" role="status">{t(reason)}</p>}
      </section>
    </Modal>
  );
}
