use std::ops::Range;
use pulldown_cmark::{
    CowStr, Event, Tag, HeadingLevel, 
};

pub trait Plugin {
    /*
    The size of `slice` passed into `check_slice`.
    */
    fn window_size() -> usize;
    /*
    The min amount of new items to be inserted.
    */
    fn new_items() -> usize;
    /*
    Recieves a slice the size of `window_size`, containing (Index, Event) items.
    Returns `None` for no match, `Some(min_index..max_index)` for items that will
    be replaced in future `replace_slice` call.
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
pub struct HaxeRoundup {
    range: Option<Range<usize>>,
}

impl Plugin for HaxeRoundup {
    fn window_size() -> usize {
        4
    }

    fn new_items() -> usize {
        5
    }

    fn check_slice(&mut self, slice: &[(usize, Event)]) -> Option<Range<usize>> {
        debug_assert!(slice.len() == HaxeRoundup::window_size());
        println!("{:?}", slice);
        match slice {
            [
                (a, Event::Start(Tag::Heading(lvl, _, _))), 
                (_, Event::Start(Tag::Emphasis)),
                (_, Event::Text(CowStr::Borrowed("in case you missed it"))),
                (b, Event::End(Tag::Emphasis))
            ] => if lvl >= &HeadingLevel::H5 {
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
            _ => {}
        }

        None
    }

    fn final_check(&mut self, pos:usize) -> Option<Range<usize>> {
        dbg!();
        if let Some(ref mut range) = self.range {
            range.end = pos;

        }
        self.range.clone()
    }

    fn replace_slice<'a>(&self, slice: &[(usize, Event<'a>)]) -> Vec<Event<'a>> {
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