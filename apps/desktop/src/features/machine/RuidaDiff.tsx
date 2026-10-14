import { TriangleAlert } from 'lucide-react';
import type { ReactNode } from 'react';
import type { RuidaPlannedChange, RuidaSetting, RuidaVerified } from '@/api/types';
import { t, type I18nKey } from '@/i18n';
import { formatValue, unitKey } from './ruidaSettings';

/** Nombre legible de un ajuste por su clave (los textos viven en los catálogos `ruida` de i18n). */
export function settingName(key: string): string {
  return t(`ruida.set.name.${key}` as I18nKey);
}

type Kinds = ReadonlyMap<string, RuidaSetting['kind']>;

export function kindsOf(settings: readonly RuidaSetting[]): Kinds {
  return new Map(settings.map((s) => [s.key, s.kind]));
}

function show(kinds: Kinds, key: string, value: number): string {
  const kind = kinds.get(key) ?? 'raw';
  return formatValue(kind, value, t(unitKey(kind)));
}

/** Cada cambio con el valor actual y el nuevo, uno al lado del otro (paso 3 de la seguridad). */
export function RuidaDiffTable({ changes, kinds }: { changes: readonly RuidaPlannedChange[]; kinds: Kinds }) {
  return (
    <table className="mc-table mset-table" aria-label={t('ruida.set.review.title')}>
      <thead>
        <tr>
          <th scope="col">{t('ruida.set.col.name')}</th>
          <th scope="col">{t('ruida.set.col.addr')}</th>
          <th scope="col">{t('ruida.set.review.col.current')}</th>
          <th scope="col">{t('ruida.set.review.col.new')}</th>
        </tr>
      </thead>
      <tbody>
        {changes.map((c) => (
          <tr key={c.key}>
            <th scope="row">{settingName(c.key)}</th>
            <td className="num">{c.addr}</td>
            <td className="num">{show(kinds, c.key, c.current)}</td>
            <td className="num"><strong>{show(kinds, c.key, c.new)}</strong></td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}

/** Lo releído tras escribir, junto a lo pedido (paso 4 de la seguridad). */
export function RuidaVerifyTable({ rows, kinds }: { rows: readonly RuidaVerified[]; kinds: Kinds }) {
  return (
    <table className="mc-table mset-table" aria-label={t('ruida.set.report.col.actual')}>
      <thead>
        <tr>
          <th scope="col">{t('ruida.set.col.name')}</th>
          <th scope="col">{t('ruida.set.report.col.expected')}</th>
          <th scope="col">{t('ruida.set.report.col.actual')}</th>
          <th scope="col" />
        </tr>
      </thead>
      <tbody>
        {rows.map((v) => (
          <tr key={v.key} data-critical={v.ok ? undefined : ''}>
            <th scope="row">{settingName(v.key)}</th>
            <td className="num">{show(kinds, v.key, v.expected)}</td>
            <td className="num">{v.actual === null ? t('ruida.set.notReported') : show(kinds, v.key, v.actual)}</td>
            <td>{v.ok ? t('ruida.set.report.match') : <strong>{t('ruida.set.report.differs')}</strong>}</td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}

/** Aviso en una línea con el triángulo de las demás advertencias de Máquina. */
export function WarnLine({ children, role }: { children: ReactNode; role?: 'alert' | 'note' }) {
  return (
    <p className="warn-line" role={role ?? 'note'}>
      <TriangleAlert size={14} aria-hidden="true" />
      <span>{children}</span>
    </p>
  );
}
