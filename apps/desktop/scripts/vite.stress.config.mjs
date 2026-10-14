// Configuración de Vite solo para `canvas-stress.mjs`: la misma de la app pero sin recarga en caliente
// ni vigilancia de archivos, para que ediciones a mitad de una medición no reinicien la página.
import { mergeConfig } from 'vite';
import base from '../vite.config.ts';

export default mergeConfig(base, {
  server: { hmr: false, watch: null },
});
