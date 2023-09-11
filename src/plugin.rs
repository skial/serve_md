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

    fn finalize(&mut self, pos: usize) -> Option<Range<usize>>;

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
                (a, Event::Start(Tag::Heading(HeadingLevel::H5, _, _))), 
                (_, Event::Start(Tag::Emphasis)),
                (_, Event::Text(CowStr::Borrowed("in case you missed it"))),
                (b, Event::End(Tag::Emphasis))
            ] => {
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

    fn finalize(&mut self, pos:usize) -> Option<Range<usize>> {
        dbg!("finalizing");
        if let Some(ref mut range) = self.range {
            range.end = pos;

        }
        self.range.clone()
    }

    fn replace_slice<'a>(&self, slice: &[(usize, Event<'a>)]) -> Vec<Event<'a>> {
        println!("{:?}", slice);
        let mut iter = slice
        .iter().skip(1);
        let mut r = vec![
            Event::Html(CowStr::Borrowed("<details open>")),
            Event::SoftBreak,
            Event::Html(CowStr::Borrowed("<summary>")),
            // TODO remove unwrap calls.
            iter.next().unwrap().1.to_owned(),
            iter.next().unwrap().1.to_owned(),
            iter.next().unwrap().1.to_owned(),
            Event::Html(CowStr::Borrowed("</summary>"))
        ];
        iter.skip(1).for_each(|t| r.push(t.1.to_owned()));
        r.push(Event::Html(CowStr::Borrowed("</details>")));
        r
    }
}