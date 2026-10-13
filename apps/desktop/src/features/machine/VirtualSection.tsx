import { Zap } from 'lucide-react';
import { useState } from 'react';
import type { SimulatedFault } from '@/api/types';
import { t, type I18nKey } from '@/i18n';
import { useStore } from '@/store';
import { selectIsVirtual } from '@/store/machine';
import { selectIsRuida } from './ruidaPorts';
import { VirtualBadge } from './VirtualBadge';

/** Fallas que se pueden provocar, en el orden del menú. */
export const FAULT_LABEL: Record<SimulatedFault, I18nKey> = {
  flame: 'machine.virtual.fault.flame',
  tilt: 'machine.virtual.fault.tilt',
  alarm3: 'machine.virtual.fault.alarm3',
  disconnect: 'machine.virtual.fault.disconnect',
};

/**
 * Solo con una máquina virtual: recuerda que es un emulador y permite provocar fallas (llama,
 * alarma 3, desconexión) para ver cómo reacciona la interfaz.
 */
export function VirtualSection() {
  const virtual = useStore(selectIsVirtual);
  // Las fallas simuladas son de las máquinas GRBL virtuales; la virtual de Ruida no las tiene.
  const ruida = useStore(selectIsRuida);
  const simulateFault = useStore((s) => s.simulateFault);
  const [fault, setFault] = useState<SimulatedFault | ''>('');
  if (!virtual || ruida) return null;
  return (
    <section className="section mc-virtual" aria-labelledby="sec-virtual">
      <h3 id="sec-virtual" className="panel-title">
        {t('machine.virtual.title')} <VirtualBadge />
      </h3>
      <p className="hint">{t('machine.virtual.note')}</p>
      <div className="row">
        <div className="field grow">
          <label htmlFor="sel-fault" className="field__label">{t('machine.virtual.fault')}</label>
          <select id="sel-fault" className="select" value={fault} onChange={(e) => setFault(e.target.value as SimulatedFault | '')}>
            <option value="">{t('machine.virtual.faultPick')}</option>
            {(Object.keys(FAULT_LABEL) as SimulatedFault[]).map((f) => (
              <option key={f} value={f}>{t(FAULT_LABEL[f])}</option>
            ))}
          </select>
        </div>
        <button type="button" className="btn" disabled={fault === ''} onClick={() => fault !== '' && void simulateFault(fault)}>
          <Zap size={14} aria-hidden="true" />
          {t('machine.virtual.faultRun')}
        </button>
      </div>
      <p className="hint">{t('machine.virtual.faultHint')}</p>
    </section>
  );
}
