import { Minimize2 } from 'lucide-react';
import { t, type I18nKey } from '@/i18n';
import { fmt } from '@/lib/platform';
import { selectActiveDevice, useStore } from '@/store';
import { fitToBed, placeAt } from './actions';
import { allBox, designBox } from './model';
import type { SimpleData } from './hooks';
import { ProblemList } from './Problems';

const SPOTS: I18nKey[] = [
  'simple.place.tl', 'simple.place.t', 'simple.place.tr',
  'simple.place.l', 'simple.place.center', 'simple.place.r',
  'simple.place.bl', 'simple.place.b', 'simple.place.br',
];

/** Paso 3: colocar el diseño en la cama (arrastrando en el lienzo, centrando o llevándolo a una esquina). */
export function StepPlace({ data }: { data: SimpleData }) {
  const doc = useStore((s) => s.doc);
  const bed = useStore((s) => selectActiveDevice(s).bed);
  const box = designBox(doc) ?? allBox(doc);
  const w = box ? box[1][0] - box[0][0] : 0;
  const h = box ? box[1][1] - box[0][1] : 0;
  return (
    <>
      <p className="simple__size num" role="status">
        {box ? t('simple.place.size', { w: fmt(w, 1), h: fmt(h, 1), bw: fmt(bed[0], 0), bh: fmt(bed[1], 0) }) : t('simple.problem.nothing')}
      </p>
      {box && <p className="hint num">{t('simple.place.at', { x: fmt(box[0][0], 1), y: fmt(box[0][1], 1) })}</p>}
      <p className="hint">{t('simple.place.drag')}</p>
      <div className="simple__bedgrid" role="group" aria-label={t('simple.place.where')}>
        {SPOTS.map((key, i) => (
          <button key={key} type="button" className="simple__spot" data-center={i === 4} title={t(key)} aria-label={t(key)} disabled={!box} onClick={() => placeAt(i)}>
            <span aria-hidden="true">{i === 4 ? t('simple.place.centerShort') : ''}</span>
          </button>
        ))}
      </div>
      <button type="button" className="btn simple__btn" disabled={!box} onClick={() => fitToBed()}>
        <Minimize2 size={15} aria-hidden="true" />
        {t('simple.fix.fit')}
      </button>
      <ProblemList step="place" problems={data.problems} />
    </>
  );
}
