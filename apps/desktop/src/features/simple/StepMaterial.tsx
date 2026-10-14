import { Cpu, Library, Zap } from 'lucide-react';
import { useId } from 'react';
import { t } from '@/i18n';
import { selectActiveDevice, useStore } from '@/store';
import { classWatts, classesOfKind } from '@/features/materials/machineClass';
import { describePick } from '@/features/materials/pick';
import type { MachineClass } from '@/api/types';
import { changeMachine, chooseMaterial, chooseWatts } from './actions';
import { ColorCard } from './ColorCard';
import { PICK_KEY, materialReasons } from './model';
import type { SimpleData } from './hooks';
import { ProblemList } from './Problems';
import { useSimple } from './store';

/**
 * "¿De cuántos watts es tu láser?": con la potencia se eligen los ajustes de la clase correcta.
 * Se recuerda para esa máquina. Sin saberla, se usan los ajustes más prudentes.
 */
function WattsQuestion({ data }: { data: SimpleData }) {
  const id = useId();
  const { kind, machineClass } = data.machine;
  if (kind === null) return null;
  return (
    <div className="simple__field" data-testid="watts">
      <label htmlFor={id} className="simple__label"><Zap size={16} aria-hidden="true" />{t('simple.watts.ask')}</label>
      <select id={id} className="select simple__select" value={machineClass ?? ''} onChange={(e) => chooseWatts((e.target.value || null) as MachineClass | null)}>
        <option value="">{t('simple.watts.unknown')}</option>
        {classesOfKind(kind).map((c) => (
          <option key={c} value={c}>{t('simple.watts.option', { w: classWatts(c) })}</option>
        ))}
      </select>
      <p className="hint">{machineClass === null ? t('simple.watts.missing') : t('simple.watts.help')}</p>
    </div>
  );
}

/** De dónde salió el ajuste de cada operación, en palabras simples. */
function PickNotes({ data }: { data: SimpleData }) {
  const { choice, machine } = data;
  if (!choice) return null;
  const lines = [
    ['simple.action.cut', choice.cut] as const,
    ['simple.action.engrave', choice.engrave] as const,
  ].flatMap(([label, pick]) => {
    if (!pick) return [];
    const d = describePick(pick.result, machine.machineClass);
    const key = PICK_KEY[d.key as keyof typeof PICK_KEY];
    return [{ label, text: t(key, d.params) }];
  });
  return (
    <ul className="simple__picks">
      {lines.map((l) => (
        <li key={l.label} className="hint">{t('simple.pick.line', { action: t(l.label), text: l.text })}</li>
      ))}
    </ul>
  );
}

/** Paso 2: la máquina, el material y qué hacer con cada color. */
export function StepMaterial({ data }: { data: SimpleData }) {
  const machineId = useId();
  const materialId = useId();
  const devices = useStore((s) => s.devices);
  const device = useStore(selectActiveDevice);
  const connected = useStore((s) => s.connection.connected);
  const materialKey = useSimple((s) => s.materialKey);
  const actions = useSimple((s) => s.actions);
  const dropped = useSimple((s) => s.dropped);
  const { choices, choice, groups, problems } = data;
  return (
    <>
      <div className="simple__field">
        <label htmlFor={machineId} className="simple__label"><Cpu size={16} aria-hidden="true" />{t('simple.machine')}</label>
        <select
          id={machineId}
          className="select simple__select"
          value={device.id}
          disabled={connected}
          title={connected ? t('machine.disconnectFirst') : undefined}
          onChange={(e) => changeMachine(e.target.value)}
        >
          {devices.map((d) => (
            <option key={d.id} value={d.id}>{d.name}</option>
          ))}
        </select>
        {connected && <p className="hint">{t('machine.disconnectFirst')}</p>}
      </div>

      <WattsQuestion data={data} />

      <div className="simple__field">
        <label htmlFor={materialId} className="simple__label"><Library size={16} aria-hidden="true" />{t('simple.material')}</label>
        <select id={materialId} className="select simple__select" value={materialKey ?? ''} onChange={(e) => e.target.value && chooseMaterial(e.target.value)}>
          <option value="">{t('simple.material.pick')}</option>
          {choices.map((c) => (
            <option key={c.key} value={c.key}>{c.label}</option>
          ))}
        </select>
        {choices.length === 0 && <p className="hint">{t('simple.material.none')}</p>}
        {choice && <p className="hint">{t('simple.material.sets')}</p>}
        {materialReasons(choice, data.machine.kind).map((r) => (
          <p key={r.key} className="hint simple__reason" role="status" data-reason={r.key}>{t(r.key, r.params)}</p>
        ))}
        <PickNotes data={data} />
      </div>

      {dropped.length > 0 && (
        <p className="simple__problem simple__dropped" data-level="warn" role="status">
          {t('simple.dropped', { names: dropped.map((d) => d.name).join(', ') })}
        </p>
      )}

      <ProblemList step="material" problems={problems} />

      <h3 className="simple__sub">{t('simple.colors')}</h3>
      {groups.length > 0 && !groups.some((g) => actions[g.color.toLowerCase()]) && <p className="hint">{t('simple.colors.help')}</p>}
      {groups.length === 0 ? (
        <p className="hint">{t('simple.colors.none')}</p>
      ) : (
        <ul className="simple__colors">
          {groups.map((g) => (
            <ColorCard key={g.color} group={g} choice={choice} />
          ))}
        </ul>
      )}
    </>
  );
}
