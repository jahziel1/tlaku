// Lee el tema guardado antes de pintar para evitar el destello. Archivo aparte: la CSP de Tauri no admite scripts en línea.
try {
  var t = localStorage.getItem('tlaku.theme');
  if (t === 'light' || t === 'dark') document.documentElement.dataset.theme = t;
} catch (e) {
  /* sin almacenamiento: se queda el tema oscuro */
}
