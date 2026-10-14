# 04 - Cobertura: lo que falta para un primer lanzamiento público

Parte de [la cobertura funcional](README.md).

## 3. Lo que falta para un primer lanzamiento público

Criterio: una primera versión pública (0.x, "beta") debe ser segura de usar sobre una máquina real, no perder el trabajo del usuario y no mostrar nada que no funcione. El resto de LightBurn puede llegar por versiones.

Las listas se escribieron antes de la ola 1; cada punto lleva ahora su estado (**hecho** con la fila de la sección 2, o **pendiente**).

### 3.1 Imprescindible antes de publicar

1. **Probar con hardware real** (la Falcon2 está disponible): conectar, encuadre, un corte y un grabado, pausa, detención y alarma. Sin esto, todo lo de máquina es teoría. Documentar el resultado y marcar en la app lo que sigue siendo experimental (Ruida, el K40, la cámara en el cabezal, el ojo de pez, el buscador de centro y, por línea de comandos, el galvo) y probar con una máquina real Print & Cut y el tiling. **Pendiente.**
2. **Autoguardado y recuperación** tras un cierre inesperado. **Hecho** (sección 2.4).
3. **Quitar o terminar la herramienta Medir** (D44). **Hecho.**
4. **Voltear horizontal/vertical** (D26, P0) y teclas de rotación rápida (D24). **Hecho.**
5. **Cortar solo la selección** (M15): casillas «Solo la selección» y «Usar el origen de la selección» en el diálogo de inicio, el panel Trabajo y la vista previa. Es P0. **Hecho** (ola 2).
6. **Bloquear y orden de dibujo** (D28, D29). **Hecho**; ocultar objetos sueltos sigue sin comando.
7. **Exportar SVG y DXF** (D92, D93). **Hecho** (también PNG y PDF).
8. **Importar más imágenes** (D87) y **opciones de importación** (D91). **Hecho.**
9. **Preferencias mínimas** (D12, M51). **Hecho** salvo el idioma, "volver a Seleccionar" y las pulgadas en los campos de máquinas y cámara.
10. **Ayuda mínima**: menú Ayuda, manual, Acerca de y `README.md` de la raíz. **Hecho**; faltan publicar los enlaces del repositorio y de donaciones.
11. **Menú contextual del lienzo** (D09) y **snapping a nodos, puntos medios e intersecciones** (D77). **Hecho.**
12. **Curvas dibujadas** (D32, P0). **Hecho.**
13. **Habilitar `.lbrn`/`.lbrn2` en la interfaz** (D97). **Hecho** (cierre de la ola 1).

### 3.2 Importante para que la gente migre desde LightBurn

1. **Importar `.lbrn2` / `.lbrn`** (D97). **Hecho** en el motor y en la app (con referencia de estructura MIT citada en `THIRD_PARTY.md`); la ola 3 le añadió los ajustes avanzados.
2. **Importar PDF / AI** (D85, D86), **exportar PDF** (D98) y el portapapeles entre aplicaciones (D16). **Hecho.** Exportar AI (D94) sigue sin existir.
3. **Atajos configurables** (D11). **Hecho.** El perfil "tipo LightBurn" (D10: Ctrl+L, Ctrl+R, Ctrl+E...) **pendiente**.
4. **Idioma inglés** (M52): el público de LightBurn es en su mayoría anglófono. **Hecho** para la interfaz; faltan los mensajes del motor y el manual.
5. **Ruida en la app** (M23, M50, M26, M24 y M28): CO2 es la segunda familia más usada. **Hecho en la ola 4, EXPERIMENTAL** (probado solo contra el emulador UDP). Pendiente: probarlo con una máquina real (ruida.md 10.6), escribir ajustes de la controladora, y borrar/ejecutar documentos de su memoria.
6. **Conexión por red para GRBL (TCP y WebSocket)** y para Ruida (UDP) en la app (M50). **Hecha en la ola 4** (Ruida, experimental), con «Probar conexión» y la máquina virtual por red; ninguna probada con una máquina real.
7. **Herramientas de forma y edición frecuentes.** **Hechas:** polígono y estrella (D34), QR y códigos de barras (D43), igualar tamaños (D70), espejo sobre línea (D27), Medir (D44). **Hechos en la ola 3:** dirección y punto de inicio (D57), Break Apart (D55), Optimizar formas (D56), Cut Shapes (D65), Trim con su herramienta del lienzo (D49), Resize Slots (D71), círculo tangente (D46), banda elástica (D59), asistente booleano (D64) y copiar a lo largo de un trazo (D69). **Pendientes:** la herramienta de clic por esquina del redondeo (D60) y las manijas y la malla 4×4 de deformar (D58).
8. **Texto.** **Hechos:** texto soldado (D38), curvado (D39), fuentes de una línea (D41). Texto variable / serie / CSV (D42/M38): **hecho** en la ola 3. **Pendientes:** alineación vertical y RTL (D37).
9. **Corte.** **Hechos** en la ola 2: modo perforación (C13), lead-in/out y overcut (C15/C16), pausas y corte pasante (C17), modo punto (C18), rampa (C20), sub-capas (C30), prioridad y escala de potencia por objeto (C31), ajustes de optimización (C32/M49), copiar/pegar ajustes y borrar capa (C06), relleno por regiones y agrupación (C28/C27), imagen sin tramar, realce y quitar motas (C48/C51), incremento de ángulo (C50). Hechos en la ola 3: puentes manuales (herramienta `U`) y la decisión sobre la potencia mínima en líneas (C08, ver las discrepancias); la potencia de puente (C14/D45) entró en la ola 4. **Pendientes:** velocidad en mm/s (C07), valores por defecto por color y máquina (C35).
10. **Máquina.** **Hechos** en la ola 2: posicionar el láser con clic (M17), ajuste de velocidad y potencia en vivo (M18), encuadre de contorno (M12), lista de comprobación del trabajo (M53) y posición final y pasos configurables de la ventana Mover. **Hechos** en la ola 4: macros (M20), Z, Focus, Fire y posiciones guardadas en Mover (M16/M31), encuadre en bucle y con el láser a baja potencia (M11), paquetes de usuario con máquinas, materiales y preferencias (`.tlakubundle`, M4), ajustes `$` de GRBL con respaldo (M25) y automatización por UDP (M54). **Pendientes:** lo que falta en los ajustes de dispositivo (M3, M5, M6) y las cámaras en los paquetes (M4); las preferencias y los atajos ya viajaban en `.tlakuprefs` (D99).
11. **Print & Cut** (M37): **hecho** en la ola 5, con ajuste sin escala, de escala uniforme o afín, medido con el láser o con la cámara (las imágenes no giran con el registro). Los **tabs manuales** (D45) entraron en la ola 3 y su potencia de puente en la ola 4.
12. **Nesting** (D76): **hecho** en la ola 3 (`tlaku-nest`).
13. **Selecciones especiales** (D21) y **grupos anidados** (D30): **hechos.** Menú contextual (D09): **hecho.** **Guías arrastrables** (D78): **hechas** en la ola 3 (faltan las guías temporales de alineación).
14. **Rendimiento de la interfaz** con miles de objetos: medir, y si hace falta pasar el lienzo a WebGL o añadir un índice espacial. **Pendiente** (ola 6).

### 3.3 Puede esperar

- **Galvo / fibra / EZCAD** (M30, M40-M46, C47, C59): el motor y la línea de comandos existen desde la ola 5 (`tlaku-galvo`, `tlaku galvo`), experimentales y solo contra emulador; falta la interfaz en la app y, sobre todo, una tarjeta con cabezal para la prueba mínima de `docs/hardware/galvo.md` sección 9. EZCAD3/DLC y BSL siguen fuera de alcance. <!-- GALVO-APP -->
- **Funciones P2 de edición:** deformar con manijas en el lienzo y malla 4×4 (D58), paneles flotantes (D04), modo principiante completo (D18; el del editor de capa, C60, ya existe). Ya hechas en la ola 3: casco convexo (D59), círculo tangente (D46), biblioteca de arte (D47), arreglo virtual (D67), copy along path (D69), Move Together/Dock (D74), trazado por lotes (D80), máscara de imagen (D81), imagen a bitmap (D82), auto-agrupar (D31) y nesting (D76); notas, plantillas y nueva ventana (D19) quedan Parcial. Ya hechas en la ola 1: interruptores de transformación (D17), rotación por dos puntos (D25), imprimir (D98).
- **Importadores raros:** formatos propietarios de máquina (D90, y `.rd`/`.sc` de M22), exportar AI (D94). Ya hechos: PLT/HPGL (D88), G-code (D89), exportar imagen (D95).
- **Control avanzado:** el alimentador (M47) quedó Parcial en la ola 5 (solo lo usa el tiling) y los ajustes de controlador DSP (C59) existen solo para el galvo, sin interfaz; faltan el PWM de Ruida, el PPI y el offset de U. Llegaron en la ola 4: el eje U y el láser 2 (M32, Parcial), la lista de archivos del controlador (M24, Parcial y solo lectura), la automatización UDP (M54, Hecho), el Center Finder (M48) y la cámara en el cabezal (M36), estos dos Parcial y por validar con hardware real.
- **Corte avanzado:** nada de la lista original; el corte avanzado de la ola 2 ya está hecho, y la dirección y el punto de inicio manuales (D57) entraron en la ola 3.
- **Tiling** (M39): ventaja competitiva, hecha en la ola 5; no bloqueaba la migración.
- **Historial con previsualización** (D14), calidad de bordes (D08), estimación calibrada con tiempos reales (C58), arcos G2/G3 en el G-code (hoy aplana las curvas).
