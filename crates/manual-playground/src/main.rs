#![forbid(unsafe_code)]

mod server;
mod state;

use server::run;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    let bind = bind_argument()?;
    run(&bind)
}

fn bind_argument() -> Result<String, Box<dyn Error + Send + Sync>> {
    let mut arguments = std::env::args().skip(1);
    let mut bind = "127.0.0.1:8787".to_owned();
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--bind" => {
                bind = arguments.next().ok_or("--bind requires an address")?;
            }
            "--help" | "-h" => {
                println!("manual-playground [--bind ADDRESS]");
                std::process::exit(0);
            }
            _ => return Err(format!("unknown argument: {argument}").into()),
        }
    }
    Ok(bind)
}
