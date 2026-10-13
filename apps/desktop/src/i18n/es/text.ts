/** Textos del área `text`. Todas las claves empiezan con `text.` (lo verifica i18n.test.ts). */
export const text = {
  // ---- historial -----------------------------------------------------------
  'text.history.edit': 'Editar texto',
  'text.history.convert': 'Convertir texto a trayectos',

  // ---- panel ----------------------------------------------------------------
  'text.panel.label': 'Propiedades del texto',
  'text.panel.title': 'Texto',
  'text.field.text': 'Contenido',
  'text.field.textPlaceholder': 'Escribe aquí. Intro agrega una línea.',
  'text.field.font': 'Fuente',
  'text.field.size': 'Tamaño',
  'text.field.letterSpacing': 'Espaciado',
  'text.field.lineSpacing': 'Interlineado',
  'text.field.sizeHint': 'Tamaño del em, no de la mayúscula.',
  'text.field.lineSpacingHint': 'En múltiplos del tamaño.',

  // ---- estilo y alineación --------------------------------------------------
  'text.style.bold': 'Negrita',
  'text.style.italic': 'Cursiva',
  'text.align.label': 'Alineación',
  'text.align.left': 'Alinear a la izquierda',
  'text.align.center': 'Centrar',
  'text.align.right': 'Alinear a la derecha',

  // ---- selector de fuente ----------------------------------------------------
  'text.font.search': 'Buscar fuente…',
  'text.font.loading': 'Cargando fuentes…',
  'text.font.empty': 'No hay fuentes disponibles.',
  'text.font.noMatch': 'Ninguna fuente coincide con la búsqueda.',
  'text.font.more': 'Hay {n} más: escribe para acotar la búsqueda.',
  'text.font.default': '{family} (predeterminada)',
  'text.font.styles': 'Estilos: {styles}',

  // ---- fuentes de una línea y SHX -------------------------------------------
  'text.font.sectionSystem': 'Fuentes del sistema',
  'text.font.sectionLine': 'Fuentes de una línea',
  'text.font.lineTitle': 'Fuente de un solo trazo: se graba como línea, no como contorno',
  'text.line.hint': 'Fuente de una línea: el texto sale como trazos abiertos. Usa una capa en modo Línea para grabarlos. No tiene negrita ni cursiva.',
  'text.line.credit': 'Fuentes Hershey: creadas por el Dr. A. V. Hershey (U. S. National Bureau of Standards); formato de los datos de James Hurt (Cognition, Inc.).',
  'text.shx.import': 'Importar fuente SHX…',
  'text.shx.importTitle': 'Añade fuentes SHX de AutoCAD que ya tengas. Se guardan para próximas sesiones.',
  'text.shx.pickTitle': 'Elegir fuentes SHX',
  'text.shx.imported': 'Fuentes SHX importadas: {names}',
  'text.shx.failed': 'No se pudo importar la fuente SHX: {message}',
  'text.cmd.importShx': 'Importar fuente SHX…',

  // ---- curvado y soldado -----------------------------------------------------
  'text.field.bend': 'Curvatura',
  'text.field.bendHint': 'Grados que abarca el texto sobre un arco. Positivo lo arquea hacia arriba, negativo hacia abajo y 0 lo deja recto.',
  'text.field.bendIgnored': 'La curvatura se ignora mientras el texto va sobre un trayecto.',
  'text.field.welded': 'Soldar letras',
  'text.field.weldedTitle': 'Une en un solo contorno las letras que se tocan o se solapan (útil con fuentes cursivas)',
  'text.field.weldedLine': 'No aplica a las fuentes de una línea: no tienen contornos que unir.',

  // ---- sobre trayecto --------------------------------------------------------
  'text.onPath.enable': 'Sobre trayecto',
  'text.onPath.target': 'Objeto guía',
  'text.onPath.offset': 'Desplazamiento',
  'text.onPath.flip': 'Voltear',
  'text.onPath.none': 'Dibuja una línea o una curva para poder escribir sobre ella.',
  'text.onPath.missing': 'El objeto guía ya no existe o no tiene trayectoria. Elige otro o desactiva "Sobre trayecto".',

  // ---- conversión ------------------------------------------------------------
  'text.convert': 'Convertir a trayectos',
  'text.convert.title': 'Reemplaza el texto por trayectos editables. Después ya no se puede cambiar la fuente ni el contenido.',
  'text.convert.reasonBusy': 'Espera a que termine de generarse el contorno.',
  'text.convert.reasonEmpty': 'El texto no tiene contorno todavía: escribe algo con una fuente instalada.',

  // ---- estado del contorno ----------------------------------------------------
  'text.status.busy': 'Generando contorno…',
  'text.status.error': 'No se pudo generar el contorno: {message}',
  'text.warn.title': 'Avisos del contorno',
  'text.warn.fontFallback': "La fuente '{family}' no está instalada; se usa '{used}'.",
} as const;
