//! Lógica de la herramienta `tlaku`: conecta importadores, planificación, relleno,
//! rasterizado, simulación y GRBL para ir de un archivo de diseño a G-code y a la máquina.

pub mod cli;
pub mod job;
pub mod link;
pub mod options;
pub mod report;
pub mod ruida;
pub mod sigint;
