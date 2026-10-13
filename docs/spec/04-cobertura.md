# 04 - Cobertura funcional real de Tlaku frente al inventario de LightBurn

Auditoría hecha el 2026-10-13 sobre el código (último commit: `62a8841`, "Fase 4, oleada C"), no sobre los documentos, y **actualizada el mismo día tras la ola 1** (commit `736d472`, "Ola 1: base para publicar"): cada fila que tocó la ola se volvió a verificar en el código, no se tomó del mensaje del commit. Para cada una de las 214 funciones de los inventarios [01](01-diseno-edicion.md), [02](02-capas-corte-imagen.md) y [03](03-maquinas-avanzadas.md) se buscó el comando, el componente, la función del motor y sus pruebas. Los conteos del documento [00-resumen.md](00-resumen.md) se contrastaron con el código.

## Cómo leer este documento

- **Identificadores.** `D` = inventario 01 (diseño y edición, F01-F100), `C` = inventario 02 (capas, corte e imagen, F01-F60), `M` = inventario 03 (máquinas y avanzadas, filas 1-54). Se prefijan porque los inventarios 01 y 02 repiten la numeración `F01...`. El inventario 03 tiene 54 filas, no 53 como declara 00-resumen.md: la fila 21 mezcla prioridades (P0 para G-code, P1 para `.rd`) y 00-resumen no la cuenta. Aquí se cuenta como P0, así que el total real es 214 (68 P0, 86 P1, 60 P2) y no 213 (67/86/60).
- **Hecho**: funciona de punta a punta (motor más interfaz) y el flujo principal está completo. Las carencias secundarias se anotan como "Menor: ..." en la columna "Qué falta" y no cambian el estado. **Parcial**: falta una parte que un usuario notaría. **Solo motor/CLI**: existe en el motor o en `tlaku` pero la app no lo ofrece. **Falta**: no hay código. **No aplica**: decisión explícita de no hacerlo (ver nota abajo).
- **Evidencia.** `ui:` = `apps/desktop/src/`; `tauri:` = `apps/desktop/src-tauri/src/`; las rutas `crates/...` son del motor. Una evidencia "ninguna referencia" significa que se buscó por nombre, por comando y por palabra clave en `crates/` y en `apps/desktop/src` y no apareció nada.
- **Alcance de "Hecho".** Ninguna función de máquina está probada con hardware real (lo declara 00-resumen.md y sigue siendo cierto): "Hecho" significa que funciona contra el emulador (`tlaku-emu`) y las máquinas virtuales de la app, con pruebas automáticas (alrededor de 1340 `#[test]` en Rust, entre los crates y `src-tauri`, y 1430 casos en la interfaz; de estos, 10 de `i18n.test.ts` fallan hoy por el inglés a medio hacer, ajeno a la ola 1). La promesa de una prueba con la Falcon2 sigue pendiente.
- **"No aplica": ninguna.** No hay en el repositorio una decisión explícita de descartar una función del inventario. Lo más cercano es el alcance del galvo (EZCAD3/DLC y BSL fuera de alcance, `docs/hardware/galvo.md`), que afecta a M41. Candidatas razonables a marcar "No aplica" cuando se decida: D90 (formatos propietarios `.sc/.scpro2/.mage`), D94 (exportar AI), D99 (`.lbzip`; los `.lbprefs` ya los sustituye `.tlakuprefs`) y M54 (UDP de LightBurn).

## 1. Resumen

### Por prioridad

|  | Hecho | Parcial | Solo motor/CLI | Falta | No aplica | Total |
|---|---:|---:|---:|---:|---:|---:|
| P0 | 60 | 7 | 1 | 0 | 0 | 68 |
| P1 | 47 | 16 | 4 | 19 | 0 | 86 |
| P2 | 13 | 3 | 1 | 43 | 0 | 60 |
| Total | 120 | 26 | 6 | 62 | 0 | 214 |

Antes de la ola 1: 88 hechas, 31 parciales, 6 solo motor/CLI y 89 faltantes (P0: 53 / 13 / 1 / 1).

### Por área

|  | Hecho | Parcial | Solo motor/CLI | Falta | No aplica | Total |
|---|---:|---:|---:|---:|---:|---:|
| Diseño y edición | 69 | 6 | 0 | 25 | 0 | 100 |
| Capas, corte e imagen | 37 | 7 | 2 | 14 | 0 | 60 |
| Máquinas y avanzadas | 14 | 13 | 4 | 23 | 0 | 54 |

### Por área y prioridad

| Área | Prio | Hecho | Parcial | Solo motor/CLI | Falta | No aplica | Total |
|---|---|---:|---:|---:|---:|---:|---:|
| Diseño y edición | P0 | 32 | 0 | 0 | 0 | 0 | 32 |
| Diseño y edición | P1 | 28 | 4 | 0 | 7 | 0 | 39 |
| Diseño y edición | P2 | 9 | 2 | 0 | 18 | 0 | 29 |
| Capas, corte e imagen | P0 | 20 | 2 | 0 | 0 | 0 | 22 |
| Capas, corte e imagen | P1 | 13 | 5 | 2 | 3 | 0 | 23 |
| Capas, corte e imagen | P2 | 4 | 0 | 0 | 11 | 0 | 15 |
| Máquinas y avanzadas | P0 | 8 | 5 | 1 | 0 | 0 | 14 |
| Máquinas y avanzadas | P1 | 6 | 7 | 2 | 9 | 0 | 24 |
| Máquinas y avanzadas | P2 | 0 | 1 | 1 | 14 | 0 | 16 |

**Lectura rápida.** De 214 funciones, 120 están hechas (56 %), 26 parciales, 6 solo en el motor o el CLI y 62 faltan. En P0 (el MVP) hay 60 de 68 hechas (88 %); 7 parciales (C32, C51, M3, M5, M6, M11, M16), 1 solo motor/CLI (M15) y ninguna falta. Diseño y edición queda con sus 32 P0 completas. Lo que más falta ya no es el entorno de diseño sino las olas 2 a 5 del [plan](../plan-completo.md): ajustes de corte avanzados (perforación, lead-in, sub-capas, prioridad por objeto), diseño avanzado (recortar, separar, deformar, nesting), máquinas (Ruida en la app, K40, red, macros), Print & Cut y galvo. Sigue sin haber ninguna prueba con hardware real.

**Lo que la ola 1 dejó a medias o no coincide con su mensaje.**

- **Importar `.lbrn`/`.lbrn2` (D97):** la lista de extensiones de la interfaz no los incluía; corregido al cerrar la ola 1 (`ui:features/history/paths.ts`).
- **Las unidades en pulgadas** (mensaje: "unidades mm/pulgadas centralizadas") solo se aplican hoy en la herramienta Medir y en los campos de la ventana de Preferencias. La barra numérica, la barra de estado y los diálogos siguen en milímetros (aceptan `5in` al escribir). Por eso D12 queda en "Parcial".
- **Las formas paramétricas** (rectángulo, elipse, polígono, estrella) guardan sus parámetros en `DesignObject.shape` (`lib/paramShape.ts`) y se pueden editar al reabrir el proyecto.
- **Importar PDF y AI** trae solo trayectos: texto con fuentes, imágenes incrustadas, degradados, patrones y recortes se omiten con aviso. Un AI antiguo (PostScript) se rechaza.
- **El inglés** se completó al cerrar la ola 1 (commit `6ef7abc`): toda la interfaz, con cambio en vivo; faltan los mensajes del motor y el manual.
- Los enlaces del repositorio y de donaciones del menú Ayuda siguen en `null` (`ui:features/help/links.ts`): la app dice "enlace pendiente de publicar".

**Discrepancias con los documentos que siguen abiertas.**

- `docs/spec/00-resumen.md` anuncia que M3 "cubre los 67 P0". Con el código en la mano, 60 de los 68 P0 (contando la fila M21) están hechos; los 8 restantes: M15 cortar solo la selección (solo motor) y 7 parciales (C32, C51, M3, M5, M6, M11, M16).
- `tlaku-geom/src/nesting.rs` se llama "nesting", pero solo calcula la profundidad de contención; no hay empaquetado de piezas.
- El planificador de corte (`tlaku-plan`) soporta grupo y prioridad por objeto, pero `tlaku-pipeline` siempre pasa `priority: 0` y `group: None` (`build.rs:180-181`), así que esas dos palancas no se pueden usar.
- Funciones que existen en el motor pero no están cableadas: relleno por forma (`Grouping::PerShape`), `Sampling::PassThrough`, `enhance` de Adjust Image, bytes de override del streamer GRBL, `JobOptions.selection`, `remove_redundant_points`.

**Discrepancias resueltas por la ola 1:** el `README.md` de la raíz ya describe la aplicación y los 20 crates; la herramienta Medir ya mide (D44); voltear (D26) ya existe.

## 2. Tablas por área

### 2.1 Diseño y edición (inventario 01)

| ID | Función | Prio | Estado | Qué falta | Evidencia |
|---|---|---|---|---|---|
| D01 | Lienzo (Workspace) | P0 | Hecho | — | ui:features/canvas/CanvasView.tsx, canvas2d.ts; tema claro/oscuro (`view.theme`) |
| D02 | Paleta de colores / capas | P0 | Hecho | Menor: solo una capa de guías (equivale a T1); no hay T2. | ui:features/layers/ColorPalette.tsx, lib/palette.ts (`GUIDE_ENTRY`: nunca sale a la máquina) |
| D03 | Lista de capas (vista de diseño) | P0 | Hecho | Menor: sin "parpadeo" del objeto con clic derecho. | ui:features/layers/LayersPanel.tsx; comando `layer.selectObjects` |
| D04 | Barras y ventanas acoplables | P1 | Parcial | El panel derecho tiene pestañas y ancho ajustable, pero no se puede arrastrar, flotar, reordenar ni restaurar el diseño. | ui:features/shell/Dock.tsx, store/view.ts |
| D05 | Barra de estado | P1 | Hecho | Menor: no muestra dx/dy/longitud/ángulo de la herramienta activa. | ui:features/shell/StatusBar.tsx (cursor, zoom, selección y tamaño, máquina) |
| D06 | Barra Numeric Edits | P0 | Hecho | Menor: la preferencia de unidades (mm o pulgadas) todavía no llega a esta barra: los campos siguen en mm y aceptan la unidad al escribir (`5in`, `50%`). | ui:features/canvas/NumericBar.tsx, expr.ts, ExprField.tsx; lib/units.ts (`parseLength`) |
| D07 | Zoom y paneo | P0 | Hecho | Menor: sin "encuadrar selección" (hay ajustar a cama y al diseño). | ui:features/canvas/useViewportControls.ts, ZoomControls.tsx; `view.zoomIn/Out/fitBed/fitContent`, herramienta Mano (`H`) |
| D08 | Estilo de vista | P1 | Hecho | Menor: sin ajuste de calidad de bordes. | ui:commands/definitions/view.ts (`view.wireframe`, `W`) |
| D09 | Menú contextual | P1 | Hecho | Menor: el menú de fila de la lista de capas sigue siendo aparte; no hay entradas para copiar ajustes de capa ni convertir formas a trayectos (solo texto). | ui:features/contextmenu/ (model.ts, ContextMenu.tsx, contextmenu.test.tsx), montado en canvas/CanvasView.tsx (`ContextMenuHost`); captura 28-menu-contextual.png. Con objetos: editar texto, convertir a trayectos, vectorizar imagen, cortar/copiar/pegar, agrupar, bloquear, orden, voltear y girar, alinear, selecciones; sobre el fondo: pegar, seleccionar, ajustar la vista |
| D10 | Atajos por defecto | P0 | Hecho | Menor: el esquema es propio (V, N, L, R, E, G, T, H, M...) y no reproduce el de LightBurn; no hay un perfil "tipo LightBurn" listo. Los atajos ya se reasignan (D11). | ui:commands/definitions/*.ts, commands/shortcuts.ts; tabla generada en apps/desktop/README.md (`npm run readme`); diálogo `F1` |
| D11 | Editor de atajos | P1 | Hecho | Menor: el diálogo `F1` y el manual muestran siempre los atajos de fábrica, no los personalizados. | ui:features/settings/ShortcutsPanel.tsx, shortcutsModel.ts, commands/shortcuts.ts (`sanitizeOverrides`); buscar, cambiar, añadir otro, quitar, restaurar uno o todos, aviso de conflicto con "quitárselo", exportar e importar (settings.test.ts); Edición > Editar atajos de teclado… |
| D12 | Preferencias de edición | P1 | Parcial | Ventana Preferencias (`Ctrl/Cmd+,`) con idioma, unidades, pasos de flechas (normal, Mayús, Alt), paso de giro, distancia del imán, desplazamiento al duplicar, tema, rejilla, autoguardado y recientes. Falta que las pulgadas se vean fuera de Medir y de la propia ventana (barra numérica, barra de estado y diálogos siguen en mm), la opción de volver a Seleccionar tras dibujar y el contraste de la rejilla. | ui:features/settings/SettingsDialog.tsx, fields.tsx; store/prefs.ts (`PREF_SPECS`, 12 claves); lib/units.ts; captura 29-preferencias.png |
| D13 | Deshacer / rehacer | P0 | Hecho | — | ui:store/document.ts (historial con etiquetas y fusión de ráfagas), store/history.test.ts |
| D14 | Historial de deshacer | P2 | Hecho | Menor: salta a un punto del historial, pero no lo previsualiza antes de saltar. | ui:features/history/HistoryPanel.tsx, timeline.ts; `jumpTo` |
| D15 | Portapapeles básico | P0 | Hecho | Menor: pega desplazado (`duplicateOffset`, 5 mm por omisión y configurable), no centrado en el cursor. | ui:commands/definitions/edit.ts (copiar, cortar, pegar, pegar en el mismo lugar, duplicar, eliminar) |
| D16 | Portapapeles entre apps | P1 | Hecho | Menor: solo viaja SVG o texto plano (no imágenes ni los formatos propios de Illustrator o Corel); depende de que el sistema deje usar `navigator.clipboard` y, si no, sigue con el portapapeles interno. | ui:features/canvas/systemClipboard.ts, svgClip.ts (svgClip.test.ts); `edit.copy`/`edit.cut` también escriben SVG, `edit.copySvg` (`Ctrl/Cmd+Alt+C`) avisa; `edit.paste` lee el SVG ajeno o un texto |
| D17 | Interruptores de transformación | P2 | Hecho | Menor: no hay sesgar (Skew) y por eso tampoco su interruptor. | ui:commands/definitions/arrange.ts (`arrange.xform.move\|scale\|rotate\|center\|ratio`), store `transformAllow`; menú Organizar |
| D18 | Modo principiante | P2 | Falta | No hay modo principiante. | ninguna referencia en ui: |
| D19 | Nueva ventana, notas, plantillas | P2 | Falta | Sin segunda ventana, notas de proyecto ni plantillas. | ninguna referencia en ui: |
| D20 | Selección | P0 | Hecho | Menor: sin Tab para ciclar objetos. | ui:features/canvas/hit.ts, useCanvasTools.ts (caja de izquierda a derecha = contenido, de derecha a izquierda = cruzada; Mayús suma/quita; Esc) |
| D21 | Selecciones especiales | P1 | Hecho | — | ui:commands/definitions/edit.ts (`edit.selectInvert`, `selectOpen`, `selectOpenFill`, `selectLayer`, `selectContained`, `selectSmaller`, `selectCircles`), features/arrange/special.ts, SizeSelectDialog.tsx (`arrange.circles.*`) |
| D22 | Manejadores de transformación | P0 | Hecho | Menor: no hay sesgar (skew). Escalar y rotar con manijas, Mayús/Alt y Esc sí. | ui:features/canvas/transform.ts, handles.ts, useCanvasTools.ts |
| D23 | Nudge con flechas | P0 | Hecho | Menor: pasos de 1 / 10 / 0,1 mm por omisión (LightBurn usa 5 mm); ya son configurables en Preferencias. | ui:commands/definitions/edit.ts (`edit.nudge.*`); store/prefs.ts (`nudgeStep`, `nudgeStepBig`, `nudgeStepFine`) |
| D24 | Rotación rápida por teclado | P0 | Hecho | Menor: sin teclas de 10 o 45 grados; `.` y `,` siguen ocupadas por la velocidad de la vista previa. | ui:commands/definitions/arrange.ts (`arrange.rotate.*`: `[` y `]` 90 grados, `Mayús+[`/`]` el paso configurable, 15 por omisión, `Alt+[`/`]` 5 grados); store `rotateStep`; transform-commands.test.ts |
| D25 | Rotación/escala por dos puntos | P2 | Hecho | — | ui:`arrange.twoPoint` (`Alt+T`), features/canvas/pickTool.ts (pivote, punto de referencia y destino; `Mayús` también escala), lib/geometry.ts; tools.test.ts |
| D26 | Voltear H/V | P0 | Hecho | Menor: `Ctrl/Cmd+Mayús+V` (el de LightBurn) ya es "pegar en el mismo lugar": el vertical usa `Alt+V`. | ui:`arrange.flip.h` (`Alt+H` o `Ctrl/Cmd+Mayús+H`), `arrange.flip.v` (`Alt+V`), features/arrange/transformOps.ts (`flipMatrix`); transform-commands.test.ts |
| D27 | Espejo sobre línea | P1 | Hecho | Menor: crea una copia reflejada y deja el original; no hay opción de mover el original. | ui:`arrange.mirrorLine` (`Alt+Mayús+M`), pickTool.ts (modo `mirror`), lib/geometry.ts (`reflectAcrossLine`) |
| D28 | Bloquear formas | P1 | Hecho | Menor: lo bloqueado no se puede seleccionar con el ratón, así que solo se desbloquea todo de una vez; ocultar objetos sueltos sigue sin comando (`setObjectFlags` no tiene usos en la interfaz). | ui:`arrange.lock` (`Ctrl/Cmd+L`), `arrange.unlockAll` (`Ctrl/Cmd+Mayús+L`); mover, borrar, seleccionar y transformar respetan `locked`; la importación de LightBurn conserva los objetos bloqueados |
| D29 | Orden de dibujo | P1 | Hecho | — | ui:`arrange.order.front\|back\|forward\|backward` (`Ctrl+RePág`, `Ctrl+AvPág`, `RePág`, `AvPág`), features/arrange/order.ts (`reorderObjects`); menú contextual > Orden de dibujo |
| D30 | Agrupar / desagrupar | P0 | Hecho | Menor: un clic elige siempre el grupo de más afuera; para tocar un miembro hay que desagrupar un nivel (`Ctrl/Cmd+Mayús+G` quita un nivel cada vez). | ui:lib/groups.ts (cadena `"g1/g2"` dentro de `DesignObject.group`), commands/definitions/edit.ts (`edit.group`, `edit.ungroup`); groups.test.ts |
| D31 | Auto-group | P2 | Falta | Sin auto-agrupar. | ninguna referencia |
| D32 | Dibujar líneas / Bézier | P0 | Hecho | — | ui:features/canvas/pen.ts (pen.test.ts), useCanvasTools.ts (`downLine`: arrastrar al poner un nodo saca las manijas), tools.ts; `Mayús` fija 45 grados también en las manijas |
| D33 | Rectángulo y elipse | P0 | Hecho | Menor: sin radio negativo ni "desde el centro" con Ctrl. El rectángulo y la elipse son formas editables en la ventana Propiedades de forma mientras no se editen sus nodos; los parámetros se guardan en el documento. | ui:features/canvas/shapes.ts, lib/paramShape.ts, ShapeProps.tsx, NumericBar.tsx (`cornerRadius`) |
| D34 | Polígono, estrella y afines | P1 | Hecho | Menor: no hay triángulo, pentágono ni octágono como comandos aparte (se obtienen con 3, 5 u 8 lados) ni estrella de doble radio. Los parámetros viven en la sesión: al reabrir el proyecto la forma es un trayecto. | ui:commands/definitions/tools.ts (`tool.polygon` `G`, `tool.star` `Mayús+G`), features/canvas/shapeParams.ts, shapeParams.test.ts, lib/bezpath.ts (`regularPolygonPoints`, `starPoints`, `roundedPolygonBez`); 3 a 100 lados o puntas, radio interno en % y redondeo |
| D35 | Ventana Shape Properties | P1 | Parcial | Ventana "Propiedades de forma" para rectángulo, elipse, polígono y estrella, con los parámetros guardados en el documento, más prioridad de corte y escala de potencia por objeto (C31). Siguen faltando las propiedades de texto e imagen en la misma ventana. | ui:features/canvas/ShapeProps.tsx, lib/paramShape.ts (`editShape`), shapeUi.test.tsx, cutProps.test.tsx; `panel.shapeProps`; barra numérica y panel de texto |
| D36 | Crear/editar texto | P0 | Hecho | — | ui:features/text/*, canvas/textTool.ts; crates/tlaku-text (rustybuzz, kerning, ligaduras, varias líneas) |
| D37 | Opciones de texto | P0 | Hecho | Menor: sin alineación vertical, sin mayúsculas forzadas y sin RTL (árabe/hebreo). | ui:features/text/TextPanel.tsx; crates/tlaku-text/src/spec.rs (`TextSpec`) |
| D38 | Texto soldado (Welded) | P1 | Hecho | Menor: no aplica a las fuentes de una línea (no tienen contornos que unir). | ui:features/text/TextPanel.tsx (casilla "Soldar letras", campo `welded` de `TextObject`); crates/tlaku-text/src/effects.rs (`weld_paths`); outline.rs |
| D39 | Texto curvado (Bend) | P1 | Hecho | Menor: es un arco circular de hasta +-360 grados (no hay deformación libre tipo Distort) y se ignora mientras el texto va sobre un trayecto. | ui:features/text/TextPanel.tsx (campo "Curvatura", `bend_deg`); crates/tlaku-text/src/effects.rs (`bend_path`, `bend_radius`) |
| D40 | Aplicar path a texto | P1 | Hecho | Menor: sin alineación X/Y sobre el trayecto (hay desplazamiento y voltear lado). | ui:features/text/outlineSync.ts; crates/tlaku-text/src/path_sampler.rs; `text_outline` con `path` |
| D41 | Gestor de fuentes y SHX | P2 | Hecho | Menor: sin favoritas ni ocultar fuentes; el lector SHX no admite `bigfont` (CJK). | ui:features/text/FontPicker.tsx (sección "Fuentes de una línea"), fonts.ts, shx.ts (`tools.importShxFont`); crates/tlaku-text/src/linefont.rs (15 fuentes Hershey incluidas, `data/hershey/NOTICE.txt`), shx.rs (`shapes 1.0/1.1` y `unifont 1.0`); lineFonts.test.ts |
| D42 | Texto variable | P1 | Falta | Sin texto variable (fecha/hora, serie, CSV, valores de capa) ni "bake". | ninguna referencia; no hay dependencia `csv` en Cargo.lock |
| D43 | Códigos de barras y QR | P1 | Hecho | Menor: el código sale como trayecto ya generado: no se edita después ni es dato variable (D42/M38). Sin PDF417, Aztec ni GS1. | ui:features/codes/* (CodesDialog.tsx, codes.test.ts), commands/definitions/codes.ts (`tools.insertCode`, menú Herramientas > Insertar código…); tauri:edit/codes.rs (`generate_code`); crates/tlaku-codes (QR, Data Matrix, Code 128, Code 39, EAN-13, EAN-8, UPC-A, ITF, Codabar; 20 pruebas); captura 31-codigo-qr.png |
| D44 | Medir | P1 | Hecho | — | ui:features/canvas/measureTool.ts, measure.ts (measure.test.ts), MeasureReadout.tsx; `tool.measure` (`M`): distancia, ángulo y Δx/Δy entre dos puntos; un clic en una forma da nodos, trazos, tamaño, perímetro, área, diámetro |
| D45 | Tabs / puentes | P1 | Parcial | Puentes automáticos por capa line (cantidad o separación, omitir interiores, avisos frente al kerf, vista previa y G-code). Faltan puentes manuales arrastrables y potencia de puente. | crates/tlaku-geom/src/tabs.rs, tlaku-pipeline/src/tabs.rs; ui:features/layers/LayerEditor.tsx |
| D46 | Círculo tangente | P2 | Falta | Sin círculo tangente. | ninguna referencia |
| D47 | Biblioteca de arte | P2 | Falta | Sin biblioteca de arte. | ninguna referencia |
| D48 | Editar nodos | P0 | Hecho | Menor: faltan alinear, recortar y extender nodos. Hay insertar, borrar, línea/curva, suave/esquina/simétrico, romper, unir, cerrar, caja de nodos, X/Y exactas. | ui:features/canvas/nodeTool.ts, nodeedit.ts, nodeActions.ts, NodeBar.tsx |
| D49 | Trim Shapes | P1 | Falta | Sin Trim Shapes. | ninguna referencia |
| D50 | Convertir a path | P0 | Hecho | Texto a trayectos (`convertTextToPaths`). Rectángulo, elipse, polígono, estrella y códigos QR o de barras ya son trayectos desde que se crean. | ui:features/text/convert.ts |
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
| D70 | Igualar ancho/alto | P2 | Hecho | — | ui:`arrange.same.width\|height\|widthKeep\|heightKeep` en commands/definitions/arrange.ts, features/arrange/equalize.ts; edit-ops.test.ts |
| D71 | Resize Slots | P2 | Falta | Sin Resize Slots. | ninguna referencia |
| D72 | Alinear | P0 | Hecho | — | ui:features/arrange/align.ts, AlignDialog.tsx; `arrange.align.*`, `arrange.alignTo.selection\|object\|bed` |
| D73 | Distribuir | P1 | Hecho | — | ui:`arrange.distribute.h\|v` |
| D74 | Move Together y Dock | P2 | Falta | Sin Move Together ni Dock. | ninguna referencia |
| D75 | Mover selección a posición | P1 | Hecho | — | ui:`arrange.centerOnBed`, `arrange.alignTo.bed` + alinear a bordes de la cama |
| D76 | Nesting | P2 | Falta | Sin nesting. `tlaku-geom/nesting.rs` solo calcula la profundidad de anidamiento (contención), no empaqueta. | crates/tlaku-geom/src/nesting.rs:82 |
| D77 | Snapping | P0 | Hecho | Menor: no ajusta a líneas (perpendicular o tangente) ni tiene modo pegajoso; `Alt` solo lo suspende. | ui:features/canvas/snapPoints.ts (nodos, puntos medios, centros e intersecciones; snapPoints.test.ts), snap.ts, SnapBar.tsx; `view.snap*`; distancia en Preferencias (`snapPx`) |
| D78 | Guías | P2 | Parcial | Existe la capa de guías (nunca sale a la máquina) pero no se arrastran guías desde las reglas ni hay alineación temporal con Alt. | ui:lib/palette.ts (`GUIDE_ENTRY`), features/canvas/Ruler.tsx (solo regla) |
| D79 | Trace Image | P1 | Hecho | — | ui:features/trace/*; crates/tlaku-trace (vtracer, modo spline/polygon, umbral, manchas, desenfoque, vista previa); tauri:edit/trace.rs |
| D80 | Trace multiarchivo | P2 | Falta | Sin trazado por lotes. | ninguna referencia |
| D81 | Máscara / recorte de imagen | P2 | Falta | Sin máscara/recorte de imagen. | ninguna referencia |
| D82 | Imagen a mapa de bits y opciones de imagen | P2 | Falta | Sin convertir vectores a bitmap ni guardar bitmap procesado. | ninguna referencia |
| D83 | Importar SVG | P0 | Hecho | Menor: recorte `clip-path`/`mask` real, grupos y texto con fuentes instaladas (usvg). El DPI de un SVG sin unidades se elige en "Importar con opciones…" (96 por omisión; detecta Illustrator y sugiere el tamaño a 72 DPI). | crates/tlaku-svg/src/lib.rs:174-243; tlaku-pipeline/src/import.rs (`ImportExtras.dpi`); tauri `import_file`, `import_file_with` |
| D84 | Importar DXF | P0 | Hecho | Menor: sin opción de tolerancia de auto-cierre en el importador; el texto se ignora con aviso (como LightBurn). Detecta unidades ($INSUNITS), arcos, elipses, splines, bloques/INSERT, color ACI/true color; la unidad de un DXF sin unidades se elige en "Importar con opciones…". | crates/tlaku-dxf/src/lib.rs; tlaku-pipeline/src/import.rs (`ImportExtras.dxf_unit_mm`) |
| D85 | Importar AI | P1 | Hecho | Menor: solo vectores. Un AI antiguo (PostScript) se rechaza con un mensaje que dice cómo convertirlo. Ver D86. | crates/tlaku-pipeline/src/import_pdf.rs (`input_kind`: "ai" va al lector PDF); ui:features/history/paths.ts (`IMPORT_EXTENSIONS`) |
| D86 | Importar PDF | P1 | Hecho | Menor: solo se importan trayectos. Texto con fuentes, imágenes incrustadas, degradados, patrones y recortes se omiten con aviso. Se elige la página en las opciones de importación; una capa por color. | crates/tlaku-pipeline/src/import_pdf.rs (`lopdf` solo para leer la estructura; intérprete propio de `m l c v y h re`, `cm`, `q`/`Q`, `Do`); import_pdf tests |
| D87 | Importar imágenes | P0 | Hecho | Menor: TIFF y TGA se convierten a PNG para mostrarse en la ventana (`convert_to_png`). | crates/tlaku-raster/Cargo.toml:8 (`png`, `jpeg`, `bmp`, `webp`, `tiff`, `gif`, `tga`); tlaku-raster/src/gray.rs (`decode_hinted`); tlaku-pipeline/src/import.rs (`input_kind`); history.test.ts:193 |
| D88 | Importar PLT/HPGL | P2 | Hecho | Menor: supone 40 unidades por mm (0,025 mm) y lo avisa; el tamaño se corrige con la escala de las opciones de importación. Una capa por pluma. | crates/tlaku-pipeline/src/import_hpgl.rs (IN, PU, PD, PA, PR, SP, PC, IP, SC, CI, AA, AR, EA, ER, RA, RR, PE) |
| D89 | Importar G-code | P2 | Hecho | Menor: solo cuenta lo que quema (G1, G2, G3 con M3/M4 y S mayor que 0); no importa los viajes rápidos G0. | crates/tlaku-pipeline/src/import_gcode.rs (una capa por nivel de potencia, hasta 8, o una sola) |
| D90 | Otros formatos de máquina | P2 | Falta | Sin importar .rd, .sc, .nc, .mage... (el motor solo escribe .rd). | crates/tlaku-ruida/src/rd.rs es exportación |
| D91 | Opciones de importación | P0 | Hecho | Menor: sin opción de centrar al importar, de permitir capas T ni de tolerancia de cierre. `Ctrl/Cmd+I` y arrastrar siguen importando sin preguntar; el diálogo está en Archivo > Importar con opciones…. | ui:features/fileio/ImportOptionsDialog.tsx, actions.tsx (`importWithOptions`), types.ts (`ImportDialogOptions`); tauri:edit/files.rs (`import_file_with`); tlaku-pipeline `ImportExtras` (escala, DPI, unidad DXF, agrupar, capa destino, página PDF, G-code por potencia) |
| D92 | Exportar SVG | P1 | Hecho | — | ui:features/fileio/ExportDialog.tsx, `fileio.exportSvg`; crates/tlaku-export/src/svg.rs (mm reales, curvas Bézier, un grupo por capa, imágenes incrustadas o enlazadas, texto como trayectos); captura 30-exportar.png |
| D93 | Exportar DXF | P1 | Hecho | Menor: solo mm; la versión es AC1018 (2004) o AC1015 (2000). | ui:`fileio.exportDxf`; crates/tlaku-export/src/dxf_out.rs (LWPOLYLINE y SPLINE, una capa por color, color ACI o true color, tolerancia) |
| D94 | Exportar AI | P2 | Falta | Sin exportar AI. El PDF vectorial a escala 1:1 (D98) se abre en Illustrator. | ninguna referencia a AI en crates/tlaku-export |
| D95 | Exportar imágenes | P2 | Hecho | Menor: solo PNG (resolución en ppp, margen, fondo transparente); sin JPG ni BMP. | ui:`fileio.exportPng`; crates/tlaku-export/src/png_out.rs (tiny-skia, curvas exactas, tope de 30 000 px por lado y 100 millones de píxeles) |
| D96 | Proyecto nativo | P0 | Hecho | Menor: sin plantillas. Formato `.tlaku` (JSON versionado; migra la versión 1 sin pérdida), recientes (10 por omisión, configurable de 0 a 30, quita los que ya no existen), aviso de cambios sin guardar al abrir, crear nuevo y cerrar la ventana, y copia de recuperación automática (ver la tabla de necesidades). | ui:features/history/files.ts, unsaved.ts, recent.ts, windowEvents.ts; tauri `save_project`/`load_project`; docs/app-api.md |
| D97 | Importar `.lbrn`/`.lbrn2` | P1 | Hecho | Formas, grupos, texto, imágenes, capas y ajustes; lo que no tiene equivalente se avisa. Menor: colores de capa aproximados. | crates/tlaku-lbrn; tlaku-pipeline/src/import_lbrn.rs y import_lbrn/; ui:features/history/paths.ts (`lbrn`, `lbrn2`) |
| D98 | Imprimir / PDF | P2 | Hecho | Menor: Imprimir genera el PDF a escala 1:1 y lo abre en el visor del sistema; no hay diálogo de impresión propio. | ui:`fileio.print` (`Ctrl/Cmd+P`), `fileio.exportPdf`; features/fileio/actions.tsx (`printDesign`); tauri:edit/files.rs (`print_design`); crates/tlaku-export/src/pdf.rs (hojas A4, A3, Carta, Oficio o ajustada) |
| D99 | Preferencias y bundles | P2 | Parcial | Se exportan e importan las preferencias y los atajos en un `.tlakuprefs` (con aviso de claves y comandos desconocidos). Faltan los bundles con dispositivos, cámaras y bibliotecas (M4); los materiales siguen en su propio `.tlakumat`. | ui:features/settings/bundle.ts, files.ts, persist.ts; tauri:prefs.rs (`export_preferences`, `import_preferences`); settings.test.ts; features/materials/files.ts |
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
| C31 | Cut order priority (por objeto) y Power Scale | P1 | Parcial | Interfaz hecha: campos Prioridad de corte y Escala de potencia (varios objetos, vacío = automática/100 %), comandos Cortar antes/después y números sobre el lienzo (Ver). Falta que el pipeline las use: hoy pasa `priority: 0` y `group: None` al planificador. | ui:features/canvas/cutProps.ts, ShapeProps.tsx, PriorityBadges.tsx, cutProps.test.tsx; crates/tlaku-pipeline/src/build.rs:180-181 |
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
| M22 | Importar G-code / .rd / .plt / .nc / .hpgl / .sc | P2 | Parcial | G-code, `.nc`, `.plt` y `.hpgl` se importan como geometría (D88, D89). Faltan `.rd` y `.sc`. | crates/tlaku-pipeline/src/import_gcode.rs, import_hpgl.rs; `input_kind` (nc, gcode, gc, ngc, plt, hpgl, hpg) |
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
| M51 | Preferencias | P1 | Parcial | Ventana de Preferencias con 12 ajustes (D12). Faltan actualización automática, DPI por omisión, opciones de importación recordadas, cámara y registro de trabajos. | ui:features/settings/*, store/prefs.ts; tauri:prefs.rs (`preferences.json` en la carpeta de configuración) |
| M52 | Multilenguaje | P1 | Parcial | Español e inglés completos con cambio en vivo y detección del idioma del sistema; prueba de completitud. Faltan los mensajes del motor (Rust) y el manual en inglés. | ui:i18n/index.ts, i18n/en/, i18n.test.ts |
| M53 | Job checklist y protecciones | P1 | Parcial | Antes de enviar consulta el estado y no transmite si hay Alarm/Hold/Door/Run...; verifica la cama con overscan; diálogo de confirmación. No hay lista de comprobación configurable ni protección del botón Iniciar. | crates/tlaku-grbl/src/preflight.rs; tauri `start_job`; ui:features/machine/StartJobDialog.tsx |
| M54 | Automatización por UDP | P2 | Falta | Sin automatización por UDP. | ninguna referencia |

### 2.4 Lo que un usuario real necesita y el inventario no cubre bien

| Necesidad | Estado | Detalle y evidencia |
|---|---|---|
| Importar PDF | Hecho | Trayectos del contenido de página, una capa por color, con elección de página. Texto con fuentes, imágenes incrustadas, degradados y recortes se omiten con aviso (`crates/tlaku-pipeline/src/import_pdf.rs`, `lopdf` solo para leer la estructura). Antes: "formato no soportado". |
| Importar AI / EPS | Parcial | AI moderno (compatible con PDF) sí, por el mismo lector; un AI antiguo (PostScript) se rechaza con instrucciones para convertirlo. EPS sigue sin ninguna ruta (`input_kind` no lo reconoce). |
| Importar `.lbrn` / `.lbrn2` (migrar desde LightBurn) | Solo motor/CLI | El motor lee formas (rectángulo, elipse, trayecto, grupo, texto, imagen incrustada), capas y ajustes (`crates/tlaku-lbrn`, `tlaku-pipeline/src/import_lbrn*`; 9 archivos de prueba), pero **la app lo rechaza**: `.lbrn` y `.lbrn2` no están en `IMPORT_EXTENSIONS` (`ui:features/history/paths.ts:4`). Es el mayor freno para migrar y se arregla con dos extensiones y una prueba. |
| Exportar SVG / DXF | Hecho | Archivo > Exportar SVG… y Exportar DXF… con diálogo de opciones, todo el diseño o solo la selección (`crates/tlaku-export`, `ui:features/fileio/`). Antes quien dejaba Tlaku solo salía por G-code. También hay PNG y PDF (D95, D98). |
| Importar imágenes WebP, TIFF, GIF, TGA | Hecho | La crate `image` se compila con `png`, `jpeg`, `bmp`, `webp`, `tiff`, `gif` y `tga` (`crates/tlaku-raster/Cargo.toml:8`); el TGA se reconoce por la extensión porque no tiene firma. |
| Fuentes de una línea (SHX / Hershey) | Hecho | 15 fuentes Hershey incluidas y lector SHX (`shapes` y `unifont`) para fuentes que el usuario ya tenga (`crates/tlaku-text/src/linefont.rs`, `shx.rs`; `ui:features/text/FontPicker.tsx`, `shx.ts`). Salen como trazos abiertos para una capa de Línea. Sin `bigfont`. |
| Polígono, estrella | Hecho | Herramientas `G` y `Mayús+G` con ventana Propiedades de forma (D34, D35). |
| QR y códigos de barras | Hecho | `crates/tlaku-codes` (QR, Data Matrix, Code 128, Code 39, EAN-13, EAN-8, UPC-A, ITF, Codabar) y `ui:features/codes/`; Herramientas > Insertar código…. El resultado es un trayecto, no un código editable (D43). |
| Ventana de preferencias | Hecho | Edición > Preferencias… (`Ctrl/Cmd+,`), con 12 ajustes y archivos `.tlakuprefs` (D12, D99, M51). Pendientes: que las pulgadas lleguen a todos los campos y los ajustes de cámara, DPI y registro de trabajos. |
| Idioma inglés (y otros) | Parcial | Interfaz completa en español e inglés con cambio en vivo (commit `6ef7abc`). Los mensajes del motor (advertencias, errores, ayuda de `clap`) y el manual siguen en español. |
| Atajos configurables | Hecho | Editor en Preferencias > Atajos y en Edición > Editar atajos de teclado…; se guardan con las preferencias y viajan en el `.tlakuprefs` (D11). No hay un perfil "tipo LightBurn". |
| Autoguardado y recuperación ante cierre inesperado | Hecho | Una copia cada 60 s por omisión (10 a 3600 s) y solo si hay cambios, en `<datos de la app>/recuperacion/` (hasta 10 copias, 128 MB cada una); se borra al guardar o cerrar con normalidad, y al abrir tras un cierre forzado ofrece recuperar o descartar (`tauri:autosave.rs`, `ui:features/recovery/`, recovery.test.ts). |
| Archivos recientes | Hecho | Hasta 10 por omisión (0 a 30 en Preferencias), quita los que ya no existen y se puede vaciar la lista (`features/history/recent.ts`, menú Archivo). Se guardan en `localStorage` de la ventana, no en la carpeta de configuración. |
| Arrastrar y soltar archivos | Hecho | Hecho (`DropOverlay.tsx`), con avisos de formatos no soportados. |
| Rendimiento con diseños grandes | Parcial | El motor es rápido: medido en esta auditoría con `tlaku` en modo release, 20 000 círculos se planifican y se convierten en 10 MB de G-code en 1,2 s; 5 000 en 0,7 s. En la interfaz no hay mediciones: el lienzo es Canvas2D, con un mapa de bits en caché de toda la escena durante el zoom y el paneo (se repinta completo al cambiar el documento, `canvas2d.ts`), y el "hit-test" recorre linealmente todos los objetos (`hit.ts`, `pickObject`); no hay índice espacial ni pruebas de carga con miles de objetos. El ajuste magnético a nodos pone un tope por objeto (20 000 segmentos) para mantener ágil el arrastre. La arquitectura prevista en 00-resumen.md era un lienzo WebGL; el actual es 2D. La vista previa tiene una prueba de rendimiento del modelo (`preview/model.test.ts`). |
| Manual o ayuda | Hecho | Menú Ayuda con Manual de usuario (17 páginas con buscador, dentro de la app y en `docs/manual/`), Atajos de teclado, Novedades, Reportar un problema, Donar y Acerca de (licencia MIT y créditos de `THIRD_PARTY.md`) (`ui:features/help/`, help.test.tsx). Pendiente: los enlaces del repositorio y de donaciones (`links.ts` en `null`). |
| Instaladores multiplataforma | Hecho | Flujo de GitHub Actions para macOS (universal), Windows (NSIS y MSI) y Linux (AppImage y deb), sin firma (`docs/publicar.md`, `.github/workflows/release.yml`). Aún sin actualizador automático. |
| Prueba con hardware real | Falta | Ninguna. Todo se validó contra emuladores. Es el principal riesgo para publicar. |

## 3. Lo que falta para un primer lanzamiento público

Criterio: una primera versión pública (0.x, "beta") debe ser segura de usar sobre una máquina real, no perder el trabajo del usuario y no mostrar nada que no funcione. El resto de LightBurn puede llegar por versiones.

Las listas se escribieron antes de la ola 1; cada punto lleva ahora su estado (**hecho** con la fila de la sección 2, o **pendiente**).

### 3.1 Imprescindible antes de publicar

1. **Probar con hardware real** (la Falcon2 está disponible): conectar, encuadre, un corte y un grabado, pausa, detención y alarma. Sin esto, todo lo de máquina es teoría. Documentar el resultado y marcar en la app lo que sigue siendo experimental (Ruida). **Pendiente.**
2. **Autoguardado y recuperación** tras un cierre inesperado. **Hecho** (sección 2.4).
3. **Quitar o terminar la herramienta Medir** (D44). **Hecho.**
4. **Voltear horizontal/vertical** (D26, P0) y teclas de rotación rápida (D24). **Hecho.**
5. **Cortar solo la selección** (M15): el motor ya lo soporta (`JobOptions.selection`); falta una casilla en el diálogo de inicio y en el encuadre. Es P0. **Pendiente** (ola 2).
6. **Bloquear y orden de dibujo** (D28, D29). **Hecho**; ocultar objetos sueltos sigue sin comando.
7. **Exportar SVG y DXF** (D92, D93). **Hecho** (también PNG y PDF).
8. **Importar más imágenes** (D87) y **opciones de importación** (D91). **Hecho.**
9. **Preferencias mínimas** (D12, M51). **Hecho** salvo el idioma, "volver a Seleccionar" y las pulgadas en todos los campos.
10. **Ayuda mínima**: menú Ayuda, manual, Acerca de y `README.md` de la raíz. **Hecho**; faltan publicar los enlaces del repositorio y de donaciones.
11. **Menú contextual del lienzo** (D09) y **snapping a nodos, puntos medios e intersecciones** (D77). **Hecho.**
12. **Curvas dibujadas** (D32, P0). **Hecho.**
13. **Habilitar `.lbrn`/`.lbrn2` en la interfaz** (D97): el importador existe, la lista de extensiones lo rechaza. **Pendiente**, de una línea.

### 3.2 Importante para que la gente migre desde LightBurn

1. **Importar `.lbrn2` / `.lbrn`** (D97). Motor **hecho** (con referencia de estructura MIT citada en `THIRD_PARTY.md`); falta habilitarlo en la app (punto 13 de arriba).
2. **Importar PDF / AI** (D85, D86), **exportar PDF** (D98) y el portapapeles entre aplicaciones (D16). **Hecho.** Exportar AI (D94) sigue sin existir.
3. **Atajos configurables** (D11). **Hecho.** El perfil "tipo LightBurn" (D10: Ctrl+L, Ctrl+R, Ctrl+E...) **pendiente**.
4. **Idioma inglés** (M52): el público de LightBurn es en su mayoría anglófono. **Pendiente** (hay una base en el árbol).
5. **Ruida en la app** (M23, M50, M26 y M24): CO2 es la segunda familia más usada. Hoy es solo CLI y experimental. **Pendiente** (ola 4).
6. **Conexión por red para GRBL (TCP)** y para Ruida (UDP) en la app (M50). **Pendiente** (ola 4).
7. **Herramientas de forma y edición frecuentes.** **Hechas:** polígono y estrella (D34), QR y códigos de barras (D43), igualar tamaños (D70), espejo sobre línea (D27), Medir (D44). **Pendientes:** Trim (D49), Break Apart (D55), Fillet (D60), Cut Shapes (D65), Optimizar formas (D56), invertir dirección y punto de inicio (D57).
8. **Texto.** **Hechos:** texto soldado (D38), curvado (D39), fuentes de una línea (D41). **Pendientes:** texto variable / serie / CSV (D42/M38), alineación vertical y RTL (D37).
9. **Corte:** modo perforación (C13), lead-in/out y overcut (C15/C16), sub-capas (C30), prioridad y escala de potencia por objeto (C31), panel de Optimización (C32/M49), copiar/pegar ajustes y borrar capa (C06), potencia mínima en líneas (C08), velocidad en mm/s (C07). **Pendiente** (ola 2).
10. **Máquina:** macros (M20), posicionar láser con clic (M17), overrides de velocidad/potencia en vivo (M18), Z/Focus en Mover (M16/M31), frame continuo y de casco (M11/M12), importar/exportar perfiles de dispositivo (M3), bundles de dispositivos, cámaras y bibliotecas (M4). **Pendiente** (olas 2 y 4); las preferencias y los atajos ya viajan en `.tlakuprefs` (D99).
11. **Print & Cut** (M37) y **tabs manuales** (D45). **Pendiente.**
12. **Nesting** (D76): hoy no existe pese al nombre del módulo. **Pendiente.**
13. **Selecciones especiales** (D21) y **grupos anidados** (D30): **hechos.** Menú contextual (D09): **hecho.** **Guías arrastrables** (D78): pendiente.
14. **Rendimiento de la interfaz** con miles de objetos: medir, y si hace falta pasar el lienzo a WebGL o añadir un índice espacial. **Pendiente** (ola 6).

### 3.3 Puede esperar

- **Galvo / fibra / EZCAD** (M30, M40-M45, C47, C59, M46): requiere hardware y documentación que no hay; EZCAD3/DLC y BSL ya están fuera de alcance.
- **Funciones P2 de edición:** warp (D58), casco convexo (D59), círculo tangente (D46), biblioteca de arte (D47), arreglo virtual (D67), copy along path (D69), Resize Slots (D71), Move Together/Dock (D74), trazado por lotes (D80), máscara de imagen (D81), imagen a bitmap (D82), notas/plantillas/nueva ventana (D19), modo principiante (D18/C60), auto-agrupar (D31). Ya hechas en la ola 1: interruptores de transformación (D17), rotación por dos puntos (D25), imprimir (D98).
- **Importadores raros:** formatos propietarios de máquina (D90, y `.rd`/`.sc` de M22), exportar AI (D94). Ya hechos: PLT/HPGL (D88), G-code (D89), exportar imagen (D95).
- **Control avanzado:** eje U y láser 2 (M32), feeder (M47), Center Finder (M48), cámara en el cabezal (M36), lista de archivos del controlador (M24), automatización UDP (M54), ajustes de controlador DSP (C59).
- **Corte avanzado:** dot mode (C18), ramp (C20), pausas de inicio/fin (C17), hide backlash (C34), flood fill (C28), incremento de ángulo (C50), modo `PassThrough` y relleno por forma en la app (C48, C27; ya existen en el motor, solo hay que cablearlos).
- **Tiling** (M39): ventaja competitiva, pero no bloquea la migración.
- **Historial con previsualización** (D14), calidad de bordes (D08), estimación calibrada con tiempos reales (C58), modelo de lente de ojo de pez (M33), arcos G2/G3 en el G-code (hoy aplana las curvas).

## 4. Lo que Tlaku tiene y LightBurn no (para la comunicación del proyecto)

Cada punto está verificado en el código. Donde no se pudo confirmar que LightBurn carezca de la función, se marca "(verificar)": conviene comprobarlo antes de presentarlo como ventaja, siguiendo la nota de `docs/ideas-mejoras.md`.

1. **Planificador de corte explicable.** Cada paso del orden de corte lleva la regla que lo decidió (capa, prioridad, interior antes que contenedor, solape, cercanía) y los conflictos se avisan en vez de pisarse en silencio; también incluye solapes parciales y 2-opt/Or-opt. Visible en la app (explicaciones por capa) y con `tlaku plan`. Responde a la queja más repetida del foro (`crates/tlaku-plan`).
2. **Emulador de controladoras y máquinas virtuales.** `tlaku-emu` imita GRBL 1.1, Falcon2 Pro, grblHAL, FluidNC, GRBL-LPC, Marlin y Smoothie; la app lo ofrece como puerto (`virtual:falcon2@5`) con fallas simuladas (llama, inclinación, ALARM:3, desconexión). Permite aprender, probar y reportar errores sin máquina. (Verificar: LightBurn no tiene un equivalente conocido.)
3. **Seguridad antes de mover la máquina.** El envío consulta el estado y se niega si hay Alarm/Hold/Door/Run; la app nunca envía `$X`, `$H` ni `~` por su cuenta; la consola pide confirmación y explica cada `ALARM:N` / `error:N`; el encuadre y el inicio avisan si el overscan sale de la cama *antes* de empezar (queja 02-Q9).
4. **Overscan por física y encuadre que lo incluye.** Se calcula con la aceleración real de la máquina (`v^2/2a`) en vez de un porcentaje, y el rectángulo del encuadre ya lo contiene.
5. **Importadores que avisan.** Nada se pierde en silencio: unidades DXF ausentes, SVG a 96 DPI frente a 72 DPI de Illustrator (con el tamaño alternativo), texto sin contorno, recorte `clip-path`/`mask` real, bloques DXF; y, desde la ola 1, lo que un PDF, un HPGL, un G-code o un proyecto de LightBurn trae y Tlaku no puede representar (ajustes de capa sin equivalente, formas omitidas con su cantidad, texto sin contorno). (Verificar contra LightBurn.)
6. **Código abierto MIT con formatos abiertos y versionados.** Proyecto `.tlaku` (JSON con migraciones), biblioteca de materiales `.tlakumat` versionada que no se pierde (un archivo dañado se aparta y no se sobrescribe), contrato app-motor documentado (`docs/app-api.md`) y todo ejecutable desde el CLI `tlaku` (automatización y pruebas con archivos dorados).
7. **Dialectos de G-code y catálogo de máquinas.** GRBL, GRBL-M3, grblHAL, FluidNC, GRBL-LPC, Smoothie, Marlin y G-code personalizado con editor de dialecto; asistente "Agregar máquina" con detección de solo lectura y un catálogo de 21 modelos con fuentes; envío línea a línea para Marlin/Smoothie y aviso de posición perdida.
8. **Cámara con marcas grabadas y validación cruzada.** La precisión se mide (validación de dejar una marca fuera, RANSAC, corrección por altura de material medida a varias alturas), no se estima. (Verificar frente a la alineación de LightBurn.)
9. **Calibraciones con pruebas medibles.** Scanning offset como separación medida entre ida y vuelta con aviso si el overscan es menor, prueba de una vuelta del rotativo, prueba de materiales por cuadrícula con cualquier par de parámetros (velocidad, potencia, pasadas, intervalo).
10. **Interfaz que explica.** Ningún botón gris sin motivo (cada comando deshabilitado devuelve la razón), vista previa con reproducción en tiempo real del trabajo (con viajes, overscan, grabado y corte diferenciados) e historial con salto a cualquier punto.
11. **Salir sin quedar atrapado.** Exportación a SVG con medidas reales y curvas, DXF con SPLINE exactos, PDF vectorial a escala 1:1 y PNG a la resolución que se elija, con solo la selección si se quiere (`crates/tlaku-export`). Fuentes de una línea Hershey incluidas, sin instalar nada (licencia y créditos en `THIRD_PARTY.md`).
12. **Velocidad del motor.** Medido en esta auditoría: 20 000 figuras a un G-code de 10 MB en 1,2 s con el planificador completo, sin colgarse (la queja 02-Q1 de LightBurn son cuelgues con diseños complejos).

**No comunicar todavía (aunque se mencionan en los documentos):**

- *Soporte de K40 (M2 Nano)*: está investigado en `docs/hardware/controladoras.md` como oportunidad (LightBurn no lo soporta), pero **no hay código**. Solo un K40 con la placa cambiada a GRBL-LPC/Smoothie funciona hoy.
- *Tiling*: planificado, no existe.
- *Ruida*: hay archivo `.rd`, sesión UDP y emulador en el motor, pero es experimental, sin hardware real y solo desde el CLI. LightBurn sí soporta Ruida, así que no es una ventaja.
- *Galvo*: ninguno.
- *Importar proyectos de LightBurn desde la app*: el motor lo hace, pero la interfaz todavía rechaza `.lbrn`/`.lbrn2` (D97). No presumirlo hasta habilitarlo.
- *Inglés*: la base existe pero no hay selector funcional.

