
use crate::formats::*;
use anyhow::anyhow;
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
    // --- Http server options.
    /// Set the root directory to serve .md files from
    #[arg(long)]
    pub root:Option<String>,
    /// The port to bind the serve_md server too
    #[arg(long, default_value_t = 8083)]
    pub port:u16,
    
    // --- Markdown options.
    /// Enables parsing tables
    #[arg(short, long)]
    pub tables:bool,
    /// Enables parsing footnotes
    #[arg(short, long)]
    pub footnotes:bool,
    /// Enables parsing strikethrough
    #[arg(short, long)]
    pub strikethrough:bool,
    /// Enables parsing tasklists
    #[arg(short = 'l', long)]
    pub tasklists:bool,
    /// Enables smart punctuation
    #[arg(short = 'p', long)]
    pub smart_punctuation:bool,
    /// Enables header attributes
    #[arg(short = 'a', long)]
    pub header_attributes:bool,
    /// The type of front matter
    #[arg(short = 'm', long, value_enum)]
    pub front_matter:Option<MatterFormats>,

    // --- Plugin options.
    /// Enables parsing emoji shortcodes, using GitHub flavoured shortcodes
    #[arg(short, long)]
    pub emoji_shortcodes:bool,
    /// Enables converting headers into collapsible sections using the <details> element
    #[arg(short = 'k', long, value_parser = parse_collapsible_headers)]
    pub collapsible_headers:Option<(u8, String)>,

    // ---
    /// Use a configuration file instead
    #[arg(short, long)]
    #[serde(skip)]
    config:Option<String>,
}

// @see https://github.com/clap-rs/clap/blob/7f8df272d90afde89e40de086492e1c9f5749897/examples/typed-derive.rs#L24
fn parse_collapsible_headers(s: &str) -> Result<(u8, String), Box<dyn std::error::Error + Send + Sync + 'static>> {
    #[cfg(debug_assertions)]
    dbg!(s);
    assert!(s.len() > 2);
    let mut level = 0;
    let mut iter = s.chars();
    if let (Some('h'), Some(b)) = (iter.next(), iter.next()) {
        if let Some(digit) = b.to_digit(10) {
            level = digit as u8;
        }
        match iter.next() {
            Some(':' | '=') => {},
            Some(_) => {
                return Err(anyhow!("Third character after `h{}` must be a colon `:` or equals sign `=`.", level).into());
            },
            None => {}
        }
    } else {
        iter = s.chars();
    }
    
    Ok((level, iter.as_str().to_string()))
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
                        if let Ok(ncli) = State::try_from((buf.as_str(), valid_ext)) {
                            *self = ncli;
                        }
                        #[cfg(debug_assertions)]
                        dbg!(toml::to_string_pretty(&self).ok());
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