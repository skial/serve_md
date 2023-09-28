use regex::Match;
use gray_matter::Pod;
use core::ops::Range;
use std::{collections::HashMap, str};

#[derive(Debug, Clone)]
pub struct RefDefMatter<'input> {
    slice:&'input [u8],
    range:Option<Range<usize>>,
}

impl<'input> RefDefMatter<'input> {

    pub fn new(slice:&'input [u8]) -> RefDefMatter<'input> {
        RefDefMatter { range: None, slice, }
    }

    pub fn scan(&mut self) {
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
                    #[cfg(debug_assertions)]
                    dbg!(i, str::from_utf8(&[x]).unwrap());
                    break;
                }
            }
        }

        #[cfg(debug_assertions)]
        {
            dbg!(&self.range);
            if let Some(ref mut r) = self.range {
                dbg!(str::from_utf8(&self.slice[r.start..r.end]).unwrap());
    
            }
        }
        
    }

    pub fn parse_gray_matter(&'input mut self) -> Option<Pod> {
        use regex::Regex;

        if let Some(r) = &self.range {
            // Currently confused why removing `gray_matter` & `lines`, merging
            // into a single iterator causes the inferred type to change & fail
            // type checking. What changes when assigning vs a long iter chain?
            let gray_matter = &self.slice[r.start..r.end];
            
            let lines = gray_matter
            .split(|c| c.eq(&b'\n'))
            .map( str::from_utf8 )
            .filter_map(Result::ok);

            // It would be nice to have regex syntax highlighting & compile time
            // checks to make sure its valid. Clippy? cargo extension?? IDE extension???
            if let Ok(re) = Regex::new(r#"\[(?<id>[^\[\]]+)\]:\s(?<uri>[^\n\r"]+)("(?<title>[^"]+)")?"#) {
                let mut map:HashMap<String, Pod> = HashMap::new();

                lines
                .filter_map(|line| {
                    re.is_match(line)
                    .then_some(re.captures_iter(line))
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
            title.is_some().then_some(("title".to_string(), title
                .map_or(
                    Pod::Null,
                    |t| Pod::String(t.as_str().to_string())
                )
            )) 
        ]
        .into_iter()
        .flatten()
        .collect::<HashMap<_, _>>()
    }
    
}