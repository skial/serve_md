use core::ops::Range;
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
    fn replace_slice<'input>(&self, slice: &[(usize, Event<'input>)]) -> Vec<Event<'input>>;
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
        println!("{slice:?}");
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

    fn final_check(&mut self, pos: usize) -> Option<Range<usize>> {
        #[cfg(debug_assertions)]
        dbg!(pos);
        if let Some(ref mut range) = self.range {
            range.end = pos;
        }
        self.range.clone()
    }

    fn replace_slice<'input>(&self, slice: &[(usize, Event<'input>)]) -> Vec<Event<'input>> {
        #[cfg(debug_assertions)]
        println!("{slice:?}");
        let mut r = vec![
            Event::Html(CowStr::Borrowed("<details open>")),
            Event::SoftBreak,
            Event::Html(CowStr::Borrowed("<summary>")),
        ];
        if let (Some((_, a)), Some((_, b)), Some((_, c))) 
             = (slice.get(1), slice.get(2), slice.get(3)) 
        {
            r.extend([a.clone(), b.clone(), c.clone()]);
        }
        r.push(Event::Html(CowStr::Borrowed("</summary>")));
        r.extend(slice.iter().skip(5).map(|t| t.1.clone()));
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

    /// Checks for the existence of a single emoji shortcode `:{value}:`.
    fn check_slice(&mut self, slice: &[(usize, Event)]) -> Option<Range<usize>> {
        match slice {
            [(i, Event::Text(value))] => {
                value
                .find(':').and_then(|start| {
                    value[start+1..].find(':').map(|end| ((start + 1)..=(start + end)) )
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

    /// Replaces every occurance of a valid shortcode `:{value}:` with its emoji.
    fn replace_slice<'input>(&self, slice: &[(usize, Event<'input>)]) -> Vec<Event<'input>> {
        match slice {
            [(_, /*event @ */Event::Text(value))] => {
                let mut ranges = vec![];
                let mut range = None;
                for value in value.char_indices() {
                    match range {
                        None => if value.1 == ':' {
                            range = Some(value.0..0);
                        }
                        Some(incomplete) if value.1 == ':' => {
                            if value.0+1 - incomplete.start > 2 {
                                ranges.push( incomplete.start..value.0+1 );

                            }
                            range = None;
                        }
                        _ => {}
                    }
                }
                if let Some(incomplete) = range {
                    if incomplete.end == 0 { 
                        let tmp = incomplete.start..value.len();
                        if tmp.len() > 2 {
                            ranges.push( tmp );
                        }
                        //range = None;
                    }
                }
                ranges.reverse();
                let mut result = value.clone().into_string();
                #[cfg(debug_assertions)]
                dbg!(&ranges);
                for range in ranges {
                    let opt = value.get(range.clone())
                    .map(|s| (s, emojis::get_by_shortcode(&s[1..s.len()-1])) )
                    .and_then(|(s, emoji)| {
                        #[cfg(debug_assertions)]
                        dbg!(&s, &emoji);
                        emoji.map(emojis::Emoji::as_str).map(|e| (s, e))
                    });
                    #[cfg(debug_assertions)]
                    dbg!(&opt);
                    if let Some((s, val)) = opt {
                        result = result.replace(s, val);
                    }
                }
                #[cfg(debug_assertions)]
                dbg!(&result);
                vec![Event::Text(CowStr::Boxed(result.into()))]
            },
            _ => {
                slice.iter().map(|t| t.1.clone() ).collect()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::vec;

    use pulldown_cmark::HeadingLevel;
    use pulldown_cmark::Tag;
    use pulldown_cmark::Event;
    use pulldown_cmark::CowStr;

    use super::CollapsibleHeaders;
    use super::Emoji;
    use super::Plugin;

    #[test]
    fn emoji_test_check_and_replace_slice() {
        let mut plugin = Emoji{};
        let input = [
            (0, Event::Text(CowStr::Borrowed("Random text w/ shortcode :+1: emoji :smile: mixed in. :tada:"))),
            (1, Event::Text(CowStr::Borrowed(":rocket::rocket::rocket:"))),
        ];
        let mut ranges = vec![];
        let expected = [
            Event::Text(CowStr::Borrowed("Random text w/ shortcode 👍 emoji 😄 mixed in. 🎉")),
            Event::Text(CowStr::Borrowed("🚀🚀🚀")),
        ];
        for slice in input.windows(plugin.window_size()) {
            ranges.push( plugin.check_slice(slice) );
        }
        assert!(!ranges.is_empty());
        assert_eq!(ranges.len(), 2);
        assert_eq!(ranges.iter().filter(|o| o.is_some()).count(), 2);
        let mut results = vec![];
        for op in ranges {
            if let Some(range) = op {
                results.extend_from_slice( &plugin.replace_slice(&input[range]) )
            } else {
                assert!(false);
            }
        }
        assert!(!results.is_empty());
        assert_eq!(expected.len(), results.len());
        for i in 0..expected.len() {
            assert_eq!(expected[i], results[i]);
        }
    }

    #[test]
    fn emoji_test_incomplete_shortcode() {
        let mut plugin = Emoji{};
        let input = [
            (0, Event::Text(CowStr::Borrowed(":+1::+1:+1:"))),
        ];
        let mut ranges = vec![];
        let expected = [
            Event::Text(CowStr::Borrowed("👍👍+1:")),
        ];
        for slice in input.windows(plugin.window_size()) {
            ranges.push( plugin.check_slice(slice) );
        }
        dbg!(&ranges);
        assert!(!ranges.is_empty());
        assert_eq!(ranges.len(), 1);
        assert_eq!(ranges.iter().filter(|o| o.is_some()).count(), 1);
        let mut results = vec![];
        for op in ranges {
            if let Some(range) = op {
                results.extend_from_slice( &plugin.replace_slice(&input[range]) )
            } else {
                assert!(false);
            }
        }
        assert!(!results.is_empty());
        assert_eq!(expected.len(), results.len());
        for i in 0..expected.len() {
            assert_eq!(expected[i], results[i]);
        }
    }

    #[test]
    fn ch_test_check_and_replace_slice() {
        use pretty_assertions::assert_eq;

        let input = [
            (0, Event::Start(Tag::Heading(HeadingLevel::H6, None, vec![]))),
            (1, Event::Start(Tag::Emphasis)),
            (2, Event::Text(CowStr::Borrowed("text"))),
            (3, Event::End(Tag::Emphasis)),
            (4, Event::End(Tag::Heading(HeadingLevel::H6, None, vec![]))),
            (5, Event::Start(Tag::Paragraph)),
            (6, Event::Text(CowStr::Borrowed("some test text. a lil bit more."))),
            (7, Event::End(Tag::Paragraph)),
            (8, Event::Rule),
            (9, Event::Start(Tag::Paragraph)),
            (10, Event::Text(CowStr::Borrowed("Some more text that should not be captured by the plugin."))),
            (11, Event::End(Tag::Paragraph)),
            (12, Event::Start(Tag::Heading(HeadingLevel::H6, None, vec![]))),
            (13, Event::Start(Tag::Emphasis)),
            (14, Event::Text(CowStr::Borrowed("text"))),
            (15, Event::End(Tag::Emphasis)),
            (16, Event::End(Tag::Heading(HeadingLevel::H6, None, vec![]))),
        ];

        let level = 5;
        let mut ranges = vec![];
        let mut plugin = CollapsibleHeaders::new(level, "text".to_string());
        for slice in input.windows(plugin.window_size()) {
            if let Some(range) = plugin.check_slice(slice) {
                ranges.push( range );
            }
        }
        // Checks for any partially open ranges.
        if let Some(range) = plugin.final_check(input.len()) {
            ranges.push( range );
        }

        dbg!(&level, &ranges);
        // As HeadingLevel::H6 is the max no matches should exist.
        if level > 6 {
            assert!(ranges.is_empty());
        } else {
            assert_eq!(ranges.len(), 2);
            assert_eq!(ranges[0], 0..8);
            assert_eq!(ranges[1], 12..17);

            let output = &plugin.replace_slice(&input[ranges[0].clone()]);
            assert!(!output.is_empty());
            assert_eq!([
                Event::Html(CowStr::Borrowed("<details open>")),
                Event::SoftBreak,
                Event::Html(CowStr::Borrowed("<summary>")),
                Event::Start(Tag::Emphasis),
                Event::Text(CowStr::Borrowed("text")),
                Event::End(Tag::Emphasis),
                Event::Html(CowStr::Borrowed("</summary>")),
                Event::Start(Tag::Paragraph),
                Event::Text(CowStr::Borrowed("some test text. a lil bit more.")),
                Event::End(Tag::Paragraph),
                Event::Html(CowStr::Borrowed("</details>")),
            ][..], output[..]);

            let output = &plugin.replace_slice(&input[ranges[1].clone()]);
            dbg!(&input[ranges[1].clone()]);
            dbg!(&output);
            assert!(!output.is_empty());
            assert_eq!([
                Event::Html(CowStr::Borrowed("<details open>")),
                Event::SoftBreak,
                Event::Html(CowStr::Borrowed("<summary>")),
                Event::Start(Tag::Emphasis),
                Event::Text(CowStr::Borrowed("text")),
                Event::End(Tag::Emphasis),
                Event::Html(CowStr::Borrowed("</summary>")),
                Event::Html(CowStr::Borrowed("</details>")),
            ][..], output[..]);
        }
    }
}