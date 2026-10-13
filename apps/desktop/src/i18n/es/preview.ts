/** Textos del área `preview`. Todas las claves empiezan con `preview.` (lo verifica i18n.test.ts). */
export const preview = {
  // ---- comandos y atajos ----------------------------------------------------------
  'preview.cmd.play': 'Reproducir el recorrido',
  'preview.cmd.pause': 'Pausar el recorrido',
  'preview.cmd.slower': 'Reproducción más lenta',
  'preview.cmd.faster': 'Reproducción más rápida',
  'preview.reason.unavailable': 'Abre la vista previa y espera a que se calcule el trabajo.',

  // ---- estado del cálculo -----------------------------------------------------------
  'preview.updating': 'Actualizando…',
  'preview.error.title': 'No se pudo calcular la vista previa',
  'preview.error.generic': 'El motor no indicó el motivo.',
  'preview.error.hintBed': 'Mueve el diseño dentro de la cama o reduce el overscan de las capas de relleno e imagen.',
  'preview.error.hintEmpty': 'Activa la salida de alguna capa que tenga objetos visibles.',
  'preview.error.retry': 'Reintentar',

  // ---- reproducción -----------------------------------------------------------------
  'preview.play': 'Reproducir',
  'preview.pause': 'Pausar',
  'preview.toStart': 'Volver al inicio',
  'preview.toEnd': 'Ir al final',
  'preview.speed': 'Velocidad de reproducción',
  'preview.speed.value': '{n}x',
  'preview.speed.title': '{n} veces la velocidad real del trabajo',
  'preview.time': '{now} de {total}',
  'preview.time.approx': 'Tiempo aproximado: el motor no entregó el tiempo de cada movimiento, así que se estima por distancia y velocidad.',
  'preview.time.approxBadge': 'aprox.',
  'preview.layerStart': 'Empieza la capa {name}',

  // ---- leyenda y filtros ---------------------------------------------------------------
  'preview.legend.title': 'Leyenda',
  'preview.legend.travel': 'Rápidos',
  'preview.legend.cut': 'Corte',
  'preview.legend.engrave': 'Grabado',
  'preview.legend.overscan': 'Overscan',
  'preview.legend.hide': 'Ocultar {name}',
  'preview.legend.show': 'Mostrar {name}',
  'preview.legend.power': 'Cuanto más transparente el trazo, menos potencia.',

  // ---- resultados -----------------------------------------------------------------------
  'preview.stats.title': 'Resultados',
  'preview.stats.total': 'Tiempo total',
  'preview.stats.distance': '{dist} en {time}',
  'preview.stats.estimate': 'Estimación con el modelo de aceleración del perfil de la máquina. El tiempo real puede ser distinto, sobre todo con imágenes.',
  'preview.stats.estimateBadge': 'Estimado',

  // ---- capas -----------------------------------------------------------------------------
  'preview.layers.title': 'Por capa',
  'preview.mode.line': 'Línea',
  'preview.mode.fill': 'Relleno',
  'preview.mode.image': 'Imagen',
  'preview.layer.share': '{pct} % del total',
  'preview.layer.saved': 'Ahorra {pct} % de recorrido en vacío',
  'preview.layer.expand': 'Ver cómo se planificó',
  'preview.layer.collapse': 'Ocultar la explicación',
  'preview.layer.highlight': 'Resaltar en el lienzo',
  'preview.layer.unhighlight': 'Quitar el resaltado',
  'preview.layer.noExplain': 'El planificador no dejó explicaciones para esta capa.',
  'preview.layer.clickToHighlight': 'Clic para resaltar la capa en el lienzo.',
  'preview.layer.noFigures': 'El motor aún no indica dónde está cada figura: el resaltado marca la capa completa.',
  'preview.order.title': 'Orden de las capas',
  'preview.warnings.title': 'Advertencias',
} as const;
