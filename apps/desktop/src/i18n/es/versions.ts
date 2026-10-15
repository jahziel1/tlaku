/** Historial de versiones del proyecto (`features/versions`). Todas las claves empiezan con `versions.`. */
export const versions = {
  // ---- comandos ----
  'versions.cmd.open': 'Historial de versiones…',
  'versions.cmd.save': 'Guardar versión…',
  'versions.reason.noFile': 'Guarda el proyecto en un archivo para tener historial de versiones.',
  'versions.history.restore': 'Restaurar versión',

  // ---- diálogo principal ----
  'versions.title': 'Historial de versiones',
  'versions.subtitle': 'Proyecto: {name}',
  'versions.close': 'Cerrar',
  'versions.noFile.title': 'Este documento aún no tiene archivo',
  'versions.noFile.body':
    'Sin archivo no hay historial: mientras tanto la copia de recuperación protege lo que haces. Guarda el proyecto para empezar a conservar versiones, o abre el historial de otro proyecto.',
  'versions.toolbar.save': 'Guardar versión…',
  'versions.toolbar.openOther': 'Historial de otro archivo…',
  'versions.projects.label': 'Otros proyectos con historial',
  'versions.projects.choose': 'Elegir un proyecto…',
  'versions.projects.option': '{name} · {n} versiones',
  'versions.projects.missing': '(el archivo ya no está)',
  'versions.loading': 'Cargando versiones…',
  'versions.loadFailed': 'No se pudo leer el historial: {error}',

  // ---- lista ----
  'versions.list.label': 'Versiones guardadas',
  'versions.list.empty':
    'Todavía no hay versiones de este proyecto. Se crean al guardar (se conserva lo que había antes), cada pocos minutos mientras editas y cuando usas «Guardar versión…».',
  'versions.kind.saved': 'Guardado',
  'versions.kind.auto': 'Automática',
  'versions.kind.manual': 'Con nombre',
  'versions.kind.restore': 'Antes de restaurar',
  'versions.row.objects.one': '{n} objeto',
  'versions.row.objects.other': '{n} objetos',
  'versions.row.layers.one': '{n} capa',
  'versions.row.layers.other': '{n} capas',
  'versions.row.thumbAlt': 'Miniatura de la versión',

  // ---- detalle ----
  'versions.detail.empty': 'Elige una versión de la lista para verla, compararla o restaurarla.',
  'versions.detail.preview': 'Vista previa',
  'versions.detail.noThumb': 'Sin vista previa (el diseño está vacío o es demasiado denso).',
  'versions.detail.size': '{size} en el documento, {stored} en disco',
  'versions.detail.restoreNew': 'Restaurar como pestaña nueva',
  'versions.detail.restoreHere': 'Reemplazar el documento abierto…',
  'versions.detail.highlight': 'Resaltar en el lienzo lo que cambia',
  'versions.detail.delete': 'Borrar esta versión',
  'versions.reason.notOpen': 'El archivo no está abierto en ninguna pestaña: ábrelo para reemplazar su documento.',
  'versions.reason.nothingToHighlight': 'No hay objetos del documento abierto que resaltar.',

  // ---- comparación ----
  'versions.compare.title': 'Qué cambiaría al restaurarla',
  'versions.compare.loading': 'Comparando con el documento abierto…',
  'versions.compare.failed': 'No se pudo comparar: {error}',
  'versions.compare.needOpen': 'Abre el archivo en una pestaña para compararla con el documento actual.',
  'versions.compare.same': 'Es igual al documento abierto: restaurarla no cambiaría nada.',
  'versions.compare.lost.one': 'Se perdería {n} objeto agregado después.',
  'versions.compare.lost.other': 'Se perderían {n} objetos agregados después.',
  'versions.compare.back.one': 'Volvería {n} objeto que se quitó después.',
  'versions.compare.back.other': 'Volverían {n} objetos que se quitaron después.',
  'versions.compare.reverted.one': '{n} objeto volvería a como estaba.',
  'versions.compare.reverted.other': '{n} objetos volverían a como estaban.',
  'versions.compare.layersLost': 'Se quitarían las capas: {names}.',
  'versions.compare.layersBack': 'Volverían las capas: {names}.',
  'versions.compare.layersChanged': 'Cambiarían los ajustes de las capas: {names}.',
  'versions.compare.notes': 'Las notas del proyecto cambiarían.',
  'versions.compare.guides': 'Las guías cambiarían.',
  'versions.compare.device': 'El dispositivo del proyecto cambiaría.',

  // ---- borrar ----
  'versions.delete.title': 'Borrar versión',
  'versions.delete.message': 'Se borrará «{name}» del historial. No se puede deshacer.',
  'versions.delete.confirm': 'Borrar',
  'versions.delete.cancel': 'Conservar',

  // ---- reemplazar el documento ----
  'versions.replace.title': 'Reemplazar el documento',
  'versions.replace.message':
    'El documento abierto de «{name}» se reemplazará por la versión «{when}». Lo que tienes ahora se guarda antes como versión («Antes de restaurar») y además puedes usar Deshacer.',
  'versions.replace.confirm': 'Restaurar',
  'versions.replace.cancel': 'Cancelar',

  // ---- pie y ajustes ----
  'versions.footer.summary.one': '{n} versión · {size}',
  'versions.footer.summary.other': '{n} versiones · {size}',
  'versions.footer.dir': 'Carpeta: {dir}',
  'versions.settings.title': 'Ajustes del historial',
  'versions.settings.enabled': 'Conservar la versión anterior en cada guardado y hacer instantáneas automáticas',
  'versions.settings.location': 'Dónde guardar las versiones',
  'versions.settings.location.app': 'En la carpeta de datos de la app (recomendado)',
  'versions.settings.location.beside': 'Junto al archivo, en la carpeta oculta «.tlaku-versions»',
  'versions.settings.locationHint':
    'Junto al archivo viaja con el proyecto (y con la nube que sincronice la carpeta); en la carpeta de la app no ensucia tus carpetas. Al cambiarlo, el historial anterior sigue visible.',
  'versions.settings.autoMinutes': 'Instantánea automática cada (minutos; 0 = nunca)',
  'versions.settings.recentHours': 'Conservar todas las de las últimas (horas)',
  'versions.settings.dailyDays': 'Después, una por día hasta hace (días)',
  'versions.settings.weeklyWeeks': 'Después, una por semana hasta hace (semanas)',
  'versions.settings.maxMb': 'Tope de tamaño por proyecto (MB)',
  'versions.settings.maxVersions': 'Tope de versiones por proyecto',
  'versions.settings.help':
    'La limpieza borra primero lo más viejo y nunca toca la versión más reciente ni las versiones con nombre. Más allá del último escalón se borra todo lo automático.',
  'versions.settings.apply': 'Aplicar',
  'versions.settings.applied': 'Ajustes del historial guardados.',
  'versions.settings.failed': 'No se pudieron guardar los ajustes del historial: {error}',
  'versions.settings.clearAll': 'Borrar todo el historial de este proyecto…',
  'versions.clear.title': 'Borrar el historial',
  'versions.clear.message': 'Se borrarán las {n} versiones de «{name}», incluidas las que tienen nombre. No se puede deshacer.',
  'versions.clear.confirm': 'Borrar todo',

  // ---- «Guardar versión…» ----
  'versions.name.title': 'Guardar versión',
  'versions.name.label': 'Nombre de la versión',
  'versions.name.placeholder': 'Por ejemplo: antes de cambiar el corte',
  'versions.name.hint':
    'Guarda una copia del documento tal como está ahora, con los cambios que aún no hayas guardado. Las versiones con nombre no se borran solas.',
  'versions.name.save': 'Guardar versión',
  'versions.name.cancel': 'Cancelar',

  // ---- avisos ----
  'versions.notice.captureFailed':
    'No se pudo guardar la versión anterior en el historial: {error}. El proyecto se guarda de todos modos.',
  'versions.notice.autoFailed': 'No se pudo hacer la instantánea automática del historial: {error}',
  'versions.notice.needFile': 'Guarda primero el proyecto en un archivo: las versiones se guardan por archivo.',
  'versions.notice.saved': 'Versión «{name}» guardada en el historial.',
  'versions.notice.saveFailed': 'No se pudo guardar la versión: {error}',
  'versions.notice.openedCopy': 'La versión «{when}» de «{name}» se abrió en una pestaña nueva. Guárdala con otro nombre para conservarla.',
  'versions.notice.restored': 'Se restauró la versión «{when}». Puedes deshacerlo; el estado anterior quedó en el historial.',
  'versions.notice.restoreFailed': 'No se pudo restaurar la versión: {error}',
  'versions.notice.notOpen': 'Ese archivo no está abierto en ninguna pestaña: restaura como pestaña nueva.',
  'versions.notice.deleted.one': 'Se borró {n} versión.',
  'versions.notice.deleted.other': 'Se borraron {n} versiones.',
  'versions.notice.deleteFailed': 'No se pudo borrar: {error}',
} as const;
