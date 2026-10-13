# 04 - Cobertura funcional real de Tlaku frente al inventario de LightBurn

Auditoría hecha el 2026-10-13 sobre el código (último commit: `62a8841`, "Fase 4, oleada C"), no sobre los documentos. Para cada una de las 214 funciones de los inventarios [01](01-diseno-edicion.md), [02](02-capas-corte-imagen.md) y [03](03-maquinas-avanzadas.md) se buscó el comando, el componente, la función del motor y sus pruebas. Los conteos del documento [00-resumen.md](00-resumen.md) se contrastaron con el código.

## Cómo leer este documento

- **Identificadores.** `D` = inventario 01 (diseño y edición, F01-F100), `C` = inventario 02 (capas, corte e imagen, F01-F60), `M` = inventario 03 (máquinas y avanzadas, filas 1-54). Se prefijan porque los inventarios 01 y 02 repiten la numeración `F01...`. El inventario 03 tiene 54 filas, no 53 como declara 00-resumen.md: la fila 21 mezcla prioridades (P0 para G-code, P1 para `.rd`) y 00-resumen no la cuenta. Aquí se cuenta como P0, así que el total real es 214 (68 P0, 86 P1, 60 P2) y no 213 (67/86/60).
- **Hecho**: funciona de punta a punta (motor más interfaz) y el flujo principal está completo. Las carencias secundarias se anotan como "Menor: ..." en la columna "Qué falta" y no cambian el estado. **Parcial**: falta una parte que un usuario notaría. **Solo motor/CLI**: existe en el motor o en `tlaku` pero la app no lo ofrece. **Falta**: no hay código. **No aplica**: decisión explícita de no hacerlo (ver nota abajo).
- **Evidencia.** `ui:` = `apps/desktop/src/`; `tauri:` = `apps/desktop/src-tauri/src/`; las rutas `crates/...` son del motor. Una evidencia "ninguna referencia" significa que se buscó por nombre, por comando y por palabra clave en `crates/` y en `apps/desktop/src` y no apareció nada.
- **Alcance de "Hecho".** Ninguna función de máquina está probada con hardware real (lo declara 00-resumen.md y sigue siendo cierto): "Hecho" significa que funciona contra el emulador (`tlaku-emu`) y las máquinas virtuales de la app, con pruebas automáticas (alrededor de 1180 `#[test]` en Rust y 1050 casos en la interfaz). La promesa de una prueba con la Falcon2 sigue pendiente.
- **"No aplica": ninguna.** No hay en el repositorio una decisión explícita de descartar una función del inventario. Lo más cercano es el alcance del galvo (EZCAD3/DLC y BSL fuera de alcance, `docs/hardware/galvo.md`), que afecta a M41. Candidatas razonables a marcar "No aplica" cuando se decida: D90 (formatos propietarios `.sc/.scpro2/.mage`), D94 (exportar AI), D99 (`.lbprefs`/`.lbzip`, que Tlaku sustituye con sus propios archivos) y M54 (UDP de LightBurn).

## 1. Resumen

### Por prioridad

|  | Hecho | Parcial | Solo motor/CLI | Falta | No aplica | Total |
|---|---:|---:|---:|---:|---:|---:|
| P0 | 53 | 13 | 1 | 1 | 0 | 68 |
| P1 | 30 | 16 | 4 | 36 | 0 | 86 |
| P2 | 5 | 2 | 1 | 52 | 0 | 60 |
| Total | 88 | 31 | 6 | 89 | 0 | 214 |

### Por área

|  | Hecho | Parcial | Solo motor/CLI | Falta | No aplica | Total |
|---|---:|---:|---:|---:|---:|---:|
| Diseño y edición | 37 | 14 | 0 | 49 | 0 | 100 |
| Capas, corte e imagen | 37 | 7 | 2 | 14 | 0 | 60 |
| Máquinas y avanzadas | 14 | 10 | 4 | 26 | 0 | 54 |

### Por área y prioridad

| Área | Prio | Hecho | Parcial | Solo motor/CLI | Falta | No aplica | Total |
|---|---|---:|---:|---:|---:|---:|---:|
| Diseño y edición | P0 | 25 | 6 | 0 | 1 | 0 | 32 |
| Diseño y edición | P1 | 11 | 6 | 0 | 22 | 0 | 39 |
| Diseño y edición | P2 | 1 | 2 | 0 | 26 | 0 | 29 |
| Capas, corte e imagen | P0 | 20 | 2 | 0 | 0 | 0 | 22 |
| Capas, corte e imagen | P1 | 13 | 5 | 2 | 3 | 0 | 23 |
| Capas, corte e imagen | P2 | 4 | 0 | 0 | 11 | 0 | 15 |
| Máquinas y avanzadas | P0 | 8 | 5 | 1 | 0 | 0 | 14 |
| Máquinas y avanzadas | P1 | 6 | 5 | 2 | 11 | 0 | 24 |
| Máquinas y avanzadas | P2 | 0 | 0 | 1 | 15 | 0 | 16 |

**Lectura rápida.** De 214 funciones, 88 están hechas (41 %), 31 parciales, 6 solo en el motor o el CLI y 89 faltan. En P0 (el MVP) hay 53 de 68 hechas (78 %); 13 parciales, 1 solo motor/CLI y 1 faltan. El corazón de capas, corte e imagen y el control de máquinas GRBL están muy completos; lo que más falta es el entorno de diseño que un usuario de LightBurn espera (voltear, formas, medir, exportar, importar más formatos, menú contextual) y todo lo que rodea al uso diario (preferencias, autoguardado, idioma, ayuda).

**Discrepancias con los documentos.**

- `README.md` (raíz) sigue diciendo "Fase 1: motor sin interfaz... Todavía no hay interfaz gráfica" y lista solo 12 crates; hoy hay 17 crates, una app Tauri completa e instaladores. Es lo primero que ve un visitante.
- La herramienta **Medir** (`M`) figura en la paleta y en el menú Herramientas, pero no tiene lógica: el lienzo la trata como Seleccionar (D44).
- `docs/spec/00-resumen.md` anuncia que M3 "cubre los 67 P0". Con el código en la mano, 53 de los 68 P0 (contando la fila M21) están hechos. Los otros 15: D26 voltear (falta), M15 cortar solo la selección (solo motor) y 13 parciales: D24 rotación rápida, D30 grupos anidados, D32 curvas dibujadas, D77 snapping a nodos, D87 formatos de imagen, D91 opciones de importación, C32 opciones del planificador, C51 Adjust Image, M3 perfiles importables/exportables, M5 y M6 ajustes de dispositivo, M11 encuadre y M16 ventana Mover.
- `tlaku-geom/src/nesting.rs` se llama "nesting", pero solo calcula la profundidad de contención; no hay empaquetado de piezas.
- El planificador de corte (`tlaku-plan`) soporta grupo y prioridad por objeto, pero `tlaku-pipeline` siempre pasa `priority: 0` y `group: None` (`build.rs:180-181`), así que esas dos palancas no se pueden usar.
- Funciones que existen en el motor pero no están cableadas: relleno por forma (`Grouping::PerShape`), `Sampling::PassThrough`, `enhance` de Adjust Image, bytes de override del streamer GRBL, `JobOptions.selection`, `remove_redundant_points`.

## 2. Tablas por área

### 2.1 Diseño y edición (inventario 01)

| ID | Función | Prio | Estado | Qué falta | Evidencia |
|---|---|---|---|---|---|
| D01 | Lienzo (Workspace) | P0 | Hecho | — | ui:features/canvas/CanvasView.tsx, canvas2d.ts; tema claro/oscuro (`view.theme`) |
| D02 | Paleta de colores / capas | P0 | Hecho | Menor: solo una capa de guías (equivale a T1); no hay T2. | ui:features/layers/ColorPalette.tsx, lib/palette.ts (`GUIDE_ENTRY`: nunca sale a la máquina) |
| D03 | Lista de capas (vista de diseño) | P0 | Hecho | Menor: sin "parpadeo" del objeto con clic derecho. | ui:features/layers/LayersPanel.tsx; comando `layer.selectObjects` |
| D04 | Barras y ventanas acoplables | P1 | Parcial | El panel derecho tiene pestañas y ancho ajustable, pero no se puede arrastrar, flotar, reordenar ni restaurar el diseño. | ui:features/shell/Dock.tsx, store/view.ts |
| D05 | Barra de estado | P1 | Hecho | Menor: no muestra dx/dy/longitud/ángulo de la herramienta activa. | ui:features/shell/StatusBar.tsx (cursor, zoom, selección y tamaño, máquina) |
| D06 | Barra Numeric Edits | P0 | Hecho | Menor: sin selector global mm/in (las unidades se escriben en la expresión: `5in`, `50%`). | ui:features/canvas/NumericBar.tsx, expr.ts, ExprField.tsx |
| D07 | Zoom y paneo | P0 | Hecho | Menor: sin "encuadrar selección" (hay ajustar a cama y al diseño). | ui:features/canvas/useViewportControls.ts, ZoomControls.tsx; `view.zoomIn/Out/fitBed/fitContent`, herramienta Mano (`H`) |
| D08 | Estilo de vista | P1 | Hecho | Menor: sin ajuste de calidad de bordes. | ui:commands/definitions/view.ts (`view.wireframe`, `W`) |
| D09 | Menú contextual | P1 | Falta | No hay menú contextual en el lienzo (convertir a trayectos, bloquear, agrupar...). Solo existe el menú de fila en la lista de capas. | ui:features/canvas/useCanvasTools.ts:655 (el evento `contextmenu` solo se usa para terminar una polilínea); RowMenu.tsx es de capas |
| D10 | Atajos por defecto | P0 | Hecho | El esquema es propio (V, N, L, R, E, T, H...) y no reproduce el de LightBurn. Faltan los atajos de funciones que no existen (voltear, rotar 90 grados, orden de dibujo...). | ui:commands/definitions/*.ts, commands/shortcuts.ts; tabla generada en apps/desktop/README.md; diálogo `F1` |
| D11 | Editor de atajos | P1 | Falta | El diálogo de atajos (F1) solo lista y busca. No se puede reasignar, limpiar, restablecer, exportar ni importar. | ui:features/shell/ShortcutsDialog.tsx; los atajos están fijos en commands/definitions/ |
| D12 | Preferencias de edición | P1 | Falta | No hay ventana de preferencias: snap fijo a 8 px, paso de flechas fijo (1 / 10 / 0,1 mm), unidades solo mm, sin opción de "volver a Seleccionar". | ui:features/canvas/snap.ts (`SNAP_PX`), commands/definitions/edit.ts (`NUDGE_STEPS`) |
| D13 | Deshacer / rehacer | P0 | Hecho | — | ui:store/document.ts (historial con etiquetas y fusión de ráfagas), store/history.test.ts |
| D14 | Historial de deshacer | P2 | Hecho | Menor: salta a un punto del historial, pero no lo previsualiza antes de saltar. | ui:features/history/HistoryPanel.tsx, timeline.ts; `jumpTo` |
| D15 | Portapapeles básico | P0 | Hecho | Menor: pega desplazado, no centrado en el cursor. | ui:commands/definitions/edit.ts (copiar, cortar, pegar, pegar en el mismo lugar, duplicar, eliminar) |
| D16 | Portapapeles entre apps | P1 | Falta | El portapapeles es interno de la app: no lee ni escribe el del sistema (no se pega desde Inkscape/Illustrator/Corel). | ui:store/document.ts `copyObjects` -> `set({ clipboard })` |
| D17 | Interruptores de transformación | P2 | Falta | No hay interruptores globales de mover/escalar/rotar/sesgar. | ninguna referencia en ui: |
| D18 | Modo principiante | P2 | Falta | No hay modo principiante. | ninguna referencia en ui: |
| D19 | Nueva ventana, notas, plantillas | P2 | Falta | Sin segunda ventana, notas de proyecto ni plantillas. | ninguna referencia en ui: |
| D20 | Selección | P0 | Hecho | Menor: sin Tab para ciclar objetos. | ui:features/canvas/hit.ts, useCanvasTools.ts (caja de izquierda a derecha = contenido, de derecha a izquierda = cruzada; Mayús suma/quita; Esc) |
| D21 | Selecciones especiales | P1 | Parcial | Solo "invertir selección" y "seleccionar objetos de la capa". Faltan formas abiertas, abiertas en capa de relleno, contenidas, menores que la seleccionada y círculos por tamaño. | ui:commands/definitions/edit.ts (`edit.selectInvert`), layers.ts (`layer.selectObjects`) |
| D22 | Manejadores de transformación | P0 | Hecho | Menor: no hay sesgar (skew). Escalar y rotar con manijas, Mayús/Alt y Esc sí. | ui:features/canvas/transform.ts, handles.ts, useCanvasTools.ts |
| D23 | Nudge con flechas | P0 | Hecho | Menor: pasos fijos (1 / 10 / 0,1 mm; LightBurn usa 5 mm) y no configurables. | ui:commands/definitions/edit.ts (`edit.nudge.*`) |
| D24 | Rotación rápida por teclado | P0 | Parcial | Solo ángulo libre en la barra numérica y pasos de 15 grados con Mayús al arrastrar. No hay teclas de 90/45/10/5 grados (`.` y `,` están ocupadas por la velocidad de la vista previa). | ui:features/canvas/NumericBar.tsx, transform.ts (`rotateByPointer`) |
| D25 | Rotación/escala por dos puntos | P2 | Falta | Sin rotación/escala por dos puntos. | ninguna referencia en ui: |
| D26 | Voltear H/V | P0 | Falta | No existe voltear horizontal/vertical (es P0). Solo se consigue un reflejo arrastrando una manija más allá del borde. | ui:commands/definitions/ sin `flip`; features/canvas/transform.test.ts:68 solo prueba el reflejo por arrastre |
| D27 | Espejo sobre línea | P1 | Falta | Sin espejo sobre una línea. | ninguna referencia en ui: |
| D28 | Bloquear formas | P1 | Parcial | El modelo ya tiene `locked`/`hidden` y mover, borrar, seleccionar y transformar lo respetan, y `setObjectFlags` existe, pero ningún comando, menú ni botón permite bloquear u ocultar objetos. | ui:store/document.ts (`setObjectFlags`, solo la define); 0 usos en componentes |
| D29 | Orden de dibujo | P1 | Falta | Sin enviar adelante/atrás ni al frente/fondo. | ninguna referencia en ui: |
| D30 | Agrupar / desagrupar | P0 | Parcial | Solo grupos planos (un id de grupo por objeto). No hay grupos anidados. | ui:lib/groups.ts, commands/definitions/edit.ts (`edit.group`, `edit.ungroup`) |
| D31 | Auto-group | P2 | Falta | Sin auto-agrupar. | ninguna referencia |
| D32 | Dibujar líneas / Bézier | P0 | Parcial | Polilíneas clic a clic con cierre (`C`), Mayús a 45 grados y auto-unión (`arrange.join`). Las curvas no se dibujan (clic + arrastre): solo se obtienen editando nodos (segmento a curva). | ui:commands/definitions/tools.ts, features/canvas/useCanvasTools.ts (`downLine`), nodeActions.ts |
| D33 | Rectángulo y elipse | P0 | Hecho | Menor: el radio de esquina se fija al dibujar y luego la forma es un trayecto (no paramétrica); sin radio negativo ni "desde el centro" con Ctrl. | ui:features/canvas/shapes.ts, NumericBar.tsx (`cornerRadius`) |
| D34 | Polígono, estrella y afines | P1 | Falta | No hay polígono, estrella ni afines (herramientas de dibujo: seleccionar, nodos, línea, rectángulo, elipse, texto, mano, medir). | ui:store/selection.ts (`TOOL_IDS`) |
| D35 | Ventana Shape Properties | P1 | Parcial | La barra numérica, el panel de texto y el editor de capa cubren partes; no hay ventana de propiedades por tipo de forma, ni prioridad de corte ni escala de potencia por objeto. | ui:features/canvas/NumericBar.tsx, features/text/TextPanel.tsx |
| D36 | Crear/editar texto | P0 | Hecho | — | ui:features/text/*, canvas/textTool.ts; crates/tlaku-text (rustybuzz, kerning, ligaduras, varias líneas) |
| D37 | Opciones de texto | P0 | Hecho | Menor: sin alineación vertical, sin mayúsculas forzadas y sin RTL (árabe/hebreo). | ui:features/text/TextPanel.tsx; crates/tlaku-text/src/spec.rs (`TextSpec`) |
| D38 | Texto soldado (Welded) | P1 | Parcial | No hay opción de texto soldado: se logra convirtiendo a trayectos y usando Soldar a mano. | ui:features/text/convert.ts + `arrange.weld` |
| D39 | Texto curvado (Bend) | P1 | Falta | Sin texto curvado (Bend/Distort). Existe texto sobre trayecto, que es otra función. | ninguna referencia |
| D40 | Aplicar path a texto | P1 | Hecho | Menor: sin alineación X/Y sobre el trayecto (hay desplazamiento y voltear lado). | ui:features/text/outlineSync.ts; crates/tlaku-text/src/path_sampler.rs; `text_outline` con `path` |
| D41 | Gestor de fuentes y SHX | P2 | Parcial | Selector de fuentes del sistema con búsqueda y estilos. Faltan favoritas, ocultar, categorías y fuentes de una línea (SHX / Hershey). | ui:features/text/FontPicker.tsx, fonts.ts; tauri:edit/text.rs (`list_fonts`) |
| D42 | Texto variable | P1 | Falta | Sin texto variable (fecha/hora, serie, CSV, valores de capa) ni "bake". | ninguna referencia; no hay dependencia `csv` en Cargo.lock |
| D43 | Códigos de barras y QR | P1 | Falta | Sin QR ni códigos de barras (no hay `qrcode`, `rxing` ni similar en Cargo.lock). | Cargo.lock |
| D44 | Medir | P1 | Falta | La herramienta Medir está en la paleta, el menú y con la tecla `M`, pero no hace nada: el lienzo la trata como Seleccionar. Es un botón engañoso. | ui:store/selection.ts (`measure` en TOOL_IDS); useCanvasTools.ts `onDown` solo ramifica rect/ellipse/line/nodes/text/pan, el resto cae en `downSelect`; 0 referencias a measure en features/canvas |
| D45 | Tabs / puentes | P1 | Parcial | Puentes automáticos por capa line (cantidad o separación, omitir interiores, avisos frente al kerf, vista previa y G-code). Faltan puentes manuales arrastrables y potencia de puente. | crates/tlaku-geom/src/tabs.rs, tlaku-pipeline/src/tabs.rs; ui:features/layers/LayerEditor.tsx |
| D46 | Círculo tangente | P2 | Falta | Sin círculo tangente. | ninguna referencia |
| D47 | Biblioteca de arte | P2 | Falta | Sin biblioteca de arte. | ninguna referencia |
| D48 | Editar nodos | P0 | Hecho | Menor: faltan alinear, recortar y extender nodos. Hay insertar, borrar, línea/curva, suave/esquina/simétrico, romper, unir, cerrar, caja de nodos, X/Y exactas. | ui:features/canvas/nodeTool.ts, nodeedit.ts, nodeActions.ts, NodeBar.tsx |
| D49 | Trim Shapes | P1 | Falta | Sin Trim Shapes. | ninguna referencia |
| D50 | Convertir a path | P0 | Hecho | Texto a trayectos (`convertTextToPaths`). Rectángulo y elipse ya son trayectos. No hay códigos de barras que convertir. | ui:features/text/convert.ts |
| D51 | Auto-Join | P0 | Hecho | — | ui:`arrange.join` (tolerancia 0,05 mm por defecto); crates/tlaku-geom/src/join.rs; geometry_op `join` |
| D52 | Close Path | P1 | Hecho | — | ui:`arrange.close`; crates/tlaku-geom/src/join.rs (`close_paths`, 0,5 mm por defecto) |
| D53 | Cerrar con tolerancia | P1 | Hecho | — | ui:features/arrange/CleanupDialog.tsx, cleanup.ts |
| D54 | Borrar duplicados | P1 | Hecho | — | ui:`arrange.dedupe`; crates/tlaku-geom/src/dedupe.rs |
| D55 | Break Apart | P1 | Falta | Sin Break Apart (partir un trayecto en segmentos). "Romper en el nodo" es otra operación. | ninguna referencia |
| D56 | Optimizar formas | P1 | Falta | Sin Optimizar formas (suavizar, ajustar a líneas/arcos, conteo de nodos). El motor tiene `remove_redundant_points` pero no se expone. | crates/tlaku-geom/src/util.rs:137 (sin comando) |
| D57 | Dirección y punto de inicio | P1 | Falta | Sin invertir dirección ni fijar nodo de inicio. | ninguna referencia |
| D58 | Warp / Deform | P2 | Falta | Sin Warp / Deform. | ninguna referencia |
| D59 | Contorno "banda elástica" | P2 | Falta | Sin contorno envolvente (casco convexo). | ninguna referencia |
| D60 | Radio / Fillet | P1 | Falta | Sin Radio / Fillet de esquinas (solo el radio al dibujar un rectángulo). | ninguna referencia |
| D61 | Offset de formas | P0 | Hecho | — | ui:features/arrange/OffsetDialog.tsx; geometry_op `offset` (redondo, inglete, bisel); crates/tlaku-geom/src/offset.rs |
| D62 | Booleanas: unión, resta, intersección | P0 | Hecho | — | ui:features/arrange/BooleanDialog.tsx (unión, diferencia con A elegible, intersección, xor); crates/tlaku-geom/src/boolean.rs |
| D63 | Weld | P0 | Hecho | — | ui:`arrange.weld`; crates/tlaku-geom/src/boolean.rs (`weld`) |
| D64 | Asistente booleano | P2 | Falta | Hay un diálogo solo para la diferencia (elegir A); no hay asistente con vista previa para las demás operaciones. | ui:features/arrange/BooleanDialog.tsx |
| D65 | Cut Shapes | P1 | Falta | Sin Cut Shapes. | ninguna referencia |
| D66 | Grid Array | P0 | Hecho | Menor: sin desfases, espejo alterno ni rotación aleatoria con semilla. Hay filas/columnas, separación por hueco o por paso, vista previa y aviso de salir de la cama. | ui:features/arrange/ArrayDialog.tsx, arrays.ts |
| D67 | Arreglo virtual | P2 | Falta | Sin arreglo virtual. | ninguna referencia |
| D68 | Circular Array | P1 | Hecho | — | ui:features/arrange/ArrayDialog.tsx (copias, ángulo, radio, centro, rotar copias) |
| D69 | Copy Along Path | P2 | Falta | Sin Copy Along Path. | ninguna referencia |
| D70 | Igualar ancho/alto | P2 | Falta | Sin igualar ancho/alto. | ninguna referencia |
| D71 | Resize Slots | P2 | Falta | Sin Resize Slots. | ninguna referencia |
| D72 | Alinear | P0 | Hecho | — | ui:features/arrange/align.ts, AlignDialog.tsx; `arrange.align.*`, `arrange.alignTo.selection\|object\|bed` |
| D73 | Distribuir | P1 | Hecho | — | ui:`arrange.distribute.h\|v` |
| D74 | Move Together y Dock | P2 | Falta | Sin Move Together ni Dock. | ninguna referencia |
| D75 | Mover selección a posición | P1 | Hecho | — | ui:`arrange.centerOnBed`, `arrange.alignTo.bed` + alinear a bordes de la cama |
| D76 | Nesting | P2 | Falta | Sin nesting. `tlaku-geom/nesting.rs` solo calcula la profundidad de anidamiento (contención), no empaqueta. | crates/tlaku-geom/src/nesting.rs:82 |
| D77 | Snapping | P0 | Parcial | Ajusta a bordes y centros de cajas envolventes de objetos, cama y cuadrícula (8 px, Alt lo desactiva). No ajusta a nodos, puntos medios, intersecciones ni líneas, ni tiene modo pegajoso. | ui:features/canvas/snap.ts, SnapBar.tsx; `view.snap*` |
| D78 | Guías | P2 | Parcial | Existe la capa de guías (nunca sale a la máquina) pero no se arrastran guías desde las reglas ni hay alineación temporal con Alt. | ui:lib/palette.ts (`GUIDE_ENTRY`), features/canvas/Ruler.tsx (solo regla) |
| D79 | Trace Image | P1 | Hecho | — | ui:features/trace/*; crates/tlaku-trace (vtracer, modo spline/polygon, umbral, manchas, desenfoque, vista previa); tauri:edit/trace.rs |
| D80 | Trace multiarchivo | P2 | Falta | Sin trazado por lotes. | ninguna referencia |
| D81 | Máscara / recorte de imagen | P2 | Falta | Sin máscara/recorte de imagen. | ninguna referencia |
| D82 | Imagen a mapa de bits y opciones de imagen | P2 | Falta | Sin convertir vectores a bitmap ni guardar bitmap procesado. | ninguna referencia |
| D83 | Importar SVG | P0 | Hecho | Menor: sin selector 96/72 DPI (asume 96 y avisa; detecta Illustrator y sugiere el tamaño a 72 DPI). Recorte `clip-path`/`mask` real, grupos y texto con fuentes instaladas (usvg). | crates/tlaku-svg/src/lib.rs:174-243; tlaku-pipeline/src/import.rs; tauri `import_file` |
| D84 | Importar DXF | P0 | Hecho | Menor: sin opción de tolerancia de auto-cierre en el importador; el texto se ignora con aviso (como LightBurn). Detecta unidades ($INSUNITS), arcos, elipses, splines, bloques/INSERT, color ACI/true color. | crates/tlaku-dxf/src/lib.rs |
| D85 | Importar AI | P1 | Falta | Sin importar .ai (el formato moderno es PDF). | crates/tlaku-pipeline/src/import.rs (`input_kind`: solo svg, dxf, png, jpg, jpeg, bmp) |
| D86 | Importar PDF | P1 | Falta | Sin importar PDF (no hay lopdf/pdfium/mupdf en Cargo.lock). Un .pdf se rechaza con "formato no soportado". | crates/tlaku-pipeline/src/tests.rs:156; ui:features/history/history.test.ts:194 |
| D87 | Importar imágenes | P0 | Parcial | Solo PNG, JPG y BMP. Faltan GIF, TIFF, TGA y WebP (la crate `image` se compila solo con png, jpeg y bmp). | crates/tlaku-raster/Cargo.toml:8; tlaku-pipeline/src/import.rs |
| D88 | Importar PLT/HPGL | P2 | Falta | Sin importar PLT/HPGL. | ninguna referencia |
| D89 | Importar G-code | P2 | Falta | Sin importar G-code como geometría. | ninguna referencia |
| D90 | Otros formatos de máquina | P2 | Falta | Sin importar .rd, .sc, .nc, .mage... (el motor solo escribe .rd). | crates/tlaku-ruida/src/rd.rs es exportación |
| D91 | Opciones de importación | P0 | Parcial | La importación es automática y sin diálogo: agrupa en capas por color, selecciona y ajusta la vista, y avisa de todo. Faltan opciones (agrupar, centrar, permitir capas T, DPI SVG, unidades DXF, tolerancia de cierre). | ui:features/history/files.ts `importPaths` |
| D92 | Exportar SVG | P1 | Falta | Sin exportar SVG. | ui:`file.exportGcode` es el único comando de exportación en el menú Archivo |
| D93 | Exportar DXF | P1 | Falta | Sin exportar DXF. | ninguna referencia (`dxf` crate solo se usa para leer) |
| D94 | Exportar AI | P2 | Falta | Sin exportar AI. | ninguna referencia |
| D95 | Exportar imágenes | P2 | Falta | Sin exportar imagen. | ninguna referencia |
| D96 | Proyecto nativo | P0 | Hecho | Menor: sin plantillas. Formato `.tlaku` (JSON versionado; migra la versión 1 sin pérdida), recientes, aviso de cambios sin guardar al abrir, crear nuevo y cerrar la ventana. | ui:features/history/files.ts, unsaved.ts, windowEvents.ts; tauri `save_project`/`load_project`; docs/app-api.md |
| D97 | Importar `.lbrn`/`.lbrn2` | P1 | Falta | Sin importar .lbrn/.lbrn2. Es la vía principal de migración. | ninguna referencia en crates/ ni ui: |
| D98 | Imprimir / PDF | P2 | Falta | Sin imprimir ni exportar a PDF. | ninguna referencia |
| D99 | Preferencias y bundles | P2 | Falta | Sin importar/exportar preferencias ni bundles. Solo hay import/export de la biblioteca de materiales (`.tlakumat`). | ui:features/materials/files.ts |
| D100 | Arrastrar y soltar archivos | P1 | Hecho | — | ui:features/history/DropOverlay.tsx, windowEvents.ts, history.test.ts |

### 2.2 Capas, corte e imagen (inventario 02)

| ID | Función | Prio | Estado | Qué falta | Evidencia |
|---|---|---|---|---|---|
| C01 | Ventana Cuts/Layers | P0 | Hecho | Menor: sin copiar/pegar ajustes ni borrar capa desde la lista. | ui:features/layers/LayersPanel.tsx, LayerEditor.tsx, RowMenu.tsx |
| C02 | Output on/off por capa | P0 | Hecho | — | ui:LayersPanel.tsx (`output`); la capa de guías nunca sale; crates/tlaku-pipeline/src/layers.rs |
| C03 | Show/Hide por capa | P0 | Hecho | — | ui:LayersPanel.tsx (`visible`) |
| C04 | Air assist por capa | P1 | Hecho | — | ui:LayerEditor.tsx (`air`); crates/tlaku-doc (`GcodeDialect.air_on/air_off`); aviso si el dialecto no tiene aire |
| C05 | Orden de capas + "Sort Cuts Last" | P0 | Hecho | — | ui:features/layers/order.ts; crates/tlaku-pipeline/src/build.rs (`layer_order` auto/file, `order` fijado); `tlaku gcode --layer-order` |
| C06 | Copiar/pegar ajustes, borrar capa, seleccionar objetos de capa | P1 | Parcial | Hay "seleccionar objetos de la capa" y fijar posición. Faltan copiar/pegar ajustes entre capas y borrar capa. | ui:features/layers/actions.ts, LayersPanel.tsx |
| C07 | Editor de ajustes: Nombre, Modo, Speed, Max Power, Pass count | P0 | Hecho | Menor: la velocidad solo está en mm/min (sin mm/s). | ui:features/layers/LayerEditor.tsx, fields.ts |
| C08 | Min Power | P1 | Parcial | `min_power` solo se usa en capas de imagen. En line/fill no hay potencia mínima en esquinas. | tlaku-doc `LayerSettings.min_power`; ui:LayerEditor.tsx (campo en modo image) |
| C09 | Constant vs Variable power (M3/M4, GRBL) | P0 | Hecho | Es un ajuste del dispositivo (dinámico/constante), como en LightBurn. | tlaku-doc `DeviceProfile.power_mode`; crates/tlaku-grbl/src/gcode.rs; ui:features/machine/DeviceKindField.tsx |
| C10 | Modo Line | P0 | Hecho | — | crates/tlaku-pipeline/src/build.rs; tlaku-core/src/toolpath.rs |
| C11 | Passes | P0 | Hecho | — | `LayerSettings.passes`; ui:LayerEditor.tsx |
| C12 | Kerf offset | P1 | Hecho | — | crates/tlaku-geom/src/kerf.rs; ui:LayerEditor.tsx (`kerf`); la vista previa coincide con el G-code |
| C13 | Perforation mode | P1 | Falta | Sin modo perforación (corta X, salta Y). | ninguna referencia en crates/ ni ui: |
| C14 | Tabs / bridges (manual y auto) | P1 | Parcial | Ver D45: puentes automáticos sí; manuales y potencia de puente no. | crates/tlaku-geom/src/tabs.rs; tlaku-pipeline/src/tabs.rs; ui:LayerEditor.tsx |
| C15 | Lead-in / Lead-out | P2 | Falta | Sin lead-in / lead-out. | ninguna referencia |
| C16 | Overcut | P2 | Falta | Sin overcut. | ninguna referencia |
| C17 | Start/End pause + Cut Through | P2 | Falta | Sin pausas de inicio/fin ni Cut Through. | ninguna referencia |
| C18 | Dot mode | P2 | Falta | Sin modo punto (dot mode). | ninguna referencia |
| C19 | Z offset / Z step por pasada | P1 | Hecho | Menor: Z relativo por capa y por pasada, con inversión de signo; solo si la máquina tiene eje Z. No hay movimiento de Z en la ventana Mover ni autoenfoque. | crates/tlaku-pipeline/src/zaxis.rs; tlaku-doc (`z_offset`, `z_step`, `z_axis`, `z_invert`); ui:LayerEditor.tsx |
| C20 | Ramp length | P2 | Falta | Sin ramp length. | ninguna referencia |
| C21 | Modo Fill (scanline) | P0 | Hecho | — | crates/tlaku-fill/src/scan.rs; ui:LayerEditor.tsx (modo fill) |
| C22 | Line interval / LPI / DPI | P0 | Hecho | — | `LayerSettings.interval`; ui:LayerEditor.tsx (intervalo y DPI) |
| C23 | Scan angle | P0 | Hecho | — | `LayerSettings.angle` (fill e image) |
| C24 | Cross-hatch | P1 | Hecho | — | `LayerSettings.crosshatch` |
| C25 | Bidireccional | P0 | Hecho | — | `LayerSettings.bidirectional` |
| C26 | Overscanning | P0 | Hecho | Calculado con la física del movimiento (`v^2/2a`) y verificado contra la cama antes de iniciar. | crates/tlaku-sim/src/plan.rs; `LayerSettings.overscan` (auto o mm) |
| C27 | Fill grouping | P1 | Solo motor/CLI | El relleno por forma (`Grouping::PerShape`) existe en el motor pero ni el pipeline ni la app ni el CLI lo exponen. | crates/tlaku-fill/src/lib.rs:56, cluster.rs; 0 usos en tlaku-pipeline |
| C28 | Flood fill | P2 | Falta | Sin flood fill (declarado pendiente en 00-resumen.md). | docs/spec/00-resumen.md; ninguna referencia en crates/ |
| C29 | Offset fill | P1 | Hecho | — | `LayerSettings.fill_style = offset`; crates/tlaku-fill/src/offset.rs |
| C30 | Sub-capas (hasta 11) | P1 | Falta | Sin sub-capas. | docs/spec/00-resumen.md ("Pendiente de M4") |
| C31 | Cut order priority (por objeto) y Power Scale | P1 | Falta | No hay prioridad por objeto ni escala de potencia: el pipeline pasa `priority: 0` y `group: None` al planificador. | crates/tlaku-pipeline/src/build.rs:180-181 |
| C32 | Optimization Settings (orden/ caminos) | P0 | Parcial | El planificador de `tlaku-plan` es completo y explicable (capa/grupo/prioridad, interiores primero, solapes, vecino más cercano + 2-opt/Or-opt, punto de inicio y sentido). En la app las opciones van fijas (por defecto) y no hay panel de optimización; grupo y prioridad no llegan al planificador. | crates/tlaku-plan/src/lib.rs; tlaku-pipeline/src/build.rs:186 (`PlanOptions { start, ..Default }`); `tlaku plan`; ui:features/preview/Results.tsx muestra las explicaciones |
| C33 | Remove overlapping lines | P1 | Parcial | El planificador ordena según solapes y `arrange.dedupe` quita objetos idénticos, pero no hay una opción que quite segmentos duplicados entre formas distintas. | crates/tlaku-plan/src/overlap.rs; tlaku-geom/src/dedupe.rs |
| C34 | Hide backlash (orden) | P2 | Falta | Sin hide backlash. | ninguna referencia |
| C35 | Default layer settings | P1 | Parcial | Valores de fábrica fijos por color; los presets se hacen con la biblioteca de materiales. No hay valores por defecto por color y máquina. | ui:lib/layers.ts, lib/palette.ts; features/materials/* |
| C36 | Modo Image: parámetros generales | P0 | Hecho | — | ui:LayerEditor.tsx (intervalo/DPI, invertir, bidireccional, overscan, ángulo, pasadas); crates/tlaku-raster |
| C37 | Threshold | P0 | Hecho | — | crates/tlaku-raster/src/dither.rs; `ImageMode::threshold` |
| C38 | Dither (Floyd-Steinberg) | P0 | Hecho | — | `ImageMode::floyd` |
| C39 | Stucki | P0 | Hecho | — | `ImageMode::stucki` |
| C40 | Jarvis | P0 | Hecho | — | `ImageMode::jarvis` |
| C41 | Atkinson | P1 | Hecho | — | `ImageMode::atkinson` |
| C42 | Ordered (Bayer) | P1 | Hecho | Menor: solo Bayer 4x4. | `ImageMode::bayer4`; tlaku-raster `BayerSize` |
| C43 | Newsprint | P2 | Hecho | — | `ImageMode::newsprint`; crates/tlaku-raster/src/screen.rs |
| C44 | Halftone (cell/angle) | P2 | Hecho | — | `ImageMode::halftone` (celda y ángulo); screen.rs |
| C45 | Sketch | P2 | Hecho | — | `ImageMode::sketch`; crates/tlaku-raster/src/sketch.rs |
| C46 | Grayscale | P1 | Hecho | Menor: la curva potencia/gris no se calibra con un asistente (idea en docs/ideas-mejoras.md). | `ImageMode::grayscale`; `ImageSettings`/`min_power` |
| C47 | 3D Slice (galvo) | P2 | Falta | Sin 3D Slice (galvo). | ninguna referencia |
| C48 | Pass-through | P1 | Solo motor/CLI | El remuestreo `Sampling::PassThrough` existe en `tlaku-raster`, pero el pipeline solo usa `Sampling::Interval`: no hay ajuste en la capa ni en la app. | crates/tlaku-raster/src/resample.rs:17; tlaku-pipeline/src/build.rs:366 |
| C49 | Dot width correction | P1 | Hecho | — | `ImageSettings.dot_width`; ui:LayerEditor.tsx |
| C50 | Angle increment (imagen/relleno) | P2 | Falta | Sin incremento de ángulo entre pasadas. | ninguna referencia |
| C51 | Adjust Image (brillo, contraste, gamma, enhance, denoise, presets) | P0 | Parcial | Brillo, contraste, gamma e invertir en el editor de capa. El motor tiene enhance (radio/cantidad) sin exponer; faltan denoise, presets y vista previa por imagen. | crates/tlaku-raster/src/adjust.rs (`ImageAdjust.enhance_*` sin campo en `ImageSettings`); ui:LayerEditor.tsx |
| C52 | Scanning Offset Adjustment | P1 | Hecho | Menor: no lee/escribe el offset en una Ruida (solo GRBL/G-code, tabla en el perfil). | crates/tlaku-pipeline/src/scan_offset.rs; `DeviceProfile.scan_offsets`, `scan_initial_offset`; ui:features/machine/DeviceExtras.tsx, features/devices |
| C53 | Fast Whitespace Scan | P1 | Hecho | Menor: salta filas en blanco por defecto, sin interruptor. | crates/tlaku-raster/src/scan.rs (`skip_blank_rows`, true por defecto) |
| C54 | Material Library | P1 | Hecho | Menor: asignar sí; no hay modo "Link" (enlazar capa con preset). Formato `.tlakumat` versionado, archivo dañado se aparta. | ui:features/materials/*; tauri:edit/materials.rs; docs/app-api.md |
| C55 | Material Test | P1 | Hecho | — | ui:features/materials/TestDialog.tsx; crates/tlaku-pipeline/src/material_test.rs (`material_test`) |
| C56 | Interval Test | P2 | Hecho | — | `TestParam::interval` en la prueba de materiales |
| C57 | Preview | P0 | Hecho | Menor: viajes, overscan, grabado y corte se distinguen por trazo, no hay sombreado continuo por potencia. | ui:features/preview/* (reproducción, velocidad, resultados por capa); `JobPreview.move_times` |
| C58 | Estimación de tiempo | P0 | Hecho | Falta calibrar con tiempos reales (pendiente declarado en 00-resumen.md); asume parada entre rápidos y cortes. | crates/tlaku-sim; `JobPreview.total_s`; ui:features/preview/Results.tsx |
| C59 | Ajustes dependientes de controlador (PWM freq, PPI, U offset, Frequency, Q-pulse, Wobble, timings) | P2 | Falta | Sin parámetros de controlador DSP/galvo (PWM, PPI, frecuencia, Q-pulse, wobble...). El `.rd` solo lleva velocidad/potencia con límites de seguridad. | crates/tlaku-ruida/src/rd.rs, safety.rs |
| C60 | Beginner mode | P2 | Falta | Sin modo principiante. | ninguna referencia |

### 2.3 Máquinas y funciones avanzadas (inventario 03)

La fila M21 mezcla prioridades (P0 G-code, P1 `.rd`); se cuenta como P0.

| ID | Función | Prio | Estado | Qué falta | Evidencia |
|---|---|---|---|---|---|
| M1 | Asistente "Find My Laser" (autodetección USB) | P1 | Hecho | Solo detecta por serie, de solo lectura (no mueve ni cambia nada). Sin detección por red (igual que LightBurn). | tauri:probe.rs; ui:features/devices/steps.tsx (detección); crates/tlaku-grbl/src/link.rs |
| M2 | Creación manual de dispositivo (wizard de 6 pasos) | P0 | Hecho | Menor: sin conexión Ethernet/TCP en el asistente. Catálogo de 21 modelos con fuentes y máquinas virtuales (equivalen a "No Machine"). | ui:features/devices/AddMachineWizard.tsx, catalog.ts, wizardStore.ts |
| M3 | Gestor de dispositivos y perfiles | P0 | Parcial | Lista, crea, duplica, edita y borra perfiles (`devices.json`). Faltan importar/exportar perfiles, marcar por defecto, ordenar e ícono. | ui:features/machine/DeviceManager.tsx; tauri:edit/devices.rs; `list_devices`, `save_device`, `delete_device` |
| M4 | Bundles de usuario (.lbzip) | P1 | Falta | Sin bundles de usuario (ajustes, atajos, dispositivos, cámaras, bibliotecas). | ninguna referencia |
| M5 | Device Settings: área, origen, unidades | P0 | Parcial | Área y origen (4 esquinas), aceleraciones y velocidades máximas por eje. Faltan offset del puntero rojo, unidades dispositivo/control (todo es mm y mm/min) e intercambio X/Y. | tlaku-doc `DeviceProfile`; ui:features/machine/DeviceFields.tsx |
| M6 | Device Settings: GRBL/G-code básicos | P0 | Parcial | Hay S máx., baudios, búfer RX, G-code de inicio/fin, volver al origen, aire y lectura de `$$` para proponer valores. Faltan auto-home, intercambio X/Y, red, DTR y modo síncrono (siempre usa conteo de caracteres). | ui:features/machine/DeviceFields.tsx, DeviceDialect.tsx; tauri:probe.rs |
| M7 | Modos M3/M4 y dispositivo "GRBL" vs "GRBL-M3" | P0 | Hecho | — | `DeviceKind::Grbl` / `GrblM3`; crates/tlaku-grbl/src/gcode.rs |
| M8 | Custom G-code (flavor, plantillas, variables) | P1 | Hecho | Menor: plantilla de dialecto (potencia, encendido/apagado, aire, comentarios, longitud de línea), no plantillas por cada evento (pausa, aborto...). | tlaku-doc `GcodeDialect`; ui:features/machine/DeviceDialect.tsx; docs/hardware/dialectos-gcode.md |
| M9 | Start / End scripts G-code | P0 | Hecho | — | `DeviceProfile.start_gcode`, `end_gcode` |
| M10 | Ventana Laser: Start/Pause/Stop | P0 | Hecho | — | ui:features/machine/MachinePanel.tsx, JobSection.tsx; comandos `machine.start\|pause\|stop`; tauri `start_job`, `pause_job`, `resume_job`, `cancel_job` |
| M11 | Frame (bounding box) | P0 | Parcial | Recorre el rectángulo con overscan y láser apagado (avisa antes de iniciar si no cabe). Falta repetir en bucle, encuadre con láser a baja potencia y recorrido de casco. | tauri `frame_job`; crates/tlaku-grbl/src/frame.rs; `tlaku frame`; ui:`machine.frame` |
| M12 | Rubber-band frame (casco convexo) | P1 | Falta | Sin frame de casco convexo. | ninguna referencia |
| M13 | Start From: Absolute Coords / User Origin / Current Position | P0 | Hecho | — | ui:features/machine/StartFromSection.tsx; `JobOptions.start_from`; tlaku-pipeline/src/place.rs |
| M14 | Job Origin de 9 puntos | P0 | Hecho | — | ui:features/machine/OriginPicker.tsx; `JobOptions.job_origin` |
| M15 | Cut Selected Graphics / Use Selection Origin | P0 | Solo motor/CLI | El motor ya filtra por `JobOptions.selection`, pero la app siempre manda `selection: null`: no hay opción "cortar solo la selección" ni "usar origen de la selección". | ui:store/preview.ts:45; crates/tlaku-pipeline/src/job.rs |
| M16 | Ventana Move: jog, home, origen, finish position | P0 | Parcial | Mover con pasos y velocidad, mantener pulsado (`$J`), fijar/borrar/ir al origen, home y desbloqueo con confirmación. Faltan Z/A, posiciones guardadas, mover desde el cero de máquina, posición final, Fire y Focus. | ui:features/machine/JogSection.tsx, useHoldJog.ts, AlarmActionDialog.tsx; tauri `jog` |
| M17 | Position Laser (clic en el espacio de trabajo) | P1 | Falta | Sin "posicionar láser" con clic en el lienzo. | ninguna referencia |
| M18 | Ajuste de velocidad/potencia durante el trabajo (GCode) | P1 | Falta | Sin ajuste de velocidad/potencia en vivo. El streamer conoce los bytes de override de GRBL (0x91...) pero nada los expone. | crates/tlaku-grbl/src/streamer.rs:70 (definidos, sin uso en tauri ni ui) |
| M19 | Consola | P0 | Hecho | — | ui:features/machine/ConsolePanel.tsx, consoleExplain.ts, commandSafety.ts, grblCodes.ts; tauri `machine_command`, `machine_realtime`; `tlaku cmd` |
| M20 | Macros | P1 | Falta | Sin macros. | docs/spec/00-resumen.md ("Pendiente de M4") |
| M21 | Guardar/ejecutar archivos de máquina | P0 | Hecho | G-code (P0): exportar a archivo, atómico, con advertencias. El `.rd` (P1) solo se exporta por CLI (`tlaku ruida`); no hay "ejecutar archivo de máquina". | ui:`file.exportGcode`; tauri `export_gcode`; crates/tlaku-ruida/src/rd.rs; tlaku-cli/src/ruida.rs |
| M22 | Importar G-code / .rd / .plt / .nc / .hpgl / .sc | P2 | Falta | Sin importar archivos de máquina. | ninguna referencia |
| M23 | Send a la memoria del controlador (DSP) | P1 | Solo motor/CLI | Ruida: enviar guardando en la memoria o ejecutando, solo por CLI y marcado EXPERIMENTAL (sin probar con máquina real). No está en la app. | crates/tlaku-cli/src/ruida.rs; tlaku-ruida/src/session.rs, link.rs |
| M24 | File List (archivos en controlador) | P1 | Falta | Sin lista de archivos del controlador. | ninguna referencia |
| M25 | Machine Settings (Ruida/TopWisdom/GRBL) | P1 | Parcial | Lee `$$` para proponer valores de perfil y la consola escribe `$n=v` con confirmación. Sin editor de ajustes del firmware, `.lbset`, respaldo automático ni calibración de ejes; nada para Ruida. | ui:features/machine/DeviceManager.tsx (propuesta), commandSafety.ts; tauri:probe.rs |
| M26 | Get Controller Info (Ruida) | P2 | Solo motor/CLI | `tlaku ruida status` lee el estado, no horas, trabajos ni recorrido acumulado. | crates/tlaku-cli/src/ruida.rs |
| M27 | Scanning Offset Adjustment | P1 | Hecho | Menor: no lee ni escribe en una controladora Ruida. Tabla de offset por velocidad y offset inicial, con prueba. | tlaku-doc `ScanOffset`; crates/tlaku-pipeline/src/scan_offset.rs; ui:features/machine/DeviceExtras.tsx |
| M28 | Rotary DSP (Ruida) | P1 | Falta | Sin rotativo DSP (Ruida). | ninguna referencia en tlaku-ruida |
| M29 | Rotary GCode (eje Y reemplazado, eje Z/A) | P1 | Hecho | Menor: sustituye el eje Y (rodillo/mandril), no usa un eje A aparte. | tlaku-doc `RotarySettings`; crates/tlaku-pipeline/src/rotary.rs; ui:features/machine/DeviceExtras.tsx, DeviceRotaryTest.tsx; tauri `rotary_test` |
| M30 | Rotary galvo | P2 | Falta | Sin rotativo de galvo. | docs/hardware/galvo.md |
| M31 | Eje Z / Focus Z | P1 | Parcial | Z por capa y por pasada, relativo, con inversión. Faltan mover Z en la ventana, Focus Z, optimizar Z y autoenfoque. | crates/tlaku-pipeline/src/zaxis.rs; `DeviceProfile.z_axis`, `z_invert` |
| M32 | Eje U y Laser 2 | P2 | Falta | Sin eje U ni segundo láser. | ninguna referencia |
| M33 | Cámara: calibración de lente | P1 | Hecho | Menor: usa tablero de ajedrez generado (SVG/PDF) y el modelo Brown-Conrady; sin AprilTag ni modelo de ojo de pez (declarado pendiente). | crates/tlaku-camera/src/calibration.rs, chessboard.rs; ui:features/camera/CalibrationDialog.tsx; tauri:edit/camera.rs |
| M34 | Cámara: alineación | P1 | Hecho | Alineación manual de 4 puntos o con marcas grabadas (cruces) con RANSAC, validación cruzada, y corrección por altura de material. | crates/tlaku-camera/src/marks.rs, height.rs, homography.rs; ui:features/camera/AlignmentSection.tsx, MarksDialog.tsx |
| M35 | Cámara: overlay, trace, captura | P1 | Parcial | Fondo en vivo corregido de lente (WebGL) bajo el diseño, con opacidad, visible y congelar. Faltan trazar desde la imagen, guardar el fondo como bitmap, cámaras de red y presets (solo cámaras locales por `getUserMedia`). | ui:features/camera/CameraView.tsx, corrector.ts, session.ts, capture.ts |
| M36 | Cámara montada en cabezal | P2 | Falta | Sin cámara montada en el cabezal. | ninguna referencia |
| M37 | Print & Cut | P1 | Falta | Sin Print & Cut. | docs/spec/00-resumen.md (M5) |
| M38 | Texto variable / serie / CSV | P1 | Falta | Sin texto variable / serie / CSV. | ninguna referencia |
| M39 | Tiling / divisiones de trabajos grandes | P2 | Falta | Sin tiling (LightBurn 2.1 tampoco lo tiene; es una ventaja prevista). | docs/spec/00-resumen.md (M5) |
| M40 | Split Marking y Repeat Marking (galvo) | P2 | Falta | Sin Split/Repeat Marking (galvo). | docs/hardware/galvo.md (solo investigación) |
| M41 | Galvo: asistente de alta | P2 | Falta | Sin galvo. EZCAD2 está planificado; EZCAD3/DLC y BSL quedan fuera de alcance por falta de documentación. | docs/hardware/galvo.md:73-74 |
| M42 | Galvo: framing con luz roja | P2 | Falta | Sin galvo. | docs/hardware/galvo.md |
| M43 | Galvo: calibración de campo/lente | P2 | Falta | Sin galvo. | docs/hardware/galvo.md |
| M44 | Galvo: temporización | P2 | Falta | Sin galvo. | docs/hardware/galvo.md |
| M45 | Galvo: puertos IO | P2 | Falta | Sin galvo. | docs/hardware/galvo.md |
| M46 | Cylinder Correction, Taper Warp (galvo/rotary) | P2 | Falta | Sin corrección de cilindro ni taper warp. | ninguna referencia |
| M47 | Feeder (auto-feeder) | P2 | Falta | Sin feeder. | ninguna referencia |
| M48 | Center Finder | P2 | Falta | Sin Center Finder. | ninguna referencia |
| M49 | Optimization Settings | P1 | Parcial | Igual que C32: planificador completo y explicable, sin panel de opciones en la app. | crates/tlaku-plan; tlaku-pipeline/src/build.rs:186 |
| M50 | Conexión de red GRBL (TCP) y Ethernet Ruida (UDP) | P1 | Solo motor/CLI | Ruida por UDP (sondeo, envío, estado, pausa, detención) solo desde el CLI y EXPERIMENTAL. La app solo conecta por puerto serie o máquina virtual; GRBL por TCP no existe. | crates/tlaku-ruida/src/link.rs; tlaku-cli/src/ruida.rs; tauri `list_ports`/`connect` (serie y virtual) |
| M51 | Preferencias | P1 | Falta | No hay ventana de preferencias (autosave, auto-update, cuadrículas, snaps, DPI, importación, cámara, registro de trabajos). Solo hay tema claro/oscuro y archivos recientes. | ui:`view.theme`; features/history/recent.ts |
| M52 | Multilenguaje | P1 | Falta | Solo español. Las claves están tipadas y centralizadas (`t()`), lo que facilita agregar inglés, pero no hay selector ni otro idioma. | ui:i18n/index.ts ("hoy solo español"), index.html `lang="es"` |
| M53 | Job checklist y protecciones | P1 | Parcial | Antes de enviar consulta el estado y no transmite si hay Alarm/Hold/Door/Run...; verifica la cama con overscan; diálogo de confirmación. No hay lista de comprobación configurable ni protección del botón Iniciar. | crates/tlaku-grbl/src/preflight.rs; tauri `start_job`; ui:features/machine/StartJobDialog.tsx |
| M54 | Automatización por UDP | P2 | Falta | Sin automatización por UDP. | ninguna referencia |

### 2.4 Lo que un usuario real necesita y el inventario no cubre bien

| Necesidad | Estado | Detalle y evidencia |
|---|---|---|
| Importar PDF | Falta | Un `.pdf` se rechaza ("formato no soportado"). No hay dependencia de PDF en `Cargo.lock`. `input_kind` solo acepta svg, dxf, png, jpg, jpeg, bmp (`crates/tlaku-pipeline/src/import.rs`). |
| Importar AI / EPS | Falta | Igual que PDF; un AI moderno es un PDF. EPS sin ninguna ruta. |
| Importar `.lbrn` / `.lbrn2` (migrar desde LightBurn) | Falta | Sin ninguna referencia en el código. Es el mayor freno para migrar: sin esto cada proyecto hay que reconstruirlo desde un SVG. |
| Exportar SVG / DXF | Falta | Sin ningún exportador. Hoy solo se sale por G-code (`file.exportGcode`), proyecto `.tlaku` y `.rd` por CLI. Quien deja Tlaku no se lleva su diseño en un formato abierto. |
| Importar imágenes WebP, TIFF, GIF, TGA | Falta | Solo PNG/JPG/BMP: la crate `image` se compila con `features = [png, jpeg, bmp]`. |
| Fuentes de una línea (SHX / Hershey) | Falta | Solo contornos de fuentes del sistema (`tlaku-text`). Para grabar texto de un solo trazo hay que usar otro programa. |
| Polígono, estrella | Falta | Las herramientas son seleccionar, nodos, línea, rectángulo, elipse, texto, mano y medir (el último sin función). |
| QR y códigos de barras | Falta | Sin generador (ninguna dependencia de `qrcode`/`rxing`). |
| Ventana de preferencias | Falta | No existe. Hay constantes fijas: snap 8 px, pasos de flecha, unidad mm, tema oscuro/claro por comando. |
| Idioma inglés (y otros) | Falta | Solo español (`ui:i18n/es*`). Las claves tipadas con `t()` hacen viable agregar inglés, y los mensajes del motor (advertencias, errores, ayuda de `clap`) también están en español, lo que hay que externalizar. |
| Atajos configurables | Falta | Atajos fijos en `commands/definitions`; el diálogo F1 solo los muestra. El registro central de comandos es la base correcta para hacerlos configurables. |
| Autoguardado y recuperación ante cierre inesperado | Falta | No existe (ninguna referencia a autosave/recuperación). Sí hay aviso de cambios sin guardar al abrir, nuevo y cerrar la ventana (`features/history/unsaved.ts`, `windowEvents.ts`), que no ayuda ante un cierre forzado o una falla. |
| Archivos recientes | Hecho | Hecho: 10 archivos, quita los que ya no existen (`features/history/recent.ts`, menú Archivo). Se guardan en `localStorage` de la ventana, no en la carpeta de configuración. |
| Arrastrar y soltar archivos | Hecho | Hecho (`DropOverlay.tsx`), con avisos de formatos no soportados. |
| Rendimiento con diseños grandes | Parcial | El motor es rápido: medido en esta auditoría con `tlaku` en modo release, 20 000 círculos se planifican y se convierten en 10 MB de G-code en 1,2 s; 5 000 en 0,7 s. En la interfaz no hay mediciones: el lienzo es Canvas2D, con un mapa de bits en caché de toda la escena durante el zoom y el paneo (se repinta completo al cambiar el documento, `canvas2d.ts`), y el "hit-test" recorre linealmente todos los objetos (`hit.ts`, `pickObject`); no hay índice espacial ni pruebas de carga con miles de objetos. La arquitectura prevista en 00-resumen.md era un lienzo WebGL; el actual es 2D. La vista previa tiene una prueba de rendimiento del modelo (`preview/model.test.ts`). |
| Manual o ayuda | Falta | Solo el diálogo de atajos (F1), textos de ayuda en campos y avisos con motivo. No hay manual, guía de inicio, menú Ayuda, acerca de, ni enlace a documentación. La documentación existente (`docs/`) está dirigida a desarrolladores. |
| Instaladores multiplataforma | Hecho | Flujo de GitHub Actions para macOS (universal), Windows (NSIS y MSI) y Linux (AppImage y deb), sin firma (`docs/publicar.md`, `.github/workflows/release.yml`). Aún sin actualizador automático. |
| Prueba con hardware real | Falta | Ninguna. Todo se validó contra emuladores. Es el principal riesgo para publicar. |

## 3. Lo que falta para un primer lanzamiento público

Criterio: una primera versión pública (0.x, "beta") debe ser segura de usar sobre una máquina real, no perder el trabajo del usuario y no mostrar nada que no funcione. El resto de LightBurn puede llegar por versiones.

### 3.1 Imprescindible antes de publicar

1. **Probar con hardware real** (la Falcon2 está disponible): conectar, encuadre, un corte y un grabado, pausa, detención y alarma. Sin esto, todo lo de máquina es teoría. Documentar el resultado y marcar en la app lo que sigue siendo experimental (Ruida).
2. **Autoguardado y recuperación** tras un cierre inesperado (hoy no existe). Un archivo de recuperación periódico en la carpeta de configuración y la oferta de restaurarlo al abrir.
3. **Quitar o terminar la herramienta Medir** (D44): es un botón que no hace nada en la paleta y en el menú.
4. **Voltear horizontal/vertical** (D26, P0) y teclas de rotación rápida (D24). Es una de las acciones más usadas en cualquier diseño y solo se consigue arrastrando una manija.
5. **Cortar solo la selección** (M15): el motor ya lo soporta (`JobOptions.selection`); falta una casilla en el diálogo de inicio y en el encuadre. Es P0 en el inventario.
6. **Bloquear/ocultar objetos y orden de dibujo** (D28, D29): el modelo ya los respeta; falta exponerlos (comando, menú contextual del lienzo, botones).
7. **Exportar SVG y DXF** (D92, D93): para no atrapar al usuario y para pasar piezas a otros programas.
8. **Importar más imágenes** (WebP, TIFF, GIF; D87), y las **opciones de importación** (D91): DPI 72/96 y unidades DXF elegibles en un diálogo, no solo en avisos.
9. **Preferencias mínimas** (D12, M51): unidades, pasos de flecha, tolerancia de snap, autoguardado, idioma, "volver a Seleccionar".
10. **Ayuda mínima**: menú Ayuda con guía de primer uso (conectar, origen, encuadre, iniciar), enlace al manual y la ventana "Acerca de"; actualizar el `README.md` de la raíz (todavía dice que no hay interfaz).
11. **Menú contextual del lienzo** (D09) y **snapping a nodos/puntos medios/intersecciones** (D77, P0 parcial).
12. **Curvas dibujadas** (D32, P0): clic + arrastre para crear Bézier; hoy solo se logran convirtiendo segmentos desde el editor de nodos.

### 3.2 Importante para que la gente migre desde LightBurn

1. **Importar `.lbrn2` / `.lbrn`** (D97). Sin esto cada migrante empieza de cero. Hay una referencia de estructura MIT (`lbrn2-to-svg`) citada en el inventario; respetar la política de código de terceros (`THIRD_PARTY.md`).
2. **Importar PDF / AI** (D85, D86) y **exportar AI/PDF** si se puede; el portapapeles entre aplicaciones (D16).
3. **Atajos configurables y un perfil "tipo LightBurn"** (D10, D11) para que las manos no se pierdan (Ctrl+L, Ctrl+R, Ctrl+E... son distintos hoy).
4. **Idioma inglés** (M52): el público de LightBurn es en su mayoría anglófono; el foro es en inglés.
5. **Ruida en la app** (M23, M50, M26 y M24): CO2 es la segunda familia más usada. Hoy es solo CLI y experimental; hay que probarlo con una máquina real, sumarlo al asistente "Agregar máquina" y a la ventana de máquina.
6. **Conexión por red para GRBL (TCP)** y para Ruida (UDP) en la app (M50).
7. **Herramientas de forma y edición frecuentes**: polígono/estrella (D34), QR y códigos de barras (D43), Trim (D49), Break Apart (D55), Fillet (D60), Cut Shapes (D65), Optimizar formas (D56), invertir dirección/punto de inicio (D57), igualar tamaños (D70), espejo sobre línea (D27), Medir de verdad (D44).
8. **Texto**: texto soldado (D38), fuentes de una línea (D41), texto variable / serie / CSV (D42/M38), alineación vertical y RTL (D37).
9. **Corte**: modo perforación (C13), lead-in/out y overcut (C15/C16), sub-capas (C30), prioridad y escala de potencia por objeto (C31), panel de Optimización (C32/M49), copiar/pegar ajustes y borrar capa (C06), potencia mínima en líneas (C08), velocidad en mm/s (C07).
10. **Máquina**: macros (M20), posicionar láser con clic (M17), overrides de velocidad/potencia en vivo (M18), Z/Focus en Mover (M16/M31), frame continuo y de casco (M11/M12), importar/exportar perfiles de dispositivo (M3), bundles (M4/D99).
11. **Print & Cut** (M37) y **tabs manuales** (D45): muy usados en piezas ya impresas y cortes finos.
12. **Nesting** (D76): hoy no existe pese al nombre del módulo.
13. **Selecciones especiales** (D21), grupos anidados (D30), menú contextual completo y guías arrastrables (D78).
14. **Rendimiento de la interfaz** con miles de objetos: medir, y si hace falta pasar el lienzo a WebGL o añadir un índice espacial.

### 3.3 Puede esperar

- **Galvo / fibra / EZCAD** (M30, M40-M45, C47, C59, M46): requiere hardware y documentación que no hay; EZCAD3/DLC y BSL ya están fuera de alcance.
- **Funciones P2 de edición**: warp (D58), casco convexo (D59), círculo tangente (D46), biblioteca de arte (D47), arreglo virtual (D67), copy along path (D69), Resize Slots (D71), Move Together/Dock (D74), trazado por lotes (D80), máscara de imagen (D81), imagen a bitmap (D82), rotación por dos puntos (D25), interruptores de transformación (D17), notas/plantillas/nueva ventana (D19), modo principiante (D18/C60), auto-agrupar (D31), imprimir (D98).
- **Importadores raros**: PLT/HPGL (D88), G-code (D89), formatos de máquina (D90/M22), exportar imagen (D95).
- **Control avanzado**: eje U y láser 2 (M32), feeder (M47), Center Finder (M48), cámara en el cabezal (M36), lista de archivos del controlador (M24), automatización UDP (M54), ajustes de controlador DSP (C59).
- **Corte avanzado**: dot mode (C18), ramp (C20), pausas de inicio/fin (C17), hide backlash (C34), flood fill (C28), incremento de ángulo (C50), modo `PassThrough` y relleno por forma en la app (C48, C27; ya existen en el motor, solo hay que cablearlos).
- **Tiling** (M39): ventaja competitiva, pero no bloquea la migración.
- **Historial con previsualización** (D14), calidad de bordes (D08), estimación calibrada con tiempos reales (C58), modelo de lente de ojo de pez (M33), arcos G2/G3 en el G-code (hoy aplana las curvas).

## 4. Lo que Tlaku tiene y LightBurn no (para la comunicación del proyecto)

Cada punto está verificado en el código. Donde no se pudo confirmar que LightBurn carezca de la función, se marca "(verificar)": conviene comprobarlo antes de presentarlo como ventaja, siguiendo la nota de `docs/ideas-mejoras.md`.

1. **Planificador de corte explicable.** Cada paso del orden de corte lleva la regla que lo decidió (capa, prioridad, interior antes que contenedor, solape, cercanía) y los conflictos se avisan en vez de pisarse en silencio; también incluye solapes parciales y 2-opt/Or-opt. Visible en la app (explicaciones por capa) y con `tlaku plan`. Responde a la queja más repetida del foro (`crates/tlaku-plan`).
2. **Emulador de controladoras y máquinas virtuales.** `tlaku-emu` imita GRBL 1.1, Falcon2 Pro, grblHAL, FluidNC, GRBL-LPC, Marlin y Smoothie; la app lo ofrece como puerto (`virtual:falcon2@5`) con fallas simuladas (llama, inclinación, ALARM:3, desconexión). Permite aprender, probar y reportar errores sin máquina. (Verificar: LightBurn no tiene un equivalente conocido.)
3. **Seguridad antes de mover la máquina.** El envío consulta el estado y se niega si hay Alarm/Hold/Door/Run; la app nunca envía `$X`, `$H` ni `~` por su cuenta; la consola pide confirmación y explica cada `ALARM:N` / `error:N`; el encuadre y el inicio avisan si el overscan sale de la cama *antes* de empezar (queja 02-Q9).
4. **Overscan por física y encuadre que lo incluye.** Se calcula con la aceleración real de la máquina (`v^2/2a`) en vez de un porcentaje, y el rectángulo del encuadre ya lo contiene.
5. **Importadores que avisan.** Nada se pierde en silencio: unidades DXF ausentes, SVG a 96 DPI frente a 72 DPI de Illustrator (con el tamaño alternativo), texto sin contorno, recorte `clip-path`/`mask` real, bloques DXF. (Verificar contra LightBurn.)
6. **Código abierto MIT con formatos abiertos y versionados.** Proyecto `.tlaku` (JSON con migraciones), biblioteca de materiales `.tlakumat` versionada que no se pierde (un archivo dañado se aparta y no se sobrescribe), contrato app-motor documentado (`docs/app-api.md`) y todo ejecutable desde el CLI `tlaku` (automatización y pruebas con archivos dorados).
7. **Dialectos de G-code y catálogo de máquinas.** GRBL, GRBL-M3, grblHAL, FluidNC, GRBL-LPC, Smoothie, Marlin y G-code personalizado con editor de dialecto; asistente "Agregar máquina" con detección de solo lectura y un catálogo de 21 modelos con fuentes; envío línea a línea para Marlin/Smoothie y aviso de posición perdida.
8. **Cámara con marcas grabadas y validación cruzada.** La precisión se mide (validación de dejar una marca fuera, RANSAC, corrección por altura de material medida a varias alturas), no se estima. (Verificar frente a la alineación de LightBurn.)
9. **Calibraciones con pruebas medibles.** Scanning offset como separación medida entre ida y vuelta con aviso si el overscan es menor, prueba de una vuelta del rotativo, prueba de materiales por cuadrícula con cualquier par de parámetros (velocidad, potencia, pasadas, intervalo).
10. **Interfaz que explica.** Ningún botón gris sin motivo (cada comando deshabilitado devuelve la razón), vista previa con reproducción en tiempo real del trabajo (con viajes, overscan, grabado y corte diferenciados) e historial con salto a cualquier punto.
11. **Velocidad del motor.** Medido en esta auditoría: 20 000 figuras a un G-code de 10 MB en 1,2 s con el planificador completo, sin colgarse (la queja 02-Q1 de LightBurn son cuelgues con diseños complejos).

**No comunicar todavía (aunque se mencionan en los documentos):**

- *Soporte de K40 (M2 Nano)*: está investigado en `docs/hardware/controladoras.md` como oportunidad (LightBurn no lo soporta), pero **no hay código**. Solo un K40 con la placa cambiada a GRBL-LPC/Smoothie funciona hoy.
- *Tiling*: planificado, no existe.
- *Ruida*: hay archivo `.rd`, sesión UDP y emulador en el motor, pero es experimental, sin hardware real y solo desde el CLI. LightBurn sí soporta Ruida, así que no es una ventaja.
- *Galvo*: ninguno.

