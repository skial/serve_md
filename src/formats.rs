use clap::ValueEnum;
use core::fmt::Display;
use crate::matter::RefDefMatter;
use anyhow::{Error, Result, anyhow};
use serde_derive::{Deserialize, Serialize};
use gray_matter::{Pod, ParsedEntity, Matter, engine::{YAML, JSON, TOML}};

#[repr(u8)]
pub enum ConfigFormats {
    Json = GenericFormats::Json as u8,
    Yaml = GenericFormats::Yaml as u8,
    Toml = GenericFormats::Toml as u8,
}

impl TryFrom<&str> for ConfigFormats {
    type Error = anyhow::Error;
    fn try_from(value: &str) -> core::result::Result<Self, Self::Error> {
        match value {
            "json"      => Ok(ConfigFormats::Json),
            "toml"      => Ok(ConfigFormats::Toml),
            "yaml"      => Ok(ConfigFormats::Yaml),
            x           => Err(anyhow!("{x} extension not supported. Use one of json, toml or yaml.")),
        }
    }
}

#[repr(u8)]
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, ValueEnum, Deserialize, Serialize)]
pub enum MatterFormats {
    Refdef = 0,
    Json = GenericFormats::Json as u8,
    Yaml = GenericFormats::Yaml as u8,
    Toml = GenericFormats::Toml as u8,
}

impl Display for MatterFormats {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if self == & MatterFormats::Refdef { write!(f, "refdef") } else {
            let x:Result<GenericFormats, _> = self.try_into();
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

impl MatterFormats {
    fn as_matter(self, input: &str) -> Option<ParsedEntity> {
        match self {
            MatterFormats::Json => Some(Matter::<JSON>::new().parse(input)),
            MatterFormats::Toml => Some(Matter::<TOML>::new().parse(input)),
            MatterFormats::Yaml => Some(Matter::<YAML>::new().parse(input)),
            MatterFormats::Refdef => None,
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
pub enum PayloadFormats {
    Html     = 1,
    Markdown = 2,
    Json     = GenericFormats::Json as u8,
    Yaml     = GenericFormats::Yaml as u8,
    Toml     = GenericFormats::Toml as u8,
    Csv      = GenericFormats::Csv as u8,
    Pickle   = GenericFormats::Pickle as u8,
    Postcard = GenericFormats::Postcard as u8,
    Cbor     = GenericFormats::Cbor as u8,
}

impl Display for PayloadFormats {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            PayloadFormats::Html     => write!(f, "html"),
            PayloadFormats::Markdown => write!(f, "md"),
            _ => {
                let x:Result<GenericFormats, _> = self.try_into();
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

impl TryFrom<&str> for PayloadFormats {
    type Error = Error;
    fn try_from(value: &str) -> core::result::Result<Self, Self::Error> {
        match value {
            "json"      => Ok(PayloadFormats::Json),
            "toml"      => Ok(PayloadFormats::Toml),
            "yaml"      => Ok(PayloadFormats::Yaml),
            "html"      => Ok(PayloadFormats::Html),
            "md"        => Ok(PayloadFormats::Markdown),
            "pickle"    => Ok(PayloadFormats::Pickle),
            "cbor"      => Ok(PayloadFormats::Cbor),
            "csv"       => Ok(PayloadFormats::Csv),
            "postcard"  => Ok(PayloadFormats::Postcard),
            x           => Err(anyhow!("{} extension not supported.", x)),
        }
    }
}

#[repr(u8)]
#[derive(Debug, PartialEq, Eq)]
pub enum GenericFormats {
    Json = 3,
    Yaml,
    Toml,
    Csv,
    Pickle,
    Postcard,
    Cbor,
}

impl Display for GenericFormats {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", match self {
            GenericFormats::Json     => "json",
            GenericFormats::Yaml     => "yaml",
            GenericFormats::Toml     => "toml",
            GenericFormats::Csv      => "csv",
            GenericFormats::Pickle   => "pickle",
            GenericFormats::Postcard => "postcard",
            GenericFormats::Cbor     => "cbor",
        })
    }
}

impl TryFrom<&u8> for GenericFormats {
    type Error = anyhow::Error;
    fn try_from(value: &u8) -> core::result::Result<Self, Self::Error> {
        match value {
            x if x == &(GenericFormats::Json      as u8) => Ok(GenericFormats::Json),
            x if x == &(GenericFormats::Yaml      as u8) => Ok(GenericFormats::Yaml),
            x if x == &(GenericFormats::Toml      as u8) => Ok(GenericFormats::Toml),
            x if x == &(GenericFormats::Csv       as u8) => Ok(GenericFormats::Csv),
            x if x == &(GenericFormats::Pickle    as u8) => Ok(GenericFormats::Pickle),
            x if x == &(GenericFormats::Postcard  as u8) => Ok(GenericFormats::Postcard),
            x if x == &(GenericFormats::Cbor      as u8) => Ok(GenericFormats::Cbor),
            x => Err(anyhow!("{} is not recognised as a GenericFormat.", x))
        }
    }
}

impl TryFrom<&PayloadFormats> for GenericFormats {
    type Error = anyhow::Error;
    fn try_from(value: &PayloadFormats) -> core::result::Result<Self, Self::Error> {
        match value {
            PayloadFormats::Html | PayloadFormats::Markdown => Err(anyhow!("{} is not a GenericFormat.", value)),
            PayloadFormats::Json     => Ok(GenericFormats::Json),
            PayloadFormats::Yaml     => Ok(GenericFormats::Yaml),
            PayloadFormats::Toml     => Ok(GenericFormats::Toml),
            PayloadFormats::Csv      => Ok(GenericFormats::Csv),
            PayloadFormats::Pickle   => Ok(GenericFormats::Pickle),
            PayloadFormats::Postcard => Ok(GenericFormats::Postcard),
            PayloadFormats::Cbor     => Ok(GenericFormats::Cbor),
        }
    }
}

impl TryFrom<&MatterFormats> for GenericFormats {
    type Error = anyhow::Error;
    fn try_from(value: &MatterFormats) -> core::result::Result<Self, Self::Error> {
        match value {
            MatterFormats::Refdef => Err(anyhow!("{} is not a GenericFormat.", value)),
            MatterFormats::Json     => Ok(GenericFormats::Json),
            MatterFormats::Yaml     => Ok(GenericFormats::Yaml),
            MatterFormats::Toml     => Ok(GenericFormats::Toml),
        }
    }
}