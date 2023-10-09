use std::sync::Arc;

use serve_md_core::{determine, state::State};

#[test]
fn test_determine_with_existing_file() {
    let path = format!("{}\\resources\\hio.md", env!("CARGO_MANIFEST_DIR"));
    let v = determine(&path, Arc::new(State::default()));
    dbg!(&v);
    assert!( v.is_ok() );
}

#[test]
fn test_determine_with_missing_file() {
    use pretty_assertions::assert_eq;

    let path = format!("{}\\resources\\who.md", env!("CARGO_MANIFEST_DIR"));
    let v = determine(&path, Arc::new(State::default()));
    dbg!(&v);
    match v {
        Ok(_) => {},
        Err(e) => {
            assert_eq!(
                e.to_string(),
                format!("There was an error trying to read the markdown file {path}"),
            );
            let dwn = e.downcast_ref::<std::io::Error>();
            match dwn {
                Some(ee) => {
                    match ee.kind() {
                        std::io::ErrorKind::NotFound => {
                            assert!(true);
                            return;
                        }
                        _ => {},
                    }
                }
                _ => {},
            }
        }
    }
    assert!( false, "Should have returned an error." )
}


#[test]
fn test_determine_with_incorrect_extension() {
    use pretty_assertions::assert_eq;

    let path = format!("{}\\resources\\who.xxx", env!("CARGO_MANIFEST_DIR"));
    let v = determine(&path, Arc::new(State::default()));
    dbg!(&v);
    match v {
        Ok(_) => {},
        Err(e) => {
            assert_eq!(
                e.to_string(),
                format!("File path {path} not found."),
            );
            return;
        }
    }
    assert!( false, "Should have returned an error." )
}