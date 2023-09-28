use clap::ValueEnum;
use core::fmt::Display;
use std::convert::{TryInto, TryFrom};
use crate::matter::RefDefMatter;
use anyhow::{Error, Result, anyhow};
use serde_derive::{Deserialize, Serialize};
use gray_matter::{Pod, ParsedEntity, Matter as GrayMatter, engine::{YAML, JSON, TOML}};

#[repr(u8)]
pub enum Config {
    Json = Generic::Json as u8,
    Yaml = Generic::Yaml as u8,
    Toml = Generic::Toml as u8,
}

impl TryFrom<&str> for Config {
    type Error = anyhow::Error;
    fn try_from(value: &str) -> core::result::Result<Self, Self::Error> {
        match value {
            "json"      => Ok(Config::Json),
            "toml"      => Ok(Config::Toml),
            "yaml"      => Ok(Config::Yaml),
            x           => Err(anyhow!("{x} extension not supported. Use one of json, toml or yaml.")),
        }
    }
}

#[repr(u8)]
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, ValueEnum, Deserialize, Serialize)]
pub enum Matter {
    Refdef = 0,
    Json = Generic::Json as u8,
    Yaml = Generic::Yaml as u8,
    Toml = Generic::Toml as u8,
}

impl Display for Matter {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if self == & Matter::Refdef { write!(f, "refdef") } else {
            let x:Result<Generic, _> = self.try_into();
            match x {
                Ok(gf) => {
                    gf.fmt(f)
                },
                Err(_) => {
                    Err(core::fmt::Error{})
                }
            }
        }
    }
}

impl Matter {
    fn as_matter(self, input: &str) -> Option<ParsedEntity> {
        match self {
            Matter::Json => Some(GrayMatter::<JSON>::new().parse(input)),
            Matter::Toml => Some(GrayMatter::<TOML>::new().parse(input)),
            Matter::Yaml => Some(GrayMatter::<YAML>::new().parse(input)),
            Matter::Refdef => None,
        }
    }

    pub fn as_pod(self, input: &str) -> Option<(Pod, Vec<u8>)> {
        let pod = if let Some(matter) = self.as_matter(input) {
            let buf = matter.content.as_bytes().to_vec();
            matter.data.map(move |p| (p.clone(), buf))
        } else {
            let buf = &input.as_bytes();
            let mut refdef = RefDefMatter::new(buf);
            refdef.scan();
            refdef.parse_gray_matter().map(|p| (p, buf.to_vec()))
        };
        
        pod
    }
}

#[repr(u8)]
#[derive(Debug, PartialEq, Eq)]
pub enum Payload {
    Html     = 1,
    Markdown = 2,
    Json     = Generic::Json as u8,
    Yaml     = Generic::Yaml as u8,
    Toml     = Generic::Toml as u8,
    Csv      = Generic::Csv as u8,
    Pickle   = Generic::Pickle as u8,
    Postcard = Generic::Postcard as u8,
    Cbor     = Generic::Cbor as u8,
}

impl Display for Payload {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Payload::Html     => write!(f, "html"),
            Payload::Markdown => write!(f, "md"),
            _ => {
                let x:Result<Generic, _> = self.try_into();
                match x {
                    Ok(gf) => {
                        gf.fmt(f)
                    },
                    Err(_) => {
                        Err(core::fmt::Error{})
                    }
                }
            }
        }
    }
}

impl TryFrom<&str> for Payload {
    type Error = Error;
    fn try_from(value: &str) -> core::result::Result<Self, Self::Error> {
        match value {
            "json"      => Ok(Payload::Json),
            "toml"      => Ok(Payload::Toml),
            "yaml"      => Ok(Payload::Yaml),
            "html"      => Ok(Payload::Html),
            "md"        => Ok(Payload::Markdown),
            "pickle"    => Ok(Payload::Pickle),
            "cbor"      => Ok(Payload::Cbor),
            "csv"       => Ok(Payload::Csv),
            "postcard"  => Ok(Payload::Postcard),
            x           => Err(anyhow!("{} extension not supported.", x)),
        }
    }
}

#[repr(u8)]
#[derive(Debug, PartialEq, Eq)]
pub enum Generic {
    Json = 3,
    Yaml,
    Toml,
    Csv,
    Pickle,
    Postcard,
    Cbor,
}

impl Display for Generic {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", match self {
            Generic::Json     => "json",
            Generic::Yaml     => "yaml",
            Generic::Toml     => "toml",
            Generic::Csv      => "csv",
            Generic::Pickle   => "pickle",
            Generic::Postcard => "postcard",
            Generic::Cbor     => "cbor",
        })
    }
}

impl TryFrom<&u8> for Generic {
    type Error = anyhow::Error;
    fn try_from(value: &u8) -> core::result::Result<Self, Self::Error> {
        match value {
            x if x == &(Generic::Json      as u8) => Ok(Generic::Json),
            x if x == &(Generic::Yaml      as u8) => Ok(Generic::Yaml),
            x if x == &(Generic::Toml      as u8) => Ok(Generic::Toml),
            x if x == &(Generic::Csv       as u8) => Ok(Generic::Csv),
            x if x == &(Generic::Pickle    as u8) => Ok(Generic::Pickle),
            x if x == &(Generic::Postcard  as u8) => Ok(Generic::Postcard),
            x if x == &(Generic::Cbor      as u8) => Ok(Generic::Cbor),
            x => Err(anyhow!("{} is not recognised as a Generic format.", x))
        }
    }
}

impl TryFrom<&Payload> for Generic {
    type Error = anyhow::Error;
    fn try_from(value: &Payload) -> core::result::Result<Self, Self::Error> {
        match value {
            Payload::Html | Payload::Markdown => Err(anyhow!("{} is not a Generic format.", value)),
            Payload::Json     => Ok(Generic::Json),
            Payload::Yaml     => Ok(Generic::Yaml),
            Payload::Toml     => Ok(Generic::Toml),
            Payload::Csv      => Ok(Generic::Csv),
            Payload::Pickle   => Ok(Generic::Pickle),
            Payload::Postcard => Ok(Generic::Postcard),
            Payload::Cbor     => Ok(Generic::Cbor),
        }
    }
}

impl TryFrom<&Matter> for Generic {
    type Error = anyhow::Error;
    fn try_from(value: &Matter) -> core::result::Result<Self, Self::Error> {
        match value {
            Matter::Refdef => Err(anyhow!("{} is not a Generic format.", value)),
            Matter::Json     => Ok(Generic::Json),
            Matter::Yaml     => Ok(Generic::Yaml),
            Matter::Toml     => Ok(Generic::Toml),
        }
    }
}