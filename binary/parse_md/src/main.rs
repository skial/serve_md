use std::ffi::OsStr;
use std::fs::File;
use std::io::Write;
use std::sync::Arc;
use std::path::Path;
use anyhow::anyhow;
use clap::Parser as CliParser;
use serve_md_core::generate_payload_from_path;
use serve_md_core::formats::Payload as PayloadFormats;
use serve_md_core::state::State as Cli;
use anyhow::Result;

fn main() -> Result<()> {
    let mut cli = Cli::parse();
    cli.load_config();
    cli.set_missing();

    #[cfg(debug_assertions)]
    dbg!(&cli);

    let state = Arc::new(cli);

    if let Some(p) = state.file.as_ref() {
        let context: Option<(&Path, bool)> = state.output.as_ref()
            .map(Path::new)
            .map(|p| (p, p.exists()));
        let ext = context
            .and_then(|(p, _)| p.extension())
            .and_then(OsStr::to_str)
            .and_then(|s| PayloadFormats::try_from(s).ok())
            .or(Some(PayloadFormats::Html))
            ;
        let input = Path::new(p);
        
        let res = generate_payload_from_path(input, Arc::clone(&state))
            .and_then(|payload| payload.into_response_for(&ext.unwrap()))
            .and_then(|payload| 
            aquire_output(context).map(|writer| (payload, writer))
        );

        match res {
            Result::Ok((payload, mut writer)) => {
                match writer.write(&payload[..]) {
                    Result::Ok(_) => {
                        return Result::Ok(())
                    },
                    Err(e) => {
                        return Err(e.into())
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
            match File::options().write(true).open(path) {
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