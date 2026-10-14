import { Crosshair, EyeOff, Route, Spline, Square, TriangleAlert } from 'lucide-react';
import { useEffect, useState } from 'react';
import { getApi } from '@/api';
import type { GalvoState, GalvoStepInfo } from '@/api/types';
import { t } from '@/i18n';
import { fmt } from '@/lib/platform';
import { useStore } from '@/store';
import { sendBlockedReason } from '@/store/machine';
import { GalvoExperimentalBadge, GalvoNotApplicable, moveText, stepLabel } from './GalvoNotice';
import { GALVO_VID_PID, hex16, inputBits, SOURCE_WAVELENGTH_NM, selectIsGalvo } from './galvoPorts';

/** Cada cuánto se pide el estado de la tarjeta mientras hay conexión (ms). */
const POLL_MS = 1000;

/** Lo que el panel hace; el panel conectado lo cablea al almacén. */
export interface GalvoActions {
  onFrame: () => void;
  onContour: () => void;
  onRedRun: () => void;
  onRedLight: (on: boolean) => void;
  onStop: () => void;
  onContinue: () => void;
}

interface PanelProps {
  connected: boolean;
  state: GalvoState | null;
  /** Paso que se está marcando. */
  step: GalvoStepInfo | null;
  /** Vueltas del encuadre. */
  loops: number;
  /** Motivo por el que no se puede encuadrar ni marcar ahora (clave ya traducida), o `null`. */
  blocked: string | null;
  actions: GalvoActions;
}

function Row({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <tr>
      <th scope="row">{label}</th>
      <td className="num">{children}</td>
    </tr>
  );
}

/** Las entradas de la tarjeta, bit a bit, SOLO LECTURA. */
export function GalvoInputs({ inputs, known }: { inputs: number; known: boolean }) {
  return (
    <div className="mc-galvo-inputs">
      <h4 className="mc-sub">{t('galvo.panel.inputs')}</h4>
      {known ? (
        <>
          <p className="num" aria-label={t('galvo.panel.inputs.value')}>{hex16(inputs)}</p>
          <ul className="mc-bits" aria-label={t('galvo.panel.inputs.bits')}>
            {inputBits(inputs).map(({ bit, on }) => (
              <li key={bit} className="mc-bit num" data-on={on || undefined} title={t(on ? 'galvo.panel.inputs.high' : 'galvo.panel.inputs.low', { bit })}>
                <span className="sr-only">{t(on ? 'galvo.panel.inputs.high' : 'galvo.panel.inputs.low', { bit })}</span>
                <span aria-hidden="true">{bit}</span>
              </li>
            ))}
          </ul>
        </>
      ) : (
        <p className="hint">{t('galvo.panel.inputs.unknown')}</p>
      )}
      <p className="hint">{t('galvo.panel.inputs.note')}</p>
    </div>
  );
}

/**
 * Panel del galvo (EXPERIMENTAL): estado de la tarjeta, encuadre con el puntero rojo (y apagarlo), entradas en
 * solo lectura y lo que no aplica con su motivo. Recibe el estado ya leído para poder probarse por separado.
 */
export function GalvoPanel({ connected, state, step, loops, blocked, actions }: PanelProps) {
  const waiting = state?.waiting_move ?? null;
  const working = !!state?.working;
  return (
    <section className="section" aria-labelledby="sec-galvo">
      <h3 id="sec-galvo" className="panel-title">
        {t('galvo.panel.title')} <GalvoExperimentalBadge />
      </h3>
      <div className="warn-line" role="note">
        <TriangleAlert size={14} aria-hidden="true" />
        <div>
          <p>{t('galvo.experimental.note')}</p>
          <p className="warn-line__key">{t('galvo.panel.invisible')}</p>
        </div>
      </div>
      {!connected && <p className="hint">{t('galvo.panel.needConnect')}</p>}
      {connected && state && (
        <>
          {waiting && (
            <div className="mc-galvo-wait" role="alert">
              <p><strong>{t('galvo.panel.waiting')}</strong> {stepLabel(waiting)}</p>
              <p className="num">{moveText(waiting)}</p>
              <div className="row">
                <button type="button" className="btn btn--accent grow" onClick={actions.onContinue}>{t('galvo.move.continue')}</button>
                <button type="button" className="btn btn--danger" onClick={actions.onStop}>{t('galvo.move.stop')}</button>
              </div>
            </div>
          )}
          {step && working && !waiting && <p className="hint num" role="status">{t('galvo.panel.marking', { step: stepLabel(step) })}</p>}
          <table className="mc-table" aria-label={t('galvo.panel.title')}>
            <tbody>
              <Row label={t('galvo.panel.board')}>{state.board}</Row>
              <Row label={t('galvo.panel.usb')}>{state.virtual_machine ? t('galvo.panel.usb.virtual') : GALVO_VID_PID}</Row>
              <Row label={t('galvo.panel.serial')}>{state.serial}</Row>
              <Row label={t('galvo.panel.version')}>{state.version}</Row>
              <Row label={t('galvo.panel.status')}>
                {state.busy ? t('galvo.panel.status.busy') : state.ready ? t('galvo.panel.status.ready') : t('galvo.panel.status.wait')} ({hex16(state.status_raw)})
              </Row>
              <Row label={t('galvo.panel.field')}>
                {t('galvo.panel.field.value', { mm: fmt(state.field_mm, 0), source: t(`galvo.source.${state.source}`), nm: SOURCE_WAVELENGTH_NM[state.source] })}
              </Row>
              <Row label={t('galvo.panel.position')}>
                {state.position_known ? `X ${fmt(state.position[0], 2)} · Y ${fmt(state.position[1], 2)}` : t('galvo.panel.position.unknown')}
              </Row>
              <Row label={t('galvo.panel.pointer')}>{state.red_light_on ? t('galvo.panel.pointer.on') : t('galvo.panel.pointer.off')}</Row>
              <Row label={t('galvo.panel.source')}>{state.laser_may_be_on ? t('galvo.panel.source.maybe') : t('galvo.panel.source.off')}</Row>
            </tbody>
          </table>
          {state.laser_may_be_on && (
            <p className="warn-line mc-galvo-stop" role="alert">
              <TriangleAlert size={14} aria-hidden="true" />
              <span>{t('galvo.stop.text')}</span>
            </p>
          )}
          <h4 className="mc-sub">{t('galvo.panel.frame')}</h4>
          <p className="hint">{t('galvo.panel.frame.help')}</p>
          <div className="row">
            <button type="button" className="btn grow" disabled={blocked !== null} title={blocked ?? t('galvo.panel.frame.rect.help')} onClick={actions.onFrame}>
              <Crosshair size={15} aria-hidden="true" />{t('galvo.panel.frame.rect')}
            </button>
            <button type="button" className="btn grow" disabled={blocked !== null} title={blocked ?? t('galvo.panel.frame.contour.help')} onClick={actions.onContour}>
              <Spline size={15} aria-hidden="true" />{t('galvo.panel.frame.contour')}
            </button>
          </div>
          <div className="row">
            <button type="button" className="btn grow" disabled={blocked !== null} title={blocked ?? t('galvo.panel.redRun.help')} onClick={actions.onRedRun}>
              <Route size={15} aria-hidden="true" />{t('galvo.panel.redRun')}
            </button>
            <button type="button" className="btn grow" disabled={working || state.red_light_on} title={working ? t('reason.jobRunning') : t('galvo.panel.pointer.on.help')} onClick={() => actions.onRedLight(true)}>
              <Crosshair size={15} aria-hidden="true" />{t('galvo.panel.pointer.turnOn')}
            </button>
          </div>
          <div className="row">
            <button type="button" className="btn grow" disabled={!state.red_light_on} title={state.red_light_on ? undefined : t('galvo.panel.pointer.alreadyOff')} onClick={() => actions.onRedLight(false)}>
              <EyeOff size={15} aria-hidden="true" />{t('galvo.panel.pointer.turnOff')}
            </button>
            <button type="button" className="btn btn--danger grow" onClick={actions.onStop}>
              <Square size={15} aria-hidden="true" />{t('galvo.panel.stop')}
            </button>
          </div>
          {working && loops > 0 && !step && <p className="hint num" role="status">{t('galvo.panel.loops', { n: loops })}</p>}
          <GalvoInputs inputs={state.inputs} known={state.inputs_known} />
        </>
      )}
      <h4 className="mc-sub">{t('galvo.panel.notApplicable')}</h4>
      <ul className="mc-list">
        {(['galvo.console.reason', 'galvo.override.reason', 'galvo.mset.reason', 'galvo.pause.reason'] as const).map((k) => (
          <li key={k}><GalvoNotApplicable reason={k} /></li>
        ))}
      </ul>
      <p className="hint">{t('galvo.panel.noG')}</p>
    </section>
  );
}

/** El panel con su estado vivo; solo aparece con un perfil galvo. */
export function GalvoSection() {
  const isGalvo = useStore(selectIsGalvo);
  const connected = useStore((s) => s.connection.connected);
  // Cambia con cada trabajo, encuadre o salto: es cuando conviene volver a leer.
  const jobState = useStore((s) => s.jobState);
  const machineState = useStore((s) => s.status?.state);
  const waitStep = useStore((s) => s.galvoWait);
  const step = useStore((s) => s.galvoStep);
  const loops = useStore((s) => s.galvoLoops);
  const blockedKey = useStore(sendBlockedReason);
  const frameJob = useStore((s) => s.frameJob);
  const frameHull = useStore((s) => s.frameHull);
  const redRun = useStore((s) => s.galvoRedRun);
  const setRedLight = useStore((s) => s.setGalvoRedLight);
  const cancel = useStore((s) => s.cancelJob);
  const confirm = useStore((s) => s.confirmGalvoMove);
  const [state, setState] = useState<GalvoState | null>(null);
  useEffect(() => {
    if (!isGalvo || !connected) return;
    let alive = true;
    const read = async () => {
      try {
        const s = await getApi().galvoState();
        if (alive) setState(s);
      } catch {
        // La conexión puede cerrarse entre dos lecturas: la próxima lo corrige.
        if (alive) setState(null);
      }
    };
    void read();
    const timer = setInterval(() => void read(), POLL_MS);
    return () => {
      alive = false;
      clearInterval(timer);
    };
  }, [isGalvo, connected, jobState, machineState, waitStep]);
  if (!isGalvo) return null;
  const blocked = blockedKey ? t(blockedKey) : null;
  return (
    <GalvoPanel
      connected={connected}
      state={connected ? state : null}
      step={step}
      loops={loops}
      blocked={blocked}
      actions={{
        onFrame: () => void frameJob(),
        onContour: () => void frameHull(),
        onRedRun: () => void redRun(),
        onRedLight: (on) => void setRedLight(on),
        onStop: () => void cancel(),
        onContinue: () => void confirm(),
      }}
    />
  );
}
