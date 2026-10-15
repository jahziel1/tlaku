import { Undo2 } from 'lucide-react';
import { useEffect, useMemo } from 'react';
import { getApi } from '@/api';
import { t, setLanguage } from '@/i18n';
import { applyThemeToDocument } from '@/store/view';
import { LivePanel, useLiveSnapshot } from './LivePanel';
import { RemoteLiveSource } from './remoteSource';
import './live.css';

/**
 * Raíz de la ventana aparte de la vista en vivo (etiqueta `live`). Es de SOLO LECTURA: no carga
 * preferencias, no abre la máquina ni ningún documento; solo escucha lo que le manda la ventana
 * principal (`live://route`, `live://state`) y lo dibuja con el mismo `LivePanel` que la ventana
 * flotante. Su capacidad (`capabilities/live.json`) tampoco le deja llamar a otra cosa.
 */
export function DetachedLive() {
  const source = useMemo(() => new RemoteLiveSource(), []);
  const snap = useLiveSnapshot(source);
  // El idioma y el tema son los de la ventana principal (el idioma se fija antes de pintar los textos).
  setLanguage(snap.lang);
  useEffect(() => {
    applyThemeToDocument(snap.theme);
    document.documentElement.lang = snap.lang;
  }, [snap.theme, snap.lang]);

  useEffect(() => {
    const api = getApi();
    let cancelled = false;
    const offs: (() => void)[] = [];
    void (async () => {
      const route = await api.on('live://route', (text) => void source.applyRoute(text));
      const state = await api.on('live://state', (text) => void source.applyState(text));
      if (cancelled) {
        route();
        state();
        return;
      }
      offs.push(route, state);
      // Ya escucha: que la ventana principal mande la ruta y el estado.
      await api.liveWindowHello().catch(() => undefined);
    })();
    return () => {
      cancelled = true;
      offs.forEach((f) => f());
    };
  }, [source]);

  return (
    <div className="live-ext" key={snap.lang}>
      <LivePanel
        source={source}
        actions={
          <button type="button" className="btn live-ext__back" title={t('live.ext.backTip')} onClick={() => void getApi().liveWindowClose().catch(() => undefined)}>
            <Undo2 size={14} aria-hidden="true" />
            <span>{t('live.ext.back')}</span>
          </button>
        }
      />
    </div>
  );
}
