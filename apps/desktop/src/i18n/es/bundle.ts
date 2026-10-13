/** Textos de los paquetes de configuración `.tlakubundle`. Todas las claves empiezan con `bundle.` (lo verifica i18n.test.ts). */
export const bundle = {
  // ---- comandos ---------------------------------------------------------------
  'bundle.cmd.export': 'Exportar paquete de configuración…',
  'bundle.cmd.import': 'Importar paquete de configuración…',

  // ---- diálogo ----------------------------------------------------------------
  'bundle.close': 'Cerrar',
  'bundle.cancel': 'Cancelar',
  'bundle.title.export': 'Exportar paquete de configuración',
  'bundle.title.import': 'Importar paquete de configuración',
  'bundle.section.devices': 'Máquinas',
  'bundle.section.materials': 'Materiales',
  'bundle.section.prefs': 'Preferencias y atajos',

  // ---- exportar ---------------------------------------------------------------
  'bundle.export.hint': 'Guarda en un solo archivo lo que elijas, para llevarlo a otra computadora o compartirlo.',
  'bundle.export.devices': '{n} perfiles de máquina, con sus macros y posiciones guardadas, y cuál es la máquina activa',
  'bundle.export.materials': '{n} ajustes de material de la biblioteca',
  'bundle.export.prefs': 'Las preferencias y los atajos de teclado propios',
  'bundle.export.none': 'Elige al menos una sección.',
  'bundle.export.action': 'Exportar…',

  // ---- importar ---------------------------------------------------------------
  'bundle.import.hint': 'Elige un paquete, revisa qué cambiaría y marca las secciones que quieres aplicar. No se escribe nada hasta que confirmes.',
  'bundle.import.pick': 'Elegir archivo…',
  'bundle.import.another': 'Elegir otro archivo…',
  'bundle.import.meta': '{name} · creado el {date} con Tlaku {app}',
  'bundle.import.metaNoApp': '{name} · creado el {date}',
  'bundle.import.preview': 'Vista previa',
  'bundle.import.absent': 'El paquete no incluye esta sección.',
  'bundle.import.nothing': 'Sin cambios: ya está todo igual.',
  'bundle.import.empty': 'El paquete no trae ninguna sección.',
  'bundle.import.apply': 'Aplicar',
  'bundle.import.applying': 'Aplicando…',
  'bundle.import.noneChecked': 'Marca al menos una sección que traiga cambios.',

  // ---- resumen de cada sección --------------------------------------------------
  'bundle.devices.summary': '{added} nuevas · {replaced} reemplazan a una con el mismo id · {unchanged} sin cambios',
  'bundle.devices.replaced': 'Se reemplazarán: {names}',
  'bundle.devices.active': 'Máquina activa: {name}',
  'bundle.devices.invalid': '{n} perfiles descartados por no ser válidos',
  'bundle.materials.summary': '{added} nuevos · {skipped} ya estaban · lo que tienes se conserva',
  'bundle.materials.renamed': '{n} chocaban con otro material distinto del mismo id y entran como copia con id nuevo',
  'bundle.materials.invalid': '{n} ajustes descartados por no ser válidos',
  'bundle.prefs.summary': '{prefs} preferencias y {shortcuts} comandos con atajos propios',
  'bundle.prefs.ignored': '{n} preferencias no se reconocen y se ignoran',
  'bundle.prefs.unknown': '{n} atajos son de comandos que esta versión no tiene (se conservan)',
  'bundle.prefs.replaceShortcuts': 'Si el paquete trae atajos, reemplazan a los actuales.',

  // ---- motivos de descarte ------------------------------------------------------
  'bundle.invalid.item': '«{id}»: {reason}',
  'bundle.reason.notObject': 'no es un perfil',
  'bundle.reason.noId': 'no tiene id',
  'bundle.reason.noName': 'no tiene nombre',
  'bundle.reason.invalid': 'datos no válidos ({fields})',
  'bundle.reason.duplicate': 'id repetido en el paquete',

  // ---- avisos y errores ---------------------------------------------------------
  'bundle.file.filter': 'Paquete de configuración de Tlaku',
  'bundle.file.exportTitle': 'Exportar paquete de configuración',
  'bundle.file.importTitle': 'Importar paquete de configuración',
  'bundle.notice.exported': 'Paquete guardado en {path}.',
  'bundle.notice.exportFailed': 'No se pudo exportar el paquete: {error}',
  'bundle.notice.imported': 'Paquete «{name}» aplicado: {devices} máquinas, {materials} materiales nuevos y {prefs} preferencias.',
  'bundle.notice.devicesFailed': '{n} máquinas no se pudieron guardar.',
  'bundle.notice.importFailed': 'No se pudo leer el paquete «{name}»: {error}',
  'bundle.blocked.job': 'No se puede importar un paquete mientras hay un trabajo en curso.',
  'bundle.err.notBundle': 'El archivo no es un paquete de configuración de Tlaku.',
  'bundle.err.materialsUnavailable': 'No se pudo leer la biblioteca de materiales, así que no se tocó.',
} as const;
