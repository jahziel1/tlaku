import { X } from 'lucide-react';
import { useEffect, useId, useMemo, useRef, useState } from 'react';
import { getApi } from '@/api';
import type { ProbeResult } from '@/api/api';
import type { DeviceKind } from '@/bindings/DeviceKind';
import { t, type I18nKey } from '@/i18n';
import { errorMessage } from '@/lib/errors';
import { parseGrblVersion } from '@/lib/device';
import { withoutRuidaPort } from '@/features/machine/ruidaPorts';
import { useStore } from '@/store';
import { Modal } from '@/ui/Modal';
import { findEntry } from './catalog';
import { compareWithMachine, defaultMachineChoices, type CompareField } from './compare';
import { detectionFrom, rankCatalog } from './filter';
import { buildProfile } from './profile';
import { DetectStep, ModelStep, OTHER_ID, ReviewStep, SaveStep, type ProbeState } from './steps';
import { useDevicesWizard } from './wizardStore';
import './devices.css';

export type WizardStep = 'detect' | 'model' | 'review' | 'save';
export const STEPS: { id: WizardStep; label: I18nKey }[] = [
  { id: 'detect', label: 'catalog.step.detect' },
  { id: 'model', label: 'catalog.step.model' },
  { id: 'review', label: 'catalog.step.review' },
  { id: 'save', label: 'catalog.step.save' },
];

/** Marco con los pasos numerados (solo presentación). */
export function StepList({ current }: { current: WizardStep }) {
  const idx = STEPS.findIndex((s) => s.id === current);
  return (
    <ol className="dev-steps" aria-label={t('catalog.steps')}>
      {STEPS.map((s, i) => (
        <li key={s.id} aria-current={s.id === current ? 'step' : undefined} data-done={i < idx || undefined}>
          <span className="dev-steps__n num">{i + 1}</span> {t(s.label)}
        </li>
      ))}
    </ol>
  );
}

/** Dialogo "Agregar máquina": detectar, elegir modelo, revisar contra la máquina y guardar. */
export function AddMachineWizard() {
  const titleId = useId();
  const close = useDevicesWizard((s) => s.close);
  // La detección es de puertos serie: la máquina virtual de Ruida no es una opción aquí.
  const allPorts = useStore((s) => s.ports);
  const ports = useMemo(() => withoutRuidaPort(allPorts), [allPorts]);
  const refreshPorts = useStore((s) => s.refreshPorts);
  const connected = useStore((s) => s.connection.connected);
  const storePort = useStore((s) => s.selectedPort);
  const storeBaud = useStore((s) => s.baud);
  const devices = useStore((s) => s.devices);
  const saveDevice = useStore((s) => s.saveDevice);
  const setActiveDevice = useStore((s) => s.setActiveDevice);
  const setSelectedPort = useStore((s) => s.setSelectedPort);
  const setBaud = useStore((s) => s.setBaud);
  const notify = useStore((s) => s.notify);

  const [step, setStep] = useState<WizardStep>('detect');
  const [port, setPort] = useState(storePort ?? '');
  const [baud, setProbeBaud] = useState(storeBaud);
  const [probe, setProbe] = useState<ProbeState>({ status: 'idle' });
  const [probedBaud, setProbedBaud] = useState<number | null>(null);
  const [query, setQuery] = useState('');
  const [showAll, setShowAll] = useState(false);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [otherKind, setOtherKind] = useState<DeviceKind>('grbl');
  const [choices, setChoices] = useState<Set<CompareField>>(new Set());
  const [name, setName] = useState('');
  /** Ruida: IP de la controladora, escrita a mano. */
  const [host, setHost] = useState('');
  const [nameTouched, setNameTouched] = useState(false);
  const [saving, setSaving] = useState(false);
  /** Detecciones lanzadas: una respuesta tardía de una anterior se descarta. */
  const run = useRef(0);
  const alive = useRef(true);

  useEffect(() => {
    alive.current = true;
    void refreshPorts();
    return () => {
      alive.current = false;
    };
  }, [refreshPorts]);

  // Si la lista de puertos llega después, se elige el primero.
  useEffect(() => {
    if (!port && ports[0]) setPort(ports[0]);
  }, [ports, port]);

  const result: ProbeResult | null = probe.status === 'done' ? probe.result : null;
  const detection = useMemo(() => detectionFrom(result), [result]);
  const ranked = useMemo(() => rankCatalog({ query, detection, includeIncompatible: showAll }), [query, detection, showAll]);
  const entry = selectedId && selectedId !== OTHER_ID ? (findEntry(selectedId) ?? null) : null;
  const kind: DeviceKind = entry ? entry.kind : selectedId === OTHER_ID ? otherKind : (result?.suggested_kind ?? 'grbl');
  const comparison = useMemo(() => (result?.settings ? compareWithMachine(entry, result.settings) : null), [entry, result]);

  const defaultName = entry ? (entry.generic ? entry.model : `${entry.brand} ${entry.model}`) : t('catalog.other.name');
  const shownName = nameTouched ? name : defaultName;
  const built = useMemo(
    () => buildProfile({ entry, kind, name: shownName || defaultName, takenIds: devices.map((d) => d.id), probedBaud, settings: result?.settings ?? null, useMachine: choices }),
    [entry, kind, shownName, defaultName, devices, probedBaud, result, choices],
  );

  const detect = async () => {
    const mine = ++run.current;
    setProbe({ status: 'probing' });
    try {
      const r = await getApi().probePort({ port, baud });
      if (!alive.current || mine !== run.current) return;
      setProbe({ status: 'done', result: r });
      setProbedBaud(baud);
      setShowAll(false);
      // Se parte del tipo detectado: lo que difiera queda a la vista del usuario en "Revisar".
      setOtherKind(r.suggested_kind);
    } catch (e) {
      if (!alive.current || mine !== run.current) return;
      setProbe({ status: 'error', message: errorMessage(e) });
      setProbedBaud(null);
    }
  };

  const select = (id: string) => {
    setSelectedId(id);
    setNameTouched(false);
    setChoices(defaultMachineChoices(compareWithMachine(id === OTHER_ID ? null : (findEntry(id) ?? null), result?.settings ?? null)));
  };

  const hasSettings = !!comparison && !comparison.nothingRead;
  // Revisar sirve para comparar; sin ajustes leídos solo se avisa, pero el paso se muestra igual.
  const order = STEPS.map((s) => s.id);
  const idx = order.indexOf(step);
  const canNext = step === 'detect' ? true : step === 'model' ? selectedId !== null : step === 'review' ? true : false;
  const nameOk = shownName.trim() !== '';

  const save = async () => {
    if (!nameOk || saving) return;
    setSaving(true);
    const profile = built.profile.kind === 'ruida' ? { ...built.profile, connection: { type: 'udp' as const, host: host.trim() } } : built.profile;
    const ok = await saveDevice(profile);
    if (!alive.current) return;
    setSaving(false);
    if (!ok) return;
    setActiveDevice(profile.id);
    if (!connected) {
      if (probe.status === 'done' && port) setSelectedPort(port);
      setBaud(profile.baud);
    }
    notify('success', t('catalog.saved', { name: profile.name }));
    close();
  };

  const oldGrbl = !!result && result.family === 'grbl' && parseGrblVersion(result.firmware)?.modernEnough === false;
  const chosenKind = built.profile.kind;
  const kindMismatch =
    result && result.family !== 'unknown' && result.suggested_kind !== chosenKind && !(entry?.alsoKinds ?? []).includes(result.suggested_kind)
      ? { detected: result.suggested_kind, chosen: chosenKind }
      : null;

  return (
    <Modal labelId={titleId} onClose={close} closeOnBackdrop={false} backdropClassName="dev-backdrop" cardClassName="dev-modal">
      <header className="dev-head">
        <div>
          <h2 id={titleId} className="dev-title">{t('catalog.title')}</h2>
          <p className="hint">{t('catalog.subtitle')}</p>
        </div>
        <button type="button" className="icon-btn" title={t('catalog.cancel')} aria-label={t('catalog.cancel')} onClick={close}>
          <X size={16} aria-hidden="true" />
        </button>
      </header>
      <StepList current={step} />
      <div className="dev-body">
        {step === 'detect' && (
          <DetectStep
            ports={ports}
            port={port}
            baud={baud}
            probe={probe}
            machineConnected={connected}
            onPort={setPort}
            onBaud={setProbeBaud}
            onRefresh={() => void refreshPorts()}
            onDetect={() => void detect()}
          />
        )}
        {step === 'model' && (
          <ModelStep
            ranked={ranked}
            query={query}
            onQuery={setQuery}
            selectedId={selectedId}
            onSelect={select}
            probe={result}
            showAll={showAll}
            onShowAll={setShowAll}
            otherKind={otherKind}
            onOtherKind={setOtherKind}
          />
        )}
        {step === 'review' && (
          <ReviewStep
            comparison={hasSettings ? comparison : null}
            useMachine={choices}
            onToggle={(field, on) =>
              setChoices((c) => {
                const next = new Set(c);
                if (on) next.add(field);
                else next.delete(field);
                return next;
              })
            }
            kindMismatch={kindMismatch}
            oldGrbl={oldGrbl}
          />
        )}
        {step === 'save' && <SaveStep name={shownName} onName={(n) => { setName(n); setNameTouched(true); }} built={built} host={host} onHost={setHost} />}
      </div>
      <footer className="dev-foot">
        {step === 'detect' && (
          <button type="button" className="btn" onClick={() => setStep('model')}>{t('catalog.detect.skip')}</button>
        )}
        <span className="grow" />
        <button type="button" className="btn" onClick={close}>{t('catalog.cancel')}</button>
        {idx > 0 && <button type="button" className="btn" onClick={() => setStep(order[idx - 1]!)}>{t('catalog.back')}</button>}
        {step !== 'save' ? (
          <button type="button" className="btn btn--accent" disabled={!canNext} onClick={() => setStep(order[idx + 1]!)}>{t('catalog.next')}</button>
        ) : (
          <button type="button" className="btn btn--accent" disabled={!nameOk || saving} onClick={() => void save()}>
            {saving ? t('catalog.saving') : t('catalog.save')}
          </button>
        )}
      </footer>
    </Modal>
  );
}
