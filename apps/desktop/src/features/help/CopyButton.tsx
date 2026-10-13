import { Check, Copy } from 'lucide-react';
import { useEffect, useRef, useState } from 'react';
import { t } from '@/i18n';
import { copyText } from './clipboard';

/** Botón que copia `text` y confirma un instante. Sustituye a abrir enlaces, que la app no hace. */
export function CopyButton({ text, label }: { text: string; label?: string }) {
  const [done, setDone] = useState(false);
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  useEffect(() => () => clearTimeout(timer.current), []);
  const aria = label ?? t('help.copy');
  return (
    <button
      type="button"
      className="help__copy"
      aria-label={done ? t('help.copied') : aria}
      title={done ? t('help.copied') : aria}
      onClick={() => {
        void copyText(text).then((ok) => {
          if (!ok) return;
          setDone(true);
          clearTimeout(timer.current);
          timer.current = setTimeout(() => setDone(false), 1600);
        });
      }}
    >
      {done ? <Check size={13} aria-hidden="true" /> : <Copy size={13} aria-hidden="true" />}
      <span>{done ? t('help.copied') : t('help.copy')}</span>
    </button>
  );
}
