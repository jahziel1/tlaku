import { FolderOpen, Shapes, Sparkles } from 'lucide-react';
import type { LucideIcon } from 'lucide-react';
import { t, type I18nKey } from '@/i18n';
import { fmt } from '@/lib/platform';
import { useStore } from '@/store';
import { openArtLibrary, openExample, openFile } from './actions';
import { designBox } from './model';
import type { SimpleData } from './hooks';

function BigChoice({ icon: Icon, title, text, onClick }: { icon: LucideIcon; title: I18nKey; text: I18nKey; onClick: () => void }) {
  return (
    <button type="button" className="simple__choice" onClick={onClick}>
      <Icon size={30} aria-hidden="true" />
      <span className="simple__choice-title">{t(title)}</span>
      <span className="simple__choice-text">{t(text)}</span>
    </button>
  );
}

/** Paso 1: abrir o importar el diseño, elegir uno de la biblioteca de arte o probar un ejemplo. */
export function StepOpen({ data }: { data: SimpleData }) {
  const doc = useStore((s) => s.doc);
  const box = designBox(doc);
  return (
    <>
      <div className="simple__choices">
        <BigChoice icon={FolderOpen} title="simple.open.file" text="simple.open.file.text" onClick={() => void openFile()} />
        <BigChoice icon={Shapes} title="simple.open.art" text="simple.open.art.text" onClick={openArtLibrary} />
        <BigChoice icon={Sparkles} title="simple.open.example" text="simple.open.example.text" onClick={() => void openExample()} />
      </div>
      <p className="simple__status" role="status" data-ok={data.groups.length > 0}>
        {data.groups.length === 0
          ? t('simple.open.empty')
          : t('simple.open.have', {
              n: data.groups.reduce((a, g) => a + g.count, 0),
              colors: data.groups.length,
              size: box ? `${fmt(box[1][0] - box[0][0], 0)} × ${fmt(box[1][1] - box[0][1], 0)} mm` : '—',
            })}
      </p>
    </>
  );
}
