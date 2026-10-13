import { useCommand } from '@/commands/useCommand';
import { COMMAND_ICONS } from './icons';

interface Props {
  id: string;
  /** `icon` = solo ícono; `labeled` = ícono y texto. */
  variant?: 'icon' | 'labeled';
  accent?: boolean;
  danger?: boolean;
}

/** Botón ligado a un comando: etiqueta, atajo, motivo de deshabilitado y estado marcado salen del registro. */
export function CommandButton({ id, variant = 'icon', accent, danger }: Props) {
  const cmd = useCommand(id);
  const Icon = COMMAND_ICONS[id];
  const toggles = cmd.checked !== undefined;
  const cls = variant === 'icon' ? 'icon-btn' : `btn${accent ? ' btn--accent' : ''}${danger ? ' btn--danger' : ''}`;
  return (
    <button
      type="button"
      className={cls}
      title={cmd.tooltip}
      aria-label={variant === 'icon' ? cmd.label : undefined}
      aria-disabled={!cmd.enabled}
      aria-pressed={toggles && (id.startsWith('view.') || id.startsWith('tool.')) ? cmd.checked : undefined}
      data-checked={cmd.checked}
      onClick={() => cmd.enabled && cmd.run()}
    >
      {Icon && <Icon size={variant === 'icon' ? 17 : 15} aria-hidden="true" />}
      {variant === 'labeled' && <span>{cmd.label}</span>}
    </button>
  );
}
