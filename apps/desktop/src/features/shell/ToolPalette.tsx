import { TOOL_IDS } from '@/store/selection';
import { t } from '@/i18n';
import { CommandButton } from './CommandButton';

/** Paleta vertical de herramientas; el orden es el de TOOL_IDS. */
export function ToolPalette() {
  return (
    <div className="toolpalette" role="toolbar" aria-orientation="vertical" aria-label={t('toolbar.tools')}>
      {TOOL_IDS.map((tool, i) => (
        <div key={tool} className="toolpalette__slot" data-gap={i === 2 || i === 6}>
          <CommandButton id={`tool.${tool}`} />
        </div>
      ))}
    </div>
  );
}
