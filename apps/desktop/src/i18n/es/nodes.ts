/** Textos del área `nodes`. Todas las claves empiezan con `nodes.` (lo verifica i18n.test.ts). */
export const nodes = {
  // ---- comandos --------------------------------------------------------------
  'nodes.cmd.insert': 'Insertar nodo en el punto medio',
  'nodes.cmd.delete': 'Borrar nodos',
  'nodes.cmd.toLine': 'Segmento a recta',
  'nodes.cmd.toCurve': 'Segmento a curva',
  'nodes.cmd.corner': 'Nodo en esquina',
  'nodes.cmd.smooth': 'Nodo suave',
  'nodes.cmd.symmetric': 'Nodo simétrico',
  'nodes.cmd.break': 'Romper el trayecto en el nodo',
  'nodes.cmd.join': 'Unir extremos',
  'nodes.cmd.close': 'Cerrar trayecto',

  // ---- por qué no se puede ----------------------------------------------------
  'nodes.reason.notTool': 'Elige la herramienta Editar nodos (N).',
  'nodes.reason.noNodes': 'Selecciona uno o más nodos del trazo.',
  'nodes.reason.needSegment': 'Selecciona un segmento (clic en él) o dos nodos contiguos.',
  'nodes.reason.cantBreak': 'Selecciona un nodo que no sea el extremo de un trayecto abierto.',
  'nodes.reason.needEnds': 'Selecciona exactamente dos extremos de trayectos abiertos del mismo objeto.',
  'nodes.reason.noOpen': 'Los nodos seleccionados no pertenecen a ningún trayecto abierto.',

  // ---- pasos de deshacer -----------------------------------------------------------
  'nodes.history.move': 'Mover nodos',
  'nodes.history.nudge': 'Mover nodos con el teclado',
  'nodes.history.coord': 'Fijar la posición de un nodo',
  'nodes.history.handle': 'Mover manejador',
  'nodes.history.insert': 'Insertar nodo',
  'nodes.history.delete': 'Borrar nodos',
  'nodes.history.toLine': 'Segmento a recta',
  'nodes.history.toCurve': 'Segmento a curva',
  'nodes.history.type': 'Cambiar tipo de nodo',
  'nodes.history.break': 'Romper trayecto',
  'nodes.history.join': 'Unir extremos',
  'nodes.history.close': 'Cerrar trayecto',

  // ---- barra de nodos -----------------------------------------------------------------
  'nodes.bar.label': 'Edición de nodos',
  'nodes.bar.x': 'X',
  'nodes.bar.y': 'Y',
  'nodes.bar.hint': 'Posición exacta en el documento, en mm. Con varios nodos es el centro de su caja y mueve todos. Acepta expresiones.',
  'nodes.bar.one': '1 nodo seleccionado',
  'nodes.bar.many': '{n} nodos seleccionados',
  'nodes.bar.none': 'Sin nodos seleccionados. Clic, Mayús+clic o arrastra una caja; doble clic en un segmento agrega un nodo.',
  'nodes.bar.pick': 'Haz clic en un trazo para editar sus nodos.',
  'nodes.bar.notVector': 'Los textos y las imágenes no tienen nodos editables: conviértelos a trazos.',
  'nodes.bar.type': 'Tipo de nodo',
  'nodes.bar.segment': 'Segmento',
  'nodes.bar.path': 'Trayecto',

  // ---- medidas en vivo -------------------------------------------------------------------
  'nodes.tip.point': 'X {x} · Y {y} mm',
} as const;
