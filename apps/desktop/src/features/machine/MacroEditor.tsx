import { ArrowDown, ArrowUp, Plus, Trash2 } from 'lucide-react';
import { useId } from 'react';
import type { DeviceProfile, Macro } from '@/api/types';
import { t } from '@/i18n';
import { MAX_MACROS, planMacro } from './macros';

/** Mensaje del problema de una macro, o null si está bien. */
function problemText(macro: Macro, rxBuffer: number): string | null {
  if (macro.name.trim() === '') return t('machine.macros.err.name');
  const plan = planMacro(macro, rxBuffer);
  if (plan.ok) return null;
  switch (plan.error.code) {
    case 'empty':
      return t('machine.macros.err.empty');
    case 'tooMany':
      return t('machine.macros.err.tooMany', { max: plan.error.max });
    case 'realtime':
      return t('machine.macros.err.realtime', { line: plan.error.line, text: plan.error.text });
    case 'line':
      return t('machine.macros.err.line', { line: plan.error.line });
  }
}

function MacroRow({ macro, index, total, rxBuffer, onChange, onRemove, onMove }: {
  macro: Macro;
  index: number;
  total: number;
  rxBuffer: number;
  onChange: (patch: Partial<Macro>) => void;
  onRemove: () => void;
  onMove: (delta: -1 | 1) => void;
}) {
  const id = useId();
  const problem = problemText(macro, rxBuffer);
  const plan = planMacro(macro, rxBuffer);
  const willAsk = plan.ok && plan.confirm && !macro.confirm;
  return (
    <div className="mc-macro" data-invalid={problem ? true : undefined}>
      <div className="row">
        <label htmlFor={`${id}-name`} className="field__label grow">{t('machine.macros.nameN', { n: index + 1 })}</label>
        <button type="button" className="icon-btn" disabled={index === 0} title={t('machine.macros.up')} aria-label={t('machine.macros.up')} onClick={() => onMove(-1)}>
          <ArrowUp size={14} aria-hidden="true" />
        </button>
        <button type="button" className="icon-btn" disabled={index === total - 1} title={t('machine.macros.down')} aria-label={t('machine.macros.down')} onClick={() => onMove(1)}>
          <ArrowDown size={14} aria-hidden="true" />
        </button>
        <button type="button" className="icon-btn" title={t('machine.macros.remove')} aria-label={t('machine.macros.removeN', { n: index + 1 })} onClick={onRemove}>
          <Trash2 size={14} aria-hidden="true" />
        </button>
      </div>
      <input id={`${id}-name`} className="input" value={macro.name} maxLength={40} aria-invalid={problem ? true : undefined} onChange={(e) => onChange({ name: e.target.value })} />
      <label htmlFor={`${id}-gcode`} className="field__label">{t('machine.macros.gcode')}</label>
      <textarea
        id={`${id}-gcode`}
        className="input num mc-textarea"
        rows={4}
        spellCheck={false}
        aria-invalid={problem ? true : undefined}
        aria-describedby={`${id}-help`}
        value={macro.gcode}
        onChange={(e) => onChange({ gcode: e.target.value })}
      />
      <label className="mc-check">
        <input type="checkbox" checked={macro.confirm} onChange={(e) => onChange({ confirm: e.target.checked })} />
        <span>{t('machine.macros.confirm')}</span>
      </label>
      {problem && <p className="hint hint--danger" role="alert">{problem}</p>}
      {willAsk && <p id={`${id}-help`} className="hint">{t('machine.macros.willAsk')}</p>}
      {!willAsk && <p id={`${id}-help`} className="hint">{t('machine.macros.help')}</p>}
    </div>
  );
}

/** Editor de las macros del perfil (hasta diez): nombre, G-code de varias líneas y confirmar antes de enviar. */
export function MacroEditor({ draft, edit }: { draft: DeviceProfile; edit: (patch: Partial<DeviceProfile>) => void }) {
  const macros = draft.macros;
  const set = (next: Macro[]) => edit({ macros: next });
  return (
    <div className="mc-macros">
      <p className="hint mc-help">{t('machine.macros.intro', { max: MAX_MACROS })}</p>
      {macros.length === 0 && <p className="hint">{t('machine.macros.empty')}</p>}
      {macros.map((m, i) => (
        <MacroRow
          key={i}
          macro={m}
          index={i}
          total={macros.length}
          rxBuffer={draft.rx_buffer}
          onChange={(patch) => set(macros.map((x, j) => (j === i ? { ...x, ...patch } : x)))}
          onRemove={() => set(macros.filter((_, j) => j !== i))}
          onMove={(delta) => {
            const next = [...macros];
            const [item] = next.splice(i, 1);
            next.splice(i + delta, 0, item!);
            set(next);
          }}
        />
      ))}
      <div className="row">
        <button type="button" className="btn" disabled={macros.length >= MAX_MACROS} title={macros.length >= MAX_MACROS ? t('machine.macros.max', { max: MAX_MACROS }) : undefined} onClick={() => set([...macros, { name: t('machine.macros.newName', { n: macros.length + 1 }), gcode: '', confirm: true }])}>
          <Plus size={14} aria-hidden="true" />{t('machine.macros.add')}
        </button>
      </div>
    </div>
  );
}
