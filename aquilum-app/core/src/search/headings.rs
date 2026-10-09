use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use std::ops::Range;

#[derive(Debug)]
pub struct Heading {
    pub level: u8,
    pub title: String,
    pub start: usize,
    pub content: Range<usize>,
}

pub fn headings(source: &str) -> Vec<Heading> {
    let mut found = Vec::<Heading>::new();
    let mut level = None;
    let mut start = 0;
    let mut title = String::new();

    for (event, range) in Parser::new_ext(source, Options::all()).into_offset_iter() {
        match event {
            Event::Start(Tag::Heading { level: found_level, .. }) => {
                level = Some(found_level as u8);
                start = range.start;
                title.clear();
            }
            Event::Text(text) | Event::Code(text) if level.is_some() => title.push_str(&text),
            Event::End(TagEnd::Heading(_)) => {
                if let Some(level) = level.take() {
                    found.push(Heading {
                        level,
                        title: title.trim().to_owned(),
                        start,
                        content: range.end..source.len(),
                    });
                }
            }
            _ => {}
        }
    }

    for index in 0..found.len() {
        let level = found[index].level;
        found[index].content.end = found[index + 1..]
            .iter()
            .find(|next| next.level <= level)
            .map_or(source.len(), |next| next.start);
    }
    found
}

pub fn enclosing(headings: &[Heading], offset: usize) -> Option<&Heading> {
    headings.iter().rev().find(|heading| heading.start <= offset)
}

pub fn matching<'a>(headings: &'a [Heading], wanted: &str) -> Vec<&'a Heading> {
    let needle = wanted.trim().trim_start_matches('#').trim().to_lowercase();
    headings
        .iter()
        .filter(|heading| heading.title.to_lowercase() == needle)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{enclosing, headings, matching, Heading};

    const NOTE: &str = "# Один\n\nтекст\n\n## Два\n\nещё\n\n### Три\n\nглубже\n\n## Два\n\nдубль\n";

    fn line(source: &str, heading: &Heading) -> String {
        source[heading.start..heading.content.start].to_owned()
    }

    #[test]
    fn reads_level_title_and_section_bounds() {
        let list = headings(NOTE);
        assert_eq!(
            list.iter()
                .map(|heading| (heading.level, heading.title.as_str()))
                .collect::<Vec<_>>(),
            [(1, "Один"), (2, "Два"), (3, "Три"), (2, "Два")]
        );
        assert_eq!(line(NOTE, &list[0]), "# Один\n");
        assert_eq!(&NOTE[list[1].content.clone()], "\nещё\n\n### Три\n\nглубже\n\n");
        assert_eq!(&NOTE[list[2].content.clone()], "\nглубже\n\n");
        assert_eq!(&NOTE[list[3].content.clone()], "\nдубль\n");
    }

    #[test]
    fn a_heading_without_a_trailing_newline_closes_at_the_end_of_the_text() {
        let source = "# Один";
        let list = headings(source);
        assert_eq!(line(source, &list[0]), "# Один");
        assert!(list[0].content.is_empty());
    }

    #[test]
    fn a_setext_heading_keeps_its_underline_out_of_the_section() {
        let source = "Заголовок\n=========\n\nтекст\n";
        let list = headings(source);
        assert_eq!((list[0].level, list[0].title.as_str()), (1, "Заголовок"));
        assert_eq!(&source[list[0].content.clone()], "\nтекст\n");
    }

    #[test]
    fn ignores_hashes_inside_a_code_block() {
        let list = headings("# Настоящий\n\n```\n# Ненастоящий\n```\n");
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].title, "Настоящий");
    }

    #[test]
    fn finds_the_heading_that_encloses_an_offset() {
        let list = headings(NOTE);
        let offset = NOTE.find("глубже").unwrap();
        assert_eq!(enclosing(&list, offset).unwrap().title, "Три");
        assert_eq!(enclosing(&list, 0).unwrap().title, "Один");
    }

    #[test]
    fn reports_every_heading_with_the_same_title() {
        let list = headings(NOTE);
        assert_eq!(matching(&list, "## Два").len(), 2);
        assert_eq!(matching(&list, "три").len(), 1);
        assert!(matching(&list, "Нет такого").is_empty());
    }
}
