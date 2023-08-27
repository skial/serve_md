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
    HeadingLevel
};

use clap::Parser as CliParser;

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

    // TODO allow `config.{toml,json}` parsed to load the exact values above?
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
            commonmark_as_str(path).into_response()
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

fn commonmark_as_str(path:String) -> String {
    println!("raw {:?}", path);
    let content = fetch_md(&path);
    if let Some(content) = content {
        println!("loaded");
        str::from_utf8(content.as_slice()).unwrap().to_string()
    } else {
        println!("failed to find");
        "".to_owned()
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
            // All roundups all start with link references which are used for metadata/front matter.
            // pulldown-cmark ignores successive reference keys.
            // Attempt to remove header meta, run a parser for each line, store refdefs etc.
            let needle = b"#";
            let index = buf.as_slice().windows( needle.len() ).position(|w| w == needle );
            println!("{:?}", index);
            let pair = buf.split_at(index.unwrap());
            println!("{:?}", str::from_utf8(pair.0).unwrap());
            println!("{:?}", str::from_utf8(pair.1).unwrap());

            // TODO Add support for yaml / tomal front matter from pulldown-cmark.

            let mut md_opt = Options::empty();
            md_opt.insert(Options::ENABLE_HEADING_ATTRIBUTES);
            let mut md_parser = CmParser::new_ext(str::from_utf8(&buf).unwrap(), md_opt);

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

            //println!("{:?}", ranges);

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
            }

            let mut html_output = String::new();
            html::push_html(&mut html_output,  new_collection.into_iter());

            println!("{}", &html_output);

            for i in md_parser.reference_definitions().iter() {
                println!("{:?}", i);
            }

            /*for event in collection.iter() {
                println!("{:?}", event);
            }*/
            return html_output
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