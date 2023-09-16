use std::ops::Range;
use pulldown_cmark::{
    CowStr, Event, Tag, HeadingLevel, 
};

pub trait Plugin {
    /*
    The size of `slice` passed into `check_slice`.
    */
    fn window_size(&self) -> usize;
    /*
    The min amount of new items to be inserted.
    */
    fn new_items(&self) -> usize;
    /*
    Recieves a slice the size of `window_size`, containing `(Index, Event)` items.
    Returns `Some(min_index..max_index)` for items that will be replaced in 
    future `replace_slice` call.
    */
    fn check_slice(&mut self, slice: &[(usize, Event)]) -> Option<Range<usize>>;

    fn final_check(&mut self, pos: usize) -> Option<Range<usize>>;

    /*
    Recieves a slice the size of a range `max - min` returned by an earlier 
    call to `check_slice`, which will be replaced by the returned array 
    of `Event`'s.
    */
    fn replace_slice<'a>(&self, slice: &[(usize, Event<'a>)]) -> Vec<Event<'a>>;
}

#[derive(Default)]
pub struct CollapsibleHeaders {
    range: Option<Range<usize>>,
    level: u8,
    text: String,
}

impl CollapsibleHeaders {
    pub fn new(level: u8, text: String) -> CollapsibleHeaders {
        CollapsibleHeaders { level, text, ..Default::default() }
    }
}

impl Plugin for CollapsibleHeaders {
    fn window_size(&self) -> usize {
        4
    }

    fn new_items(&self) -> usize {
        5
    }

    fn check_slice(&mut self, slice: &[(usize, Event)]) -> Option<Range<usize>> {
        debug_assert!(slice.len() == self.window_size());
        #[cfg(debug_assertions)]
        println!("{:?}", slice);
        match slice {
            [
                (a, Event::Start(Tag::Heading(lvl, _, _))), 
                (_, Event::Start(Tag::Emphasis)),
                (_, Event::Text(CowStr::Borrowed(v))),
                (b, Event::End(Tag::Emphasis))
            ] => if (*lvl as u8) >= self.level && v == &self.text.as_str() {
                if let Some(ref mut range) = self.range {
                    range.end = *b;
                    let r = range.clone();
                    self.range = None;
                    return Some(r);
                }
                
                if self.range.is_none() {
                    self.range = Some(*a..*b);
                }
            },
            [(idx, Event::Start(Tag::Heading(lvl, _, _))), ..] => if lvl < &HeadingLevel::H5 {
                if let Some(ref mut range) = self.range {
                    range.end = *idx;
                    let r = range.clone();
                    self.range = None;
                    return Some(r);
                }
            },
            [(idx, Event::Rule), ..] => {
                if let Some(ref mut range) = self.range {
                    range.end = *idx;
                    let r = range.clone();
                    self.range = None;
                    return Some(r);
                }
            },
            _ => {}
        }

        None
    }

    fn final_check(&mut self, pos:usize) -> Option<Range<usize>> {
        #[cfg(debug_assertions)]
        dbg!();
        if let Some(ref mut range) = self.range {
            range.end = pos;
        }
        self.range.clone()
    }

    fn replace_slice<'a>(&self, slice: &[(usize, Event<'a>)]) -> Vec<Event<'a>> {
        #[cfg(debug_assertions)]
        println!("{:?}", slice);
        let mut r = vec![
            Event::Html(CowStr::Borrowed("<details open>")),
            Event::SoftBreak,
            Event::Html(CowStr::Borrowed("<summary>")),
        ];
        if let (Some((_, a)), Some((_, b)), Some((_, c))) 
             = (slice.get(1), slice.get(2), slice.get(3)) 
        {
            r.extend([a.to_owned(), b.to_owned(), c.to_owned()]);
        }
        r.push(Event::Html(CowStr::Borrowed("</summary>")));
        r.extend(slice.iter().skip(5).map(|t| t.1.to_owned()));
        r.push(Event::Html(CowStr::Borrowed("</details>")));
        r
    }
}

#[derive(Default)]
pub struct Emoji;

impl Plugin for Emoji {
    fn window_size(&self) -> usize {
        1
    }

    fn new_items(&self) -> usize {
        1
    }

    fn check_slice(&mut self, slice: &[(usize, Event)]) -> Option<Range<usize>> {
        match slice {
            [(i, Event::Text(value))] => {
                value
                .find(':').and_then(|start| {
                    value[start+1..].find(':').map(|end| (start+1..start+end+1) )
                })
                .and_then(|range| {
                    #[cfg(debug_assertions)]
                    dbg!(&value[range.clone()]);
                    emojis::get_by_shortcode(&value[range])
                } )
                .map(|_| i.to_owned()..(i+1).to_owned())

            },
            _ => {
                None
            }
        }
    }

    fn final_check(&mut self, _: usize) -> Option<Range<usize>> {
        None
    }

    fn replace_slice<'a>(&self, slice: &[(usize, Event<'a>)]) -> Vec<Event<'a>> {
        match slice {
            [(_, event @ Event::Text(value))] => {
                let pair = value
                .find(':')
                .and_then(|start| {
                    value[start+1..].find(':').map(|end| (start+1..start+end+1) )
                })
                .and_then(|range| emojis::get_by_shortcode(&value[range.clone()]).map(|emoji| (range, emoji)) )
                .map(|tp| (tp.0, CowStr::Borrowed(tp.1.as_str())));
                if let Some((range, emoji)) = pair {
                    // Include the `:` characters again.
                    vec![Event::Text( value.replace(&value[range.start-1..range.end+1], &emoji).into() )]
                } else {
                    #[cfg(debug_assertions)]
                    dbg!(event);
                    vec![event.to_owned()]
                }
            },
            _ => {
                slice.iter().map(|t| t.1.to_owned() ).collect()
            }
        }
    }
}