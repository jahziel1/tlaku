//! Soporte para controladoras GRBL (láseres de diodo como la Creality Falcon2) y, con un
//! [`link::LinkProfile`] propio, para Marlin y Smoothieware.

pub mod frame;
pub mod gcode;
pub mod link;
pub mod net;
pub mod preflight;
pub mod serial;
pub mod streamer;
