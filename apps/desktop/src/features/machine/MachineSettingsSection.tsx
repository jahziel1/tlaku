import { SlidersHorizontal } from 'lucide-react';
import { t } from '@/i18n';
import { useCommand } from '@/commands/useCommand';
import { useStore } from '@/store';
import { GalvoNotApplicable } from './GalvoNotice';
import { LhyNotApplicable } from './LhyNotice';
import { selectIsGalvo } from './galvoPorts';
import { selectIsLhy } from './lhyPorts';
import { selectIsRuida } from './ruidaPorts';

/** Botón del panel Máquina que abre la ventana «Ajustes de la máquina» (los `$n` de GRBL). */
export function MachineSettingsSection() {
  const cmd = useCommand('machine.settings');
  // Los `$n` son de GRBL: con Ruida no hay nada que mostrar (ver «Controladora Ruida»).
  const ruida = useStore(selectIsRuida);
  const lhy = useStore(selectIsLhy);
  const galvo = useStore(selectIsGalvo);
  if (ruida) return null;
  // El galvo no tiene ajustes `$`: se dice por qué en lugar del botón.
  if (galvo) {
    return (
      <section className="section" aria-labelledby="sec-mset">
        <h3 id="sec-mset" className="panel-title">{t('mset.section')}</h3>
        <GalvoNotApplicable reason="galvo.mset.reason" />
      </section>
    );
  }
  // El K40 no tiene ajustes `$`: se dice por qué en lugar del botón.
  if (lhy) {
    return (
      <section className="section" aria-labelledby="sec-mset">
        <h3 id="sec-mset" className="panel-title">{t('mset.section')}</h3>
        <LhyNotApplicable reason="lhymicro.mset.reason" />
      </section>
    );
  }
  return (
    <section className="section" aria-labelledby="sec-mset">
      <h3 id="sec-mset" className="panel-title">{t('mset.section')}</h3>
      <button type="button" className="btn" disabled={!cmd.enabled} title={cmd.tooltip} onClick={cmd.run}>
        <SlidersHorizontal size={14} aria-hidden="true" />
        {cmd.label}
      </button>
      <p className="hint">{cmd.reason ?? t('mset.section.help')}</p>
    </section>
  );
}
