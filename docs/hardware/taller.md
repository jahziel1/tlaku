# Validación del taller contra la práctica documentada

Investigación hecha el 2026-10-13 para validar, sin hardware, lo implementado en la fase 4: rotativo, eje Z,
corrección de barrido, puentes y cámara. Sala limpia: solo se leyó documentación pública (la de LightBurn como
referencia de comportamiento, sin copiar textos), manuales y páginas de fabricantes, foros y artículos. No se leyó
código GPL. Todo está resumido con palabras propias; cada dato lleva su URL.

Escala de confianza usada abajo: **[doc]** documentación oficial del fabricante o de LightBurn; **[foro]** reporte de
usuarios (anecdótico); **[mía]** deducción propia, no confirmada por ninguna fuente.

---

## 1. Rotativo

### 1.1 Cómo se configura en la práctica

Hay dos formas de que la controladora gire el accesorio:

1. **Reemplazar el eje Y**: se desconecta el motor de Y y el rotativo se conecta a su puerto (o a Z/A si hay un puerto
   propio). El software "habla Y" y alguien convierte mm de superficie a mm de Y de la controladora.
2. **Escalar en firmware** (`$101` de GRBL, pasos/mm de Y): el fabricante o el usuario cambian los pasos por mm para que
   1 mm de Y sea 1 mm de superficie. Ortur documenta cambiar `$101` de 100 a 80 al usar su YRR en una LM3
   ([foro LightBurn, LM3](https://forum.lightburnsoftware.com/t/ortur-lm3-w-rotary-thinks-a-1-inch-square-is-1-25-in/88602)) [foro];
   obliga a conmutar el valor cada vez que se alterna entre plano y rotativo, y la F y la aceleración quedan en mm
   de superficie solo mientras el valor esté cambiado.

LightBurn eligió la opción 1 con escalado en software: el usuario declara el tipo, "mm per rotation" (los mm de Y que
debe recorrer el eje para una vuelta completa del accesorio), y el diámetro (rodillo o pieza)
([LightBurn, rotativo GCode](https://docs.lightburnsoftware.com/2.1/Reference/RotaryMode/RotaryModeGCode/)) [doc].
Datos de esa página:

| Tema | Rodillo (roller) | Mandril (chuck) |
|---|---|---|
| Sujeción | La pieza descansa sobre ruedas | Mordazas la agarran |
| Diámetro de la pieza | **No afecta la salida** (solo calcula la circunferencia para el diseño) | **Afecta la salida**; se reintroduce por pieza; usar el diámetro mayor si es cónica |
| Diámetro del rodillo | Obligatorio, casi nunca cambia | No aplica |
| Prueba "Test" | Debe girar 360° la rueda motriz, no la pieza | La pieza gira 360° |
| Ajuste de mm/vuelta | Si gira de más, bajarlo; si de menos, subirlo | Igual |

Otros puntos de la misma página [doc]:
- "Mirror output" (invertir la salida) si el resultado sale invertido en el eje de rotación.
- Si se usa el puerto de Y, el eje de rotación es Y; con puerto propio, Z o A.
- Recomienda **Start From = Current Position** (el rotativo no tiene origen absoluto; se coloca a mano).
- `ALARM:2` (soft limits): el recorrido del rotativo supera el máximo de Y; se desactiva temporalmente con `$20=0`.
- Deslizamiento en rodillos: material antideslizante, bandas de goma, peso dentro de la pieza o **bajar la aceleración
  del eje rotativo en el firmware**.
- Zona plana al final de un grabado: holgura (backlash) del engranaje. Se compensa con una línea de potencia muy baja
  (0.1 %) unos mm por debajo del diseño, como primera capa, para tomar el juego.
- El mismo documento da la fórmula de pasos por grado para Smoothieware: `(200 × microsteps × reducción) / 360`;
  ejemplo 200·8·2/360 = 8.89 pasos/grado (3200 pasos por vuelta).

**Fórmulas** (derivadas de lo anterior y consistentes con lo que reportan los usuarios):
- Rodillo: mm de Y por mm de superficie = `mm_per_rotation / (π · D_rodillo)`.
- Mandril: `mm_per_rotation / (π · D_pieza)`.
- Si el eje del motor mueve el rodillo 1:1 y se usa `$101`: pasos/mm = `(pasos_motor · microsteps · reducción) / (π · D_rodillo)`
  (fórmula general de pasos/mm con la circunferencia como recorrido por vuelta;
  [tutorial GRBL, ejemplo de correa GT2: 2 mm · 20 dientes = 40 mm/vuelta](https://zbotic.in/grbl-setup-tutorial-cnc-controller-configuration-step-by-step/)) [mía/foro].

**Importante: "mm per rotation" NO es la circunferencia del rodillo.** Un foro lo confunde y otro lo desmiente; los
valores reales de fábrica lo confirman (siguiente sección). Esto valida que Tlaku guarde `mm_per_rotation`
y `roller_diameter` como dos números independientes.

### 1.2 Valores típicos de rotativos de diodo

| Accesorio | Valores | Fuente |
|---|---|---|
| **xTool RA2 Pro / RA1, rodillo** | "steps per rotation" (= mm/vuelta) **32 mm** fija; diámetro del rodillo **17 mm** fijo; diámetro de la pieza se mide. Escala resultante 32/(π·17) = **0.599** | [xTool, artículo 588](https://support.xtool.com/article/588) [doc] |
| **xTool RA2 Pro, mandril** | mm/vuelta **128 mm** fija ("no debe cambiarse"); diámetro de la pieza se mide. Con D=80: 128/(π·80) = 0.509. Eje Y en D1 Pro, Z en D1; "mirror output" si el motor queda hacia el riel izquierdo | misma fuente [doc] |
| **Creality Falcon (rodillo)** | Sin cifras oficiales. Un usuario encontró que 41 mm/vuelta con rodillo de 17 mm daba el tamaño correcto (probó también 40.29/16.8); escala 0.768 | [foro Creality](https://forum.creality.com/t/falcon-rotary-roller-technical-specification/33195) [foro] |
| **Ortur YRR 2.0 (rodillo)** | Sin tabla oficial. Reportes: rodillo de 20 mm con 62.83 mm/vuelta (= circunferencia, escala 1.0); otro usuario terminó en 63.5. En la LM3 el manual manda `$101=80` | [LightBurn, Ortur YRR](https://forum.lightburnsoftware.com/t/ortur-lm3-w-rotary-thinks-a-1-inch-square-is-1-25-in/88602), [blog de calibración](https://hagensieker.com/2021/01/27/ortur-yrr-rotary-roller-set-up/) [foro] |
| **Atomstack R3** | Rodillo de 16.5 mm introducido por un usuario; 51 mm/vuelta hallado con el botón Test (con un láser Vevor) | [foro](https://forum.lightburnsoftware.com/t/configure-r3-rotary-in-lightburn-with-grbl-controler/60428), [foro](https://forum.lightburnsoftware.com/t/atomstack-r3-rotary-roller-and-a-vevor-10-watt-laser-engraver/165741) [foro] |
| **OMTech** | No se halló un rotativo de diodo con valores oficiales. Los de CO2 (eje en tarjeta Ruida) quedan fuera del alcance GRBL | (sin datos) |

Conclusiones: los valores de fábrica solo existen de forma oficial para xTool; los demás se calibran con el botón Test o
con un trabajo de tamaño conocido. La escala puede ser mayor o menor que 1. La sensibilidad es alta: pasar de 63.75 a
63.88 mm/vuelta movió el grabado unos 19 mm en una taza ([foro LightBurn](https://forum.lightburnsoftware.com/t/ortur-lm3-w-rotary-thinks-a-1-inch-square-is-1-25-in/88602)) [foro]; por eso hace falta una calibración fina.

### 1.3 Problemas comunes

- **La pieza se desliza** (la causa más habitual de "no pasa dos veces por el mismo sitio"): antideslizante, gomas, peso,
  y bajar `$120`/`$121` (aceleración) ([foro, Ortur LM2 Pro S2 + YRR2.0](https://forum.lightburnsoftware.com/t/ortur-lm2-pro-s2-and-yrr2-0-the-laser-does-not-pass-through-the-same-place-twice/125502)) [foro]. LightBurn también atribuye el "temblor" de líneas a una aceleración de Y demasiado alta ([guía de scan offset](https://docs.lightburnsoftware.com/2.1/Guides/ScanningOffsetAdjustment/)) [doc].
- **Velocidad sobre la superficie**: no encontré documentación de LightBurn sobre si reescala F al usar rotativo. Es
  una decisión de diseño que se justifica físicamente (energía por mm de superficie constante) [mía].
- **Aceleración de Y**: con escala k, la aceleración en la superficie es `$121 / k`. Con k<1 (xTool: 0.6) la superficie
  acelera 1.67 veces más de lo que marca `$121`, o sea más deslizamiento [mía].
- **Soft limits y homing**: el rotativo en el puerto de Y no tiene final de carrera; todos los tutoriales mandan
  desactivar soft limits/homing en el perfil de rotativo y guardar el perfil aparte
  ([blog](https://hagensieker.com/2021/01/27/ortur-yrr-rotary-roller-set-up/), [LightBurn](https://docs.lightburnsoftware.com/2.1/Reference/RotaryMode/RotaryModeGCode/)) [doc/foro]. Si no, el rodillo
  gira buscando un límite hasta dar error.
- **Encuadre/origen**: LightBurn recomienda Current Position con rotativo y mover el cabezal a mano
  ([foro, homing y encuadre](https://forum.lightburnsoftware.com/t/homing-and-framing-with-a-rotary/188737)) [foro]. Hay reportes de errores del origen y del espejo
  al activar "Enable Rotary" ([foro](https://forum.lightburnsoftware.com/t/rotary-mode-is-broken-in-many-ways/144125)) [foro]; el personal los atribuyó a un orden de ajustes y los corrigió.
- **Espejo (mirror)**: el sentido de giro depende de cómo se monte el motor (xTool lo indica). Debe poder invertirse sin
  tocar `$3`, porque `$3` también invierte el modo plano.
- **Backlash**: ver 1.1 (línea de 0.1 % como primera capa).

### 1.4 Comparación con Tlaku (`rotary.rs`, "Decisiones del taller")

| Decisión de Tlaku | Práctica | Veredicto |
|---|---|---|
| Fórmula rodillo `mm_per_rotation/(π·D_rod)` y mandril `/(π·D_pieza)` | Coincide con LightBurn y con los valores xTool (32/17 y 128) | Correcto |
| Escalado en software, sin tocar `$101` | Es lo que hace LightBurn; evita tener que conmutar `$101` | Correcto |
| Y absoluta escalada, sin `G92` | Compatible con Current Position; coherente con GRBL | Correcto, pero ver límites abajo |
| F reescalada por tramo | Sin referencia en docs de LightBurn; físicamente justificada | Correcto; hay que probar con hardware |
| Aceleración/vel. máx de Y divididas por `y_scale` | Consistente | Correcto |
| Roller: el diámetro de la pieza no afecta la salida | Igual que LightBurn | Correcto (pero se valida igual `object_diameter>0`) |
| Valores por defecto 360 / 20 / 80 | 360 mm/vuelta con rodillo de 20 mm da escala 5.73; ningún accesorio real se acerca (32, 41, 51, 63.5, 128) | Riesgo: un perfil con rotativo activo sin calibrar mueve Y casi 6 veces y puede dar ALARM:2 |
| No hay opción de espejo/sentido | xTool y LightBurn la requieren | Falta |
| No se valida Y contra la cama | Correcto (el recorrido en mm de controladora es k·circunferencia); LightBurn pide `$20=0` | Falta avisar al usuario de los soft limits |

---

## 2. Eje Z en láseres

### 2.1 Convención del signo

- LightBurn documenta **Z Offset: positivo = el eje Z va "hacia adentro", acercando el cabezal al material; negativo = lo aleja**
  ([LightBurn, Cut Settings Line](https://docs.lightburnsoftware.com/2.1/Reference/CutSettingsEditor/LineMode/)) [doc].
  Esa página **no especifica el signo de "Z Step per Pass"**, y varios usuarios descubrieron que un paso positivo
  *sube* el cabezal en su máquina, por lo que hay que usar uno negativo; algunos proponen un interruptor de inversión
  global ([foro](https://forum.lightburnsoftware.com/t/understanding-z-axis-control/86844)) [foro].
- En GRBL estándar (CNC) +Z sube la herramienta. Un miembro del personal de LightBurn lo expresa así: un Z negativo
  baja la herramienta hacia la pieza, y en una máquina donde se mueve la cama, un Z negativo sube la cama; la flecha
  "arriba" del jog debe, por tanto, alejar la cama del láser ([foro, prueba de foco con Z invertida](https://forum.lightburnsoftware.com/t/focus-test-moving-z-axis-in-incorrect-direction/180890)) [foro/personal].
- Como resultado, en máquinas reales el signo es **inconsistente** (cabezal que sube con +Z, cama que sube con +Z,
  homing arriba o abajo). La respuesta de LightBurn es un ajuste de dispositivo "Reverse Z direction" (a veces sin efecto
  sobre el jog, o duplicado con `$3`), no una convención fija
  ([foro](https://forum.lightburnsoftware.com/t/reverse-z-direction-with-no-effect/147748), [foro](https://forum.lightburnsoftware.com/t/focus-test-moving-z-axis-in-incorrect-direction/180890)) [foro].

### 2.2 ¿Relativo al foco o absoluto?

LightBurn ofrece los dos modos y los separa en Device Settings
([LightBurn, ajustes de dispositivo](https://docs.lightburnsoftware.com/2.0/Reference/DeviceSettings/DimensionsUnits/)) [doc]:
- **Enable Z axis**: al activarlo, LightBurn **siempre** emite valores de Z; por eso hay que activar *Relative Z moves only*
  o fijar "Material height" en la capa. Una altura de material de 0 "puede hacer que el cabezal choque con la pieza".
- **Relative Z moves only**: toma la altura del cabezal al iniciar el trabajo como Z=0 y todos los movimientos son
  relativos a ella (se ignora la altura del material). Un usuario del personal explica que con movimientos relativos
  "LightBurn se queda en la altura inicial y hace los movimientos Z relativos a ella" ([foro, resumen de búsqueda](https://forum.lightburnsoftware.com/t/z-axis-do-not-move-between-passes-and-layers/5829) [foro]).
- **Modo absoluto**: la Z depende de la altura del material introducida, o sea de un cero de máquina conocido.
- **Optimize Z moves**: por defecto LightBurn **vuelve a la altura inicial tras cada forma** que tenga offset, para
  evitar colisiones; optimizar lo desactiva y solo se recomienda con material plano sin obstáculos [doc].
- Z Offset y Z Step per Pass son **deltas respecto del foco/altura de partida de la capa**, no posiciones absolutas
  ([LightBurn](https://docs.lightburnsoftware.com/2.1/Reference/CutSettingsEditor/LineMode/), [foro](https://forum.lightburnsoftware.com/t/z-offset-vs-zstep-per-pass/63627)) [doc/foro]. Hay un reporte en una Neje 4 Max donde, con
  varias formas en una capa, Z bajaba en cada forma y no volvía a la altura inicial ([foro](https://forum.lightburnsoftware.com/t/neje-4-max-issue-with-automated-z-step-commands/135628)) [foro].
- Un doc de la propia LightBurn advierte que en máquinas con autoenfoque, la altura de autoenfoque suele ser el
  límite inferior (no se puede ir más profundo) [doc].

### 2.3 G0 frente a G1

En un G-code guardado por LightBurn el personal vio líneas `G0 Z1` ([foro, prueba de foco](https://forum.lightburnsoftware.com/t/focus-test-moving-z-axis-in-incorrect-direction/180890)) [foro].
El motivo práctico de usar `G0` y no `G1`: en modo láser de GRBL (`$32=1`) el láser se apaga en un G0, mientras que un
`G1 Z` con S>0 y M3 podría dejar el láser encendido durante el movimiento (hay un hilo en el foro de "al activar Z el
láser quema una línea entre las formas", [foro](https://forum.lightburnsoftware.com/t/when-i-activate-the-z-axis-the-laser-burns-a-line-between-the-shapes/4616) [foro]; la causa exacta no la verifiqué). El riesgo de choque viene del **sentido**
y del **cero**, no de G0 frente a G1: el G0 de Z va a la velocidad máxima de Z (`$112`) y no frena antes de tocar.

### 2.4 ¿Conviene un "Z invertido" en el perfil?

Sí. Dos razones: (1) el signo físico varía por máquina; (2) los usuarios acaban forzando signos negativos por capa o
por material para compensar, lo cual es frágil. LightBurn ya lo tiene como ajuste de dispositivo. Una propuesta mejor:
definir una convención interna fija (profundidad) y un `z_invert` en el perfil que solo afecta al G-code.

### 2.5 Comparación con Tlaku (`zaxis.rs`, `gcode.rs`)

| Aspecto | Tlaku | Práctica | Gravedad |
|---|---|---|---|
| Valor emitido | `z_mm = offset + step·pasada`, **absoluto**, "0 = foco" | LightBurn separa absoluto y relativo; para el absoluto exige conocer la altura del material; el relativo es lo habitual | Rompe: con Z absoluta, Z=0 es el cero de la máquina, no el foco, salvo que el usuario ponga el cero de trabajo en el foco (y no usamos G54/G10) |
| Signo | "depende de la máquina", sin ajuste | Ajuste "Reverse Z" de dispositivo; semántica de offset positivo = más cerca del material | Riesgo |
| Vuelta de Z | Tras un bloque no vuelve; al final solo `G0 X0 Y0` | LightBurn vuelve a la altura inicial tras cada forma (por defecto) | Riesgo: el último `G0 X0 Y0` se hace con el cabezal en la Z más profunda; el siguiente trabajo parte de una Z desconocida |
| G0 Z | Correcto | Igual que LightBurn | Correcto |
| División por pasada | Bloque por pasada | Compatible | Correcto |
| Simulador ignora Z | Aceptable | | Menor |
| Sin eje Z con valores ≠ 0 | Advierte y no emite | Mejor que LightBurn (que emite siempre Z al activarlo) | Correcto |

---

## 3. Scan offset (corrección de barrido bidireccional)

### 3.1 Por qué ocurre y cómo se mide

- Causas, según LightBurn: el láser tarda en disparar tras recibir la orden (fuentes y tubos varían de microsegundos a
  más), y a mayor velocidad las correas se estiran un poco. Ejemplo: 1 ms de retardo a 100 mm/s son 0.1 mm; a 500 mm/s,
  0.5 mm ([LightBurn, guía](https://docs.lightburnsoftware.com/2.1/Guides/ScanningOffsetAdjustment/)) [doc].
- Antes de ajustar, revisar correas flojas [doc]. El balanceo de líneas es otro problema (aceleración de Y).
- **Procedimiento**: rectángulo de 50 × 10 mm en Fill, intervalo 0.5 mm, overscan ≥ 5 % en GCode (Ruida lo hace solo),
  potencia que marque sin carbonizar; correr a al menos dos velocidades y **medir la separación entre los extremos de
  las líneas**. En la tabla se introduce **la mitad** del valor medido, porque el programa desplaza cada línea esa
  cantidad (una de ida y una de vuelta suman el total). Hay un "Initial Offset" aparte, que desplaza todo el grabado
  a izquierda o derecha para alinearlo con cortes vectoriales [doc].
- Los valores intermedios se calculan por interpolación [doc]. Las unidades de velocidad de la tabla no se declaran;
  los ejemplos usan mm/s [doc].
- Alternativa comunitaria: una imagen de cuadrícula autolectora, bidireccional, con umbral e intervalo 0.1 mm, donde se
  lee la figura más nítida de cada fila a 200/400/600 mm/s; ya viene con signo ([foro](https://forum.lightburnsoftware.com/t/scanning-offset-measurement-from-self-reading-grid/191158)) [foro].
  Si al introducir el valor el resultado es el doble de malo, hay que invertir el signo [foro].

### 3.2 Valores típicos

Los datos son pocos y anecdóticos (a más velocidad, más offset, aproximadamente lineal):
- Un usuario cita unos 0.025 mm a 50 mm/s y 0.3 mm a 300 mm/s (hilo de un diodo GRBL, pero esos números son de otro tipo de
  máquina no identificada) ([foro](https://forum.lightburnsoftware.com/t/offset-scan-settings/48802)) [foro]; el mismo hilo
  propone para un diodo a 8000 mm/min una columna de 0.25 mm que baja 0.03 mm por cada 1000 mm/min menos. No hay cifras por
  velocidad fiables para CO2 en lo que encontré (Ruida guarda su propia tabla en la controladora).
- Diodo GRBL: a 6000 mm/min o menos casi no hace falta; un 40 W con overscan al 50 % necesitó una tabla propia
  y los números se obtuvieron midiendo ([foro](https://forum.lightburnsoftware.com/t/scanning-offset-problems/59517)) [foro].
- En un diodo de 40 W en FluidNC la causa real del doble contorno era el motor de pasos I2S (`I2S_stream`); cambiando
  a `I2S_static` desapareció y se quitó el offset ([foro v1e](https://forum.v1e.com/t/seeing-double-lightburn-scanning-offset-adjustment-woes/31417)) [foro].
  Las ralentizaciones del planificador al rellenar el búfer también cambian el retardo, de modo que un offset fijo
  deja de ser exacto.
- No existe una tabla fiable por modelo. Conclusión: debe medirse por máquina; los valores por defecto deben ser vacíos.

### 3.3 ¿A qué líneas se aplica?

- La documentación de LightBurn **no dice** a qué líneas ni en qué sentido; solo que "cada línea se desplaza esa
  cantidad" y que por eso se introduce la **mitad** de la separación medida. Eso implica que se corrigen **las dos mitades**
  (ida y vuelta) en sentidos opuestos [doc para la mitad; la simetría es deducción mía].
- Hay un "Initial offset" aparte para centrar el conjunto respecto de los vectores.

### 3.4 Comparación con Tlaku (`scan_offset.rs`)

| Tlaku | Práctica | Gravedad |
|---|---|---|
| Solo las líneas de vuelta se desplazan `offset` mm en el sentido de barrido; las de ida no se tocan | Reparto simétrico (mitad a cada lado) y un "initial offset" opcional. Con la nuestra, las líneas de ida siguen con su retardo `d`, así que **todo el grabado queda corrido `d`** respecto de los vectores (hasta 0.3 mm a 300 mm/s) | Riesgo: el grabado y el corte pierden registro |
| El perfil guarda "mm que se desplazan las de vuelta" | El usuario mide el hueco total entre extremos y LightBurn pide la mitad | Menor: documentar exactamente qué número se introduce |
| Velocidad en mm/min | LightBurn muestra mm/s y no lo declara | Menor: etiquetar la unidad en la interfaz |
| Interpolación lineal y extremos fijos | Igual | Correcto |
| Con crosshatch cada línea se compara con su eje | Razonable | Correcto |
| Se mueve el overscan junto con la línea | Necesario; el overscan debe ser ≥ |offset| para que la línea desplazada entre ya a velocidad | Menor: avisar si overscan < offset |
| No hay generador del patrón de prueba | LightBurn describe el rectángulo de 50×10; la comunidad usa una cuadrícula autolectora | Menor: añadir al generador de pruebas |

---

## 4. Puentes (tabs)

### 4.1 Prácticas

- Ancho: por defecto 0.5 mm en LightBurn; Flux recomienda 0.5 mm o menos para que se rompan a mano
  ([LightBurn, Add Tabs](https://docs.lightburnsoftware.com/2.1/Reference/AddTabs/), [Flux](https://support.flux3dp.com/hc/en-us/articles/12441363882511-Tabs-Bridges)) [doc]. Un puente sano en contrachapado de 3 mm
  puede desaparecer en aglomerado: depende del material y del grosor.
- **El kerf se come el puente**: el haz quita material de ambos lados, así que el puente real mide aproximadamente su
  ancho menos un kerf. Un foro recomienda que sean **al menos 4 veces el kerf** (ejemplos del autor: papel 0.7 mm,
  aglomerado 0.75 mm, tela gruesa 1 mm); con 0.3 mm se cayeron piezas
  ([foro](https://forum.lightburnsoftware.com/t/issue-with-elements-falling-through-in-files-with-bridges-using-ruida-controller/119464/4)) [foro].
- Cantidad: LightBurn tiene "Even Spacing" (un puente al inicio de cada tramo y luego cada X mm; 50 mm por defecto en
  el tutorial, 40 mm en su ejemplo) y "Tabs per shape" (divide el perímetro en N+1 tramos y reparte los puentes)
  ([laseruser](https://laseruser.com/lightburn-tutorial-16/), [LightBurn](https://docs.lightburnsoftware.com/2.1/Reference/AddTabs/)) [doc/tutorial].
- "Skip inner shapes": no pone puentes en formas contenidas en otra de la misma capa [doc].
- "Tab cut power": 0 % salta el puente por completo; otro valor lo recorre con pulsos (una unión más débil) [doc].
- Solo en modo Line, no en Fill [tutorial].
- Esquinas frente a tramos rectos: LightBurn no lo trata; un tutorial de diseño aconseja ponerlos donde sea fácil
  recortarlos, **lejos de esquinas y curvas**, y en acrílico las esquinas agudas concentran tensión y agrietan (resultados de búsqueda; no
  abrí la fuente exacta, tratar como [foro/artículo] sin verificar). No encontré una fuente que mida la distancia mínima a la esquina.
- Los puentes cortados con potencia 0 son lo habitual; el tirón de LightBurn es el pulso parcial.

### 4.2 Comparación con `tabs.rs`

| Tlaku | Práctica | Gravedad |
|---|---|---|
| Hueco de `width_mm` medido sobre el contorno, después del kerf, con corte a potencia 0 (Cut, no Travel) | Coincide con "Tab cut power 0 %" y evita frenar | Correcto |
| Se desplazan al tramo recto más cercano (hasta 30 % del paso), evitando giros de más de 30° | Consistente con el consejo de evitar esquinas | Correcto |
| `count`/`spacing_mm` y límite de la mitad del perímetro | Equivalente a Tabs per shape / Even Spacing | Correcto |
| `skip_inner` | Igual que LightBurn | Correcto |
| No avisa si `width` es comparable al kerf | El puente efectivo ≈ `width − kerf` y la recomendación es ≥ 4×kerf | Riesgo: piezas que se caen |
| No hay potencia parcial en el puente | LightBurn la ofrece | Menor |
| Ancho por defecto | LightBurn 0.5 mm | Menor: revisar el valor por defecto |

---

## 5. Cámara

### 5.1 Cómo se calibra y se alinea en la práctica

Dos etapas separadas, como en LightBurn 2.1 ([calibración](https://docs.lightburnsoftware.com/2.1/Reference/Cameras/Calibration/), [alineación](https://docs.lightburnsoftware.com/2.1/Reference/Cameras/Alignment/)) [doc]:

1. **Calibración de lente** (una vez por cámara): patrón plano de alto contraste (en la 2.1 un tablero de AprilTags;
   en versiones anteriores, círculos). Se toman **5 capturas** (cuatro esquinas y el centro de la cama). Ofrece cuatro
   modelos matemáticos, incluyendo uno "Extendido" para distorsión fuerte y uno **ojo de pez verdadero**; un porcentaje
   de error por modelo, pero **un número bajo no garantiza precisión** (calibrar solo el centro lo infla), así que hay
   que comprobar que las rectas salgan rectas.
2. **Alineación con la cama**: se graba un patrón en material, sin moverlo, y se marcan sus puntos. Automática: 8
   AprilTags, al menos 4 detectados; manual: cuatro cruces, se hace clic en el centro de cada una (con zoom). Antes se
   enfoca el láser, se usan coordenadas absolutas y se declaran las alturas del material y del soporte. Hay que
   rehacerla cada vez que se mueve la cámara. Tras alinear, se verifica con la rejilla (rectas rectas, rectángulos
   cuadrados).

**Altura y paralaje** [doc/foro]:
- La alineación solo es válida para el plano donde se grabaron las marcas. Cambiar el grosor del material desplaza la
  superposición; el error depende del ángulo de la lente y del montaje y es mucho menor con una cámara estrecha mirando
  recto que con un gran angular o una cámara inclinada
  ([LightBurn, resolución de problemas](https://docs.lightburnsoftware.com/2.0/Troubleshooting/CameraTroubleshooting/)).
- Soluciones documentadas: (a) alinear a cada altura habitual y guardar un archivo por altura, importando el más cercano;
  (b) alinear sobre material elevado; (c) en LightBurn 2.1, un nombre de alineación por grosor y **"Interpolate Alignment
  Height"** para estimar alturas no medidas [doc]; (d) lo ideal es cama con altura Z ajustable que mantenga la
  superficie siempre a la misma distancia ([foro, personal](https://forum.lightburnsoftware.com/t/camera-alignment-is-accurate-only-with-the-4-targets-but-not-anything-else-you-add/31878)).
- Creality: realinear si el grosor cambia más de 3 mm o si la cámara se mueve; se guardan los parámetros para cada base
  de distinta altura ([foro/wiki](https://wiki.creality.com/en/laser-engraver/camera-calibration-guide)) [fabricante, resumen indirecto: la página no se pudo leer entera].

### 5.2 Cámaras típicas

- Tapa: la cámara ojo de pez de 5 MP en el centro de la tapa es la configuración típica de CO2 de gama baja (OMTech,
  Gweike, Roly). Distorsión fuerte, montada en un punto que se mueve al abrir/cerrar la tapa. Consejos: tope en la tapa
  para que vuelva siempre a la misma posición, quitar juego de bisagras, que la montura no se afloje (un usuario tuvo
  que bloquear con silicona el tornillo del enfoque) ([LightBurn](https://docs.lightburnsoftware.com/2.0/Troubleshooting/CameraTroubleshooting/), [blog](https://softsolder.com/2022/12/06/laser-cutter-lightburn-camera-calibration/)) [doc/foro].
- Cabezal: la cámara se mueve con el cabezal; el programa hace varias capturas, el láser se mueve y se unen en una
  superposición ("Update Overlay"); la alineación usa el mismo patrón de 4 marcas o AprilTags, y se excluye la región
  borrosa o con distorsión del borde ([LightBurn](https://docs.lightburnsoftware.com/2.1/Reference/Cameras/HeadMountedCamera/)) [doc].
- Fabricantes con cámara integrada (xTool, Creality) usan sus propias aplicaciones; xTool reconoce que una cámara
  integrada sola puede dar errores de procesado de 2 a 6 mm por baja resolución y distorsión, y vende un método con
  fotografía del teléfono que dice bajar a 0.2 mm ([xTool](https://xtool.com/blogs/xtool-academy/introducing-xtool-phone-snapshot-preview)) [marketing].

### 5.3 Precisión que reporta la gente

| Reporte | Valor | Fuente |
|---|---|---|
| Ojo de pez de 5 MP en tapa (Gweike) | ≈1 mm en el centro, hasta 8 mm en las esquinas, con puntuación de calibración ≤0.35 | [foro](https://forum.lightburnsoftware.com/t/v1-7-03-camera-calibration-issue-april-tags-not-detected-circles-detected/156376) [foro, resumen de búsqueda] |
| OMTech Polar 350, ojo de pez en tapa | Bien en el tercio central; fuera, "varios mm o hasta 1 cm" con puntuaciones <0.15 | [foro](https://forum.lightburnsoftware.com/t/camera-too-inaccurate/131262) [foro] |
| OMTech 60 W, cámara ordinaria | Diagonales de 859.0 y 861.5 mm contra 860.2 ideal, unos ±1.3 mm en 860 mm | [blog](https://softsolder.com/2022/12/06/laser-cutter-lightburn-camera-calibration/) [foro] |
| OMTech (otro usuario) | Casi todo por debajo de 1 mm, con ajuste fino ±0.2 mm por zonas | (resumen de búsqueda) [foro] |
| Roly Mk2, cámara en tapa | ≈0.5 mm de precisión | [foro](https://forum.lightburnsoftware.com/t/lightburn-2-1-01-camera-calibration-for-different-thickness-materials/190222) [foro] |
| Falcon A1 Pro | un desvío fijo de ≈2 mm en Y tras repetir la calibración | [foro Creality](https://forum.creality.com/t/recurring-2mm-y-axis-alignment-shift-camera-offset-calibration-loop-creality-falcon-a1-pro/51726) [foro] |
| xTool F1 Ultra | ≈1 mm en el centro; imagen girada ≈1° | resumen de búsqueda [foro] |

Nota: los datos marcados "resumen de búsqueda" (Gweike, OMTech de ±0.2 mm, xTool F1 Ultra) no se abrieron directamente y deben
confirmarse antes de citarlos; los demás se leyeron en la página.

Orden de magnitud realista: **±1 mm en el centro, 2 a 8 mm en los bordes con ojo de pez**, y una dependencia fuerte de la
altura. Los errores de borde apuntan al modelo de lente (ver 5.4).

### 5.4 Lo que tenemos y lo que falta

Hoy: calibración de lente con tablero de ajedrez (Zhang + Brown-Conrady `k1,k2,p1,p2,k3`), corrección de distorsión,
homografía DLT normalizada con RANSAC opcional, y alineación manual de 4 puntos (`ImageToBed`). El módulo ya
advierte que la homografía solo vale para la altura de las marcas. Faltan:

**(a) Modelo de ojo de pez.** LightBurn ofrece un modelo de ojo de pez verdadero porque los polinomios radiales de
Brown-Conrady se degradan en el borde de las lentes de 5 MP. El patrón de error de la tabla (centro bien, bordes mal)
coincide con eso. Recomiendo añadir el modelo equidistante de Kannala-Brandt (`θ_d = θ(1 + k1θ² + k2θ⁴ + k3θ⁶ + k4θ⁸)`)
como segunda opción y elegir entre ambos con una validación cruzada (fotos reservadas), no solo con el RMS global;
[mía]. También un mapa de error por zona (centro, bordes, esquinas) en el informe.

**Hecho en la ola 4:** el modelo de Kannala-Brandt y la elección automática por validación cruzada están en
`crates/tlaku-camera/src/calibration.rs`. En la prueba sintética (ojo de pez de 140°, 16 vistas, 0,1 px de ruido) el
error de reproyección de Brown-Conrady es de 0,56 px —parece bueno— pero su error de posición en un plano a 350 mm es de
45 mm rms (hasta 108 mm) contra 1,2 mm rms (hasta 3,2 mm) del modelo de ojo de pez; el error en las vistas reservadas
(0,79 px contra 0,14 px) es lo que delata a Brown-Conrady. El mapa de error por zona sigue pendiente. Todo con cámaras
sintéticas: falta validarlo con una lente real.

**(b) Parte 3 propuesta: marcas grabadas + corrección de altura.**

*Marcas grabadas.* `camera_marks(bed, n)` genera un `Document` normal (como `material_test`) con 4 cruces
(o 9: 4 esquinas, 4 puntos medios y centro) a ≥80-90 % del ancho de la cama, trazo fino, potencia baja, sin cortar. Las
posiciones de cama de cada marca se conocen por construcción (coordenadas absolutas, `Start From` absoluto), así que
no hay que medirlas. Se fotografía sin mover el material; se corrige la distorsión; las marcas se detectan por una
búsqueda alrededor de la posición esperada (convertida con la homografía provisional de 4 puntos manuales) y se
refina el centro con un ajuste de cruce subpíxel; se pasa a `estimate_homography_ransac`. Primero se mantiene la
alineación manual como respaldo (LightBurn hace lo mismo). Un informe con el residuo por marca; rechazar si algún
residuo supera ~0.3 mm; más marcas permiten medir la precisión real (validación con marcas no usadas en el ajuste).

*Altura.* Con la imagen ya sin distorsión (sin lente) un plano a altura `h` sobre la cama se proyecta como
`x ~ K [r1 r2 (t + h·r3)] [X Y 1]ᵀ`. Por tanto **la homografía del plano a altura `h` es `H(h) = [H₁ H₂ (H₃ + h·G)]`: sus
dos primeras columnas no cambian y la tercera es lineal en `h`** (con una normalización de escala común) [mía, deducción del
modelo pinhole; coincide con lo que LightBurn llama "interpolar la altura"]. Dos opciones:
  1. *Datos*: grabar y alinear a dos alturas distintas (p. ej. cama y +10 mm, un tablero del grosor que se tenga),
     normalizar la escala de las dos homografías para que sus dos primeras columnas coincidan en el sentido de mínimos
     cuadrados, y guardar `H₃` y `G = (H₃(h₂) − H₃(h₁))/(h₂ − h₁)`. Con una sola altura medida se usa la segunda opción como
     respaldo. Es exacta en teoría con lente corregida, sin requerir `K`.
  2. *Modelo*: con la `K` de la calibración y una alineación, se descompone `H` para obtener `r1, r2` (`r3 = r1 × r2`), la
     escala `λ = 1/‖K⁻¹h₁‖` y la posición de la cámara. Sirve de respaldo con una sola altura, pero depende de que
     `K` (focal) sea buena. Útil como verificación cruzada de la opción 1.

Interfaz: la altura del material (mm) entra como dato del trabajo/perfil; `pixel_to_bed(p, h)`. Almacenar en el
perfil: `{ lens, alignments: [{ height_mm, H }], interpolated: bool }` y un aviso cuando `h` quede fuera del rango
medido (extrapolar es menos fiable). Cámara en cabezal: otra alineación (otro tipo) con unión de capturas; no se
aborda ahora.

*Robustez de la montura.* Detectar que la cámara se movió comparando una referencia fija de la imagen (p. ej. una
marca permanente en la cama) con la guardada; si se desplaza más de un umbral, avisar de que hay que realinear (las
tapas se mueven, ver 5.2).

---

## 6. Prueba de materiales

### 6.1 Patrones de la comunidad

- LightBurn: cuadrícula 10 × 10 por defecto que varía **potencia y velocidad**, y puede variar intervalo o pasadas;
  cada celda es un rectángulo, con rótulos y borde opcionales; las celdas se ejecutan **de menor a mayor energía**
  (mayor velocidad, menor potencia, menor intervalo y menos pasadas primero; la página lo redacta como "mayor riesgo
  de quemar", que es contradictorio con esos ejemplos) para no quemar la muestra; avisa de **mantener las unidades de
  velocidad** (mm/s ≠ mm/min) ([LightBurn, Material Test](https://docs.lightburnsoftware.com/2.1/Reference/MaterialTest/)) [doc].
- Guía de 20 W: cuadrícula de 6 × 6, grabado con potencia 15-45 % en pasos de 5 % y velocidad 2500-6500 mm/min, con
  intervalo constante de 0.10-0.15 mm; corte con 80-100 % y 200-600 mm/min; rótulo en cada celda con potencia/velocidad y
  se guarda el ganador en la biblioteca ([Zapcraft](https://zapcraft.net/optimizing-lightburn-settings-20w-diode-lasers/)) [artículo; valores sin verificar].
- Para un diseño tipo "escalera": filas por velocidad, columnas por potencia, intervalo y pasadas fijos y rotulados arriba [doc].

### 6.2 Valores de partida (diodo 10-40 W)

Todas las cifras son **puntos de partida**: velocidades en mm/min (la tabla de xTool usa mm/s, ×60). Siempre con aire.

| Material | Operación | Valores | Fuente | Confianza |
|---|---|---|---|---|
| **Contrachapado 3 mm (nogal/palo de rosa)** | Corte, Falcon2 40 W | 100 %, 750 mm/min, 1 pasada | [Creality](https://crealityfalcon.com/blogs/product-tutorials-and-support/creality-falcon2-laser-engraving-cutting-parameter-settings) | [doc] |
| | Corte, Falcon2 22 W | 100 %, 450 mm/min, 1 pasada | misma | [doc] |
| | Corte, 20 W genérico (álamo) | 85-100 %, 250-500 mm/min, 3-6 pasadas | [Zapcraft](https://zapcraft.net/optimizing-lightburn-settings-20w-diode-lasers/) | sin verificar |
| | Corte, D1 Pro 20 W (basswood 3 mm) | 100 %, 6 mm/s (=360 mm/min), 1 pasada | [xTool (resumen)](https://www.xtool.eu/pages/material-settings) | [doc, vía resumen] |
| | Grabado | 20 % (40 W) a 35 % (22 W), 6000 mm/min | Creality | [doc] |
| **MDF 3 mm** | Corte, 20 W | 80-100 %, 120-240 mm/min, 2-5 pasadas | Zapcraft (listado) | sin verificar |
| | Corte, 20 W | 100 %, 480 mm/min, 2-3 pasadas (otra guía) | resumen de búsqueda | sin verificar |
| | Corte, 40 W (NEJE) | 50 %, 200 mm/min, 2 pasadas | [Maker Forums](https://forum.makerforums.info/t/your-opinion-at-speed-power-for-cutting-mdf/83250) (resumen de búsqueda, sin abrir) | [foro] |
| | Nota | Creality no publica MDF; MDF laminado/chapado corta peor; ventilar | | |
| **Acrílico 3 mm opaco/negro** | Corte, Falcon2 40 W | 30 % (colores) a 100 % (negro 4.6 mm: 180 mm/min), 120 mm/min, 1-3 pasadas | Creality | [doc] |
| | Corte, Falcon2 22 W | 55 %, 120 mm/min, 1-3 pasadas | Creality | [doc] |
| | Grabado | 26 % / 4000-6000 mm/min (40 W); 50 % (22 W) | Creality | [doc] |
| | Transparente | Un diodo no corta claro ni azul; solo transparentes coloreados rojos/amarillos/verdes según Creality | [xTool](https://www.atomm.com/blog/2535-what-is-xtool-d1), Creality | [doc] |
| **Cuero 0.65 mm marrón** | Grabado | 12 % (40 W) / 18 % (22 W), 6000 mm/min | Creality | [doc] |
| | Corte | 70 %, 4000 mm/min (40 W); 100 %, 2000 mm/min (22 W) | Creality | [doc] |
| **Cuero curtido vegetal 1-3 mm** | Grabado | 15-45 %, 1500-4500 mm/min | Zapcraft | sin verificar |
| | Seguridad | No cortar ni grabar cuero curtido al cromo (gases peligrosos) | Zapcraft | [artículo] |

Recomendaciones para la biblioteca de ejemplo: guardar rangos (no un solo valor) con la potencia del módulo; cuadrícula
de 6×6 de potencia × velocidad con rango centrado en el valor de arranque (±~35 %); intervalo constante (0.1-0.15 mm en
madera); rótulos con las unidades; y una advertencia explícita sobre mm/min frente a mm/s (el error más citado).

---

## Diferencias con Tlaku

Orden por gravedad. "Rompe" = puede dañar la máquina o dar un resultado inutilizable; "riesgo" = puede fallar en
situaciones comunes; "menor" = pulido.

| # | Qué hicimos | Qué hace la práctica | Gravedad | Cambio sugerido |
|---|---|---|---|---|
| 1 | Z **absoluta** (`G0 Z{offset+step·pasada}`) con "0 = foco"; no hay G54/G10 | Modo **relativo** por defecto (Z=0 es la altura al empezar) o absoluto con altura de material conocida; LightBurn avisa del choque con material=0 | **Rompe** | Emitir Z relativa (`G91`/`G0 Z±Δ`/`G90`, siguiendo la Z acumulada) por defecto; el absoluto solo como opción explícita con la altura declarada |
| 2 | Tras bajar Z no se regresa; el final hace `G0 X0 Y0` con el cabezal bajo | LightBurn vuelve a la altura inicial tras cada forma con offset (por defecto) y al final | **Riesgo** | Volver a la Z inicial antes del último `G0 X0 Y0` y al final del trabajo; opción "optimizar Z" que lo evite solo en material plano |
| 3 | Signo de Z "depende de la máquina", sin ajuste | Convención: offset positivo = más cerca del material; ajuste "Reverse Z"; usuarios fuerzan pasos negativos | **Riesgo** | Convención interna fija (profundidad: positivo = foco más profundo) y `z_invert` en el perfil, solo en el G-code; documentar la prueba de jog |
| 4 | Scan offset: solo la línea de vuelta se mueve el valor completo | LightBurn desplaza ambas mitades (valor medido / 2) y ofrece offset inicial | **Riesgo** | Repartir ±offset/2 (ida −, vuelta +) y añadir `initial_offset`; documentar qué número se introduce; opcional: modo "solo vuelta" |
| 5 | Rotativo con defaults 360/20/80 y rotativo activable sin calibrar | Ningún accesorio real se acerca (32, 41, 51, 63.5, 128); escala 5.7 mueve Y casi 6× y da ALARM:2 | **Riesgo** | Presets (xTool RA2 Pro rodillo 32/17 y mandril 128; resto "calibrar"), botón de prueba de una vuelta, asistente de calibración (trabajo de tamaño conocido), y un aviso cuando k·extensión excede `$131` |
| 6 | No hay espejo/sentido del rotativo | xTool y LightBurn tienen "mirror output"; depende del montaje | **Riesgo** | Casilla `reverse` que refleja Y dentro del bounding box del trabajo antes de escalar (no negativa la Y absoluta) |
| 7 | Rotativo: no se avisa de soft limits / homing | Todos los tutoriales mandan desactivarlos (`$20=0`, `$22=0`) y usar perfil aparte | **Riesgo** | Aviso al activar rotativo y al conectar; no cambiar nada sin que el usuario lo pida |
| 8 | Tabs: `width` sin relación con el kerf | Puente efectivo ≈ ancho − kerf; recomendado ≥ 4×kerf (con 0.3 mm se caen piezas) | **Riesgo** | Advertir si `width < 2×kerf` (rojo) o `< 4×kerf` (amarillo), y mostrar el ancho efectivo |
| 9 | Cámara: solo Brown-Conrady | Ojo de pez de 5 MP: error de 1 mm al centro y hasta 8 mm en las esquinas; LightBurn ofrece modelo de ojo de pez | **Riesgo** | Modelo equidistante (Kannala-Brandt) opcional y validación cruzada con informe por zonas |
| 10 | Cámara: homografía válida solo a la altura de las marcas | LightBurn: una alineación por altura e interpolación; paralaje de varios mm | **Riesgo** | Parte 3: alineación a dos alturas con `H(h)` lineal en la tercera columna, y `pixel_to_bed(p, h)` |
| 11 | Roller valida `object_diameter > 0` aunque no se use | LightBurn: el diámetro de la pieza no afecta en rodillo | **Menor** | Validar solo lo que se usa |
| 12 | Scan offset en mm/min sin etiqueta clara | LightBurn: mm/s no declarado | **Menor** | Etiquetar unidades; conversiones |
| 13 | Overscan puede ser menor que el offset | Línea desplazada necesita lead-in | **Menor** | Avisar si `overscan < |offset|` |
| 14 | F reescalada por tramo (sin referencia pública) | Nadie lo documenta | **Menor** | Mantener; validar con hardware; se puede ofrecer "no reescalar" para comparar |
| 15 | Tabs sin potencia parcial | LightBurn: "Tab cut power" | **Menor** | Opcional: perforación por pulsos |
| 16 | Backlash del rotativo sin ayuda | LightBurn: línea de 0.1 % como primera capa | **Menor** | Opción "tomar holgura" |
| 17 | Material test sin orden de ejecución por energía ni generador de scan offset | LightBurn ordena de menor a mayor energía; patrón de scan offset de 50×10 mm | **Menor** | Ordenar celdas; añadir modo "scan offset" y "marcas de cámara" al generador |

---

## Casos de prueba automatizables

**Rotativo**
1. `y_scale` rodillo 32/17 ≈ 0.5991; mandril 128/D=80 ≈ 0.5093; 62.83/20 ≈ 1.0; invariante de que en rodillo cambiar `object_diameter` no cambia el G-code.
2. Propiedad: para tramos aleatorios, `longitud_ctrl / (F/60) == longitud_superficie / (v/60)` (mismo tiempo) y F en tramos solo-X no cambia.
3. El G-code con rotativo no contiene `G92`, `G10` ni `G54..G59`.
4. Ida y vuelta: `current_position`/`user_origin` ctrl → superficie → ctrl es la identidad (1e-9).
5. Aceleración y vel. máx. de Y del perfil divididas por k: el tiempo estimado de un tramo Y puro coincide con la fórmula cinemática.
6. (si se añade `reverse`) el reflejo conserva el bounding box en Y y reproduce la imagen simétrica.
7. Rotativo activo con 360/20/80 genera una advertencia "sin calibrar" (cuando haya presets).

**Eje Z**
8. Modo relativo: la suma de todos los `Z` emitidos es 0 al final y el último `Z` va antes de `G0 X0 Y0`.
9. `z_invert` cambia el signo de todas las palabras Z y nada más.
10. Solo `G0 Z`, nunca `G1 Z`, y ninguna línea Z lleva S>0 ni F.
11. `z_at(i) == offset + step·i`; división en bloques solo con `step != 0` y pasadas>1.
12. Sin eje Z con `z_offset != 0`: advertencia y cero palabras Z.
13. Z se emite antes del primer movimiento XY del bloque y no se repite si no cambia.
14. Entradas no finitas devuelven error.

**Scan offset**
15. Reparto simétrico: para un relleno bidireccional, el centro de las líneas de ida y de vuelta coincide con el nominal (|Δ| < 1e-9) y la separación entre ellas es el offset completo.
16. Interpolación lineal en el rango, extremos fijos fuera de él, tablas desordenadas y valores NaN ignorados; velocidad 0 devuelve el primer valor.
17. Barrido a 90° y crosshatch: el desplazamiento va por el eje correcto.
18. Offset 0 o sin tabla: salida idéntica bit a bit.
19. Aviso si `overscan < |offset|`.
20. Conversión mm/s ↔ mm/min en importar/exportar `.lbso`-like (si se añade).

**Puentes**
21. Cada contorno elegible queda con `n` huecos cuyo largo suma `n·width` (±1e-6) y nunca más de la mitad del perímetro.
22. Ningún centro de puente a menos de `w/2` de una esquina de más de 30° cuando el contorno tiene tramo recto ≥ `3w`.
23. `skip_inner`: los huecos solo en profundidad 0; contornos abiertos intactos.
24. Los huecos se cortan a potencia 0 (`Cut`), sin cambiar el orden planificado.
25. Aviso cuando `width < 4×kerf_total`, y el ancho efectivo mostrado es `width − kerf_total`.

**Cámara**
26. Sintético: proyectar una cuadrícula de cama a alturas `h1`, `h2` con un modelo pinhole conocido; ajustar `H(h1)`, `H(h2)`; la predicha para `h3` tiene error < 0.01 px y la corrección reproduce el punto de cama < 0.05 mm.
27. Las dos primeras columnas normalizadas de `H(h)` son constantes con `h`; la tercera es afín.
28. Detección de cruces: renderizar cruces con ruido y desenfoque; el centro se recupera con error < 0.1 px; RANSAC rechaza una marca desplazada.
29. Ojo de pez sintético (modelo KB): se recuperan los coeficientes y el error en las esquinas es menor que con Brown-Conrady.
30. Detección de movimiento de la cámara: desplazar la imagen 3 px activa el aviso.

**Prueba de materiales**
31. Las celdas se ordenan de menor a mayor energía; cada rótulo contiene unidades; conteo = filas × columnas.
32. Importar tablas en mm/s multiplica por 60 al pasar a mm/min.
