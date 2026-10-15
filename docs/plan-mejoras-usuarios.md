# Plan de mejoras pedidas por usuarios

Fecha del análisis: 2026-10-14. Documento de planificación; **no cambia código** ni cuenta en los totales de
[cobertura](spec/cobertura/README.md). Cubre las 13 mejoras aprobadas (la 9, varias máquinas a la vez, quedó
fuera por decisión del usuario).

## 0. Cómo leer este documento

**Estado** (por mejora): *Hecho* (existe completo, no se agenda), *Parcial* (existe y falta algo concreto),
*No existe* (hay piezas reutilizables pero no la función).

**Esfuerzo**: S = hasta una semana de trabajo de un agente o persona; M = de una a tres semanas;
L = más de tres semanas o varias olas. Son estimaciones gruesas, no compromisos.

**Demanda**: se midió en dos fuentes públicas de LightBurn, con los números tal como estaban el 2026-10-14:

- **Tablero de peticiones oficial** (`lightburn.fider.io`, con votos reales; cada usuario vota una vez). Se
  consultó su API pública de solo lectura.
- **Foro** (`forum.lightburnsoftware.com`, Discourse): vistas, "me gusta" y respuestas de cada hilo.

**Límite honesto de la evidencia:** Reddit (r/lasercutting, r/lightburn) **no se pudo consultar**: la
herramienta de búsqueda rechaza `reddit.com`, y el acceso directo devuelve una página de bloqueo o error 429.
Los grupos de Facebook tampoco son accesibles. Por eso **no hay ninguna cita de Reddit ni de grupos** en este
documento; donde digo "demanda" me refiero solo al tablero oficial y al foro. Los votos del tablero son de
usuarios de LightBurn (de pago), no una encuesta de usuarios de Tlaku.

**Contexto competitivo (lo que LightBurn ya hizo, para no pedir lo que ya no diferencia):**

- LightBurn 2.1 (2026-05): *Quick Nest* (nesting por cajas rectangulares), historial de deshacer con ventana,
  generador de círculos tangentes, cámaras de red y **Cuttle integrado** (plantillas web de pago, tercero)
  — <https://forum.lightburnsoftware.com/t/lightburn-2-1-quick-nest-enhanced-camera-support-undo-history-and-more/189743>.
- LightBurn 2.2 en acceso anticipado (2026-09): *True Nest* por contorno real (**solo Pro**, de pago), *Multi
  Page Projects* (páginas dentro de un proyecto), importación de modelos 3D y una **API REST local**
  — <https://forum.lightburnsoftware.com/t/lightburn-2-2-00-early-access-true-nesting-center-line-trace-multi-page-projects-more/192504>.
- LightBurn **no tiene** (confirmado por su propio personal en 2023, sigue en el tablero): cola de trabajos,
  generador de bisagras, de cajas ni de engranajes, calculadora de costos, scripts internos ni historial de
  trabajos.

## 1. Pestañas / varios diseños en una ventana

**(a) Estado: Parcial.** Existe «Nueva ventana» (`Ctrl/Cmd+Mayús+N`): fila D19 de
[diseno.md](spec/cobertura/diseno.md) (Hecho, probada con ventanas reales en `npm run smoke:tauri`). Cada
ventana es una interfaz web aparte con documento, historial y selección propios; comparten la máquina (el
estado vive en `AppState`), las preferencias (`preferences.json` más el evento `storage`) y la recuperación
(`autosave.rs`, `LiveCopies`: cada copia pertenece a una ventana). Código: `src-tauri/src/window.rs`
(`new_window`, etiqueta `doc-N`, capacidad `doc-*`), `features/shell/templates.ts`, `commands/definitions/designAdvanced.ts`.

**(b) Qué falta.** No hay barra de pestañas, ni lista de documentos abiertos, ni menú «Ventana», ni forma de
arrastrar objetos de un documento a otro (queda copiar y pegar SVG, D16). Con dos ventanas el usuario las
gestiona con el administrador del sistema operativo. Límite documentado en D19: el registro de copias vivas es
de un proceso, y dos instancias de la app no se ven.

**(c) Viabilidad.**

- *Pestañas verdaderas dentro de una ventana* exigen que el estado del documento deje de ser un único
  `useStore` (`store/document.ts` guarda `doc`, `past`, `future`; `selection.ts`, `view.ts`, `preview.ts` son
  rebanadas globales). Camino realista: un "cajón" por pestaña (documento, historial, selección, vista, ruta
  del archivo, marca de cambios) que se intercambia al cambiar de pestaña, con el motor sin cambios (el
  documento ya viaja en cada llamada). El historial guarda documentos completos (`HISTORY_POINT_BUDGET`), así
  que N pestañas multiplican la memoria: hay que repartir el presupuesto entre pestañas.
- Lo que hay que atar: un trabajo en curso debe seguir ligado a su pestaña (el registro de trabajos y la vista
  en vivo guardan el documento de origen); la recuperación automática pasa de "por ventana" a "por pestaña";
  los comandos de automatización UDP (`LOAD`) y del celular apuntan a la pestaña activa.
- *Alternativa barata:* una barra de "documentos abiertos" que lista y enfoca las ventanas `doc-N` (sin
  pestañas reales). Tauri 2 ofrece `tabbing_identifier` solo en macOS; en Windows y Linux (WebKitGTK) no hay
  pestañas nativas de ventana.
- *Coordinación:* otro agente está editando la vista en vivo (`features/live/`) y el shell; las pestañas tocan
  ambos, así que conviene agendarlas cuando ese trabajo esté integrado.
- Rendimiento: cada ventana web adicional es un WebView más con su propio proceso y memoria (no medido
  aquí); las pestañas en un solo WebView evitan ese costo.

**(d) Demanda: alta.** Tablero: «Window tabs» 33 votos (2025) <https://lightburn.fider.io/posts/4118/window-tabs>;
«Project Tabs» 24 votos (2019) <https://lightburn.fider.io/posts/639/project-tabs>; «abrir varios archivos o
pestañas» con estado *started* (2026-08) <https://lightburn.fider.io/posts/5027/i-would-like-to-be-able-to-open-multiple-files-or-tabs-simultaneously-currently>.
Foro: «Two or more windows» 8 486 vistas <https://forum.lightburnsoftware.com/t/two-or-more-windows/9774>.
LightBurn respondió con *Pages* dentro de un proyecto, no con pestañas de archivos distintos.

**(e) Esfuerzo:** L para pestañas reales (decidido); S para la barra de documentos sobre las ventanas.

**(f) Cómo se prueba sin hardware.** Pruebas de vitest sobre el cajón (cambiar de pestaña conserva
documento, historial, selección y cambios sin guardar); jsdom con axe para la barra de pestañas (accesibilidad
con teclado); `smoke:tauri` para el caso de varias ventanas; prueba de memoria con `canvas-stress.mjs` con
varias pestañas; con la máquina virtual `virtual:falcon2` se comprueba que un trabajo de la pestaña A no se
mezcla con la B.

---

## 2. Generadores de piezas (cajas, bisagras, engranajes, bandejas, uniones)

**(a) Estado: No existe.** Lo más cercano: 5 SVG fijos de paneles de caja con encastre en la biblioteca de arte
(`features/art/svg/cajas-*.svg`, D47; no son paramétricos). Hay piezas reutilizables:

- Kerf por capa (C12): `crates/tlaku-geom/src/kerf.rs` (`kerf_compensate`, convención explícita de signo).
- Ajustar ranuras al grosor del material (D71): `tlaku-geom/src/slots.rs`.
- Puentes (D45) y formas paramétricas guardadas en el documento: `ShapeParams` en `tlaku-doc/src/cut.rs`
  (rectángulo, elipse, polígono, estrella) con el patrón de «Propiedades de forma» (D35): los parámetros viven
  en `DesignObject.shape` y se pierden solo si se editan los nodos.
- Prueba de materiales (C55) y de intervalo (C56): base para una pieza de prueba de encaje.

**(b) Qué falta.** Un crate de geometría (`tlaku-parts`, nombre propuesto) y un diálogo por generador:

1. **Cajas con dientes (finger joints):** medidas interiores o exteriores, grosor, ancho de diente objetivo,
   tapa abierta o cerrada, tapa de levantar, divisores con media ranura, patas; salida: un objeto por panel,
   en un grupo, más la opción de acomodarlos con el nesting existente.
2. **Bisagras flexibles (living hinge):** patrones recta escalonada, ladrillo/«lattice», rombo, onda;
   parámetros: ancho y alto de la zona, largo y separación de los cortes, escalonado, esquinas.
3. **Engranajes involutos:** módulo (o paso diametral), número de dientes, ángulo de presión, holgura
   (backlash), agujero y chavetero; aviso por socavado cuando hay pocos dientes; pares con distancia entre centros.
4. **Bandejas y cajones:** variante de 1 con lados bajos, esquinas con dientes y fondo.
5. **Cola de milano y pestañas/ranuras (tabs & slots) generales:** uniones planas entre dos bordes
   cualesquiera, con holgura de ajuste.

**Regla de diseño para que sean "objetos editables":** cada pieza sale como un `DesignObject` vectorial normal
(nodos editables, capa, grupo) y los parámetros se guardan en un registro de generador en el documento
(mismo patrón que `ShapeParams`), con «Regenerar» que reemplaza el grupo. Si el usuario edita nodos, el vínculo
se rompe (como ya pasa con el rectángulo). Los textos de la interfaz van en `es` y `en`; los parámetros en mm.

**Kerf y encaje.** No duplicar la compensación: el generador entrega la geometría **nominal** (dientes del
grosor exacto) y un parámetro **«holgura de encaje»** que ensancha o estrecha solo las ranuras (positivo =
más flojo). El kerf del láser lo compensa la capa con el offset de kerf que ya existe (C12), que trata los
contornos exteriores y los agujeros con la regla par-impar; el generador debe avisar «deja la holgura en 0 si
ya usas kerf de capa, o usa uno solo». Se agrega una **pieza de prueba de encaje** (varios pares de diente y
ranura con holguras de −0,10 a +0,10 mm), igual que la prueba de materiales, para que el usuario elija su valor
en su propio material. Se avisa si el grosor pedido difiere del grosor de la receta de material elegida.

### 2.1 Proyectos revisados (licencia leída en el archivo del repositorio)

Política: copiar solo MIT, Apache-2.0, BSD, ISC o Zlib con crédito en `THIRD_PARTY.md`; GPL/LGPL/AGPL solo se
lee; propietarios solo como referencia de funciones.

| Proyecto | Licencia (verificada) | Qué es | Veredicto |
|---|---|---|---|
| [boxes.py](https://github.com/florianfesti/boxes) (florianfesti; el sitio festi.info es una instancia alojada de este mismo código) | **GPL-3.0**: `LICENSE.txt` y encabezado de `boxes/edges.py` ("version 3 ... or any later") | Python; el generador más completo (cajas, bisagras flexibles, cientos de piezas) | Solo lectura. No copiar ni traducir línea por línea |
| [TabbedBoxMaker](https://github.com/paulh-rnd/TabbedBoxMaker) | **GPL-2.0** (`LICENSE`) | Extensión de Inkscape; el origen de la mayoría de los "box makers" | Solo lectura |
| [buxtronix/living-hinge](https://github.com/buxtronix/living-hinge) | **GPL-3.0** (`LICENSE`) | Extensión de Inkscape de bisagras | Solo lectura |
| Living Hinge Creator (drphonon, inkscape.org) | GPLv3 según su ficha pública (no se leyó repositorio) | Extensión de Inkscape | Solo lectura |
| [bmsleight/lasercut](https://github.com/bmsleight/lasercut) (OpenSCAD) | **BSD-2-Clause** (`LICENSE`) | Módulo de OpenSCAD: piezas 2D con dientes que también generan el modelo 3D | Se podría adaptar con crédito, pero es código OpenSCAD pensado para 3D: aporta poco a un generador 2D |
| [wcchun1234/CutCase](https://github.com/wcchun1234/CutCase) | **MIT** (`LICENSE`, 2026, Sunny Wong) | Diseñador web de cajas con dientes, tapas y divisores; kerf y pieza de prueba de ajuste | Permitido, pero creado en 2026 con 0 estrellas y un solo autor: sin madurez ni garantías. Sirve como lista de casos (divisores, tapa de levantar) y de validaciones |
| [mylasertools-com/laser-joints](https://github.com/mylasertools-com/laser-joints) | **MIT** (`LICENSE`) | TypeScript: uniones, dientes con llave, cola de milano | Igual: 2 estrellas, de 2026 |
| [hermes98761234/laser-cut-box-generator](https://github.com/hermes98761234/laser-cut-box-generator) | **MIT** (`LICENSE`) | TypeScript: cajas y bisagras con compensación de kerf | Igual: 0 estrellas, de 2026-10 |
| [gearforge](https://github.com/1400130-collab/gearforge), [Gear-Gen](https://github.com/CodiPixelPL/Gear-Gen) | **MIT** (`LICENSE`) | Engranajes involutos a SVG/DXF (JS y Python) | Igual: 1-2 estrellas |
| crate [`for_the_love_of_gears`](https://crates.io/crates/for_the_love_of_gears) | MIT (metadatos de crates.io y GitHub; el archivo `LICENSE` de la rama principal respondió 404) | Geometría de dientes involutos en Rust, 65 descargas | Demasiado nueva; no vale una dependencia para ~150 líneas de matemática |
| crate `boxcut` | licencia "non-standard" en crates.io | SVG de cajas de cartón | No usar |
| MakerCase, Cuttle, Atomm (generador de bisagras web) | Propietarios o servicios web | — | Solo referencia de funciones (Cuttle está integrado en LightBurn 2.1) |
| jscad-gears | MIT (aviso en su código) | Engranajes para JSCAD (modelos 3D) | No aplica: no es 2D |

No existe una biblioteca permisiva **madura** (años de uso, varios mantenedores) para ninguno de los tres
generadores; las MIT que hay son de 2026, de un solo autor y con casi cero uso.

### 2.2 Recomendación concreta

**Implementación propia en Rust (crate nuevo `tlaku-parts`, MIT), con los proyectos GPL usados solo como
lectura de parámetros y casos límite**, y los MIT recientes (CutCase, laser-joints, gearforge) como contraste
de resultados, sin copiar código. Razones:

1. La geometría es conocida y corta: el involuto es una fórmula de libro (círculo base `rb = r·cos α`, curva
   `inv(θ)`; unas 150 líneas con raíz y socavado); los dientes de una caja son segmentos alternados con
   complemento A/B; los patrones de bisagra son rejillas de ranuras. Las ideas y las fórmulas no tienen
   derechos; el código sí.
2. Copiar de boxes.py (GPL-3) contaminaría el proyecto MIT. Leerlo para saber qué parámetros ofrece (muescas
   en esquinas, divisores, "burn") está permitido y es lo que la política prevé.
3. Las MIT de 2026 se pueden copiar con crédito, pero su calidad es incierta (sin uso real, sin pruebas
   conocidas, probablemente hechas con asistentes). Importar su código nos haría responsables de errores
   ajenos sin ahorrar trabajo; el contraste con ellas sí vale como prueba cruzada.
4. Resultado nativo: salida directa en `BezPath` o polilíneas de `tlaku-core`, con las pruebas del propio
   motor (booleanas de `tlaku-geom`) y objetos editables en el documento; una dependencia en JS no daría esto.
5. Si en el futuro se copia algo MIT, queda registrado en `THIRD_PARTY.md` con crédito.

### 2.3 Viabilidad y riesgos

- Computacional: trivial; todo son decenas de contornos. Riesgo real: **geometría que no encaja** por
  redondeo, esquinas en diente-contra-diente y la regla "número impar de segmentos"; por eso las pruebas con
  invariantes (abajo) y la pieza de prueba de encaje.
- Socavado en engranajes con menos de unos 17 dientes (ángulo 20°): avisar y ofrecer corrección por
  desplazamiento del perfil.
- Bisagras: el material falla si los cortes quedan muy juntos o llegan al borde; validar márgenes mínimos y
  avisar. La flexión real depende del material (no se promete un radio).
- Plataformas/WebView: ninguna limitación; el cálculo corre en el motor y no toca el WebView.

**(d) Demanda: alta.** Bisagras: «Living Hinge Creation» **83 votos** (2018, abierto)
<https://lightburn.fider.io/posts/105/living-hinge-creation>. Cajas: foro «Tabbed Box Maker Plugin» 9 484 vistas
<https://forum.lightburnsoftware.com/t/tabbed-box-maker-plugin/2848>, «Finger Joint Box» 3 559 vistas
<https://forum.lightburnsoftware.com/t/finger-joint-box/13492>, tablero «boxes.py integration» 14 votos
<https://lightburn.fider.io/posts/2072/boxes-py-integration> y «Box Generator» 8 votos
<https://lightburn.fider.io/posts/5137/box-generator>. Engranajes: «Gear Generator» 4 votos
<https://lightburn.fider.io/posts/3712/gear-generator> y foro «Making kinetic gear sets…» 3 108 vistas
<https://forum.lightburnsoftware.com/t/making-kinetic-gear-sets-is-loads-of-fun-gears-spur-planetary-design-apps-that-export-to-svg-or-dxf-drawings/109789>.
Dovetail y tabs/slots generales: sin evidencia propia (solo «Joint design», 10 votos
<https://lightburn.fider.io/posts/1221/joint-design>). LightBurn los resolvió, de momento, enviando al usuario
a Cuttle (de pago).

**(e) Esfuerzo:** L en total, por fases: 2a cajas con dientes + pieza de prueba de encaje (M), 2b bisagras
(S-M), 2c engranajes (S-M), 2d bandejas, cola de milano y uniones generales (S-M).

**(f) Cómo se prueba sin hardware.** Invariantes en pruebas de Rust: (1) los dos bordes que se unen son
complementarios (la intersección de las áreas de pieza A y pieza B es 0 y su unión cubre el lado completo);
(2) tras aplicar `kerf_compensate`, la ranura mide grosor + holgura ± tolerancia; (3) cada panel es un contorno
cerrado sin autointersecciones y cabe en el área pedida; (4) engranajes: dos del mismo módulo engranan con la
distancia entre centros esperada y el radio base cumple `rb = r·cos α`; (5) archivos dorados de SVG por
caso; (6) el resultado entra al preflight de la máquina virtual `virtual:falcon2` y da un tiempo estimado
plausible. **Con la Falcon2 Pro (la única máquina real):** cortar la pieza de prueba de encaje en madera de 3 mm
y una caja pequeña; ese es el único modo de validar los números de holgura.

---

## 3. Calculadora de costos / precio por pieza

**(a) Estado: No existe.** Búsqueda en `apps/desktop/src` y `i18n/es`: no hay costos, precios ni tarifas. Sí hay
los insumos: tiempo estimado total y por capa (`JobPreview.total_s`, `PreviewLayer.seconds`), tiempo con láser
encendido (se obtiene de `moves` y `move_times`: kind=1 con potencia mayor que 0), área de las piezas y la
cama, grosor y receta del material (biblioteca `.tlakumat`), cantidad de copias (arreglos, tiling, texto
variable) y los tiempos reales del registro de trabajos (ver mejora 4).

**(b) Qué falta.**

- Parámetros guardados (por perfil de máquina y por material): precio de la hoja y su tamaño (o precio por cm²),
  tarifa de la máquina por hora, consumo eléctrico de la máquina en W y precio del kWh, **desgaste** (costo del
  tubo o diodo ÷ horas de vida, aplicado solo al tiempo con láser encendido), mano de obra y preparación
  (diseño, ajuste, acabado, empaque), comisión de plataforma, impuesto y **margen** (% sobre costo o sobre
  precio).
- Material por **área usada** (suma de las piezas ÷ aprovechamiento del acomodo) o por **hoja completa**
  (cuántas hojas caben según el nesting).
- Cantidad: costo fijo (preparación) repartido entre las unidades y precio por unidad con descuento.
- Salida: desglose en pantalla, copia a texto, CSV y cotización imprimible; la fórmula visible, para que el
  usuario la ajuste (un vendedor quiere ver de dónde sale el número).
- Calibración con el historial: el estimador subestima ("falta calibrar con tiempos reales", C58;
  `ideas-mejoras.md` menciona 1,5x a 2x en imágenes). La calculadora debe mostrar un factor de corrección por
  máquina, calculado de `actualS ÷ estimatedS` del registro, y avisar «tiempo estimado sin calibrar».

**(c) Viabilidad.** Alta y barata: aritmética sobre datos que ya se calculan. Riesgos: (1) la precisión del
tiempo (por eso la calibración), (2) no prometer precios: es una herramienta de apoyo, con redondeos y
monedas configurables (formato de números con `Intl`, ya en `lib/platform.ts`); (3) el aprovechamiento solo es
real si se acomodó; sin acomodo, usar el área de la caja envolvente y decirlo. Sin dependencias externas ni
limitaciones de plataforma.

**(d) Demanda: media-alta.** Tablero: «Automatic cost calculation» 30 votos
<https://lightburn.fider.io/posts/768/automatic-cost-calculation>; «Add Simple Cost Estimator to Preview Window»
27 votos <https://lightburn.fider.io/posts/510/add-simple-cost-estimator-to-preview-window>. Foro: «Any experience
with laser cost estimation software?» (801 vistas; los vendedores usan hojas de cálculo propias)
<https://forum.lightburnsoftware.com/t/any-experience-with-laser-cost-estimation-software/24437>.

**(e) Esfuerzo:** S (calculadora con tarifas y desglose), M con precios por material, cotización exportable y
calibración con el registro.

**(f) Cómo se prueba.** Pruebas unitarias con entradas conocidas (casos a mano: una hoja, tiempo fijo, margen
del 30 %); pruebas con diseños del corpus y la máquina virtual para el tiempo; propiedad: el precio crece de
forma monótona con tiempo, área, margen y cantidad. **Con la Falcon2:** comparar el tiempo estimado con el real
de 3 o 4 trabajos para fijar el factor de calibración (el historial lo anota).

---

## 4. Cola de trabajos e historial de trabajos

**(a) Estado: Parcial (historial) y No existe (cola).**

- **Historial:** «Registro de trabajos» (M51, Ayuda > Registro de trabajos…): `features/help/jobLog.ts` +
  `JobLogDialog.tsx`; hasta 200 entradas con fecha, máquina, documento, tiempo estimado y real, resultado y
  mensaje, y marca de interrupción con punto de control para continuar. Vive en `localStorage` (`tlaku.jobLog`).
- **Repetir:** «Repetir con el alimentador» (M47, Parcial): `features/feeder/repeat.ts` y `jobFlow.ts` repiten
  el **mismo** trabajo de 2 a 99 veces con avance del material entre vueltas, solo para máquinas tipo GRBL.
  El tiling (M39) envía tramos en serie y los retoma tras un cierre. Continuar un trabajo interrumpido existe
  (punto de control en `interrupted-job/`).
- **Cola con varios archivos:** no existe.

**(b) Qué falta.**

- *Historial:* no guarda **los ajustes usados** (capas, velocidad, potencia, pasadas, receta de material), ni
  el tamaño, ni el número de copias; no se puede filtrar, buscar, exportar a CSV ni abrir el proyecto desde la
  fila. Está en `localStorage` del WebView (no sobrevive igual que `preferences.json`, que sí está en la
  carpeta de configuración) y no se comparte con el celular ni con los respaldos. Debe pasar a un archivo
  en la carpeta de datos con escritura atómica (`write_atomic`) y límite razonable.
- *Cola:* lista ordenada de elementos (archivo `.tlaku`, máquina, receta de material, copias); cada elemento se
  calcula con su propio preflight; **pausa programada entre elementos** para cambiar el material, con
  confirmación («Material cambiado, iniciar») y reencuadre opcional; estado visible; reordenar, saltar,
  quitar; retomar la cola tras un cierre.
- **Seguridad (innegociable):** la cola **nunca arranca sola un elemento que cambia de material o de archivo**;
  cada inicio pasa por el diálogo de inicio y la lista de comprobación (M53). El avance automático solo se
  permite para repeticiones del mismo trabajo con el mismo material (como `repeat.ts`, cuya primera vuelta sí
  se confirma). Un láser desatendido es un riesgo de incendio; esta decisión reduce la "cola" a una cola
  guiada, no a un modo desatendido.

**(c) Viabilidad.** La orquestación es de interfaz (como `repeat.ts`: `watchJobEnd`, `waitIdle`) sobre una
máquina que ya admite un trabajo a la vez. Dependencias: poder cargar y calcular un documento que no es el
abierto (el documento viaja en cada llamada al motor, así que es posible) y, si hay pestañas, definir de
qué pestaña sale cada elemento. La cola vive en la sesión; solo su definición se guarda en un archivo.

**(d) Demanda: media (historial alta).** «A history of jobs» **71 votos** (2018)
<https://lightburn.fider.io/posts/181/a-history-of-jobs>; «Put job in queue» 7 votos
<https://lightburn.fider.io/posts/691/put-job-in-queue>; foro «Job Queues, Multiple Pages, & Automation»: el
personal responde que LightBurn no tiene cola
<https://forum.lightburnsoftware.com/t/job-queues-multiple-pages-automation/99152>. El pedido de cola es pequeño
(1 a 7 votos en tres publicaciones); el de historial, grande.

**(e) Esfuerzo:** historial enriquecido S-M; cola guiada M.

**(f) Cómo se prueba.** Vitest sobre el modelo de la cola con una máquina simulada; pruebas de la app con
`virtual:falcon2@5` y el reloj simulado de `tlaku-emu` (`clock.rs`): dos elementos, pausa, confirmación,
cancelación a media cola, error de uno que no arrastra al siguiente; prueba de que ningún camino inicia sin
confirmación. El historial se prueba con trabajos virtuales y migración desde `localStorage`.

---

## 5. Biblioteca de materiales de la comunidad

**(a) Estado: Parcial.** Existe la biblioteca de materiales (C54, `.tlakumat` versionado, prueba de materiales
C55) y unos 134 ajustes de fábrica con fuente y confianza (`crates/tlaku-doc/data/materials-default.json`).
Existe el **flujo de aportes sin servidor:** el formulario `.github/ISSUE_TEMPLATE/material_settings.yml`,
el flujo `community.yml` (comenta un resumen y el aviso de seguridad, sin escribir nada), el script
`scripts/community/issue-to-preset.mjs` (una persona convierte el issue en `community/materials/<n>.json`),
el validador `tlaku-doc/src/community.rs` (`cargo test -p tlaku-doc community`: campos, rangos, clase de
máquina contra vatios, licencia CC0) y `build.rs`, que **compila los aportes dentro de la app** con la
etiqueta «Comunidad (sin verificar)». Menú Ayuda > «Compartir ajustes de material…» abre el formulario ya
lleno (`features/help/githubForms.ts`), sin enviar nada solo.

**(b) Qué falta.**

- Hoy `community/materials/` **no tiene ningún aporte** (solo los README): no hay nada que descargar.
- Un **navegador dentro de la app** (buscar por material, grosor, operación, clase de máquina; ver notas y
  foto; «Usar en mi capa») que se actualice **sin esperar una versión nueva** de la app.
- Un catálogo publicable: un JSON generado en CI a partir de `community/materials/` y publicado en GitHub
  Pages, con versión y firma.
- Señales de confianza: cuántas personas lo confirmaron y en qué máquinas.

**(c) Viabilidad y alojamiento sin servidor propio.**

| Opción | Cómo | Pros | Contras |
|---|---|---|---|
| **A. Recomendada: catálogo estático en GitHub Pages + revisión por PR** | CI genera `catalog/v1/index.json` desde `community/materials/*.json` y lo firma (minisign; `minisign-verify` ya está en el árbol por el actualizador); la app lo baja **desde Rust** (`reqwest` 0.13 ya está en `Cargo.lock` por el actualizador), lo guarda en caché en la carpeta de configuración y lo mezcla como «Comunidad» | Cero servidores; revisión humana ya definida; trazable (cada aporte es un issue y un PR); funciona sin conexión (la biblioteca integrada sigue allí) | Moderación manual (cuello de botella si crece); un aporte tarda lo que tarde el PR |
| B. Lo que ya hay (compilado en la app) | Sin cambios | Sin red | Solo se actualiza con cada versión |
| C. Servicio propio (base de datos, cuentas) | — | Votos y reportes en vivo | Rompe "sin servidor propio", datos personales, costos, moderación 24/7 |
| D. Hilos o Discussions de GitHub | Texto libre | Barato | No estructurado; no se puede validar ni importar |

- **CSP del WebView:** `connect-src 'self' ipc:` bloquea cualquier `fetch` externo desde la interfaz. Es una
  ventaja de seguridad: la red se hace solo desde Rust, con un comando de solo lectura y destino fijo; no hay
  que ampliar la CSP ni el alcance de `opener`.
- **Pages:** el flujo `pages.yml` solo publica si `PAGES_ENABLED = true` y ahora publica `site/` a mano; habría
  que añadir el paso del catálogo (y comprobar los límites de Pages: 1 GB y ancho de banda blando, de sobra para
  un JSON de cientos de KB).
- **Privacidad:** la descarga solo revela la IP a GitHub (igual que la búsqueda de actualizaciones). Nada de
  identificadores ni telemetría; la actualización del catálogo se activa a mano o con una preferencia
  explícita. Enviar un aporte sigue siendo abrir el formulario en el navegador con la cuenta del usuario.
- **Moderación:** mantener el paso humano (el README ya lo define: sentido de los valores, coherencia con la
  foto, nada inseguro). Añadir reglas automáticas en la CI: rangos por clase de máquina (ya en `community.rs`),
  duplicados casi iguales, materiales que no deben cortarse con láser (PVC, cuero con cromo...), y rechazo de
  potencias imposibles.
- **«Misma máquina»:** no se puede **demostrar** (los datos son autodeclarados). Lo posible: (1) clase de
  máquina y vatios coherentes (ya validado), (2) modelo normalizado contra el catálogo de máquinas de la app
  (21 modelos), (3) niveles de confianza: «sin verificar» → «confirmado por N personas» cuando N reportes
  independientes (issues/comentarios del formulario de confirmación) coinciden, (4) en la app, filtrar por
  «mi máquina» y avisar cuando el ajuste es de otra clase. No sustituye la prueba en desecho que ya exigen los
  textos de seguridad.
- **Riesgo de producto:** una biblioteca sin aportes no sirve. Precondición razonable: tener decenas de aportes
  antes de construir el navegador (o poblarlo con los de fábrica, que ya traen fuente).

**(d) Demanda: media (implícita).** No encontré una petición directa de "biblioteca online" en el tablero; lo
que hay es la costumbre de compartir bibliotecas a mano en el foro: «LightBurn material library for 80W MOPA
Fiber Laser» 4 352 vistas y 20 "me gusta"
<https://forum.lightburnsoftware.com/t/lightburn-material-library-for-80w-mopa-fiber-laser/160217>;
«Recommended speed and power» 7 331 vistas
<https://forum.lightburnsoftware.com/t/recommended-speed-and-power/6979>; y en el tablero «Choose from Material
Library within Cut/Layer Settings» 27 votos <https://lightburn.fider.io/posts/1064/choose-from-material-library-within-cut-layer-settings>
(sobre la biblioteca local, no la compartida).

**(e) Esfuerzo:** M (catálogo + firma + descarga + navegador + pruebas), más el proceso de moderación.

**(f) Cómo se prueba.** Pruebas del catálogo (esquema, firma válida y alterada, versión mayor desconocida,
archivo dañado) con un servidor HTTP local, como ya hacen las pruebas de cámara de red
(`edit/camera/tests/network.rs`); la mezcla con la biblioteca local sin pisar recetas propias; la CI valida
los JSON con `cargo test -p tlaku-doc community`. No necesita hardware; la utilidad real se ve cuando alguien
usa un ajuste en su láser (primero la Falcon2 con ajustes ya conocidos).

---

## 6. Scripts / plugins con JavaScript en sandbox

**(a) Estado: No existe.** Hay tres piezas cercanas: la **automatización UDP** (M54: `PING`, `LOAD`, `START`,
`STATUS`, `STOP`; `START` abre el diálogo y no arranca solo), el **control remoto por celular** (M56) y el
registro de comandos de la interfaz (`CommandDef`, `ALL_COMMANDS`). **La lista permitida de la ola 7 (IA) no
está implementada:** la ola 7 es solo el plan de `docs/plan-completo.md` (sección «Ola 7»); no hay código con
esa lista (búsqueda `allowlist` sin resultados en `apps/desktop/src`).

**(b) Qué falta.** Un crate `tlaku-script` con el motor, una **API de diseño** mínima y una interfaz:

- La API solo describe operaciones de diseño: crear formas, mover/escalar/rotar, duplicar, alinear, booleanas,
  ajustes de capa, leer el documento (solo lectura). **Sin** acceso a máquina, archivos, red, preferencias
  ni automatización (las mismas exclusiones del plan de la ola 7).
- Las operaciones no se aplican una a una: el script devuelve un **lote de operaciones** que la interfaz
  muestra en una vista previa y aplica con **un solo paso de deshacer**, igual que el plan para la IA. Así la
  lista permitida y la vista previa se escriben una vez para scripts y para IA.
- Límites: tiempo (interrupción por reloj), memoria, profundidad de recursión y tamaño del lote. Sin
  `eval`, `import` dinámico, `fetch`, temporizadores ni acceso a módulos del sistema.
- Editor sencillo con ejemplos; guardar scripts en el documento (opcional, apagado de fábrica) y en una
  biblioteca de scripts del usuario.

**(c) Viabilidad: motores y licencias (verificadas en crates.io y en los repositorios).**

| Motor | Licencia | Lenguaje de la implementación | Sandbox | Observaciones |
|---|---|---|---|---|
| **Boa** (`boa_engine` 0.22) | Unlicense OR MIT | Rust puro | Sin acceso a nada por omisión; límites de iteraciones y recursión configurables | Más lento que QuickJS, de sobra para scripts de diseño; cruza a ARM64 sin compilador de C |
| **rquickjs** 0.14 (envoltorio de QuickJS) | MIT | C (QuickJS) con envoltorio en Rust | Límite de memoria y gancho de interrupción propios de QuickJS | Rápido y pequeño; requiere compilar C en cada objetivo (macOS universal, Windows, ARM64) y el motor es C con `unsafe` |
| Worker aislado en el WebView | — | JS del propio WebView | Sin DOM, pero en el mismo proceso; `Worker.terminate()` corta, **sin límite de memoria** | La CSP actual (`script-src 'self'`) obliga a ampliarla para un Worker en `blob:` o archivo propio; el Worker comparte la red de la CSP (`connect-src ipc:`); conviene no abrir ninguna superficie hacia `invoke` |
| quickjs-emscripten (QuickJS en WASM) | MIT en el código; GitHub lo marca "NOASSERTION" | C → WASM | Memoria acotada por WASM, dentro de un Worker | Ensamblar un WASM firmado y su carga bajo la CSP es trabajo extra |

**Recomendación:** motor **en Rust, fuera del WebView**: empezar con **Boa** (memoria segura, sin C, simple en
CI) y pasar a `rquickjs` solo si la velocidad no alcanza. Razones: los límites duros (tiempo y memoria) se
imponen mejor del lado de Rust, el WebView sigue sin ejecutar código del usuario (la CSP no se toca) y el
script queda sin camino hacia el código de la máquina, que vive en el mismo proceso.

Riesgos: (1) un script puede producir diseños enormes: límite de objetos y puntos; (2) scripts compartidos
por terceros son código ajeno: nunca se ejecutan al abrir un documento, solo con acción explícita; (3) la API
es un contrato público: versionarla desde el principio; (4) depende del trabajo de la ola 7 (lista
permitida, vista previa, un solo deshacer) o de hacerlo antes y que la IA lo reutilice.

**(d) Demanda: alta en votos, pero de usuarios avanzados.** «Control lightburn from a script» **114 votos**
(2018) <https://lightburn.fider.io/posts/205/control-lightburn-from-a-script>; «Open Api or Interface for
external Script or Extension Programming» 79 votos <https://lightburn.fider.io/posts/1089/open-api-or-interface-for-external-script-or-extension-programming>;
«Incorporate a Plugin Architecture» 40 votos <https://lightburn.fider.io/posts/256/incorporate-a-plugin-architecture>.
LightBurn respondió en 2.2 con una **API REST local** (<https://github.com/LightBurnSoftware/LightBurn-REST>), es
decir, control externo, no scripts internos con sandbox; la propuesta de Tlaku es distinta (extender el
diseño desde dentro, sin tocar la máquina).

**(e) Esfuerzo:** L (motor, API, límites, editor, documentación, pruebas de seguridad).

**(f) Cómo se prueba.** Sin hardware por completo: pruebas de Rust del sandbox (bucle infinito se corta por
tiempo; asignación enorme se corta por memoria; los nombres `fetch`, `require`, `eval`, `Function(...)`,
`globalThis.process` no existen); prueba de que el lote devuelto solo contiene operaciones de la lista (y que
agregar un comando de máquina nuevo al registro **no** lo hace visible, como pide el plan de la ola 7); pruebas
de aplicación en un paso de deshacer; ejemplos de la documentación ejecutados como pruebas.

---

## 7. Acomodo (nesting) por forma real, giros y retazos

**(a) Estado: Hecho en lo principal; faltan hojas y retazos.** Corrección a la premisa: el acomodo "par-impar"
de `tlaku-geom` (`nesting`) es solo la regla de contención (piezas y huecos). El **nesting real es el crate
`tlaku-nest`** (D76, Hecho): polígonos de no-ajuste por suma de Minkowski de partes convexas (`ops.rs`,
`convex.rs`), ajuste interior del contenedor, colocación voraz por gravedad o abajo-izquierda, **recocido
simulado** con tope de tiempo (1 a 600 s) sobre orden y rotaciones, **giros** (ninguno, 0/90/180/270, cada
cierto ángulo o libre), espejo opcional, **piezas dentro de los huecos de otras**, contenedor de forma
cualquiera (un objeto cerrado hace de material, por lo tanto **un retazo irregular ya sirve**), progreso en
vivo, cancelar, determinismo por semilla y pruebas (`tests.rs`: separación, rotaciones, contenedor
irregular, hueco). Tope de 2000 copias. UI: `features/nest/`, comando `arrange.nest`. Es decir: **ya compite
con "True Nest" de LightBurn 2.2, que es solo Pro y de pago.**

**(b) Qué falta** (todo menor; la fila D76 lo reconoce: "no hay veta, orden de corte propio ni varias hojas"):

- **Varias hojas:** cuando no cabe todo, repartir en hoja 1, hoja 2... (hoy se reporta lo que no cabe).
- **Retazos como inventario:** guardar el contorno del retazo sobrante y ofrecerlo como contenedor del
  siguiente trabajo (con la mejora 3, el costo cuenta solo el material usado).
- **Veta (dirección de la fibra):** restringir giros por pieza.
- **Prioridad y agrupación:** piezas del mismo color/capa juntas para ordenar el corte.
- **Calidad medida:** no hay en el repositorio una comparación del aprovechamiento contra SVGnest, Deepnest o
  sparrow sobre instancias públicas.

**(c) Viabilidad: opciones permisivas y licencias (leídas en cada repositorio).**

| Proyecto | Licencia real | Veredicto |
|---|---|---|
| [SVGnest](https://github.com/Jack000/SVGnest) | **MIT** (`LICENSE.txt`, J. Qiao 2015); usa Clipper (Boost) | Permitido; JS; último cambio en 2024-02. El crate ya cita la idea (no se copió código) |
| [Deepnest original (Jack000)](https://github.com/Jack000/Deepnest) | **Sin archivo de licencia** en el repositorio | No usar: sin licencia no hay permiso |
| [deepnest-next](https://github.com/deepnest-next/deepnest) | `LICENSE` es MIT, pero **`LICENSES.md` declara `main/deepnest.js` como GPLv3** (el núcleo del acomodo) y partes Boost | **No copiar**; solo lectura. Aviso: "Deepnest es MIT" es engañoso |
| [jagua-rs](https://github.com/JeroenGar/jagua-rs) (Rust, motor de colisiones, publicado en INFORMS J. Comput.) | **MPL-2.0** (`LICENSE`) | Copyleft **por archivo**: usable como **dependencia sin modificar**; `deny.toml` ya permite `MPL-2.0`. No copiar sus archivos dentro de crates nuestros |
| [sparrow](https://github.com/JeroenGar/sparrow) (Rust, heurística del estado del arte para 2D irregular) | **MIT** (`LICENSE`); depende de jagua-rs (MPL-2.0) | Solo resuelve "strip packing" (una tira de alto fijo y largo variable): no cubre hojas rectangulares múltiples ni contenedores irregulares con huecos |

**Recomendación:** **no reemplazar** `tlaku-nest`. Primero medir (instancias públicas, p. ej. las de ESICUP) y
solo si el aprovechamiento queda claramente por debajo, evaluar `sparrow` + `jagua-rs` como **motor
alterno opcional** (dependencia de Cargo, sin copiar archivos; la MIT de sparrow y la MPL de jagua-rs no
afectan al código de Tlaku mientras no se modifiquen sus archivos). Mientras tanto, agregar hojas múltiples,
retazos y veta sobre el motor propio.

**(d) Demanda: alta, y ya cubierta.** «Auto-arrange objects on available material» **113 votos**
<https://lightburn.fider.io/posts/235/auto-arrange-objects-on-available-material>; «For the new nesting
feature, I would like to be able to nest irregular polygons» (retazos que no son rectángulos)
<https://lightburn.fider.io/posts/4931/for-the-new-nesting-feature-i-would-like-to-be-able-to-nest-irregular-polygons>;
foro «True Nesting in LightBurn» <https://forum.lightburnsoftware.com/t/true-nesting-in-lightburn/192508>.
LightBurn ya respondió (Pro de pago), así que lo que Tlaku ofrece gratis es diferencial.

**(e) Esfuerzo:** M para hojas múltiples + retazos + veta; L si se decide integrar `sparrow`.

**(f) Cómo se prueba.** Pruebas existentes más propiedades (nada se solapa, todo dentro del contenedor, la
separación se respeta, misma semilla = mismo resultado); banco de aprovechamiento sobre instancias públicas
y compararlo con valores publicados de SVGnest/sparrow; hojas múltiples con casos construidos a mano
(reparto exacto). Sin hardware; con la Falcon2, cortar un acomodo real en un retazo.

---

## 8. Vista previa realista (textura y tono según potencia)

**(a) Estado: Parcial.** La vista previa (C57, `features/preview/`, `draw.ts`) dibuja en Canvas2D los trazos
de corte, grabado, viajes y overscan, con reproducción en tiempo real, y modula el **trazo por nivel de
potencia** (`POWER_LEVELS`, `LEVEL_ALPHA`). Las imágenes se previsualizan ya procesadas y tramadas
(`tlaku-raster/preview.rs`, `preview_image`). La fila C57 lo dice: "no hay sombreado continuo por potencia".

**(b) Qué falta.** Simular el aspecto final: color del material (de la receta: madera clara, acrílico negro,
cuero), tono del quemado según potencia y velocidad (curva calibrable por receta), anchura real del haz
(kerf) y del grabado (intervalo), y textura opcional (veta, grano). Útil para mostrar al cliente y para
elegir potencia antes de quemar.

**(c) Viabilidad: limitada y se debe acotar.**

- Es un **modelo**, no una predicción: el resultado real depende de lente, enfoque, aire, humedad y lote del
  material. Un "realismo" que promete lo que la máquina no da es peor que no tenerlo. Debe llamarse
  *simulación aproximada*, con la curva de tono calibrable con la prueba de materiales (mide el tono real de
  una cuadrícula con una foto o a ojo).
- Rendimiento: Canvas2D soporta el trazo con alfa por nivel; la textura y el mezclado por píxel piden WebGL
  o `OffscreenCanvas`. WebGL en WebKitGTK/Raspberry Pi es desigual; hay que mantener la vista simple como
  alternativa.
- Texturas: **sala limpia y licencias:** no usar fotografías de terceros; generar texturas procedimentales
  propias (ruido de veta) o CC0 con crédito. Peso de instalador pequeño.
- Rendimiento: 10 000 objetos ya son el límite medido en Canvas2D (`pruebas-estres.md`); el
  sombreado por píxel de una cama completa debe limitarse a la zona visible.

**Recomendación: reducir** a *vista tonal*: color de material por receta + tono de quemado por potencia/velocidad
(curva por receta) + grosor de línea a escala, en Canvas2D, sin texturas fotográficas. Las texturas
procedimentales quedan como extra opcional.

**(d) Demanda: baja.** «Method to set actual material end result color for the material library» 4 votos
<https://lightburn.fider.io/posts/2532/method-to-set-actual-material-end-result-color-for-the-material-library>;
«Patterns and/or Editable Color pallet…» 5 votos <https://lightburn.fider.io/posts/1437/patterns-and-or-editable-color-pallet-to-help-you-see-what-your-project-will-look-like>;
foro «Shade according to power preview bug?» 2 638 vistas
<https://forum.lightburnsoftware.com/t/shade-according-to-power-preview-bug/11058>. No hay evidencia de un
reclamo fuerte de "realismo"; lo más cercano es ver el tono por potencia (que LightBurn ya tiene).

**(e) Esfuerzo:** M (vista tonal con color de material y curva por receta); L si se añaden texturas y WebGL.

**(f) Cómo se prueba.** Pruebas unitarias de la curva potencia→tono (monótona, acotada, sin NaN);
capturas de pantalla comparadas (`apps/desktop/scripts/screenshots.mjs`); mediciones de tiempo de dibujo con
`canvas-stress.mjs`. **Con la Falcon2:** quemar la cuadrícula de la prueba de materiales, medir el tono real y
ajustar la curva por defecto para las recetas de diodo; sin esa prueba el realismo es una suposición.

---

## 10. Historial de versiones del proyecto

**(a) Estado: Parcial.** Hay deshacer con historial saltable (D13, D14: `HistoryPanel.tsx`; hasta un
presupuesto de puntos, `HISTORY_POINT_BUDGET`, por sesión), copia de recuperación automática
(`autosave.rs`: una copia por documento, con tope de número y tamaño, huérfanas tras cierre inesperado) y
escritura atómica de proyectos (`write_atomic`). **No hay versiones anteriores tras guardar:** guardar
sobrescribe `.tlaku` y el historial de deshacer muere al cerrar.

**(b) Qué falta.** Instantáneas al guardar (y opcionalmente por tiempo), en una carpeta de datos por
proyecto, con retención (las últimas N, más una por día durante M días, tope de tamaño), una lista con fecha y
resumen («+3 objetos, capa 2 cambió»), vista previa de miniatura, **restaurar** (reemplaza el documento
actual, como un paso deshacible) y **abrir como copia**. Comparación simple (cantidad de objetos y capas
cambiados).

**(c) Viabilidad.** Alta. El documento es JSON versionado; la mayoría de los proyectos pesa poco (las imágenes
se **referencian por ruta** y no viajan dentro, por lo que una instantánea no las protege si el archivo
cambia o se borra: hay que decirlo o guardar un hash). Riesgos: crecimiento del disco (tope y compresión),
proyectos sin ruta (recuperación ya los cubre), sincronización por nube (instantáneas en la carpeta de datos,
no junto al archivo), privacidad (contienen el diseño del cliente: borrarlas al borrar el proyecto es una
opción). Se apoya en `autosave.rs` y en `write_atomic`.

**(d) Demanda: baja-media.** «Version control» 12 votos (2018) <https://lightburn.fider.io/posts/449/version-control>;
«Save incremental» 11 votos <https://lightburn.fider.io/posts/2835/save-incremental>; «Settings History /
Timeline» 14 votos <https://lightburn.fider.io/posts/3493/settings-history-timeline>; «A Lossless Undo after
saving a project» 4 votos <https://lightburn.fider.io/posts/4042/a-lossless-undo-after-saving-a-project>.
Foro: «Lost a project, how to recover?» 4 977 vistas
<https://forum.lightburnsoftware.com/t/lost-a-project-how-to-recover/52774>. LightBurn 2.1 añadió una ventana
de historial de deshacer pero no versiones tras guardar.

**(e) Esfuerzo:** M.

**(f) Cómo se prueba.** Pruebas de Rust de la política de retención (con relojes simulados), de escritura
atómica y de límites; pruebas de la interfaz de restauración como un solo paso de deshacer; caso de proyecto
sin ruta; archivos grandes con `tlaku-bench`. No necesita hardware.

---

## 11. Linux ARM64 (Raspberry Pi) y paquetes Flatpak/Snap

**(a) Estado: No existe (solo x86_64).** `.github/workflows/release.yml` genera macOS (universal), Windows (NSIS
y MSI) y Linux (`.deb` y `.AppImage`) únicamente en `ubuntu-22.04` (x86_64); `ci.yml` corre en Linux/macOS/
Windows y la prueba de humo de la app real bajo Xvfb (`smoke-tauri.sh`). `tauri.conf.json` ya activa
`createUpdaterArtifacts` y los destinos `deb` y `appimage`.

**(b) Qué falta.**

- **ARM64 (aarch64):** una entrada más en la matriz del flujo con el runner `ubuntu-22.04-arm`
  (alojado por GitHub, gratis en repositorios públicos y **disponible de forma general desde 2025-08**:
  <https://github.blog/changelog/2025-08-07-arm64-hosted-runners-for-public-repositories-are-now-generally-available/>).
  Se instalan las mismas dependencias que en x86 (`libwebkit2gtk-4.1-dev`, `libudev-dev`, etc.). Compilar de
  forma nativa evita la limitación documentada por Tauri: **linuxdeploy no sabe hacer AppImage de ARM por
  compilación cruzada** (<https://v2.tauri.app/distribute/appimage/>); cruzar con QEMU sirve pero es muy lento.
- **Flatpak:** manifiesto, metadatos AppStream, fuentes de Cargo y npm generadas para compilar sin red
  (requisito de Flathub), entorno `org.gnome.Platform`, y permisos de dispositivo para el puerto serie y USB
  (el K40 y el galvo usan `nusb`) — guía oficial: <https://v2.tauri.app/distribute/flatpak/>.
- **Snap:** no se recomienda (ver (c)).

**(c) Viabilidad.**

- **ARM64 con `.deb` y AppImage: alta.** Las dependencias son Rust puro más `serialport` (con `libudev`) y
  `nusb` (Rust puro); Raspberry Pi OS 64 bits (Bookworm) trae `libwebkit2gtk-4.1` y una glibc (2.36) igual o
  más nueva que la de Ubuntu 22.04 (2.35) con la que se compila, así que el binario corre. La prueba de humo
  se repite en el mismo runner. Riesgos: (1) **Raspberry Pi 3 y anteriores o sistemas de 32 bits no están
  cubiertos** (solo aarch64: Pi 3B+, 4, 5 con SO de 64 bits); (2) **rendimiento del lienzo** en WebKitGTK sobre
  una Pi: Canvas2D con miles de objetos podría ir lento y WebKitGTK tiene problemas conocidos de composición
  por GPU en algunas Pi (hace falta una variable de entorno de respaldo; no lo he podido comprobar sin una Pi);
  (3) el actualizador de Tauri debe publicar la entrada `linux-aarch64` en `latest.json` (lo hace
  `tauri-action`).
- **Flatpak: media.** Es la vía de distribución preferida en muchas distribuciones, pero exige mantenimiento
  continuo (revisión de Flathub, fuentes sin red, permisos amplios de dispositivo `--device=all` que Flathub
  revisa, el usuario aún necesita estar en el grupo `dialout`/`uucp`), y la actualización automática del
  propio programa debe quedar desactivada (el actualizador solo se registra si la compilación lo configura:
  `updater.rs::is_configured`, una compilación de Flatpak simplemente no lo configuraría).
- **Snap: no conviene.** Con confinamiento estricto el puerto serie requiere conectar a mano la interfaz
  `serial-port` o una aprobación de la tienda; el modo clásico necesita revisión; da poco frente a `.deb` +
  AppImage + Flatpak.

**(d) Demanda: media.** Tablero: «ARM compatible Linux software» 49 votos
<https://lightburn.fider.io/posts/888/arm-compatible-linux-software>; «Please prioritize a Linux version of
LightBurn» 13 votos <https://lightburn.fider.io/posts/4888/please-prioritize-a-linux-version-of-lightburn>.
Foro: «LightBurn running on Raspberry Pi - Now working» **19 097 vistas, 152 respuestas, 44 "me gusta"**
<https://forum.lightburnsoftware.com/t/lightburn-running-on-raspberry-pi-now-working/89375> (la gente
ejecuta LightBurn de x86 en Pi con emulación). No encontré peticiones de Flatpak ni Snap (búsqueda `flatpak`
en el tablero: 0 resultados).

**(e) Esfuerzo:** ARM64 `.deb` + AppImage: S; Flatpak: M (más mantenimiento); Snap: no se agenda.

**(f) Cómo se prueba.** El flujo en el runner ARM compila, ejecuta las pruebas de Rust y la prueba de humo
con Xvfb; una instalación del `.deb` en un contenedor `arm64` (Docker con QEMU) comprueba dependencias. **Sin
Pi real no se mide rendimiento**: se declara *experimental* hasta que alguien de la comunidad la pruebe
(plantilla `machine_report.yml`). Con la Falcon2: conectarla desde una Pi, si alguien la presta.

---

## 12. Respaldo y sincronización de ajustes entre computadoras

**(a) Estado: Parcial (por archivo, manual).** El paquete `.tlakubundle` (M4, Hecho) lleva máquinas con sus
macros y posiciones, máquina activa, biblioteca de materiales, cámaras (calibración y configuraciones, sin
credenciales), arte propio, preferencias y atajos; al importar muestra una vista previa por secciones con
casillas y no escribe nada hasta confirmar; las configuraciones, materiales y dibujos se mezclan sin pisar lo
local. También están `.tlakuprefs` (D99), `.tlakuset` (respaldos de `$n` de GRBL, M25), `.tlakumat`
(materiales) y las preferencias en `preferences.json`. Código: `features/bundle/`, `settings/bundle.ts`,
`src-tauri/src/prefs.rs` (`export_user_bundle`, `import_user_bundle`).

**(b) Qué falta.**

- **Respaldo automático a una carpeta elegida** (por ejemplo la de iCloud, Dropbox o Syncthing): escribir
  `tlaku.tlakubundle` ahí al cambiar los ajustes o al cerrar, con la versión y la fecha.
- **Detección al abrir:** si el archivo de la carpeta es más nuevo que lo último importado en esta
  computadora, ofrecer «Importar cambios» con la misma vista previa de siempre (nunca aplicar en silencio).
- Lo que **no viaja** hoy: presets de imagen propios (C51, dicho en la fila M4), plantillas (carpeta propia),
  registro de trabajos, fuentes de una línea importadas, puertos y cámaras, que dependen de la computadora.
- Separar qué es de la persona y qué es de la computadora: preferencias de interfaz y recetas de material son
  portables; puertos serie, ids de dispositivo de cámara y rutas no (hoy `remoteEnabled` ya está en
  `NEVER_IMPORTED`).

**(c) Viabilidad.** Alta si se limita a **respaldo y restauración** por archivo o carpeta. La
sincronización bidireccional en vivo no es recomendable: conflictos entre dos computadoras, relojes
desalineados y clientes de nube que ven archivos temporales (`write_atomic` usa `.tmp` + renombrar, aceptable pero
puede dejar un `.tmp` a medias en la carpeta compartida). En Flatpak o con macOS en sandbox la app solo ve la
carpeta que el usuario eligió con el diálogo. Los secretos (URL de cámara con usuario) ya se omiten.

**Recomendación: reducir** a «Respaldo automático a carpeta + importar al abrir con vista previa»; sin fusión
automática de configuraciones divergentes.

**(d) Demanda: baja.** «default folder options in settings for saving files AND prefs» 5 votos
<https://lightburn.fider.io/posts/356/default-folder-options-in-settings-for-saving-files-and-prefs>; «Merge
material libraries» 4 votos <https://lightburn.fider.io/posts/1577/merge-material-libraries>; foro «Adding a 2nd
PC, shared resources, setup checklist?» 128 vistas
<https://forum.lightburnsoftware.com/t/adding-a-2nd-pc-shared-resources-setup-checklist-different-lb-versions/138572>.

**(e) Esfuerzo:** S-M.

**(f) Cómo se prueba.** Pruebas del modelo de importación existente (`bundle.test.tsx`) ampliadas con el caso
«archivo más nuevo / igual / más viejo» y con carpetas de prueba temporales; dos "computadoras" simuladas con
dos carpetas de configuración. Sin hardware.

---

## 13. Texto avanzado

**(a) Estado: Parcial, y mejor que LightBurn en varios puntos.** Tlaku usa `rustybuzz` 0.20 (HarfBuzz en Rust)
y `ttf-parser` 0.25 (`crates/tlaku-text`): formas de texto con kerning, ligaduras (se apagan solo cuando hay
espaciado entre letras), varias líneas, alineación, tamaño/espaciados, negrita y cursiva, **texto curvado** en
un arco de hasta ±360° (D39, `bend_deg`), **texto sobre un trayecto** con desplazamiento y voltear lado (D40),
texto soldado (D38), fuentes de una línea Hershey (15) y SHX (D41), y **texto variable** (D42). En el código
actual `shape_and_extract` (`layout.rs`) solo pasa a HarfBuzz las características de apagado de ligaduras.

**(b) Qué falta.**

- **Funciones OpenType:** ligaduras discrecionales, versalitas (`smcp`), alternativas estilísticas (`ss01`…`ss20`),
  `calt`, numerales (`lnum`/`onum`/`tnum`), fracciones, `swsh`. Se pasan como `rustybuzz::Feature` por
  intervalo; hace falta un campo en `TextSpec` (y en `TextObject` del documento) y un selector que **liste lo
  que cada fuente trae** (leyendo las tablas GSUB).
- **Fuentes variables:** `rustybuzz::Face::set_variations` existe en 0.20.1; faltan: leer los ejes (`fvar`),
  ofrecer controles de peso/ancho/inclinación, listar instancias con nombre, y aplicar la variación también a
  la extracción de contornos (`ttf-parser` con su característica `variable-fonts`); hoy `library.rs` elige una
  cara por peso 400/700 y `resolve` avisa cuando no hay negrita.
- **Texto sobre curva:** lo que hay no cubre: alineación del texto respecto del trayecto (centrado, justificado
  al largo, "ajustar al trayecto", dentro/fuera de una forma cerrada), texto en dos arcos (arriba y abajo de un
  sello) con un solo control, orientación de las letras (perpendiculares o rectas) y una vista que muestre el
  ancla.
- Lo que la fila D37 declara pendiente: alineación vertical, mayúsculas forzadas, derecha a izquierda (árabe,
  hebreo).
- Texto con estilos mezclados dentro de un mismo bloque (Fider 3743, 18 votos): no existe.

**(c) Viabilidad.** Alta para las funciones OpenType (HarfBuzz ya las implementa) y media para las variables
(riesgo en la lista de instancias por plataforma: `fontdb` 0.23 no expone ejes; leer `fvar` con `ttf-parser`;
fuentes variables del sistema pueden estar enlazadas a un archivo `.ttc`). Derecha a izquierda necesita
reordenamiento bidireccional (otro crate de licencia permisiva) y es de poca demanda aquí. Riesgo de
reproducibilidad: el mismo documento en dos computadoras debe dar el mismo contorno; las fuentes del sistema
varían, así que conviene guardar la **familia, estilo y características** y avisar de fuentes ausentes (ya
existe un aviso para fuentes ausentes).

**(d) Demanda: media.** «Alternate glyphs from opentype fonts» **33 votos** (20 comentarios)
<https://lightburn.fider.io/posts/666/alternate-glyphs-from-opentype-fonts>; kerning «Kerning tool for text» y
«Character Kerning» 13 votos cada uno <https://lightburn.fider.io/posts/1843/kerning-tool-for-text>; texto sobre
curvas: foro «Adding curved text within an outer and inner circle» 2 575 vistas
<https://forum.lightburnsoftware.com/t/adding-curved-text-within-an-outer-and-inner-circle/64859>, «Fitting
text to path» 2 141 vistas <https://forum.lightburnsoftware.com/t/fitting-text-to-path/26339>. Para **fuentes
variables** no encontré una petición específica.

**(e) Esfuerzo:** S-M para funciones OpenType y mejoras de texto sobre trayecto; M para fuentes variables.

**(f) Cómo se prueba.** Fuentes de prueba libres (SIL OFL) incluidas como fixtures de `tlaku-text`:
una con ligaduras y alternativas, una variable; pruebas de contorno (el contorno con `ss01` cambia y es igual
entre ejecuciones), pruebas de compatibilidad del documento (campos nuevos con valor por omisión, versión
del documento), capturas. Sin hardware.

---

## 14. Numeración y datos variables desde CSV

**(a) Estado: Hecho (ola A: varios CSV, imágenes/logotipos y QR/códigos variables, vista previa por fila y revisión de datos); quedan rango de filas y variables de ajuste de capa.** D42/M38 (Hecho): variables entre llaves
(`{serie:6}`, `{csv:Nombre}`, `{fila}`, fecha y hora con formato, número de copia, desplazamiento `@N`), diálogo
«Texto variable» con serie (paso, dígitos, hexadecimal), CSV (UTF-8, separador detectado, hasta 100 000
filas), avance **solo tras un trabajo que termina bien** (deshacible), «Una copia por fila», plantilla guardada
en el proyecto, resolución antes de previsualizar, enviar o exportar, y bloqueo del envío si un nombre no se
entiende. Código: `features/variables/`, `edit/variables.rs`, `tlaku-doc` (`VariableText`: un `csv_path`, una
`csv_row`, `serial`). Manual: `04b-texto-variable.md`.

**(b) Qué falta (los límites que la propia fila reconoce, más los que piden los usuarios).**

- ~~**Más de un CSV por proyecto**~~ **Hecho** (`VariableText.sources`; la principal manda, las demás se repiten o bloquean si son más cortas).
- ~~**Imágenes y logotipos variables**~~ **Hecho** para objetos imagen (`ImageObject.variable`); una forma SVG por fila queda pendiente.
- ~~**QR y códigos de barras variables**~~ **Hecho** (`VectorObject.code`, se regeneran al enviar); QR serializados con `{serie}` en el contenido.
- **Variables de ajuste de capa** (velocidad, potencia) desde una columna del CSV; hoy no.
- **Selección de filas** (rango inicial-final, filtro, saltar vacías) y **buscar y reemplazar** texto en bloque.
- Opcional: "hornear" a texto fijo con un clic (hoy se logra convirtiendo a trayectos).
- La sintaxis es propia y no se traduce desde LightBurn (decisión de sala limpia); no se pide cambiarla.

**(c) Viabilidad.** Alta: toda la resolución ocurre en la interfaz antes de enviar (`jobResolve.ts`), así que
extenderla no toca el motor, salvo que el QR variable se mueva al motor (`tlaku-codes`) para generarse al
resolver; el cuidado está en el costo con miles de filas y en que el resultado de la vista previa coincida con
el G-code (hay pruebas de eso para el texto). Imágenes variables: cuidado con rutas relativas, tamaños
distintos (ajuste a la caja del objeto original) y archivos faltantes (avisar y bloquear el envío, como ya hace
el texto con un nombre desconocido).

**(d) Demanda: la más alta de la lista.** «Multiple csv per project» **270 votos**
<https://lightburn.fider.io/posts/2163/multiple-csv-per-project>; «Variable Image / Logo Support in Variable
Text (CSV Merge)» **142 votos** (2026-04) <https://lightburn.fider.io/posts/4758/variable-image-logo-support-in-variable-text-csv-merge>;
«Serialized QR Code» 12 votos <https://lightburn.fider.io/posts/3013/serialized-qr-code>. Foro:
«Using 'auto-increment variable text'…» 2 444 vistas
<https://forum.lightburnsoftware.com/t/using-auto-increment-variable-text-option-in-the-modifier-menu/43971>.
LightBurn no las tiene (una "Variable icon/image" figura como *planned*, 12 votos).

**(e) Esfuerzo:** M (varios CSV, rango de filas y QR variable: S-M cada uno; imágenes variables: M).

**(f) Cómo se prueba.** Pruebas existentes de `features/variables/` ampliadas (resolución con dos CSV,
columnas ausentes, filas vacías, 100 000 filas); coincidencia vista previa-G-code en la máquina virtual;
imágenes variables con fixtures sintéticos; bloqueo del envío ante rutas faltantes. **Con la Falcon2:** grabar
un lote de 3 placas con serie y logotipo distinto sobre material de desecho.

---

## 15. Tabla resumen

| Mejora | Estado | Viabilidad | Demanda | Esfuerzo |
|---|---|---|---|---|
| 1. Pestañas | Parcial («Nueva ventana», D19) | Media (rediseño del estado por documento) | Alta (33, 24 votos; 8 486 vistas) | L (S la barra de documentos) |
| 2. Generadores de piezas | No existe | Alta (propio, sin dependencias) | Alta en bisagras (83 votos) y cajas (14 y 8 votos; miles de vistas); media-baja engranajes | L (por fases) |
| 3. Calculadora de costos | No existe (insumos listos) | Alta | Media-alta (30 y 27 votos) | S-M |
| 4. Cola e historial | Historial Parcial; cola No existe | Alta (historial), media (cola guiada) | Historial alta (71 votos); cola baja (7 votos) | S-M historial, M cola |
| 5. Biblioteca comunitaria | Parcial (flujo de aportes y compilación; sin aportes ni navegador) | Alta técnicamente; depende de la masa crítica | Media implícita (foros) | M |
| 6. Scripts en sandbox | No existe | Media (Boa o rquickjs; depende de la lista permitida de la ola 7) | Alta en votos (114, 79, 40), usuarios avanzados | L |
| 7. Nesting | **Hecho en lo principal** (`tlaku-nest`); faltan hojas, retazos y veta | Alta | Alta, ya cubierta (113 votos; LightBurn lo cobra en Pro) | M (mejoras); L si se cambia de motor |
| 8. Vista previa realista | Parcial (tono por potencia en trazos) | Media-baja si es "realista"; media si es tonal | Baja (4 y 5 votos) | M (tonal), L (texturas) |
| 10. Historial de versiones | Parcial (deshacer y recuperación; no tras guardar) | Alta | Baja-media (12, 11, 14 votos) | M |
| 11. Linux ARM64 / Flatpak / Snap | No existe | ARM64 alta; Flatpak media; Snap no conviene | Media (49 votos; 19 097 vistas) | S ARM64, M Flatpak |
| 12. Respaldo y sincronización | Parcial (`.tlakubundle` manual) | Alta si se reduce a respaldo en carpeta | Baja (5 y 4 votos) | S-M |
| 13. Texto avanzado | Parcial (curvo, trayecto, ligaduras base) | Alta (OpenType), media (variables) | Media (33 votos) | S-M |
| 14. Datos variables CSV | Hecho en lo básico; huecos | Alta | **Muy alta** (270 y 142 votos) | M |

## 16. Orden recomendado por olas (después de «pestañas»)

Criterios: **valor para quien vende** (hacer productos, cotizar, producir lotes), **demanda medida**, **riesgo**
(seguridad de la máquina, estado compartido, dependencias de terceros) y **dependencias entre mejoras**. Se
numeran como olas continuas tras la 7 (IA, opcional); donde la IA y los scripts comparten la lista permitida
conviene que la ola 7 llegue antes.

**Ola A: terminar lo que ya existe y tiene más votos (riesgo bajo, valor alto).**

1. **14. Datos variables:** varios CSV, imágenes/logos variables, QR variable, rango de filas. Es la demanda
   más alta (270 y 142 votos), el motor y la interfaz ya existen y el cambio es acotado.
2. **13. Texto avanzado:** funciones OpenType y mejoras sobre trayecto primero; fuentes variables después.
3. **3. Calculadora de costos:** pequeña, usa solo datos ya calculados y se vende sola a quien cotiza. Se
   deja lista la entrada para la calibración del historial (ola C).
4. **11a. Linux ARM64** (`.deb` y AppImage): solo CI, sin tocar el producto; puede hacerse en paralelo en
   cualquier momento. Etiquetar como experimental.

**Ola B: generadores (diferencial fuerte, trabajo largo pero sin riesgo de seguridad).**

5. **2a-2b: cajas con dientes + pieza de prueba de encaje, y bisagras flexibles.** Es lo más pedido de los
   generadores (83 votos de bisagras; cajas con miles de vistas) y lo que más usa quien vende cajas y
   organizadores. Antes: la mejora 7 (acomodo) ya permite poner los paneles en una hoja.
6. **2c-2d: engranajes, bandejas, cola de milano, uniones generales** (más espaciado, menos demanda).

**Ola C: producción y negocio.**

7. **4. Historial enriquecido** (ajustes, tamaño, copias; a archivo; CSV) y después la **cola guiada**. El
   historial va primero porque la calculadora (mejora 3) y la cola lo necesitan y porque es la demanda mayor
   del par (71 votos).
8. **10. Historial de versiones.** Barato sobre la infraestructura de recuperación; protege el trabajo de
   quien vende por encargo.
9. **7. Mejoras de nesting:** hojas múltiples, retazos como inventario y veta; con el costo de la mejora 3
   calculado sobre material usado.

**Ola D: comunidad y alcance.**

10. **12. Respaldo a carpeta** (reducido) y, si hay demanda, **11b. Flatpak**.
11. **5. Biblioteca comunitaria:** primero reunir aportes con el flujo actual; el navegador y el catálogo
    firmado cuando haya masa crítica (decenas de aportes) o cuando se quiera poblar con los ajustes de fábrica.
12. **8. Vista previa tonal** (reducida): útil para mostrar al cliente; sin prisa por su baja demanda.

**Ola E: extensibilidad.**

13. **6. Scripts en sandbox**, **después** de la ola 7 (IA) o junto con ella, porque comparten la lista
    permitida, la vista previa de operaciones y el paso único de deshacer. Es lo de mayor riesgo y mayor
    esfuerzo, para usuarios avanzados.

**Dependencias que justifican el orden:**

- Calculadora (3) → mejor con historial (4, calibración del tiempo) y con nesting (7, aprovechamiento).
- Generadores (2) → usan el nesting existente y la ayuda de ranuras (D71); no dependen de nada nuevo.
- Cola (4) → depende del historial y, si hay pestañas, de definir de qué pestaña sale cada elemento.
- Scripts (6) y IA (ola 7) → comparten lista permitida y vista previa; hacerlos por separado duplica el trabajo.
- Biblioteca comunitaria (5) → depende de que existan aportes; hoy hay cero.
- Pestañas (1) → conviene que la vista en vivo del otro agente esté integrada antes de tocar el estado global.

## 17. Lo que NO es viable o conviene reducir

- **Snap (11): no se agenda.** El confinamiento complica el puerto serie y el USB; aporta poco frente a
  `.deb`, AppImage y Flatpak. Flatpak queda opcional y de mantenimiento.
- **Vista previa «realista» (8): reducir a vista tonal.** Una simulación fotográfica del resultado promete lo
  que una máquina, un lente y un lote de material no garantizan; las texturas piden WebGL (desigual en
  WebKitGTK/Raspberry Pi) y arte propio. Solo hay un reclamo débil de usuarios (4 y 5 votos).
- **Sincronización en vivo (12): no se hace;** se reduce a respaldo y restauración por carpeta con vista
  previa. Los conflictos entre computadoras, los puertos y las cámaras (propios de cada PC) lo hacen frágil.
- **Cola de trabajos (4): reducir a «cola guiada».** Nunca avanza sola cuando cambia el material o el
  archivo; cada inicio pasa por el diálogo y la lista de comprobación. Un modo desatendido contradice las
  reglas de seguridad del proyecto (un láser sin vigilancia es un riesgo de incendio).
- **Biblioteca comunitaria (5): viable pero prematura.** Sin aportes no hay valor. Además, **"la misma
  máquina" no se puede demostrar**: solo hay clase, vatios y modelo declarados, y niveles de confianza por
  confirmaciones.
- **Scripts (6): viable, pero lo último.** Sin sandbox bien probado no se publica; sin Web Worker; sin
  scripts que se ejecuten al abrir un documento.
- **Nesting (7): no se reescribe.** Ya existe y es competitivo; solo se mide y se amplía. No copiar nada de
  deepnest (su núcleo es GPLv3 pese a que el repositorio se anuncie como MIT) ni usar el Deepnest original (sin
  licencia).
- **Generadores: sin biblioteca externa.** No hay opción permisiva madura; la GPL (boxes.py,
  TabbedBoxMaker, living-hinge) solo se lee.
- **Texto de derecha a izquierda (13):** fuera de este paquete salvo que aparezca demanda; necesita
  reordenamiento bidireccional y pruebas con hablantes.
- **Pestañas (1):** decidido; ver los riesgos de memoria y de coordinación con la vista en vivo.

Nada de esta lista está ya completo, salvo el nesting en su núcleo (7) y la parte básica de los datos variables
(14); esas dos solo se agendan por sus huecos.

## 18. Notas de método y límites

- Licencias: leídas en el archivo `LICENSE`/`COPYING` de cada repositorio (no en menciones de terceros), salvo
  donde se indica «según su ficha» o «metadatos de crates.io». Verificadas el 2026-10-14; las licencias
  pueden cambiar, así que se debe volver a leer el archivo antes de copiar nada y anotarlo en `THIRD_PARTY.md`.
- Demanda: el tablero `lightburn.fider.io` y el foro `forum.lightburnsoftware.com` solo representan a usuarios
  de LightBurn; Reddit y Facebook no se pudieron consultar. Las cifras son de la fecha de consulta.
- Rendimiento en Raspberry Pi y comportamiento del resultado físico (generadores, vista tonal) **no están
  medidos**; la única máquina real es una Creality Falcon2 Pro de 22 W (diodo), así que cualquier prueba física
  de CO2, fibra o galvo queda sin validar.
- Este documento no sustituye `docs/validacion-hardware.md`: toda función nueva que mueva o dispare la máquina
  debe sumar sus pruebas allí.
