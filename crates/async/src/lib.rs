use std::{
    str,
    sync::Arc,
    ffi::OsStr,
    path::Path as SysPath, 
    io::{Error, ErrorKind}, 
};

use axum::{
    extract::Path, 
    http::StatusCode, 
    response::{Html, IntoResponse, Response, Result},
};

use serve_md_core::Payload;
use serve_md_core::state::State;
use tokio::fs::{try_exists, read};
use serve_md_core::generate_payload_from_slice;
use serve_md_core::formats::Payload as PayloadFormats;

/// # Errors
/// 
/// Will return:
/// - `StatusCode::NOT_FOUND` for unresolved files.
/// - `StatusCode::BAD_REQUEST` for files not valid UTF8.
pub async fn determine(Path(path):Path<String>, state:Arc<State>) -> Result<Response> {
    #[cfg(debug_assertions)]
    dbg!(&path);
    
    let path_ext = SysPath::new(&path).extension();
    let extension = path_ext
    .and_then(OsStr::to_str)
    .and_then(|s| PayloadFormats::try_from(s).ok());

    if let Some(extension) = &extension {
        let path = path.replace(&(".".to_owned() + &extension.to_string()), ".md");
        // Handle commonmark requests early
        if extension == &PayloadFormats::Markdown {
            let buf = fetch_md(&path).await.or(Err(StatusCode::NOT_FOUND))?;
            return str::from_utf8(&buf)
                .or(Err(StatusCode::BAD_REQUEST.into()))
                .map(ToString::to_string)
                .map(IntoResponse::into_response)

        }
        let buf = generate_payload(path, state).await?
            .into_response_for(extension)
            .or(Err(StatusCode::BAD_REQUEST))?;
        
        return str::from_utf8(&buf)
            .or(Err(StatusCode::BAD_REQUEST.into()))
            .map(ToString::to_string)
            .map(|v| {
                if let PayloadFormats::Html = extension {
                    Html(v).into_response()
                } else {
                    IntoResponse::into_response(v)
                }
            })
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
        // TODO handle errors better.
        let input = fetch_md(&path).await.map_err(|_| StatusCode::NOT_FOUND)?;
        return generate_payload_from_slice(&input[..], state)
            .or(Err(StatusCode::NO_CONTENT.into()))
    }

    Err(StatusCode::NOT_FOUND.into())
}