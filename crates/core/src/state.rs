
use anyhow::anyhow;
use std::convert::TryFrom;
use clap::Parser as CliParser;
use crate::formats::{Config, Matter};
use serde_derive::{Serialize, Deserialize};

use std::{
    str, 
    fs::File, 
    io::Read,
    ffi::OsStr,
    path::Path as SysPath, 
};

#[cfg(feature = "server")]
use std::env;

// TODO idk if its appropiate rust to use an state object as a cli/bin - dual purpose and all?
#[derive(Debug, Default, CliParser, Deserialize, Serialize)]
#[serde(default = "State::default")]
pub struct State {
    // --- Http server options.
    /// The root directory to serve .md files from
    #[cfg(feature = "server")]
    #[cfg_attr(feature = "server", arg(long))]
    pub root:Option<String>,

    /// The port to bind the serve_md server too
    #[cfg(feature = "server")]
    #[cfg_attr(feature = "server", arg(long, default_value_t = 8083))]
    pub port:u16,

    // The path to the .md file to load
    #[cfg(not(feature = "server"))]
    #[cfg_attr(not(feature = "server"), arg(short = 'i', long))]
    pub file:Option<String>,

    // The path to output too
    #[cfg(not(feature = "server"))]
    #[cfg_attr(not(feature = "server"), arg(short, long))]
    pub output:Option<String>,
    
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
    pub front_matter:Option<Matter>,

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
    let mut level = 1;
    let mut iter = s.chars();
    #[cfg(debug_assertions)]
    dbg!(&iter);
    let a = iter.next();
    let b = iter.next();
    if let (Some('h'), Some(b)) = (a, b) {
        if let Some(digit) = b.to_digit(10) {
            #[cfg(debug_assertions)]
            dbg!(digit);
            match u8::try_from(digit) {
                Ok(value) if (1..=6).contains(&value) => {
                    level = value;
                }
                Ok(value) => {
                    return Err(anyhow!("Header level {value} does not fall within 1..6.").into());
                }
                Err(error) => {
                    return Err(anyhow!(error.to_string()).into());
                }
            }
        } else {
            return Err(anyhow!("Header level is not a digit, it was {b}.").into());
        }
        match iter.next() {
            Some(':' | '=') | None => {},
            Some(_) => {
                return Err(anyhow!("The third character after `h{}` must be a colon `:` or equals sign `=`.", level).into());
            },
        }
    } else {
        iter = s.chars();
    }

    let remainder = iter.as_str().to_string();
    if remainder.is_empty() {
        return Err(anyhow!("Some text to match against is required after h{level}.").into());
    }
    
    Ok((level, remainder))
}

impl State {
    // TODO either:
    //  - return Result and handle errors
    //  - continue and use sensible defaults
    //      + implement sensible defaults
    pub fn load_config(&mut self) {
        if let Some(config) = &self.config {
            let path = SysPath::new(&config);
            let mut buf = String::new();
            let possible_state = path.extension()
            .and_then(OsStr::to_str)
            .ok_or_else(|| anyhow!("Unable to convert the path {} which is of type `OsStr`, to `&str`.", path.display()))
            .and_then(Config::try_from)
            .and_then(|ext| {
                if path.exists() {
                    File::open(path)
                    .and_then(|mut file| file.read_to_string(&mut buf))
                    .map_err(|error| anyhow!(error.to_string()))
                    .and_then(|_| State::try_from((buf.as_str(), ext)) )
                } else {
                    Err(anyhow!("{} does not exist. Continuing with defaults.", path.display()))
                }
            });
            // TODO consider returning the `Result<T, E>` object instead of handling it.
            match possible_state {
                Ok(state) => {
                    *self = state;
                    #[cfg(debug_assertions)]
                    dbg!(toml::to_string_pretty(&self).ok());
                }
                Err(error) => {
                    #[cfg(debug_assertions)]
                    dbg!(error);
                }
            }
        }
    }

    #[cfg(feature = "server")]
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

    #[cfg(not(feature = "server"))]
    pub fn set_missing(&mut self) {
        
    }
}

impl TryFrom<(&str, Config)> for State {
    type Error = anyhow::Error;
    fn try_from(value: (&str, Config)) -> core::result::Result<Self, Self::Error> {
        match value.1 {
            Config::Json => Ok(serde_json::from_str(value.0)?),
            Config::Toml => Ok(toml::from_str(value.0)?),
            Config::Yaml => Ok(serde_yaml::from_str(value.0)?),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::parse_collapsible_headers;

    #[test]
    fn pch_test_ascii_digits() {
        let mut results = vec![];
        for char in '0'..='9' {
            let value = format!("h{char}:other");
            dbg!(&value);
            results.push( parse_collapsible_headers(&value) );
        }
        dbg!(&results);
        assert_eq!(results.len(), 10);
        for i in 0..=9 {
            match &results[i] {
                Err(e) => if i == 0 || i > 6 {
                    let ex = format!("Header level {i} does not fall within 1..6.");
                    let msg = e.to_string();
                    assert!( msg.contains(&ex) );
                }
                Ok(v) => {
                    assert_eq!(v.0, i as u8);
                    assert_eq!(v.1, "other");
                }
            }
        }
        
    }

    #[test]
    fn pch_test_non_ascii_digits() {
        let mut results = vec![];
        let values = ['a', 'b', 'c'];
        for char in values {
            let value = format!("h{char}:other");
            dbg!(&value);
            results.push( parse_collapsible_headers(&value) );
        }
        dbg!(&results);
        assert_eq!(results.len(), 3);
        for i in 0..3 {
            match &results[i] {
                Err(e) => {
                    let ex = format!("Header level is not a digit, it was {}.", values[i]);
                    let msg = e.to_string();
                    assert!( msg.contains(&ex) );
                }
                Ok(v) => {
                    assert_eq!(v.0, i as u8);
                    assert_eq!(v.1, "other");
                }
            }
        }
    }

    #[test]
    fn pch_test_separator() {
        let mut results = vec![];
        let values = ["h1:other", "h2=other", "h3,other"];
        for value in values {
            results.push( parse_collapsible_headers(value) );
        }
        dbg!(&results);
        assert_eq!(results.len(), 3);
        for i in 0..3 {
            match &results[i] {
                Ok(v) => {
                    assert_eq!(v.0, (i+1) as u8);
                    assert_eq!(v.1, "other");
                }
                Err(e) => {
                    let ex = format!("The third character after `h{}` must be a colon `:` or equals sign `=`.", (i+1));
                    let msg = e.to_string();
                    assert!( msg.contains(&ex) );
                }
            }
        }
    }

    #[test]
    fn pch_test_for_missing_text() {
        let mut results = vec![];
        let values = ["h1:other", "h2:", "h3"];
        for value in values {
            results.push( parse_collapsible_headers(value) );
        }
        dbg!(&results);
        assert_eq!(results.len(), 3);
        for i in 0..3 {
            match &results[i] {
                Ok(v) => {
                    assert_eq!(v.0, (i+1) as u8);
                    assert_eq!(v.1, "other");
                }
                Err(e) => {
                    let ex = format!("Some text to match against is required after h{}.", (i+1));
                    let msg = e.to_string();
                    assert!( msg.contains(&ex) );
                }
            }
        }
    }
}