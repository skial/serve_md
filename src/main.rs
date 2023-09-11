#![allow(clippy::pedantic, clippy::correctness, clippy::perf, clippy::style, clippy::restriction)]

mod matter;
mod formats;
mod plugin;

use std::{
    str, 
    env,
    fs::File, 
    io::Read, 
    sync::Arc,
    ops::Range, 
    net::SocketAddr,
    path::Path as SysPath, 
};

use axum::{ 
    Router,
    routing::get, 
    extract::Path, 
    http::StatusCode, 
    response::{Html, IntoResponse, Response, Result},
};

use pulldown_cmark::{
    html,
    Event,
    Options,
    Parser as CmParser,
};

use formats::*;
use serde::Serialize;
use gray_matter::Pod;
use serde_derive::Deserialize;
use clap::Parser as CliParser;

use crate::plugin::{HaxeRoundup, Plugin};

#[derive(Debug, Default, CliParser, Deserialize, Serialize)]
#[serde(default = "Cli::default")]
struct Cli {
    /// Set the root directory to search & serve .md files from.
    #[arg(long)]
    root:Option<String>,
    /// The port to bind the serve_md server too.
    #[arg(long, default_value_t = 8083)]
    port:u16,
    
    /// Enables parsing tables.
    #[arg(short, long)]
    tables:bool,
    /// Enables parsing footnotes.
    #[arg(short, long)]
    footnotes:bool,
    /// Enables parsing strikethrough.
    #[arg(short, long)]
    strikethrough:bool,
    /// Enables parsing tasklists.
    #[arg(short = 'l', long)]
    tasklists:bool,
    /// Enables smart punctuation.
    #[arg(short = 'p', long)]
    smart_punctuation:bool,
    /// Enables header attributes.
    #[arg(short = 'a', long)]
    header_attributes:bool,

    /// The type of front matter.
    #[arg(short = 'm', long, value_enum)]
    front_matter:Option<MatterFormats>,

    /// Use a configuration file instead.
    #[arg(short, long)]
    #[serde(skip)]
    config:Option<String>,
}

impl Cli {
    // TODO either:
    //  - return Result and handle errors
    //  - continue and use sensible defaults
    //      + implement sensible defaults
    fn load_config(&mut self) {
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
                        match Cli::try_from((buf.as_str(), valid_ext)) {
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
    fn set_missing(&mut self) {
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

impl TryFrom<(&str, ConfigFormats)> for Cli {
    type Error = anyhow::Error;
    fn try_from(value: (&str, ConfigFormats)) -> std::result::Result<Self, Self::Error> {
        match value.1 {
            ConfigFormats::Json => Ok(serde_json::from_str(value.0)?),
            ConfigFormats::Toml => Ok(toml::from_str(value.0)?),
            ConfigFormats::Yaml => Ok(serde_yaml::from_str(value.0)?),
        }
    }
}

#[tokio::main]
async fn main() {
    let mut cli = Cli::parse();
    cli.load_config();
    cli.set_missing();

    #[cfg(debug_assertions)]
    dbg!(&cli);

    let state = Arc::new(cli);
    
    // As far as I can tell, axum can't match paths with
    // file extensions? `:file.html` or `:file.md`.
    let routes = Router::new()
        .route("/", get(root))
        .route("/:path", get({
            let shared_state = Arc::clone(&state);
            move |path| determine(path, shared_state)
        }))
    ;

    let addr = SocketAddr::from(([127, 0, 0, 1], state.port));
    axum::Server::bind(&addr)
        .serve(routes.into_make_service())
        .await
        .unwrap();
}

async fn root() -> &'static str {
    "hello world _"
}

async fn determine(Path(path):Path<String>, state:Arc<Cli>) -> Result<Response> {
    #[cfg(debug_assertions)]
    dbg!(&path);
    
    let path_ext = SysPath::new(&path).extension();
    let extension = path_ext
    .and_then(|s| s.to_str())
    .and_then(|s| PayloadFormats::try_from(s).ok());

    if let Some(ref extension) = extension {
        let path = path.replace(&(".".to_owned() + &extension.to_string()), ".md");
        // Handle commonmark requests early
        if extension == &PayloadFormats::Markdown {
            return fetch_md(&path)
            .and_then(|v| {
                str::from_utf8(&v)
                .or(Err(StatusCode::BAD_REQUEST.into()))
                .map(|s| s.to_string())
            })
            .map(|s| s.into_response())

        }
        return generate_payload(path, state)?.into_response_for(&extension);

    }
    Err(StatusCode::BAD_REQUEST.into())
}

fn fetch_md(path:&String) -> Result<Vec<u8>> {
    let path = SysPath::new(&path);
    if path.exists() {
        let mut file = File::open(path).map_err(|_| StatusCode::NOT_FOUND)?;
        let mut buf = vec![];
        let _ = file.read_to_end(&mut buf);
        return Ok(buf);
    }

    Err(StatusCode::NOT_FOUND.into())
}

fn generate_payload(path:String, state:Arc<Cli>) -> Result<Payload> {
    if SysPath::new(&path).exists() {
        let mut input = fetch_md(&path)?;
        let mut pod:Pod = Pod::String("".to_owned());

        if let Some(fm) = state.front_matter {
            let op = str::from_utf8(&input[..]).ok().and_then(|s| fm.as_pod(s));
            if let Some((p, v)) = op {
                pod = p;
                input = v;
            }
        }

        let buf = &input[..];

        let mut md_opt = Options::empty();
        if state.tables {
            md_opt.insert(Options::ENABLE_TABLES);
        }
        if state.footnotes {
            md_opt.insert(Options::ENABLE_FOOTNOTES);
        }
        if state.strikethrough {
            md_opt.insert(Options::ENABLE_STRIKETHROUGH);
        }
        if state.smart_punctuation {
            md_opt.insert(Options::ENABLE_SMART_PUNCTUATION);
        }
        if state.header_attributes {
            md_opt.insert(Options::ENABLE_HEADING_ATTRIBUTES);
        }

        return if let Ok(s) = str::from_utf8(&buf) {
            let mut md_parser = CmParser::new_ext(s, md_opt);

            let mut ranges:Vec<Range<usize>> = Vec::new();
            let collection:Vec<_> = (0..).zip(&mut md_parser).collect();
            let collection = collection.as_slice();
            let mut plugin = plugin::HaxeRoundup::default();

            for slice in collection.windows(HaxeRoundup::window_size()) {
                match plugin.check_slice(slice) {
                    Some(range) => {
                        ranges.push(range);
                    },
                    None => {

                    }
                }
            }

            if let Some(range) = plugin.finalize(collection.last().unwrap().0) {
                ranges.push(range);
            }

            #[cfg(debug_assertions)]
            dbg!(&ranges);

            let mut range_idx: usize = 0;
            let mut new_collection:Vec<Event<>> = Vec::with_capacity( collection.len() + (ranges.len() * HaxeRoundup::new_items()) );

            if !ranges.is_empty() {
                debug_assert!( ranges.len() > 0 );

                let mut i:usize = 0;

                while i < collection.len() {
                    if let Some(range) = ranges.get(range_idx) {
                        let pair = &collection[i];
                        if !range.contains(&pair.0) {
                            new_collection.push(pair.1.to_owned());
                            i += 1;
                            continue;
                        }

                        new_collection.extend_from_slice( &plugin.replace_slice(&collection[range.clone()]) );

                        i += range.len();
                        range_idx += 1;
                    } else {
                        i += 1;
                        continue;
                    }

                }
            } else {
                new_collection.extend(collection.iter().map(|c| c.1.to_owned()));

            }

            assert!(new_collection.len() > 0);
            let mut html_output = String::new();
            html::push_html(&mut html_output,  new_collection.into_iter());

            // TODO consider merging other found refdefs into map, if possible at all.
            /*for i in md_parser.reference_definitions().iter() {
                println!("{:?}", i);
            }*/

            Ok(Payload { html: html_output, front_matter:pod.into() })

        } else {
            // Utf8Error
            Err(StatusCode::NO_CONTENT.into())
        }

    }

    Err(StatusCode::NOT_FOUND.into())

}

#[derive(Serialize, Deserialize, Debug)]
struct Payload {
    front_matter:serde_json::Value,
    html:String,
}

impl Payload {
    fn into_response_for(self, extension:&PayloadFormats) -> Result<Response> {
        match extension {
            PayloadFormats::Html => {
                return Ok(Html(self.html).into_response())
            }
            PayloadFormats::Json => {
                if let Ok(json) = serde_json::to_string_pretty(&self) {
                    return Ok(json.into_response())
                }
            }
            PayloadFormats::Yaml => {
                if let Ok(yaml) = serde_yaml::to_string(&self) {
                    return Ok(yaml.into_response())
                }
            }
            PayloadFormats::Toml => {
                if let Ok(toml) = toml::to_string_pretty(&self) {
                    return Ok(toml.into_response())
                }
            }
            PayloadFormats::Pickle => {
                if let Ok(pickle) = serde_pickle::to_vec(&self, Default::default()) {
                    return Ok(pickle.into_response())
                }
            }
            _ => {
                return Err(StatusCode::BAD_REQUEST.into())
            }
        }
        Err(StatusCode::BAD_REQUEST.into())
    }
}