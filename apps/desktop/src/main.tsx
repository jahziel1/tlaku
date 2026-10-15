import '@fontsource-variable/archivo/wdth.css';
import '@fontsource-variable/spline-sans-mono/wght.css';
import './styles/index.css';
import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { App } from './App';
import { DetachedLive } from './features/live/DetachedLive';
import { isLiveWindow } from './features/live/windowMode';

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    {isLiveWindow() ? <DetachedLive /> : <App />}
  </StrictMode>,
);
