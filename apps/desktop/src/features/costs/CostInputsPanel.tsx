import { useId, useState, type ReactNode } from 'react';
import { Save, Trash2 } from 'lucide-react';
import { askChoice } from '@/features/history/prompt';
import { fmt } from '@/lib/platform';
import { t } from '@/i18n';
import { useStore } from '@/store';
import { NumberField } from '@/ui/NumberField';
import { LIMITS, ROUND_STEPS, type MaterialMode, type RoundStep, type TimeSource } from './costModel';
import { CALIBRATION_MIN_JOBS, type JobMetrics } from './costMetrics';
import { useCostStore } from './costStore';
import { CURRENCIES, currencySymbol } from './currency';
import type { CostCalc } from './useCostCalc';
import { machineFor } from './costProfiles';
import { fmtClock } from '@/lib/platform';

function SelectField({ label, value, onChange, children }: { label: string; value: string; onChange: (v: string) => void; children: ReactNode }) {
  const id = useId();
  return (
    <div className="field">
      <label htmlFor={id} className="field__label">{label}</label>
      <select id={id} className="select" value={value} onChange={(e) => onChange(e.target.value)}>
        {children}
      </select>
    </div>
  );
}

/** Texto de ayuda enlazado al campo anterior para lectores de pantalla (sin ser un campo). */
const Help = ({ children }: { children: ReactNode }) => <p className="hint cost__help">{children}</p>;

function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <fieldset className="cost__group">
      <legend className="mc-sub">{title}</legend>
      {children}
    </fieldset>
  );
}

const TIME_SOURCES: readonly TimeSource[] = ['estimate', 'calibrated', 'lastReal'];

function TimeSection({ calc }: { calc: CostCalc }) {
  const source = useCostStore((s) => s.timeSource);
  const setSource = useCostStore((s) => s.setTimeSource);
  const { metrics, inputs, calibration } = calc;
  const chosenButFallback = source !== 'estimate' && calc.timeUsed !== source;
  return (
    <Section title={t('costs.section.time')}>
      <SelectField label={t('costs.time.source')} value={source} onChange={(v) => setSource(v as TimeSource)}>
        {TIME_SOURCES.map((s) => (
          <option key={s} value={s} disabled={(s === 'calibrated' && !calibration) || (s === 'lastReal' && !calc.hasLastReal)}>
            {t(`costs.time.${s}`)}
          </option>
        ))}
      </SelectField>
      {metrics.hasPreview ? (
        <p className="hint">{t('costs.time.info', { time: fmtClock(inputs.job.runS), on: fmtClock(inputs.job.laserOnS) })}</p>
      ) : (
        <p className="hint">{t('costs.time.none')}</p>
      )}
      {calibration ? (
        <p className="hint">{t('costs.time.factor', { machine: calc.device.name, factor: fmt(calibration.factor, 2), n: calibration.jobs })}</p>
      ) : (
        <p className="hint">{t('costs.time.minJobs', { n: CALIBRATION_MIN_JOBS })}</p>
      )}
      {chosenButFallback && <p className="hint hint--warn">{t('costs.time.fallback', { machine: calc.device.name })}</p>}
    </Section>
  );
}

function MachineSection({ calc, symbol }: { calc: CostCalc; symbol: string }) {
  const stored = useCostStore();
  const m = machineFor(stored, calc.device.id);
  const set = (patch: Partial<typeof m>) => stored.setMachine(calc.device.id, patch);
  return (
    <Section title={t('costs.section.machine')}>
      <p className="hint">{t('costs.machine.for', { name: calc.device.name })}</p>
      <div className="field-grid">
        <NumberField label={t('costs.machine.rate')} unit={`${symbol}/h`} value={m.hourlyRate} min={LIMITS.money.min} max={LIMITS.money.max} step={1} onCommit={(hourlyRate) => set({ hourlyRate })} />
        <NumberField label={t('costs.machine.tubePrice')} unit={symbol} value={m.tubePrice} min={LIMITS.money.min} max={LIMITS.money.max} step={10} onCommit={(tubePrice) => set({ tubePrice })} />
        <NumberField label={t('costs.machine.tubeLife')} unit="h" value={m.tubeLifeH} min={LIMITS.hours.min} max={LIMITS.hours.max} step={100} onCommit={(tubeLifeH) => set({ tubeLifeH })} />
        <NumberField label={t('costs.machine.watts')} unit="W" value={m.watts} min={LIMITS.watts.min} max={LIMITS.watts.max} step={10} onCommit={(watts) => set({ watts })} />
        <NumberField label={t('costs.machine.accessory')} unit="W" value={m.accessoryWatts} min={LIMITS.watts.min} max={LIMITS.watts.max} step={10} onCommit={(accessoryWatts) => set({ accessoryWatts })} />
        <NumberField label={t('costs.machine.kwh')} unit={`${symbol}/kWh`} value={stored.sales.kwhPrice} min={LIMITS.money.min} max={LIMITS.money.max} step={0.1} onCommit={(kwhPrice) => stored.setSales({ kwhPrice })} />
      </div>
      <Help>{t('costs.machine.rate.help')} {t('costs.machine.wear.help')} {t('costs.machine.accessory.help')}</Help>
    </Section>
  );
}

function ProfileRow() {
  const materials = useCostStore((s) => s.materials);
  const selected = useCostStore((s) => s.materialName);
  const load = useCostStore((s) => s.loadMaterialProfile);
  const save = useCostStore((s) => s.saveMaterialProfile);
  const remove = useCostStore((s) => s.deleteMaterialProfile);
  const notify = useStore((s) => s.notify);
  const [typed, setTyped] = useState<string | null>(null);
  const nameId = useId();
  const name = typed ?? selected ?? '';

  const onSave = () => {
    if (save(name)) {
      notify('success', t('costs.mat.saved', { name: name.trim() }));
      setTyped(null);
    } else {
      notify('warning', t('costs.mat.nameNeeded'));
    }
  };
  const onDelete = async () => {
    if (!selected) return;
    const choice = await askChoice({
      title: t('costs.mat.delete.title'),
      message: t('costs.mat.delete.message', { name: selected }),
      choices: [
        { id: 'ok', label: t('costs.mat.delete.confirm'), tone: 'danger' },
        { id: 'cancel', label: t('costs.mat.delete.cancel') },
      ],
      cancelId: 'cancel',
    });
    if (choice !== 'ok') return;
    remove(selected);
    setTyped(null);
    notify('info', t('costs.mat.deleted', { name: selected }));
  };

  return (
    <>
      <SelectField
        label={t('costs.mat.profile')}
        value={selected ?? ''}
        onChange={(v) => {
          setTyped(null);
          if (v) load(v);
        }}
      >
        <option value="">{t('costs.mat.noProfile')}</option>
        {materials.map((p) => (
          <option key={p.name} value={p.name}>{p.name}</option>
        ))}
      </SelectField>
      <div className="field">
        <label htmlFor={nameId} className="field__label">{t('costs.mat.name')}</label>
        <div className="row">
          <input id={nameId} className="input grow" type="text" maxLength={80} value={name} onChange={(e) => setTyped(e.target.value)} />
          <button type="button" className="btn" onClick={onSave}>
            <Save size={14} aria-hidden="true" />{t('costs.mat.save')}
          </button>
          <button type="button" className="btn" disabled={!selected} onClick={() => void onDelete()}>
            <Trash2 size={14} aria-hidden="true" />{t('costs.mat.delete')}
          </button>
        </div>
      </div>
    </>
  );
}

const MODES: readonly MaterialMode[] = ['bbox', 'area', 'sheet'];

function MaterialSection({ symbol }: { symbol: string }) {
  const material = useCostStore((s) => s.material);
  const mode = useCostStore((s) => s.materialMode);
  const setMaterial = useCostStore((s) => s.setMaterial);
  const setMode = useCostStore((s) => s.setMaterialMode);
  return (
    <Section title={t('costs.section.material')}>
      <ProfileRow />
      <SelectField label={t('costs.mat.mode')} value={mode} onChange={(v) => setMode(v as MaterialMode)}>
        {MODES.map((m) => (
          <option key={m} value={m}>{t(`costs.mat.${m}`)}</option>
        ))}
      </SelectField>
      <div className="field-grid">
        <NumberField label={t('costs.mat.sheetPrice')} unit={symbol} value={material.sheetPrice} min={LIMITS.money.min} max={LIMITS.money.max} step={1} onCommit={(sheetPrice) => setMaterial({ sheetPrice })} />
        {mode === 'sheet' ? (
          <NumberField label={t('costs.mat.piecesPerSheet')} value={material.piecesPerSheet} min={1} max={LIMITS.count.max} step={1} onCommit={(piecesPerSheet) => setMaterial({ piecesPerSheet: Math.round(piecesPerSheet) })} />
        ) : (
          <>
            <NumberField label={t('costs.mat.sheetW')} unit="mm" value={material.sheetW} min={LIMITS.mm.min} max={LIMITS.mm.max} step={10} onCommit={(sheetW) => setMaterial({ sheetW })} />
            <NumberField label={t('costs.mat.sheetH')} unit="mm" value={material.sheetH} min={LIMITS.mm.min} max={LIMITS.mm.max} step={10} onCommit={(sheetH) => setMaterial({ sheetH })} />
            <NumberField label={t('costs.mat.waste')} unit="%" value={material.wastePct} min={LIMITS.pct.min} max={LIMITS.pct.max} step={1} onCommit={(wastePct) => setMaterial({ wastePct })} />
          </>
        )}
      </div>
      <Help>{mode === 'sheet' ? t('costs.mat.piecesPerSheet.help') : t('costs.mat.waste.help')}</Help>
    </Section>
  );
}

function LaborSection({ symbol }: { symbol: string }) {
  const sales = useCostStore((s) => s.sales);
  const setSales = useCostStore((s) => s.setSales);
  return (
    <Section title={t('costs.section.labor')}>
      <div className="field-grid">
        <NumberField label={t('costs.labor.rate')} unit={`${symbol}/h`} value={sales.laborRate} min={LIMITS.money.min} max={LIMITS.money.max} step={1} onCommit={(laborRate) => setSales({ laborRate })} />
        <NumberField label={t('costs.labor.consumables')} unit={symbol} value={sales.consumables} min={LIMITS.money.min} max={LIMITS.money.max} step={0.5} onCommit={(consumables) => setSales({ consumables })} />
        <NumberField label={t('costs.labor.prep')} unit="min" value={sales.prepMin} min={LIMITS.minutes.min} max={LIMITS.minutes.max} step={5} onCommit={(prepMin) => setSales({ prepMin })} />
        <NumberField label={t('costs.labor.finish')} unit="min" value={sales.finishMin} min={LIMITS.minutes.min} max={LIMITS.minutes.max} step={1} onCommit={(finishMin) => setSales({ finishMin })} />
      </div>
      <Help>{t('costs.labor.prep.help')}</Help>
    </Section>
  );
}

function SaleSection({ metrics, tag }: { metrics: JobMetrics; tag: string }) {
  const sales = useCostStore((s) => s.sales);
  const setSales = useCostStore((s) => s.setSales);
  const quantity = useCostStore((s) => s.quantity);
  const setQuantity = useCostStore((s) => s.setQuantity);
  const pieces = useCostStore((s) => s.piecesInDesign);
  const setPieces = useCostStore((s) => s.setPiecesInDesign);
  return (
    <Section title={t('costs.section.sale')}>
      <div className="field-grid">
        <SelectField label={t('costs.sale.currency')} value={sales.currency} onChange={(currency) => setSales({ currency })}>
          {CURRENCIES.map((c) => (
            <option key={c} value={c}>{`${c} (${currencySymbol(c, tag)})`}</option>
          ))}
        </SelectField>
        <NumberField label={t('costs.sale.quantity')} value={quantity} min={1} max={LIMITS.count.max} step={1} onCommit={(n) => setQuantity(Math.round(n))} />
        <NumberField label={t('costs.sale.pieces')} value={pieces} min={1} max={LIMITS.count.max} step={1} onCommit={(n) => setPieces(Math.round(n))} />
        <div className="field cost__use">
          <button type="button" className="btn" disabled={metrics.pieces < 1 || metrics.pieces === pieces} onClick={() => setPieces(metrics.pieces)}>
            {t('costs.sale.pieces.use', { n: metrics.pieces })}
          </button>
        </div>
        <NumberField label={t('costs.sale.margin')} unit="%" value={sales.margin} min={LIMITS.margin.min} max={LIMITS.margin.max} step={5} onCommit={(margin) => setSales({ margin })} />
        <SelectField label={t('costs.sale.marginBase')} value={sales.marginBase} onChange={(v) => setSales({ marginBase: v as 'cost' | 'price' })}>
          <option value="cost">{t('costs.margin.onCost')}</option>
          <option value="price">{t('costs.margin.onPrice')}</option>
        </SelectField>
        <NumberField label={t('costs.sale.commission')} unit="%" value={sales.commission} min={LIMITS.pct.min} max={LIMITS.pct.max} step={0.5} onCommit={(commission) => setSales({ commission })} />
        <NumberField label={t('costs.sale.tax')} unit="%" value={sales.tax} min={LIMITS.pct.min} max={LIMITS.pct.max} step={0.5} onCommit={(tax) => setSales({ tax })} />
        <SelectField label={t('costs.sale.round')} value={String(sales.roundTo)} onChange={(v) => setSales({ roundTo: Number(v) as RoundStep })}>
          {ROUND_STEPS.map((r) => (
            <option key={r} value={String(r)}>{t(`costs.round.${r}`)}</option>
          ))}
        </SelectField>
      </div>
      <Help>{t('costs.sale.pieces.help')} {t('costs.sale.commission.help')} {t('costs.sale.tax.help')}</Help>
    </Section>
  );
}

/** Las entradas de la calculadora, agrupadas por tema. */
export function CostInputsPanel({ calc }: { calc: CostCalc }) {
  const symbol = currencySymbol(calc.inputs.sales.currency, calc.tag);
  return (
    <div className="cost__inputs" role="group" aria-label={t('costs.inputs')}>
      <TimeSection calc={calc} />
      <MachineSection calc={calc} symbol={symbol} />
      <MaterialSection symbol={symbol} />
      <LaborSection symbol={symbol} />
      <SaleSection metrics={calc.metrics} tag={calc.tag} />
    </div>
  );
}
