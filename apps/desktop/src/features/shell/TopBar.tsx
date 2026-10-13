import { Moon, Sun } from 'lucide-react';
import { t } from '@/i18n';
import { useCommand } from '@/commands/useCommand';
import { useStore } from '@/store';
import { CommandButton } from './CommandButton';
import { MenuBar } from './MenuBar';

function Brand() {
  return (
    <div className="brand" aria-label="Tlaku">
      <svg viewBox="0 0 64 64" width="22" height="22" aria-hidden="true">
        <path d="M14 50V14h8v28h28v8z" fill="currentColor" />
        <circle cx="44" cy="20" r="5.5" fill="var(--accent)" />
        <path d="M30 34 41 23" stroke="var(--accent)" strokeWidth="3.5" strokeLinecap="round" />
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
