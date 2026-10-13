# Ideas de mejora sobre LightBurn

Backlog de ideas para después de cubrir los P0 (ver `spec/00-resumen.md`). No son compromisos.
Las referencias `01-Qn`, `02-Qn`, `03-Qn` apuntan a las quejas del foro en cada inventario.
"(verificar)" = no confirmamos si LightBurn ya lo hace; revisarlo antes de venderlo como ventaja.

## Eficiencia de corte

| Idea | Por qué | Esfuerzo |
|---|---|---|
| **Planificador explicable**: cada salto del orden de corte dice qué regla lo decidió (capa, prioridad, interior primero, cercanía). Las reglas forman un grafo de restricciones, no una lista que se pisa. | La queja más repetida: "interiores primero" no se respeta y nadie sabe por qué (02-Q4) | Alto |
| **Mejor recorrido**: vecino más cercano + mejora 2-opt/Or-opt, eligiendo punto de inicio y sentido de cada figura. Mostrar en la vista previa el ahorro de tiempo. | Menos recorridos en vacío = trabajos más cortos (verificar qué hace LightBurn hoy) | Medio |
| **Overscan calculado por física**: distancia `v²/2a` con la aceleración real leída de la máquina (`$120/$121` en GRBL), en vez de un porcentaje a ojo. | Overscan más corto donde sobra; aviso antes de iniciar si se sale de la cama (02-Q9) | Bajo |
| **Encuadre que incluye el overscan** y avisa antes de empezar. | Hoy la alarma llega a mitad del trabajo (02-Q9) | Bajo |
| **Estimación de tiempo real**: modelo de aceleración de la máquina + autoajuste con la duración real de trabajos anteriores. | Hoy se queda corta 1.5x-2x en imágenes (02-Q3) | Medio |
| **Envío por conteo de caracteres** en GRBL con el búfer lleno en todo momento. | Tirones y desconexiones en modo Buffered (03-3) | Medio |
| **Arcos G2/G3** en vez de cientos de rectas para curvas. | G-code más pequeño, menos saturación serie, curvas más suaves (verificar) | Medio |
| **Corte de línea común y anidado (nesting)** automático de piezas. | Ahorro de material y de tiempo (verificar si LightBurn 2.x ya anida) | Alto |
| **Orden por calor**: alternar zonas para no acumular calor en piezas delgadas. | Menos deformación y quemado | Medio |
| **Offset fill y flood fill que no se cuelgan**, cancelables, con progreso. | Cuelgues y fugas de memoria (02-Q1, 02-Q2) | Medio |
| **Reanudar trabajo** desde donde se detuvo (corte de luz, pausa larga). | Pérdida de material | Medio |
| **Tiling** con solape configurable y marcas de registro automáticas. | LightBurn 2.1 no lo tiene (03-8) | Alto |

## Calibración

La idea central: **quemar un patrón de prueba, medirlo (con cámara o leyendo un número), y que el
programa calcule el ajuste solo.** Hoy casi todo se calibra a mano con regla.

| Idea | Cómo | Esfuerzo |
|---|---|---|
| **Corrección de backlash (scanning offset) con escala vernier**: se graba un patrón a varias velocidades y el usuario solo lee qué marca coincide. Con cámara, automático. | Hoy está escondida y se calibra con regla (02-Q16, 02-Q2) | Bajo (vernier) / Medio (cámara) |
| **Kerf con pieza de prueba**: cortar un cuadrado y su agujero, medir con calibrador, el programa calcula el kerf por material. | Signo y dirección confusos (02-Q5) | Bajo |
| **Pasos por mm y escuadra**: grabar una regla y una cruz grande, medir, y calcular `$100/$101` y el sesgo entre ejes. | Piezas que no miden lo que deben | Bajo |
| **Curva de potencia real**: grabar una escala de grises, fotografiarla, y crear una tabla de corrección para que el grabado se parezca a la imagen. | Diodos con poca diferencia entre tonos (02-Q8) | Medio |
| **Test de materiales leído por cámara**: sugiere la mejor combinación velocidad/potencia y la guarda en la biblioteca. | Hoy el usuario elige a ojo | Alto |
| **Foco automático**: rampa de foco + cámara para encontrar la altura con línea más fina. | Foco mal puesto = cortes que no atraviesan | Medio |
| **Cámara por altura de material**: la alineación cambia con el espesor; homografía por altura y corrección de paralaje. | Overlay desfasado, pérdida tras reinicio (03-6) | Alto |
| **Print & Cut con N puntos** y lectura de marcas por cámara. | Hoy son 2 puntos y sin cámara (03-7) | Medio |
| **Calibración de campo galvo** con rejilla grabada y fotografiada. | Dependencia de archivos del fabricante (03-11) | Alto |

## Robustez y experiencia

- **Formas abiertas visibles** y botones deshabilitados que explican por qué (01-1, 01-2).
- **Importación con verificación de tamaño**: avisar de unidades ausentes y 72 vs 96 DPI (01-5, 01-6).
- **Rendimiento con archivos densos** (01-13): motor en Rust + lienzo WebGL.
- **Biblioteca de materiales que no se pierde** al actualizar, con formato abierto y versionado (02-Q10).
- **Seguridad**: límite de potencia por máquina y confirmaciones explícitas para disparar o encuadrar con láser encendido (03-16).
- **Linux de primera clase** (03-17: según el inventario LightBurn dejó Linux tras 1.7; verificar).
- **Asistentes visuales** para origen/encuadre/inicio (03-1, 03-2) y para rotary (03-9).
