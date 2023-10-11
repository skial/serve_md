use indoc::indoc;
use serve_md_core::{
    determine, formats::Matter, generate_payload_from_path, generate_payload_from_slice,
    state::State,
};
use std::{path::{Path, PathBuf}, sync::Arc};

#[test]
fn test_determine_with_existing_file() {
    let path:PathBuf = [env!("CARGO_MANIFEST_DIR"), "resources", "hio.md"].iter().collect();
    if let Some(path) = path.to_str() {
        let v = determine(&path, Arc::new(State::default()));
        dbg!(&v);
        assert!(v.is_ok());
    } else {
        assert!(false, "Failed to build path.")
    }
    
}

#[test]
fn test_determine_with_missing_file() {
    use pretty_assertions::assert_eq;

    let path = format!("{}\\resources\\who.md", env!("CARGO_MANIFEST_DIR"));
    let v = determine(&path, Arc::new(State::default()));
    dbg!(&v);
    match v {
        Ok(_) => {}
        Err(e) => {
            assert_eq!(
                e.to_string(),
                format!("There was an error trying to read the markdown file {path}"),
            );
            let dwn = e.downcast_ref::<std::io::Error>();
            match dwn {
                Some(ee) => match ee.kind() {
                    std::io::ErrorKind::NotFound => {
                        assert!(true);
                        return;
                    }
                    _ => {}
                },
                _ => {}
            }
        }
    }
    assert!(false, "Should have returned an error.")
}

#[test]
fn test_determine_with_incorrect_extension() {
    use pretty_assertions::assert_eq;

    let path = format!("{}\\resources\\who.xxx", env!("CARGO_MANIFEST_DIR"));
    let v = determine(&path, Arc::new(State::default()));
    dbg!(&v);
    match v {
        Ok(_) => {}
        Err(e) => {
            assert_eq!(e.to_string(), format!("File path {path} not found."),);
            return;
        }
    }
    assert!(false, "Should have returned an error.")
}

#[test]
fn test_gen_payload_missing_file() {
    use pretty_assertions::assert_eq;

    let path = format!("{}\\resources\\who.xxx", env!("CARGO_MANIFEST_DIR"));
    let v = generate_payload_from_path(Path::new(&path), Arc::new(State::default()));
    dbg!(&v);
    match v {
        Ok(_) => {}
        Err(e) => {
            assert_eq!(e.to_string(), format!("Path {path} does not exist."),);
            return;
        }
    }
    assert!(false, "Should have returned an error.")
}

#[test]
fn test_gen_payload_from_path() {
    use pretty_assertions::assert_eq;

    let path:PathBuf = [env!("CARGO_MANIFEST_DIR"), "resources", "test.md"].iter().collect();
    let v = generate_payload_from_path(Path::new(&path), Arc::new(State::default()));
    dbg!(&v);
    match v {
        Ok(payload) => {
            assert_eq!(payload.front_matter, "");
            assert_eq!(
                payload.html,
                indoc! {r#"<h1>Header 1!</h1>
                <p>some text that should be informative</p>
                <ul>
                <li>list item 1</li>
                <li>list item 2</li>
                <li>list item 3</li>
                </ul>
                <h2>Header 2</h2>
                <p>hello world</p>
                "#}
            )
        }
        Err(e) => {
            assert!(false, "Should NEVER return an error. Error was {e}");
        }
    }
}

#[test]
fn test_gen_payload() {
    use pretty_assertions::assert_eq;
    let input = indoc! {r#"[key]: /uri/path "title"
    [key]: /dif/path

    # Header
    some text.
    "#};
    let mut state = State::default();
    state.front_matter = Some(Matter::Refdef);
    let expected_json = indoc! {r#"{
      "front_matter": {
        "key": [
          {
            "title": "title",
            "uri": "/uri/path "
          },
          {
            "uri": "/dif/path"
          }
        ]
      },
      "html": "<h1>Header</h1>\n<p>some text.</p>\n"
    }"#};
    let payload = generate_payload_from_slice(input.as_bytes(), Arc::new(state));
    match payload {
        Ok(payload) => {
            dbg!(&payload);
            match payload.into_response_for(&serve_md_core::formats::Payload::Json) {
                Ok(vec) => {
                    assert_eq!(std::str::from_utf8(&vec).unwrap(), expected_json)
                }
                Err(error) => {
                    dbg!(&error);
                    assert!(false, "Should NEVER return an error. Error was {error}.")
                }
            }
        }
        Err(error) => {
            dbg!(&error);
            assert!(false, "Should NEVER return an error. Error was {error}.")
        }
    }
}
