/** Accesibilidad: textos solo para lectores de pantalla y para el teclado. Todas las claves empiezan con `a11y.`. */
export const a11y = {
  // ---- navegación ----
  'a11y.skip.canvas': 'Saltar al lienzo',

  // ---- paneles flotantes ----
  'a11y.float.handle': 'Panel flotante {name}: mover o cambiar de tamaño',

  // ---- lienzo ----
  'a11y.canvas.desc': 'Lienzo de diseño: mesa de {name}, {size}. {objects}. Lo seleccionado se resume en un aviso y en la barra de estado; las capas están en el panel lateral.',
  'a11y.canvas.objects.none': 'El diseño está vacío',
  'a11y.canvas.objects.one': 'El diseño tiene 1 objeto',
  'a11y.canvas.objects.many': 'El diseño tiene {n} objetos',
  'a11y.sel.none': 'Nada seleccionado.',
  'a11y.sel.one': '1 objeto seleccionado: {w} por {h} {unit}, esquina inferior izquierda en X {x}, Y {y}.',
  'a11y.sel.many': '{n} objetos seleccionados: {w} por {h} {unit}, esquina inferior izquierda en X {x}, Y {y}.',

  // ---- trabajo ----
  'a11y.job.running': 'Trabajo en marcha.',
  'a11y.job.paused': 'Trabajo en pausa.',
  'a11y.job.finished': 'Trabajo terminado.',
  'a11y.job.cancelled': 'Trabajo cancelado.',
  'a11y.job.error': 'El trabajo se detuvo por un error.',
  'a11y.job.progress': 'Trabajo al {pct} %.',

  'a11y.manual.prev': 'Página anterior',
  'a11y.manual.next': 'Página siguiente',

  // ---- nombres de tablas ----
  'a11y.table.marks': 'Marcas de registro de la cámara',
  'a11y.table.headAlign': 'Puntos del recorrido de la alineación con el cabezal',
  'a11y.table.printcut': 'Resultado de imprimir y cortar',
  'a11y.table.variables': 'Filas de datos de las variables',
  'a11y.table.devices': 'Máquinas guardadas',
  'a11y.table.kinds': 'Tipos de máquina',
  'a11y.table.facts': 'Datos de la máquina',
  'a11y.table.log': 'Historial de trabajos',
} as const;
