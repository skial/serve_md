#![allow(clippy::pedantic, clippy::correctness, clippy::perf, clippy::style, clippy::restriction)]

use std::{
    str, 
    env,
    sync::Arc,
    fs::File, 
    io::Read, 
    ops::Range, 
    net::SocketAddr,
    path::Path as SysPath, 
    collections::HashMap,
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

use serde::Serialize;
//use refmt_serde::{Format, Refmt};
use regex::Match;
use clap::{Parser as CliParser, ValueEnum};
use gray_matter::{Pod, ParsedEntity, Matter, engine::{YAML, JSON, TOML}};
use serde_derive::Deserialize;

#[derive(CliParser, Debug)]
struct Cli {
    /// Set the root directory to search & serve .md files from.
    #[arg(long)]
    dir:Option<String>,
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
    front_matter:Option<FrontMatter>,

    // TODO allow `config.{toml,json,yaml}` parsed to load the exact values above?
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum, Debug)]
enum FrontMatter {
    Json,
    Toml,
    Yaml,
    Refdef, // probably a better name can be thought of
}

#[derive(Debug, PartialEq, Eq)]
enum PayloadFormat {
    Json,
    Toml,
    Yaml,
    Html,
    Markdown,
    Pickle,
}

#[tokio::main]
async fn main() {
    let mut cli = Cli::parse();
    if cli.dir.is_none() {
        if let Ok(path) = env::current_dir() {
            if let Some(path) = path.to_str() {
                cli.dir = Some(path.to_string());
            }
        }
    }

    println!("{:?}", cli);

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
    println!("{:?}", path);
    
    let mut path = path;
    let path_ext = SysPath::new(&path).extension();
    let extension:Option<PayloadFormat>;
    match path_ext {
        Some(x) if (x == "json") => {
            path = path.replace(".json", ".md");
            extension = Some(PayloadFormat::Json);
        }
        Some(x) if (x == "toml") => {
            path = path.replace(".toml", ".md");
            extension = Some(PayloadFormat::Toml);
        },
        Some(x) if (x == "yaml") => {
            path = path.replace(".yaml", ".md");
            extension = Some(PayloadFormat::Yaml);
        },
        Some(x) if (x == "pickle") => {
            path = path.replace(".pickle", ".md");
            extension = Some(PayloadFormat::Pickle);
        }
        Some(x) if (x == "html") => {
            path = path.replace(".html", ".md");
            extension = Some(PayloadFormat::Html);
        },
        Some(x) if (x == "md") => {
            extension = Some(PayloadFormat::Markdown);
        },
        _ => {
            extension = None;
        }
    };

    if let Some(ref extension) = extension {
        // Handle commonmark requests early
        if extension == &PayloadFormat::Markdown {
            return fetch_md(&path).map(|v| v.into_response())
        };
        let payload = parse_to_extension(path, state)?;
        match extension {
            PayloadFormat::Html => {
                return Ok(Html(payload.html).into_response());
            },
            PayloadFormat::Json => {
                if let Ok(json) = serde_json::to_string_pretty(&payload) {
                    return Ok(json.into_response())
                }
            },
            PayloadFormat::Yaml => {
                if let Ok(yaml) = serde_yaml::to_string(&payload) {
                    return Ok(yaml.into_response())
                }
            },
            PayloadFormat::Toml => {
                if let Ok(toml) = toml::to_string_pretty(&payload) {
                    return Ok(toml.into_response())
                }
            },
            PayloadFormat::Pickle => {
                if let Ok(pickle) = serde_pickle::to_vec(&payload, Default::default()) {
                    return Ok(pickle.into_response())
                }
            }
            _ => {}
            
        }

    }
    Err(StatusCode::BAD_REQUEST.into())
}

fn fetch_md(path:&String) -> Result<Vec<u8>> {
    // limit paths to resolve to cwd or its children.
    //let cwd = env::current_dir();
    let path = SysPath::new(&path);
    if path.exists() {
        let mut file = File::open(path).map_err(|_| StatusCode::NOT_FOUND)?;
        let mut buf = vec![];
        let _ = file.read_to_end(&mut buf);
        return Ok(buf);
    }

    Err(StatusCode::NOT_FOUND.into())
}

fn parse_to_extension(path:String, state:Arc<Cli>) -> Result<Payload> {
    if SysPath::new(&path).exists() {
        let buf = fetch_md(&path)?;

        let mut buf = &buf[..];
        let mut pod:Pod = Pod::Null;
        let mut matter:Option<ParsedEntity> = None;
        let mut refdef = RefDefMatter::new(buf);

        // Handle front matter with `gray_matter`
        match state.front_matter {
            Some(FrontMatter::Yaml) => {
                if let Ok(s) = str::from_utf8(&buf) {
                    println!("parse yaml");
                    let m = Matter::<YAML>::new();
                    matter = Some(m.parse(s));

                } else {
                    println!("error yaml");
                }
            },
            Some(FrontMatter::Json) => {
                if let Ok(s) = str::from_utf8(&buf) {
                    println!("parse json");
                    let m = Matter::<JSON>::new();
                    matter = Some(m.parse(s));

                } else {
                    println!("error json");
                }
            },
            Some(FrontMatter::Toml) => {
                if let Ok(s) = str::from_utf8(&buf) {
                    println!("parse toml");
                    let m = Matter::<TOML>::new();
                    matter = Some(m.parse(s));

                } else {
                    println!("error toml");
                }
            },
            Some(FrontMatter::Refdef) => {
                println!("parse refdef");
                // Would've preferred to impl custom Engine but `refdef`
                // doesnt have a delimiter, so just use Pod.
                refdef.scan();
                if let Some(m) = refdef.parse_gray_matter() {
                    pod = m;
                }
            },
            None => {}
        }

        if let Some(info) = &matter {
            if let Some(p) = &info.data {
                // update `buf` to be remaining text minus the front matter.
                pod = p.clone();
                println!("{:?}", info.content);
                buf = &info.content.as_bytes();
            }
        }

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
                    x => {
                        dbg!(x);
                    }
                }
            }

            finish_and_store(&mut ranges, &mut range, collection.last().unwrap().0+1);

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
                x => {
                    dbg!(i, str::from_utf8(&[x]).unwrap());
                    break;
                }
            }
        }

        dbg!(&self.range);
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