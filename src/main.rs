#![allow(clippy::invalid_regex, clippy::pedantic, clippy::correctness, clippy::perf, clippy::style, clippy::restriction)]

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
    response::{Html, IntoResponse, Response},
};

use pulldown_cmark::{
    CowStr,
    Options,
    Parser as CmParser,
    Event,
    html,
    Tag,
    HeadingLevel, 
    LinkDef,
};

use clap::{Parser as CliParser, ValueEnum};
use gray_matter::{ParsedEntity, Matter, engine::{YAML, JSON, TOML}};

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

    // TODO allow `config.{toml,json}` parsed to load the exact values above?
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum, Debug)]
enum FrontMatter {
    Json,
    Toml,
    Yaml,
    Refdef, // probably a better name can be thought of
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

async fn determine(Path(path):Path<String>, state:Arc<Cli>) -> Response {
    println!("{:?}", path);
    
    match SysPath::new(&path).extension() {
        Some(x) if (x == "md") => {
            println!("Tt's markdown");
            if let Some(x) = commonmark_as_str(path) {
                x.into_response()
            } else {
                StatusCode::NOT_FOUND.into_response()
            }
        },
        Some(x) if (x == "html") => {
            println!("Tt's html");
            commonmark_as_html(path, state).into_response()
        },
        x => {
            println!("Not found {:?}", x);
            StatusCode::BAD_REQUEST.into_response()
        }
    }
}

fn commonmark_as_html(path:String, state:Arc<Cli>) -> Html<String> {
    println!("html {:?}", path);
    let path = path.replace(".html", ".md");
    Html(parse_md(path, state))
}

fn commonmark_as_str(path:String) -> Option<String> {
    println!("raw {:?}", path);
    let content = fetch_md(&path);
    if let Some(content) = content {
        println!("loaded");
        if let Ok(s) = str::from_utf8(&content) {
            Some(s.to_string())
        } else {
            None
        }
        
    } else {
        println!("failed to find");
        None
    }
}

fn fetch_md(path:&String) -> Option<Vec<u8>> {
    let cwd = env::current_dir();
    println!("{:?}", cwd);
    println!("{:?}", path);
    let path = SysPath::new(&path);
    if path.exists() {
        let file = File::open(path);

        if let Ok(mut f) = file {
            let mut buf = vec![];
            let _ = f.read_to_end(&mut buf);
            return Some(buf);
        }
    }

    None
}

fn parse_md(path:String, state:Arc<Cli>) -> String {
    let cwd = env::current_dir();
    println!("{:?}", cwd);
    println!("{:?}", path);
    if SysPath::new(&path).exists() {
        let buf = fetch_md(&path);

        if let Some(buf) = buf {
            let mut buf = &buf[..];
            let mut matter:Option<ParsedEntity> = None;
            let mut map:HashMap<&str, Vec<LinkDef>> = HashMap::new();
            let mut refdef = RefDefMatter::new(buf);

            // Handle front matter with gray_matter
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
                    // Can not implement custom Engine, as gray_matter
                    // needs a delimiters to work.
                    refdef.scan();
                    if let Some(m) = refdef.parse_gray_matter() {
                        map.extend(m);
                        //map = m;
                    }
                },
                None => {}
            }

            if let Some(info) = &matter {
                if let Some(_) = &info.data {
                    // update `buf` to be remaining text minus the front matter.
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

            if let Ok(s) = str::from_utf8(&buf) {
                let mut md_parser = CmParser::new_ext(s, md_opt);

                let mut ranges:Vec<Range<usize>> = Vec::new();
                let mut range:Option<Range<usize>> = None;
                let collection:Vec<_> = (0..).zip(&mut md_parser).collect();
                let collection = collection.as_slice();

                for slice in collection.windows(4) {
                    match slice {
                        [
                            (a, Event::Start(Tag::Heading(HeadingLevel::H5, _, _))), 
                            (_, Event::Start(Tag::Emphasis)),
                            (_, Event::Text(CowStr::Borrowed("in case you missed it"))),
                            (b, Event::End(Tag::Emphasis))
                        ] => {
                            //println!("{:?}", slice);
                            finish_and_store(&mut ranges, &mut range, *b);

                            if range.is_none() {
                                range = Some(*a..*b);
                            }
                        },
                        [(idx, Event::Start(Tag::Heading(_, _, _))), _] if range.is_some() => {
                            finish_and_store(&mut ranges, &mut range, *idx+1);
                        },
                        x => {
                            println!("{:?}", x);
                        }
                    }
                }

                finish_and_store(&mut ranges, &mut range, collection.last().unwrap().0+1);

                println!("ranges are {:?}", ranges);

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

                assert!(new_collection.len() != 0);
                let mut html_output = String::new();
                html::push_html(&mut html_output,  new_collection.into_iter());

                println!("{}", &html_output);

                /*for i in md_parser.reference_definitions().iter() {
                    println!("{:?}", i);
                }*/
                return html_output

            }
            
        }
        
    }
    
    "hello world ".to_owned() + &path
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
        // Unlike valid commonmark, no newlines can exist between any part of a refdef.
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
                    println!("idx: {:?} - char: {:?}", i, str::from_utf8(&[x]).unwrap());
                    break;
                }
            }
        }

        println!("{:?}", self.range);
        if let Some(ref mut r) = self.range {
            println!("{:?}", str::from_utf8(&self.slice[r.start..r.end]).unwrap());

        }
    }

    fn parse_gray_matter(&'input mut self) -> Option<HashMap<&str, Vec<LinkDef<'_>>>> {
        use regex::Regex;

        if let Some(ref r) = self.range {
            let gray_matter = &self.slice[r.start..r.end];
            
            let lines = gray_matter
                .split(|c| c.eq(&b'\n'))
                .map( |slice| {
                    str::from_utf8(&slice)
                } )
                .filter_map(|r| r.ok())
                ;

            if let Ok(re) = Regex::new(r#"\[(?<id>[^\[\]]+)\]:\s(?<uri>[^\n"]+)("(?<title>[^"]+)")?"#) {
                let values = lines
                    .filter_map(|line| {
                        if re.is_match(line) {
                            Some(re.captures_iter(line))
        
                        } else {
                            None
                        }
                    })
                ;

                let mut map:HashMap<&'input str, Vec<LinkDef<'input>>> = HashMap::new();

                for capture in values {
                    for value in capture {
                        let id = value.name("id");
                        let uri = value.name("uri");
                        let title = value.name("title");
                        println!("captures {:?}, {:?}, {:?}", id, uri, title);

                        if let (Some(id_match), Some(uri_match)) = (id, uri) {
                            let key = id_match.as_str();
                            if map.contains_key(key) {
                                if let Some(vec) = map.get_mut(key) {
                                    vec.push(
                                        LinkDef { 
                                            dest: CowStr::Borrowed(uri_match.as_str()),
                                            span: uri_match.range(), 
                                            title: title
                                                .map(|t| {
                                                    CowStr::Borrowed(t.as_str())
                                                })
                                            }
                                    );
                                }

                            } else {
                                map
                                .insert(
                                    key, 
                                    vec![LinkDef { 
                                        dest: CowStr::Borrowed(uri_match.as_str()),
                                        span: uri_match.range(), 
                                        title: title
                                            .map(|t| {
                                                CowStr::Borrowed(t.as_str())
                                            })
                                        }]
                                    );

                            }
                            
                        };
                    }
                }

                return Some(map)

            }
        }
        None
    }
    
}
