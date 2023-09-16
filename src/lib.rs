pub mod matter;
pub mod formats;
pub mod plugin;
pub mod state;

use std::{
    str,
    io::{Error, ErrorKind}, 
    sync::Arc,
    ops::Range, 
    path::Path as SysPath, 
};

use axum::{
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
use state::State;
use gray_matter::Pod;
use tokio::fs::{try_exists, read};
use serde_derive::{Serialize, Deserialize};

use crate::plugin::{CollapsibleHeaders, Emoji, Plugin};

pub async fn determine(Path(path):Path<String>, state:Arc<State>) -> Result<Response> {
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
            let buf = fetch_md(&path).await.or(Err(StatusCode::NOT_FOUND))?;
            return str::from_utf8(&buf)
                .or(Err(StatusCode::BAD_REQUEST.into()))
                .map(|s| s.to_string())
                .map(|s| s.into_response())

        }
        return generate_payload(path, state).await?.into_response_for(extension);

    }
    Err(StatusCode::BAD_REQUEST.into())
}

async fn fetch_md(path: &String) -> std::io::Result<Vec<u8>> {
    if try_exists(path).await? {
        return read(path).await
    }

    Err(Error::from(ErrorKind::NotFound))
}

async fn generate_payload(path:String, state:Arc<State>) -> Result<Payload> {
    if tokio::fs::try_exists(&path).await.map_err(|_| StatusCode::NOT_FOUND)? {
        let mut input = fetch_md(&path).await.map_err(|_| StatusCode::NOT_FOUND)?;
        let mut pod:Pod = Pod::String("".to_owned());

        let tp = state.front_matter.and_then(|fm| 
            str::from_utf8(&input).ok().and_then(|s| fm.as_pod(s)) 
        );
        if let Some((p, v)) = tp {
            pod = p;
            input = v;
        }

        return if let Ok(s) = str::from_utf8(&input[..]) {
            let md_parser = make_commonmark_parser(s, &state);
            let plugins = make_commonmark_plugins(&state);
            let new_collection = process_commonmark_tokens(md_parser, plugins);

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

fn make_commonmark_parser<'a>(text: &'a str, state: &'a Arc<State>) -> CmParser<'a, 'a> {
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
    if state.tasklists {
        md_opt.insert(Options::ENABLE_TASKLISTS);
    }
    dbg!(md_opt);

    CmParser::new_ext(text, md_opt)
}

fn make_commonmark_plugins(state:&Arc<State>) -> Vec<Box<dyn Plugin>> {
    let mut plugins: Vec<Box<dyn Plugin>> = vec![];
    if state.emoji_shortcodes {
        plugins.push(Box::new(Emoji));
    }
    if let Some(options) = &state.collapsible_headers {
        plugins.push(Box::new(CollapsibleHeaders::new(options.0, options.1.to_owned())));
    }

    plugins
}

fn process_commonmark_tokens<'a>(parser: CmParser<'a, 'a>, mut plugins: Vec<Box<dyn Plugin>>) -> Vec<Event<'a>> {
    let mut collection_vec:Vec<_> = (0..).zip(parser).collect();
    let mut collection_slice = collection_vec.as_slice();
    let mut new_collection:Vec<Event> = vec![];
    let len = plugins.len();

    if !plugins.is_empty() {
        for (index, plugin) in plugins.iter_mut().enumerate() {
            if index != 0 && index < len {
                collection_vec = (0..).zip(new_collection).collect();
                collection_slice = collection_vec.as_slice();
            }

            new_collection = if let Some(ranges) = check_collection_with(plugin, collection_slice) {
                rewrite_collection_with(plugin, collection_slice, ranges)
            } else {
                collection_slice.iter().map(|c| c.1.to_owned()).collect()

            }

        }
    } else {
        new_collection = collection_slice.iter().map(|c| c.1.to_owned()).collect();
    }

    debug_assert!(!new_collection.is_empty());
    new_collection
}

fn check_collection_with(plugin: &mut Box<dyn Plugin>, collection: &[(usize, Event)]) -> Option<Vec<Range<usize>>> {
    let mut ranges = Vec::new();
    for slice in collection.windows(plugin.window_size()) {
        if let Some(range) = plugin.check_slice(slice) {
            ranges.push(range);
        }
    }

    // TODO maybe reuse `check_slice` but with a single item.
    // `final_check` has more meaning than a single item being passed in.
    if let Some(range) = collection.last().and_then(|item| plugin.final_check(item.0)) {
        #[cfg(debug_assertions)]
        dbg!(&range);
        ranges.push(range);
    }

    if ranges.is_empty() {
        None
    } else {
        Some(ranges)
    }
}

fn rewrite_collection_with<'a>(plugin: &mut Box<dyn Plugin>, collection: &[(usize, Event<'a>)], ranges: Vec<Range<usize>>) -> Vec<Event<'a>> {
    let mut idx:usize = 0;
    let mut range_idx: usize = 0;
    let mut plugin_collection:Vec<Event<>> = Vec::with_capacity( collection.len() + (ranges.len() * plugin.window_size()) );
    
    while idx < collection.len() {
        let pair = &collection[idx];
        if let Some(range) = ranges.get(range_idx) {
            if !range.contains(&pair.0) {
                plugin_collection.push(pair.1.to_owned());
                idx += 1;
                continue;
            }

            plugin_collection.extend_from_slice( &plugin.replace_slice(&collection[range.clone()]) );

            idx += range.len();
            range_idx += 1;
        } else {
            #[cfg(debug_assertions)]
            dbg!(&pair);
            plugin_collection.push(pair.1.to_owned());
            idx += 1;
            continue;
        }

    }

    plugin_collection
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

