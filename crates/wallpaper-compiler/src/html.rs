use super::*;
use std::collections::BTreeMap;
type Attributes = BTreeMap<String, (String, Span)>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Div,
    Span,
    Image,
}
#[derive(Debug)]
pub struct Element {
    pub kind: Kind,
    pub parent: Option<usize>,
    pub id: Option<String>,
    pub classes: Vec<String>,
    pub inline_style: Option<(String, Span)>,
    pub text: String,
    pub image: Option<RelativePath>,
    pub span: Span,
}
#[derive(Debug)]
pub struct Document {
    pub elements: Vec<Element>,
    pub stylesheets: Vec<StylesheetReference>,
}
#[derive(Debug)]
pub struct StylesheetReference {
    pub path: RelativePath,
    pub span: Span,
}

struct Parser<'a> {
    source: Source<'a>,
    position: usize,
}
impl Parser<'_> {
    fn error(&self, code: ErrorCode, start: usize) -> Diagnostic {
        self.source.error(
            code,
            Span {
                start,
                end: self.position.max(start).min(self.source.text.len()),
            },
        )
    }
    fn spaces(&mut self) {
        while self
            .source
            .text
            .as_bytes()
            .get(self.position)
            .is_some_and(|&byte| ascii_space(byte))
        {
            self.position += 1;
        }
    }
    fn name(&mut self) -> Result<&str> {
        let start = self.position;
        while self
            .source
            .text
            .as_bytes()
            .get(self.position)
            .is_some_and(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'-')
        {
            self.position += 1;
        }
        if self.position == start {
            return Err(self.error(ErrorCode::Syntax, start));
        }
        Ok(&self.source.text[start..self.position])
    }
    fn attributes(&mut self) -> Result<(Attributes, bool)> {
        let mut attributes = BTreeMap::new();
        loop {
            let before_space = self.position;
            self.spaces();
            if self.source.text[self.position..].starts_with("/>") {
                self.position += 2;
                return Ok((attributes, true));
            }
            if self.source.text[self.position..].starts_with('>') {
                self.position += 1;
                return Ok((attributes, false));
            }
            if before_space == self.position || attributes.len() >= 4 {
                return Err(self.error(ErrorCode::Syntax, self.position));
            }
            let start = self.position;
            let name = self.name()?.to_owned();
            self.spaces();
            if self.source.text.as_bytes().get(self.position) != Some(&b'=') {
                return Err(self.error(ErrorCode::Syntax, start));
            }
            self.position += 1;
            self.spaces();
            let Some(&quote @ (b'\'' | b'"')) = self.source.text.as_bytes().get(self.position)
            else {
                return Err(self.error(ErrorCode::Syntax, start));
            };
            self.position += 1;
            let value_start = self.position;
            let end = self.source.text[value_start..]
                .find(quote as char)
                .map(|offset| offset + value_start)
                .ok_or_else(|| self.error(ErrorCode::Syntax, start))?;
            if end - value_start > 4096 {
                return Err(self.error(ErrorCode::BudgetExceeded, value_start));
            }
            let value = &self.source.text[value_start..end];
            let span = Span {
                start: value_start,
                end,
            };
            let decoded = if name == "style" {
                if value.contains('&') {
                    return Err(self.error(ErrorCode::UnsupportedValue, value_start));
                }
                value.to_owned()
            } else {
                decode_text(self.source, value, value_start, 4096, true)?
            };
            self.position = end + 1;
            if attributes.insert(name, (decoded, span)).is_some() {
                return Err(self.error(ErrorCode::DuplicateAttribute, start));
            }
        }
    }
}

pub fn parse(source: Source<'_>) -> Result<Document> {
    if source.text.len() > MAX_SOURCE_BYTES {
        return Err(source.error(ErrorCode::BudgetExceeded, Span { start: 0, end: 0 }));
    }
    let mut parser = Parser {
        source,
        position: 0,
    };
    let mut document = Document {
        elements: vec![],
        stylesheets: vec![],
    };
    let mut stack: Vec<usize> = vec![];
    let mut ids = BTreeSet::new();
    while parser.position < source.text.len() {
        let start = parser.position;
        if !source.text[start..].starts_with('<') {
            let end = source.text[start..]
                .find('<')
                .map_or(source.text.len(), |offset| start + offset);
            let value = &source.text[start..end];
            if let Some(&parent) = stack
                .last()
                .filter(|&&parent| document.elements[parent].kind == Kind::Span)
            {
                let text =
                    decode_text(source, value, start, wallpaper_scene::MAX_TEXT_BYTES, false)?;
                document.elements[parent].text.push_str(&text);
            } else if !value.bytes().all(ascii_space) {
                return Err(parser.error(ErrorCode::MixedContent, start));
            }
            parser.position = end;
            continue;
        }
        if source.text[start..].starts_with("<!--") {
            if stack
                .last()
                .is_some_and(|&index| document.elements[index].kind == Kind::Span)
            {
                return Err(parser.error(ErrorCode::MixedContent, start));
            }
            let end = source.text[start + 4..]
                .find("-->")
                .map(|offset| start + 4 + offset)
                .ok_or_else(|| parser.error(ErrorCode::Syntax, start))?;
            if source.text[start + 4..end].contains("--") {
                return Err(parser.error(ErrorCode::Syntax, start));
            }
            parser.position = end + 3;
            continue;
        }
        parser.position += 1;
        if source.text[parser.position..].starts_with('/') {
            parser.position += 1;
            let name = parser.name()?.to_owned();
            parser.spaces();
            if source.text.as_bytes().get(parser.position) != Some(&b'>') {
                return Err(parser.error(ErrorCode::Syntax, start));
            }
            parser.position += 1;
            let kind = match name.as_str() {
                "div" => Kind::Div,
                "span" => Kind::Span,
                _ => return Err(parser.error(ErrorCode::UnsupportedElement, start)),
            };
            let Some(index) = stack.pop() else {
                return Err(parser.error(ErrorCode::MismatchedTag, start));
            };
            if document.elements[index].kind != kind {
                return Err(parser.error(ErrorCode::MismatchedTag, start));
            }
            continue;
        }
        if stack
            .last()
            .is_some_and(|&index| document.elements[index].kind != Kind::Div)
        {
            return Err(parser.error(ErrorCode::MixedContent, start));
        }
        let name = parser.name()?.to_owned();
        if !["div", "span", "img", "link"].contains(&name.as_str()) {
            return Err(parser.error(ErrorCode::UnsupportedElement, start));
        }
        let (mut attributes, self_closing) = parser.attributes()?;
        if name == "link" {
            if document.stylesheets.len() >= MAX_STYLESHEETS {
                return Err(parser.error(ErrorCode::BudgetExceeded, start));
            }
            if !stack.is_empty() || !document.elements.is_empty() {
                return Err(parser.error(ErrorCode::UnsupportedElement, start));
            }
            if attributes.remove("rel").map(|(value, _)| value).as_deref() != Some("stylesheet") {
                return Err(parser.error(ErrorCode::UnsupportedAttribute, start));
            }
            let (href, href_span) = attributes
                .remove("href")
                .ok_or_else(|| parser.error(ErrorCode::UnsupportedAttribute, start))?;
            let path = RelativePath::parse(&href)
                .map_err(|_| parser.error(ErrorCode::InvalidReference, start))?;
            if !path.as_str().ends_with(".css") || !attributes.is_empty() {
                return Err(parser.error(ErrorCode::UnsupportedAttribute, start));
            }
            document.stylesheets.push(StylesheetReference {
                path,
                span: href_span,
            });
            continue;
        }
        if document.elements.len() >= wallpaper_scene::MAX_NODES || stack.len() >= MAX_DEPTH {
            return Err(parser.error(ErrorCode::BudgetExceeded, start));
        }
        let kind = match name.as_str() {
            "div" => Kind::Div,
            "span" => Kind::Span,
            _ => Kind::Image,
        };
        if self_closing && kind != Kind::Image {
            return Err(parser.error(ErrorCode::Syntax, start));
        }
        let id = attributes.remove("id").map(|(value, _)| value);
        if let Some(id) = &id {
            if !identifier(id) {
                return Err(parser.error(ErrorCode::InvalidIdentifier, start));
            }
            if !ids.insert(id.clone()) {
                return Err(parser.error(ErrorCode::DuplicateId, start));
            }
        }
        let classes = attributes
            .remove("class")
            .map(|(value, _)| {
                value
                    .split_ascii_whitespace()
                    .take(17)
                    .map(str::to_owned)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        if classes.len() > 16 || classes.iter().any(|value| !identifier(value)) {
            return Err(parser.error(ErrorCode::InvalidIdentifier, start));
        }
        let inline_style = attributes.remove("style");
        let image = if kind == Kind::Image {
            let (src, _) = attributes
                .remove("src")
                .ok_or_else(|| parser.error(ErrorCode::InvalidReference, start))?;
            Some(
                RelativePath::parse(&src)
                    .map_err(|_| parser.error(ErrorCode::InvalidReference, start))?,
            )
        } else {
            None
        };
        if !attributes.is_empty() {
            return Err(parser.error(ErrorCode::UnsupportedAttribute, start));
        }
        let index = document.elements.len();
        document.elements.push(Element {
            kind,
            parent: stack.last().copied(),
            id,
            classes,
            inline_style,
            text: String::new(),
            image,
            span: Span {
                start,
                end: parser.position,
            },
        });
        if kind != Kind::Image {
            stack.push(index);
        }
    }
    if !stack.is_empty() {
        return Err(parser.error(ErrorCode::MismatchedTag, source.text.len()));
    }
    Ok(document)
}

fn decode_text(
    source: Source<'_>,
    input: &str,
    offset: usize,
    limit: usize,
    attribute: bool,
) -> Result<String> {
    let mut output = String::new();
    let mut position = 0;
    while position < input.len() {
        let start = position;
        let mut character = input[position..].chars().next().unwrap();
        position += character.len_utf8();
        if character == '&' {
            let tail = &input[position..];
            let end = tail.find(';').filter(|&end| end <= 12).ok_or_else(|| {
                source.error(
                    ErrorCode::UnsupportedValue,
                    Span {
                        start: offset + start,
                        end: offset + position,
                    },
                )
            })?;
            let entity = &tail[..end];
            character = match entity {
                "amp" => '&',
                "lt" => '<',
                "gt" => '>',
                "quot" => '"',
                "apos" => '\'',
                _ => {
                    let value = if let Some(hex) = entity
                        .strip_prefix("#x")
                        .or_else(|| entity.strip_prefix("#X"))
                    {
                        if !hex.is_empty() && hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                            u32::from_str_radix(hex, 16).ok()
                        } else {
                            None
                        }
                    } else if let Some(decimal) = entity.strip_prefix('#') {
                        if !decimal.is_empty() && decimal.bytes().all(|byte| byte.is_ascii_digit())
                        {
                            decimal.parse::<u32>().ok()
                        } else {
                            None
                        }
                    } else {
                        None
                    };
                    value.and_then(char::from_u32).ok_or_else(|| {
                        source.error(
                            ErrorCode::UnsupportedValue,
                            Span {
                                start: offset + start,
                                end: offset + position + end + 1,
                            },
                        )
                    })?
                }
            };
            position += end + 1;
        } else if character == '\r' {
            if input.as_bytes().get(position) == Some(&b'\n') {
                position += 1;
            }
            character = '\n';
        }
        if character.is_control()
            && character != '\n'
            && !(attribute && matches!(character, '\t' | '\u{c}'))
        {
            return Err(source.error(
                ErrorCode::UnsupportedValue,
                Span {
                    start: offset + start,
                    end: offset + position,
                },
            ));
        }
        if output.len() + character.len_utf8() > limit {
            return Err(source.error(
                ErrorCode::BudgetExceeded,
                Span {
                    start: offset + start,
                    end: offset + position,
                },
            ));
        }
        output.push(character);
    }
    Ok(output)
}
