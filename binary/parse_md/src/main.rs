use std::fs::File;
use std::io::Write;
use std::sync::Arc;
use std::path::Path;
use anyhow::anyhow;
use clap::Parser as CliParser;
use serve_md_core::determine;
use serve_md_core::state::State as Cli;
use core::result::Result;

fn main() -> Result<(), anyhow::Error> {
    let mut cli = Cli::parse();
    cli.load_config();
    cli.set_missing();

    #[cfg(debug_assertions)]
    dbg!(&cli);

    let state = Arc::new(cli);

    // TODO consider creating/exposing `generate_payload_from_{file, buffer, string}`
    // if I want to support stdin -> stdout, temporary input files would be needed...
    // as the lib relies on file extensions to work.
    if let Some(p) = state.file.as_ref() {
        match determine(p.to_string(), state.clone()) {
            Result::Ok(payload) => {
                let exists:Option<(&Path, bool)> = state.output.as_ref()
                    .map(|s| Path::new(s))
                    .map(|p| (p, p.exists()));

                if let Result::Ok(mut writer) = aquire_output(exists) {
                    match writer.write(&payload[..]) {
                        Result::Ok(_) => {
                            return Result::Ok(())
                        },
                        Err(e) => {
                            return Err(e.into())
                        }
                    }
                }

            }
            Err(e) => {
                return Err(e)
            }
        }
    }
    Err(anyhow!("No input detected from either -i or --file."))
}

fn aquire_output(output:Option<(&Path, bool)>) -> Result<Box<dyn Write>, anyhow::Error> {
    match output {
        None => {
            Result::Ok(Box::new(std::io::stdout()))
        }
        Some((path, true)) => {
            match File::open(path) {
                Result::Ok(file) => {
                    Result::Ok(Box::new(file))
                }
                Err(e) => {
                    Err(e.into())
                }
            }
        }
        Some((path, false)) => {
            match File::create(path) {
                Result::Ok(file) => {
                    Result::Ok(Box::new(file))
                }
                Err(e) => {
                    Err(e.into())
                }
            }
        }
    }
}