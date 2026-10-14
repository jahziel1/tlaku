//! Textos de usuario compartidos por los errores del crate.

use tlaku_i18n::tr;

/// Traduce el nombre de un dato o una regla (`what`) que los errores llevan como texto fijo
/// en español. Lo que no está en la tabla se devuelve tal cual.
pub(crate) fn what(s: &str) -> String {
    match s {
        "la cama debe medir más de 0 mm" => tr!("lhymicro.what.bed_size"),
        "la velocidad máxima debe ser mayor que 0" => tr!("lhymicro.what.max_speed"),
        "min_jog_mil no puede ser negativo" => tr!("lhymicro.what.min_jog"),
        "la velocidad" => tr!("lhymicro.what.the_speed"),
        "una coordenada" => tr!("lhymicro.what.a_coordinate"),
        "la potencia" => tr!("lhymicro.what.the_power"),
        "el trabajo se codificó para otra placa" => tr!("lhymicro.what.other_board"),
        "placa lista" => tr!("lhymicro.what.board_ready"),
        "confirmación del paquete" => tr!("lhymicro.what.packet_confirm"),
        "fin del movimiento" => tr!("lhymicro.what.end_of_motion"),
        other => other.to_string(),
    }
}
