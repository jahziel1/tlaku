# Texto

Cómo crear texto, elegir su fuente, escribirlo sobre una curva y convertirlo en trayectos editables. El texto de Tlaku se calcula a partir de las fuentes instaladas en tu sistema.

## Crear un texto

1. Elige la herramienta **Texto** (`T`).
2. Haz clic en un punto vacío del lienzo. Aparece el texto "Texto" (10 mm, fuente Arial si está instalada; si no, la primera disponible) en la capa activa, ya seleccionado.
3. Escribe en el campo **Contenido** del panel que flota junto al texto. `Intro` agrega una línea.

Para editar un texto existente, haz doble clic sobre él con **Seleccionar**, o selecciónalo: el panel **Texto** aparece cada vez que la selección es un único texto. Puedes mover, escalar y girar un texto como cualquier objeto (ver [Dibujar y editar](03-dibujar-y-editar.md)).

![Panel Texto junto a un texto seleccionado, con contenido, fuente, estilo, tamaño y la opción Sobre trayecto](../../apps/desktop/docs/screenshots/12-panel-de-texto.png)

> **Consejo:** el texto va a una capa como cualquier otro objeto. En una capa de **Línea** se recorre el contorno de las letras; en una de **Relleno** se graba su interior. En una capa de **Imagen** no se puede usar un texto (ver [Capas y ajustes de corte](05-capas-y-ajustes.md)).

## Opciones del panel

| Opción | Qué hace |
|---|---|
| **Contenido** | El texto, con varias líneas si quieres. |
| **Fuente** | Un buscador (**Buscar fuente…**) sobre las familias instaladas. Si hay muchas, escribe para acotar la lista. |
| **Negrita**, **Cursiva** | Piden esas variantes de la fuente. |
| **Alinear a la izquierda**, **Centrar**, **Alinear a la derecha** | Alineación de las líneas entre sí. |
| **Tamaño** | En mm. Es el tamaño del em, no la altura de la mayúscula: una "H" sale más baja que el valor que escribes. |
| **Espaciado** | Separación extra entre letras, en mm (admite valores negativos). |
| **Interlineado** | En múltiplos del tamaño; empieza en 1,2. |

> **Ojo:** Tlaku nunca inventa una negrita o una cursiva. Si la familia no tiene esa variante, usa la más cercana que exista y lo dice en los **Avisos del contorno**. Si la fuente que elegiste no está instalada (por ejemplo, al abrir un proyecto en otro equipo), usa otra y avisa: "La fuente '...' no está instalada; se usa '...'". Revisa el aviso antes de enviar nada.

Mientras se calcula el contorno verás **Generando contorno…**. Si falla, el panel muestra el motivo.

## Texto sobre trayecto

Para que el texto siga una línea o una curva:

1. Dibuja primero el trazo guía (una línea, polilínea, elipse...). Sin un objeto vectorial en el diseño, la casilla no se puede marcar: "Dibuja una línea o una curva para poder escribir sobre ella".
2. Selecciona el texto y marca **Sobre trayecto**.
3. En **Objeto guía** elige el trazo.
4. Ajusta el **Desplazamiento** (en mm, a lo largo del trazo) y marca **Voltear** para escribir por el otro lado.

Si borras el objeto guía, el panel avisa: elige otro o desmarca **Sobre trayecto**. El trazo guía sigue siendo un objeto normal y se cortará o grabará según su capa; ponlo en una capa sin salida (o en **Guías**) si solo lo quieres de referencia.

## Convertir a trayectos

**Convertir a trayectos** reemplaza el texto por trayectos vectoriales. Úsalo cuando necesites editar los nodos de las letras, o cuando vayas a pasar el proyecto a otro equipo sin depender de la fuente.

- Después ya no puedes cambiar la fuente ni el contenido: es un paso con **Deshacer** si te arrepientes.
- Está deshabilitado mientras se genera el contorno, o si el texto no tiene contorno todavía (escribe algo con una fuente instalada).

Un texto con contorno ya sirve para las booleanas y el **Offset** del menú **Organizar**. En cambio, **Editar nodos** y **Unir trazos…**, **Cerrar trayectos…** y **Quitar duplicados…** solo trabajan con trayectos: convierte el texto antes.

## Lo que todavía no hay

- Fuentes de una línea (Hershey y SHX), para grabar el trazo central de las letras: **(próximamente)**.
- QR y códigos de barras: **(próximamente)**.

Siguiente: [Capas y ajustes de corte](05-capas-y-ajustes.md)
