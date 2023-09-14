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

use tokio::fs::{try_exists, read};

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
use serde_derive::{Serialize, Deserialize};

use crate::plugin::{HaxeRoundup, Emoji, Plugin};

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
            let buf = fetch_md(&path).await.map_err(|_| StatusCode::NOT_FOUND)?;
            return str::from_utf8(&buf)
                .or(Err(StatusCode::BAD_REQUEST.into()))
                .map(|s| s.to_string())
                .map(|s| s.into_response())

        }
        return generate_payload(path, state).await?.into_response_for(&extension);

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
            let md_parser = CmParser::new_ext(s, md_opt);
            let mut collection_vec:Vec<_> = (0..).zip(md_parser).collect();
            let mut collection_slice = collection_vec.as_slice();
            let slice_len = collection_slice.len();
            let mut plugins:Vec<Box<dyn Plugin>> = vec![Box::new(HaxeRoundup::default()), Box::new(Emoji)];
            let mut new_collection:Vec<Event> = vec![];
            let len = plugins.len();

            for (index, plugin) in plugins.iter_mut().enumerate() {
                let mut ranges:Vec<Range<usize>> = Vec::new();
                
                if index != 0 && index < len {
                    collection_vec = (0..).zip(new_collection).collect();
                    collection_slice = collection_vec.as_slice();
                }

                let mut plugin_collection:Vec<Event<>> = Vec::with_capacity( slice_len + (ranges.len() * plugin.window_size()) );
                for slice in collection_slice.windows(plugin.window_size()) {
                    if let Some(range) = plugin.check_slice(slice) {
                        ranges.push(range);
                    }
                }
    
                // TODO maybe reuse `check_slice` but with a single item.
                // `final_check` has more meaning than a single item being passed in.
                if let Some(range) = collection_slice.last().and_then(|item| plugin.final_check(item.0)) {
                    #[cfg(debug_assertions)]
                    dbg!(&range);
                    ranges.push(range);
                }
    
                #[cfg(debug_assertions)]
                dbg!(&ranges);
    
                let mut range_idx: usize = 0;
    
                if !ranges.is_empty() {
                    debug_assert!( ranges.len() > 0 );
    
                    let mut i:usize = 0;
    
                    while i < collection_slice.len() {
                        let pair = &collection_slice[i];
                        if let Some(range) = ranges.get(range_idx) {
                            if !range.contains(&pair.0) {
                                plugin_collection.push(pair.1.to_owned());
                                i += 1;
                                continue;
                            }
    
                            plugin_collection.extend_from_slice( &plugin.replace_slice(&collection_slice[range.clone()]) );
    
                            i += range.len();
                            range_idx += 1;
                        } else {
                            #[cfg(debug_assertions)]
                            dbg!(&pair);
                            plugin_collection.push(pair.1.to_owned());
                            i += 1;
                            continue;
                        }
    
                    }
                } else {
                    plugin_collection.extend(collection_slice.iter().map(|c| c.1.to_owned()));
    
                }

                new_collection = plugin_collection;

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