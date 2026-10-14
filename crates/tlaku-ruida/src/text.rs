//! Textos de usuario compartidos por los errores del crate.

use tlaku_i18n::tr;

/// Traduce el nombre de un dato o una regla (`what`) que los errores llevan como texto fijo
/// en español. Lo que no está en la tabla (nombres de prueba, p. ej.) se devuelve tal cual.
pub(crate) fn what(s: &str) -> String {
    match s {
        "coordenada absoluta" => tr!("ruida.what.abs_coord"),
        "coordenada relativa" => tr!("ruida.what.rel_coord"),
        "potencia" => tr!("ruida.what.power"),
        "potencia (%)" => tr!("ruida.what.power_pct"),
        "coordenada" => tr!("ruida.what.coord"),
        "coordenada (mm)" => tr!("ruida.what.coord_mm"),
        "velocidad" => tr!("ruida.what.speed"),
        "velocidad (mm/min)" => tr!("ruida.what.speed_mm_min"),
        "opcode (debe tener el bit 7 en 1)" => tr!("ruida.what.opcode"),
        "láser (1 a 4)" => tr!("ruida.what.laser"),
        "subcódigo de tiempo C6" => tr!("ruida.what.time_sub"),
        "nombre de archivo (solo ASCII sin NUL)" => tr!("ruida.what.file_name_ascii"),
        "valores de escritura de memoria" => tr!("ruida.what.mem_values"),
        "bandera de capa" => tr!("ruida.what.layer_flag"),
        "subcódigo D8" => tr!("ruida.what.sub_d8"),
        "suma de archivo" => tr!("ruida.what.file_checksum"),
        "valor de memoria" => tr!("ruida.what.mem_value"),
        "nombre de elemento" => tr!("ruida.what.element_name"),
        "dirección de memoria" => tr!("ruida.what.mem_addr"),
        "tiempo" => tr!("ruida.what.time"),
        "frecuencia" => tr!("ruida.what.frequency"),
        "color" => tr!("ruida.what.color"),
        "subcódigo" => tr!("ruida.what.subcode"),
        "columnas" => tr!("ruida.what.columns"),
        "filas" => tr!("ruida.what.rows"),
        "parámetro" => tr!("ruida.what.param"),
        "dato" => tr!("ruida.what.data"),
        "sondeo" => tr!("ruida.what.probe"),
        "lectura" => tr!("ruida.what.read"),
        "respuesta de lectura" => tr!("ruida.what.read_response"),
        "fin del movimiento" => tr!("ruida.what.end_of_motion"),
        "la cama debe medir más de 0 mm" => tr!("ruida.what.bed_size"),
        "la velocidad máxima debe ser mayor que 0" => tr!("ruida.what.max_speed"),
        "la potencia máxima debe estar entre 0 y 100 %" => tr!("ruida.what.max_power"),
        "max_data_bytes debe estar entre 1 y 1470" => tr!("ruida.what.max_data_bytes"),
        "laser_tube_start debe caber en 7 bits" => tr!("ruida.what.tube_start"),
        "la velocidad" => tr!("ruida.what.the_speed"),
        "una coordenada" => tr!("ruida.what.a_coordinate"),
        "la potencia" => tr!("ruida.what.the_power"),
        "un salto D9 con opciones desconocidas (puede encender el láser)" => {
            tr!("ruida.what.d9_jump")
        }
        "la escritura de memoria (DA 01)" => tr!("ruida.what.mem_write"),
        "flujo vacío" => tr!("ruida.what.empty_stream"),
        "el nombre de archivo (E7 01) debe terminar en un único cero" => {
            tr!("ruida.what.file_name_zero")
        }
        other => other.to_string(),
    }
}
