/** Textos del área `history`. Todas las claves empiezan con `history.` (lo verifica i18n.test.ts). */
export const history = {
  // ---- pasos del historial (etiquetas de HistoryEntry) ---------------------
  'history.cut': 'Cortar',
  'history.paste': 'Pegar',
  'history.initial': 'Estado inicial',
  'history.group': 'Agrupar',
  'history.ungroup': 'Desagrupar',

  // ---- comandos ---------------------------------------------------------------
  'history.cmd.pasteInPlace': 'Pegar en el mismo lugar',
  'history.cmd.selectInvert': 'Invertir selección',
  'history.cmd.shortcuts': 'Atajos de teclado…',
  'history.cmd.panelHistory': 'Mostrar historial',

  // ---- por qué un comando no está disponible --------------------------------
  'history.reason.clipboardEmpty': 'El portapapeles está vacío: copia o corta objetos primero.',
  'history.reason.emptyDocument': 'No hay objetos en el diseño.',
  'history.reason.groupNeedsTwo': 'Selecciona dos o más objetos (sin bloquear) para agruparlos.',
  'history.reason.noGroup': 'Ningún objeto seleccionado está en un grupo.',

  // ---- menús ---------------------------------------------------------------------
  'history.menu.help': 'Ayuda',
  'history.menu.recent': 'Abrir reciente',
  'history.menu.noRecent': 'Aún no hay archivos recientes',
  'history.menu.clearRecent': 'Borrar la lista de recientes',

  // ---- panel Historial -------------------------------------------------------------
  'history.dock.title': 'Historial',
  'history.panel.label': 'Historial de cambios',
  'history.panel.hint': 'Haz clic en un paso para volver a ese punto. Lo deshecho se puede recuperar mientras no hagas otra acción.',
  'history.panel.empty': 'Aún no hay cambios. Cada acción que hagas aparecerá aquí.',
  'history.panel.jump': 'Volver a este punto: {label}',
  'history.panel.current': 'Estado actual',
  'history.panel.undone': 'Deshecho',
  'history.panel.steps': '{n} pasos de {max}',
  'history.panel.clear': 'Vaciar historial',
  'history.panel.clearTitle': 'Vaciar el historial',
  'history.panel.clearMessage': 'Se olvidarán todos los pasos para deshacer y rehacer. El diseño no cambia.',
  'history.panel.clearConfirm': 'Vaciar',
  'history.panel.cancel': 'Cancelar',

  // ---- cambios sin guardar -------------------------------------------------------------
  'history.unsaved.title': 'Cambios sin guardar',
  'history.unsaved.ask.new': '«{name}» tiene cambios sin guardar. ¿Quieres guardarlos antes de empezar un proyecto nuevo?',
  'history.unsaved.ask.open': '«{name}» tiene cambios sin guardar. ¿Quieres guardarlos antes de abrir otro proyecto?',
  'history.unsaved.ask.close': '«{name}» tiene cambios sin guardar. ¿Quieres guardarlos antes de cerrar Tlaku?',
  'history.unsaved.save': 'Guardar',
  'history.unsaved.discard': 'No guardar',
  'history.unsaved.cancel': 'Cancelar',

  // ---- avisos de archivos --------------------------------------------------------------
  'history.notice.opened': 'Proyecto abierto: {name}.',
  'history.notice.openFailed': 'No se pudo abrir {name}: {error}',
  'history.notice.recentMissing': 'No se pudo abrir {name}; se quitó de los recientes. {error}',
  'history.notice.saveFailed': 'No se pudo guardar {name}: {error}',
  'history.notice.importedFile': 'Se importaron {n} objetos de {file}.',
  'history.notice.importedWithWarnings':
    'Se importaron {n} objetos de {file}, pero el importador avisó de {w} problemas. Revisa que el diseño esté completo.',
  'history.notice.importedNothing': '{file} no tiene objetos que importar.',
  'history.notice.importFailed': 'No se pudo importar {file}: {error}',
  'history.notice.unsupported': '{file}: formato no compatible. Se admiten SVG, DXF, PDF, AI, HPGL, G-code e imágenes (PNG, JPG, BMP, WebP, TIFF, GIF y TGA).',
  'history.notice.exportFailed': 'No se pudo exportar el G-code: {error}',

  // ---- arrastrar y soltar ----------------------------------------------------------------
  'history.drop.title': 'Suelta para importar',
  'history.drop.hint': 'SVG, DXF, PNG, JPG o BMP',

  // ---- avisos (toasts) ---------------------------------------------------------------------
  'history.toast.more': 'Mostrar {n} más',
  'history.toast.less': 'Mostrar menos',

  // ---- barra superior y de estado ---------------------------------------------------------------
  'history.doc.unsaved': 'Sin guardar',
  'history.status.objects': '{n} objetos',
  'history.status.size': 'Tamaño de la selección: {w} × {h} mm',
  'history.status.saved': 'Todo guardado',
  'history.status.shortcuts': 'Atajos de teclado',

  // ---- diálogo de atajos --------------------------------------------------------------------------------
  'history.shortcuts.title': 'Atajos de teclado',
  'history.shortcuts.search': 'Buscar un comando o un atajo',
  'history.shortcuts.hint': 'Se abre con ? o con {combo}. Esc cierra.',
  'history.shortcuts.none': 'Ningún atajo coincide con «{q}».',
  'history.shortcuts.close': 'Cerrar',
  'history.shortcuts.cat.file': 'Archivo',
  'history.shortcuts.cat.edit': 'Edición',
  'history.shortcuts.cat.tools': 'Herramientas',
  'history.shortcuts.cat.view': 'Vista',
  'history.shortcuts.cat.machine': 'Máquina',
  'history.shortcuts.cat.arrange': 'Organizar',
  'history.shortcuts.cat.camera': 'Cámara',
  'history.shortcuts.cat.panels': 'Paneles y ayuda',
  'history.shortcuts.cat.canvas': 'Lienzo (gestos)',
  'history.shortcuts.step.normal': '1 mm',
  'history.shortcuts.step.big': '10 mm',
  'history.shortcuts.step.fine': '0,1 mm',
  'history.shortcuts.canvas.wheel': 'Zoom en el cursor',
  'history.shortcuts.canvas.wheelKeys': 'Rueda del ratón',
  'history.shortcuts.canvas.pinch': 'Zoom con el trackpad',
  'history.shortcuts.canvas.pinchKeys': 'Pellizco',
  'history.shortcuts.canvas.pan': 'Mover la vista',
  'history.shortcuts.canvas.panKeys': 'Botón central, Espacio + arrastrar o dos dedos',
} as const;
