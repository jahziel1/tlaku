use clap::Parser;
use std::process::ExitCode;
use tlaku_cli::cli::{Cli, error_line, run};

fn main() -> ExitCode {
    // Idioma de los avisos y errores del motor: `TLAKU_LANG`, luego `LANG`; sin ninguno, español.
    tlaku_i18n::init_from_env();
    match run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            // Un error con código (`[bed_overflow] ...`) no lo necesita quien lee la terminal.
            eprintln!("{}", error_line(&e));
            ExitCode::FAILURE
        }
    }
}
