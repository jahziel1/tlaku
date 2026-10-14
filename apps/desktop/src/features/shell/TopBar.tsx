import { Moon, Sun } from 'lucide-react';
import { t } from '@/i18n';
import { useCommand } from '@/commands/useCommand';
import { ModeSwitch, useUiMode } from '@/features/simple';
import { useStore } from '@/store';
import { CommandButton } from './CommandButton';
import { MenuBar } from './MenuBar';

function Brand() {
  return (
    <div className="brand" aria-label="Tlaku">
      {/* El mismo motivo del ícono de la app: la T cuyo trazo es el haz y termina en el punto de impacto. */}
      <svg viewBox="0 0 64 64" width="22" height="22" aria-hidden="true">
        <rect x="10" y="8" width="44" height="10" rx="2.5" fill="currentColor" />
        <rect x="27.5" y="18" width="9" height="20" fill="var(--accent)" />
        <circle cx="32" cy="46" r="11" fill="none" stroke="var(--accent)" strokeWidth="3" />
        <circle cx="32" cy="46" r="5.5" fill="currentColor" />
      </svg>
      <span className="brand__name">Tlaku</span>
    </div>
  );
}

function DocumentName() {
  const path = useStore((s) => s.filePath);
  const dirty = useStore((s) => s.dirty);
  const name = path ? (path.split(/[\\/]/).pop() ?? path) : t('doc.untitled');
  return (
    <div className="docname" title={path ?? undefined}>
      <span className="docname__text">{name}</span>
      {dirty && (
        <>
          <span className="docname__dirty" aria-hidden="true" />
          <span className="docname__state" role="status">{t('history.doc.unsaved')}</span>
        </>
      )}
    </div>
  );
}

export function TopBar() {
  const theme = useStore((s) => s.theme);
  const toggleTheme = useCommand('view.theme');
  const ThemeIcon = theme === 'dark' ? Sun : Moon;
  const simple = useUiMode() === 'simple';
  // Modo sencillo: sin menús ni botones avanzados; quedan deshacer, rehacer y el interruptor de modo.
  if (simple) {
    return (
      <header className="topbar topbar--simple">
        <Brand />
        <div className="topbar__sep" role="separator" />
        <div className="topbar__group" role="toolbar" aria-label={t('toolbar.edit')}>
          <CommandButton id="edit.undo" />
          <CommandButton id="edit.redo" />
        </div>
        <div className="topbar__spacer" />
        <DocumentName />
        <div className="topbar__spacer" />
        <ModeSwitch />
        <button type="button" className="icon-btn" title={toggleTheme.tooltip} aria-label={theme === 'dark' ? t('theme.toLight') : t('theme.toDark')} onClick={toggleTheme.run}>
          <ThemeIcon size={17} aria-hidden="true" />
        </button>
      </header>
    );
  }
  return (
    <header className="topbar">
      <Brand />
      <MenuBar />
      <div className="topbar__sep" role="separator" />
      <div className="topbar__group" role="toolbar" aria-label={t('toolbar.file')}>
        <CommandButton id="file.new" />
        <CommandButton id="file.open" />
        <CommandButton id="file.save" />
        <CommandButton id="file.import" />
        <CommandButton id="file.exportGcode" />
      </div>
      <div className="topbar__sep" role="separator" />
      <div className="topbar__group" role="toolbar" aria-label={t('toolbar.edit')}>
        <CommandButton id="edit.undo" />
        <CommandButton id="edit.redo" />
      </div>
      <div className="topbar__spacer" />
      <DocumentName />
      <div className="topbar__spacer" />
      <ModeSwitch />
      <button type="button" className="icon-btn" title={toggleTheme.tooltip} aria-label={theme === 'dark' ? t('theme.toLight') : t('theme.toDark')} onClick={toggleTheme.run}>
        <ThemeIcon size={17} aria-hidden="true" />
      </button>
      <div className="topbar__sep" role="separator" />
      <div className="topbar__group topbar__group--actions" role="toolbar" aria-label={t('toolbar.job')}>
        <CommandButton id="view.preview" variant="labeled" />
        <CommandButton id="machine.frame" variant="labeled" />
        <CommandButton id="machine.start" variant="labeled" accent />
      </div>
    </header>
  );
}
