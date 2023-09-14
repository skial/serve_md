
use crate::formats::*;
use clap::Parser as CliParser;
use serde_derive::{Serialize, Deserialize};

use std::{
    str, 
    env,
    fs::File, 
    io::Read,
    path::Path as SysPath, 
};

// TODO idk if its appropiate rust to use an state object as a cli/bin - dual purpose and all?
#[derive(Debug, Default, CliParser, Deserialize, Serialize)]
#[serde(default = "State::default")]
pub struct State {
    /// Set the root directory to search & serve .md files from.
    #[arg(long)]
    pub root:Option<String>,
    /// The port to bind the serve_md server too.
    #[arg(long, default_value_t = 8083)]
    pub port:u16,
    
    /// Enables parsing tables.
    #[arg(short, long)]
    pub tables:bool,
    /// Enables parsing footnotes.
    #[arg(short, long)]
    pub footnotes:bool,
    /// Enables parsing strikethrough.
    #[arg(short, long)]
    pub strikethrough:bool,
    /// Enables parsing tasklists.
    #[arg(short = 'l', long)]
    pub tasklists:bool,
    /// Enables smart punctuation.
    #[arg(short = 'p', long)]
    pub smart_punctuation:bool,
    /// Enables header attributes.
    #[arg(short = 'a', long)]
    pub header_attributes:bool,

    /// The type of front matter.
    #[arg(short = 'm', long, value_enum)]
    pub front_matter:Option<MatterFormats>,

    /// Use a configuration file instead.
    #[arg(short, long)]
    #[serde(skip)]
    config:Option<String>,
}

impl State {
    // TODO either:
    //  - return Result and handle errors
    //  - continue and use sensible defaults
    //      + implement sensible defaults
    pub fn load_config(&mut self) {
        if let Some(config) = &self.config {
            let path = SysPath::new(&config);
            let valid_ext = path.extension()
            .and_then(|s| s.to_str())
            .and_then(|s| ConfigFormats::try_from(s).ok());
            match valid_ext {
                Some(valid_ext) if path.exists() => {
                    if let Ok(mut file) = File::open(path) {
                        let mut buf = String::new();
                        let _ = file.read_to_string(&mut buf);
                        match State::try_from((buf.as_str(), valid_ext)) {
                            Ok(ncli) => *self = ncli,
                            _ => {}
                        }
                    }
                }
                Some(_) if !path.exists() => {
                    println!("The file {} does not exist. Continuing with defaults.", path.display())
                }
                Some(_) | None => {
                    println!("Invalid value passed into --config. Make sure the file type is one of .json, .yaml or .toml. Continuing with defaults.")
                }
            }
        }
    }

    // TODO rename to sensible defaults?
    pub fn set_missing(&mut self) {
        if self.port == 0 {
            self.port = 8083;
        }
        if self.root.is_none() {
            if let Ok(path) = env::current_dir() {
                if let Some(path) = path.to_str() {
                    self.root = Some(path.to_string());
                }
            }
        }
    }
}

impl TryFrom<(&str, ConfigFormats)> for State {
    type Error = anyhow::Error;
    fn try_from(value: (&str, ConfigFormats)) -> std::result::Result<Self, Self::Error> {
        match value.1 {
            ConfigFormats::Json => Ok(serde_json::from_str(value.0)?),
            ConfigFormats::Toml => Ok(toml::from_str(value.0)?),
            ConfigFormats::Yaml => Ok(serde_yaml::from_str(value.0)?),
        }
    }
}