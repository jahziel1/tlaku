/** Textos del área `canvas`. Todas las claves empiezan con `canvas.` (lo verifica i18n.test.ts). */
export const canvas = {
  // ---- comandos y razones -------------------------------------------------
  'canvas.cmd.closePath': 'Cerrar polilínea',
  'canvas.cmd.snap': 'Ajuste magnético',
  'canvas.cmd.snapGrid': 'Ajustar a la cuadrícula',
  'canvas.cmd.snapObjects': 'Ajustar a objetos y cama',
  'canvas.reason.noPolyline': 'No hay una polilínea en curso. Dibújala con la herramienta de línea.',
  'canvas.reason.snapOff': 'El ajuste magnético está apagado.',

  // ---- ajuste magnético --------------------------------------------------
  'canvas.snap.group': 'Ajuste magnético',

  // ---- barra de edición numérica -----------------------------------------
  'canvas.bar.label': 'Edición numérica de la selección',
  'canvas.bar.anchor': 'Punto de referencia (origen de X e Y, ancla de la escala y centro del giro)',
  'canvas.bar.anchor.0': 'Arriba a la izquierda',
  'canvas.bar.anchor.1': 'Arriba al centro',
  'canvas.bar.anchor.2': 'Arriba a la derecha',
  'canvas.bar.anchor.3': 'Centro a la izquierda',
  'canvas.bar.anchor.4': 'Centro',
  'canvas.bar.anchor.5': 'Centro a la derecha',
  'canvas.bar.anchor.6': 'Abajo a la izquierda',
  'canvas.bar.anchor.7': 'Abajo al centro',
  'canvas.bar.anchor.8': 'Abajo a la derecha',
  'canvas.bar.w': 'Ancho',
  'canvas.bar.h': 'Alto',
  'canvas.bar.rot': 'Rotación',
  'canvas.bar.radius': 'Radio de esquinas',
  'canvas.bar.hint': 'Acepta expresiones: 100/3, +5 (suma), *2 (multiplica), 50 %, 5in. Intro o Tab aplican.',
  'canvas.bar.rotHint': 'Grados, antihorario. Gira alrededor del punto de referencia.',
  'canvas.bar.radiusHint': 'Radio de las esquinas de los rectángulos que dibujes, en mm.',
  'canvas.bar.invalid': 'Expresión no válida.',
  'canvas.bar.lock': 'Proporción bloqueada: cambiar el ancho o el alto escala los dos.',
  'canvas.bar.unlock': 'Proporción libre.',
  'canvas.bar.lockLabel': 'Mantener proporción',
  'canvas.bar.rotateImage': 'Las imágenes no se pueden rotar.',

  // ---- medidas en vivo ----------------------------------------------------
  'canvas.tip.move': 'Δx {dx} · Δy {dy} mm',
  'canvas.tip.scale': '{w} × {h} mm · {pct} %',
  'canvas.tip.rotate': '{deg}°',
  'canvas.tip.size': '{w} × {h} mm',
  'canvas.tip.circle': 'Ø {d} mm',
  'canvas.tip.line': '{len} mm · {deg}°',

  // ---- historial (etiquetas de pasos de deshacer) -------------------------
  'canvas.draw.rect': 'Dibujar rectángulo',
  'canvas.draw.ellipse': 'Dibujar elipse',
  'canvas.draw.line': 'Dibujar línea',
  'canvas.draw.polyline': 'Dibujar polilínea',
  'canvas.draw.text': 'Crear texto',

  // ---- herramienta de texto ----------------------------------------------------
  'canvas.text.default': 'Texto',
} as const;
