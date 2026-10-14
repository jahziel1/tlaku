import { Sparkles, SlidersHorizontal } from 'lucide-react';
import { useCommand } from '@/commands/useCommand';
import { t } from '@/i18n';
import { useUiMode } from './mode';

/** Botón de la barra superior: pasa del modo sencillo al completo y viceversa (sin perder nada). */
export function ModeSwitch() {
  const mode = useUiMode();
  const cmd = useCommand('simple.toggle');
  const simple = mode === 'simple';
  const Icon = simple ? SlidersHorizontal : Sparkles;
  return (
    <button type="button" className="btn simple__switch" title={t(simple ? 'simple.switch.toFull.help' : 'simple.switch.toSimple.help')} onClick={cmd.run} data-mode={mode}>
      <Icon size={15} aria-hidden="true" />
      {t(simple ? 'simple.cmd.toFull' : 'simple.cmd.toSimple')}
    </button>
  );
}
