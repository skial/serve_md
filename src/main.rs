#![allow(clippy::pedantic, clippy::correctness, clippy::perf, clippy::style, clippy::restriction)]

use std::{
    str, 
    env,
    fs::File, 
    io::Read, 
    sync::Arc,
    ops::Range, 
    net::SocketAddr,
    collections::HashMap, 
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
    CowStr,
    Options,
    Parser as CmParser,
    Event,
    Tag,
    html,
    HeadingLevel, 
};

mod formats;
use formats::*;
use regex::Match;
use serde::Serialize;
use gray_matter::Pod;
use serde_derive::Deserialize;
use clap::Parser as CliParser;

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
            let mut range:Option<Range<usize>> = None;
            let collection:Vec<_> = (0..).zip(&mut md_parser).collect();
            let collection = collection.as_slice();

            // This is custom to haxe roundup markdown files. 
            // TODO
            // make this redirect to external program, window size as cli option
            // and registered program returns serialized changes.
            for slice in collection.windows(4) {
                match slice {
                    [
                        (a, Event::Start(Tag::Heading(HeadingLevel::H5, _, _))), 
                        (_, Event::Start(Tag::Emphasis)),
                        (_, Event::Text(CowStr::Borrowed("in case you missed it"))),
                        (b, Event::End(Tag::Emphasis))
                    ] => {
                        finish_and_store(&mut ranges, &mut range, *b);

                        if range.is_none() {
                            range = Some(*a..*b);
                        }
                    },
                    [(idx, Event::Start(Tag::Heading(_, _, _))), _] if range.is_some() => {
                        finish_and_store(&mut ranges, &mut range, *idx+1);
                    },
                    _ => {}
                }
            }

            finish_and_store(&mut ranges, &mut range, collection.last().unwrap().0+1);

            #[cfg(debug_assertions)]
            dbg!(&ranges);

            let mut new_collection:Vec<Event<>> = Vec::with_capacity( collection.len() + (ranges.len()*3) );

            if !ranges.is_empty() {
                let Some(range) = ranges.pop() else {
                    panic!("`ranges` should not be empty at this point.");
                };

                let mut i:usize = 0;

                while i < collection.len() {
                    let pair = &collection[i];
                    if !range.contains(&pair.0) {
                        new_collection.push(pair.1.to_owned());
                        i += 1;
                        continue;
                    }

                    new_collection.push(Event::Html(CowStr::Borrowed("<details open>")));
                    new_collection.push(Event::SoftBreak);
                    new_collection.push(Event::Html(CowStr::Borrowed("<summary>")));
                    let mut iter = range
                        .clone()
                        // skip the open h5 tag
                        .skip(1);
                    new_collection.push( collection[iter.next().unwrap()].1.to_owned() );
                    new_collection.push( collection[iter.next().unwrap()].1.to_owned() );
                    new_collection.push( collection[iter.next().unwrap()].1.to_owned() );
                    new_collection.push(Event::Html(CowStr::Borrowed("</summary>")));
                    iter
                        // skip the end h5 tag
                        .skip(1)
                        .for_each(|ridx| {
                            let (_, e) = &collection[ridx];
                            new_collection.push( e.to_owned() );
                        });
                    new_collection.push(Event::Html(CowStr::Borrowed("</details>")));

                    i += range.len();

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

fn finish_and_store(rs:&mut Vec<Range<usize>>, r:&mut Option<Range<usize>>, end:usize) {
    if let Some(ref mut v) = r {
        v.end = end;
        let _tmp = v.to_owned();
        *r = None;
        rs.push(_tmp);
    }
}

#[derive(Debug, Clone)]
struct RefDefMatter<'input> {
    slice:&'input [u8],
    range:Option<Range<usize>>,
}

impl<'input> RefDefMatter<'input> {

    fn new(slice:&'input [u8]) -> RefDefMatter<'input> {
        RefDefMatter { range: None, slice, }
    }

    fn scan(&mut self) {
        // Current each refdef needs to be contained on a single line.
        // Unlike valid commonmark, no newlines can exist between any part of a refdef,
        // it has to fit entirely on a single line.
        let mut til_end:bool = false;

        for i in 0..self.slice.len() {
            match self.slice[i] {
                b'[' if !til_end => {
                    til_end = true;
                    if self.range.is_none() {
                        self.range = Some(i..0);

                    }
                },
                b'\n' => {
                    if let Some(ref mut range) = self.range {
                        range.end = i;
                    }
                    til_end = false;
                },
                _ if til_end => {
                    continue;
                },
                _x => {
                    #[cfg(debug_assertions)]
                    dbg!(i, str::from_utf8(&[_x]).unwrap());
                    break;
                }
            }
        }

        #[cfg(debug_assertions)]
        dbg!(&self.range);
        #[cfg(debug_assertions)]
        if let Some(ref mut r) = self.range {
            dbg!(str::from_utf8(&self.slice[r.start..r.end]).unwrap());

        }
    }

    fn parse_gray_matter(&'input mut self) -> Option<Pod> {
        use regex::Regex;

        if let Some(ref r) = self.range {
            // Currently confused why removing `gray_matter` & `lines`, merging
            // into a single iterator causes the inferred type to change & fail
            // type checking. What changes when assigning vs a long iter chain?
            let gray_matter = &self.slice[r.start..r.end];
            
            let lines = gray_matter
            .split(|c| c.eq(&b'\n'))
            .map( |slice| str::from_utf8(&slice) )
            .filter_map(|r| r.ok());

            // It would be nice to have regex syntax highlighting & compile time
            // checks to make sure its valid. Clippy? cargo extension?? IDE extension???
            if let Ok(re) = Regex::new(r#"\[(?<id>[^\[\]]+)\]:\s(?<uri>[^\n\r"]+)("(?<title>[^"]+)")?"#) {
                let mut map:HashMap<String, Pod> = HashMap::new();

                lines
                .filter_map(|line| {
                    if re.is_match(line) {
                        Some(re.captures_iter(line))
    
                    } else {
                        None
                    }
                })
                .flatten()
                .map( |value| {
                    let id = value.name("id");
                    let uri = value.name("uri");
                    let title = value.name("title");
                    (id, uri, title)
                } )
                .for_each(|values| {
                    let id = values.0;
                    let uri = values.1;

                    if id.is_none() || uri.is_none() {
                        return;
                    }
                    let id = id.unwrap();
                    let uri = uri.unwrap();
                    let title = values.2;
                    
                    #[cfg(debug_assertions)]
                    dbg!(id, uri, title);
                    let key = id.as_str();
                    if let Some(Pod::Array(vec)) = map.get_mut(key) {
                        vec.push(
                            Pod::Hash(RefDefMatter::regex_to_hash_entries(uri, title))
                        );

                    } else {
                        map
                        .insert(
                            key.to_string(), 
                            Pod::Array(
                                vec![Pod::Hash(RefDefMatter::regex_to_hash_entries(uri, title))]
                            )
                        );

                    }
                } );

                return Some(Pod::Hash(map))

            }
        }

        None
    }

    fn regex_to_hash_entries(uri:Match, title:Option<Match>) -> HashMap<String, Pod> {
        [
            Some(("uri".to_string(), Pod::String(uri.as_str().to_string()))),
            if title.is_some() {
                Some(("title".to_string(), title
                    .map_or(
                        Pod::Null,
                        |t| Pod::String(t.as_str().to_string())
                    )
                ))
            } else {
                None
            }
        ]
        .into_iter()
        .flatten()
        .collect::<HashMap<_, _>>()
    }
    
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