use super::*;
use wallpaper_scene::Color;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Selector {
    Id(String),
    Class(String),
}
impl Selector {
    pub fn specificity(&self) -> u8 {
        match self {
            Self::Id(_) => 2,
            Self::Class(_) => 1,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Property {
    Left(f32),
    Top(f32),
    Width(f32),
    Height(f32),
    FontSize(f32),
    Color(Color),
    BackgroundColor(Color),
    PositionAbsolute,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Declaration {
    pub property: Property,
    pub span: Span,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Rule {
    pub selector: Selector,
    pub declarations: Vec<Declaration>,
    pub span: Span,
}
#[derive(Debug)]
pub struct Stylesheet {
    pub rules: Vec<Rule>,
    pub declaration_count: usize,
}

pub fn parse(source: Source<'_>) -> Result<Stylesheet> {
    let masked = mask_comments(source, source.text, 0)?;
    let mut position = 0;
    let mut stylesheet = Stylesheet {
        rules: vec![],
        declaration_count: 0,
    };
    while position < masked.len() {
        while masked
            .as_bytes()
            .get(position)
            .is_some_and(|&byte| ascii_space(byte))
        {
            position += 1;
        }
        if position == masked.len() {
            break;
        }
        let start = position;
        let open = masked[position..]
            .find('{')
            .map(|index| index + position)
            .ok_or_else(|| {
                source.error(
                    ErrorCode::Syntax,
                    Span {
                        start,
                        end: masked.len(),
                    },
                )
            })?;
        let value = masked[start..open].trim_ascii();
        let selector = if let Some(id) = value.strip_prefix('#').filter(|id| identifier(id)) {
            Selector::Id(id.into())
        } else if let Some(class) = value.strip_prefix('.').filter(|class| identifier(class)) {
            Selector::Class(class.into())
        } else {
            return Err(source.error(ErrorCode::UnsupportedSelector, Span { start, end: open }));
        };
        let close = masked[open + 1..]
            .find('}')
            .map(|index| index + open + 1)
            .ok_or_else(|| {
                source.error(
                    ErrorCode::Syntax,
                    Span {
                        start: open,
                        end: masked.len(),
                    },
                )
            })?;
        if stylesheet.rules.len() >= MAX_RULES {
            return Err(source.error(
                ErrorCode::BudgetExceeded,
                Span {
                    start,
                    end: close + 1,
                },
            ));
        }
        let declarations = parse_masked(source, &masked[open + 1..close], open + 1)?;
        stylesheet.declaration_count += declarations.len();
        if stylesheet.declaration_count > MAX_DECLARATIONS {
            return Err(source.error(
                ErrorCode::BudgetExceeded,
                Span {
                    start,
                    end: close + 1,
                },
            ));
        }
        stylesheet.rules.push(Rule {
            selector,
            declarations,
            span: Span {
                start,
                end: close + 1,
            },
        });
        position = close + 1;
    }
    Ok(stylesheet)
}

/// Inline declarations preserve offsets into their enclosing HTML source.
pub fn declarations(source: Source<'_>, span: Span) -> Result<Vec<Declaration>> {
    let value = source
        .text
        .get(span.start..span.end)
        .ok_or_else(|| source.error(ErrorCode::Syntax, span))?;
    let masked = mask_comments(source, value, span.start)?;
    parse_masked(source, &masked, span.start)
}
fn mask_comments(source: Source<'_>, input: &str, base: usize) -> Result<String> {
    if input.len() > MAX_SOURCE_BYTES {
        return Err(source.error(
            ErrorCode::BudgetExceeded,
            Span {
                start: base,
                end: base,
            },
        ));
    }
    let mut bytes = input.as_bytes().to_vec();
    let mut position = 0;
    while position < bytes.len() {
        if bytes[position..].starts_with(b"/*") {
            let end = input[position + 2..]
                .find("*/")
                .map(|index| position + index + 4)
                .ok_or_else(|| {
                    source.error(
                        ErrorCode::Syntax,
                        Span {
                            start: base + position,
                            end: base + input.len(),
                        },
                    )
                })?;
            for byte in &mut bytes[position..end] {
                if *byte != b'\n' && *byte != b'\r' {
                    *byte = b' ';
                }
            }
            position = end;
        } else {
            // Strings/escapes are outside this grammar; never mistake their content for comments.
            if matches!(bytes[position], b'\'' | b'"' | b'\\') {
                return Err(source.error(
                    ErrorCode::UnsupportedValue,
                    Span {
                        start: base + position,
                        end: base + position + 1,
                    },
                ));
            }
            position += 1;
        }
    }
    // Original UTF-8 remains intact outside whole comment ranges; inside is ASCII.
    Ok(String::from_utf8(bytes).unwrap())
}
fn parse_masked(source: Source<'_>, input: &str, base: usize) -> Result<Vec<Declaration>> {
    let mut declarations = vec![];
    let mut offset = 0;
    for part in input.split(';') {
        let leading = part.len() - part.trim_ascii_start().len();
        let value = part.trim_ascii();
        let span = Span {
            start: base + offset + leading,
            end: base + offset + part.len(),
        };
        offset += part.len() + 1;
        if value.is_empty() {
            continue;
        }
        if declarations.len() >= 32 {
            return Err(source.error(ErrorCode::BudgetExceeded, span));
        }
        let (name, value) = value
            .split_once(':')
            .ok_or_else(|| source.error(ErrorCode::Syntax, span))?;
        let name = name.trim_ascii();
        let value = value.trim_ascii();
        let fail = |code| source.error(code, span);
        if value.contains('!') || value.contains('{') || value.contains('}') {
            return Err(fail(ErrorCode::UnsupportedValue));
        }
        let property = match name {
            "left" => Property::Left(
                pixels(value, -32768.0, 32768.0).ok_or_else(|| fail(ErrorCode::InvalidNumber))?,
            ),
            "top" => Property::Top(
                pixels(value, -32768.0, 32768.0).ok_or_else(|| fail(ErrorCode::InvalidNumber))?,
            ),
            "width" => Property::Width(
                pixels(value, 0.0, 32768.0).ok_or_else(|| fail(ErrorCode::InvalidNumber))?,
            ),
            "height" => Property::Height(
                pixels(value, 0.0, 32768.0).ok_or_else(|| fail(ErrorCode::InvalidNumber))?,
            ),
            "font-size" => Property::FontSize(
                pixels(value, 1.0, 512.0).ok_or_else(|| fail(ErrorCode::InvalidNumber))?,
            ),
            "color" => Property::Color(color(value).ok_or_else(|| fail(ErrorCode::InvalidColor))?),
            "background-color" => Property::BackgroundColor(
                color(value).ok_or_else(|| fail(ErrorCode::InvalidColor))?,
            ),
            "position" if value == "absolute" => Property::PositionAbsolute,
            "position" => return Err(fail(ErrorCode::UnsupportedValue)),
            _ => return Err(fail(ErrorCode::UnsupportedProperty)),
        };
        declarations.push(Declaration { property, span });
    }
    Ok(declarations)
}
fn pixels(value: &str, minimum: f32, maximum: f32) -> Option<f32> {
    let value = if value == "0" {
        value
    } else {
        value.strip_suffix("px")?
    };
    if value.len() > 32 {
        return None;
    }
    let unsigned = value.strip_prefix(['+', '-']).unwrap_or(value);
    if unsigned.is_empty()
        || unsigned.ends_with('.')
        || unsigned.bytes().filter(|&byte| byte == b'.').count() > 1
        || !unsigned
            .bytes()
            .all(|byte| byte.is_ascii_digit() || byte == b'.')
        || !unsigned.bytes().any(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    let number = value.parse::<f32>().ok()?;
    (number.is_finite() && (minimum..=maximum).contains(&number)).then_some(number)
}
fn color(value: &str) -> Option<Color> {
    if value == "transparent" {
        return Some(Color([0; 4]));
    }
    let hex = value.strip_prefix('#')?;
    if !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    let mut rgba = [255; 4];
    match hex.len() {
        3 | 4 => {
            for (index, byte) in hex.bytes().enumerate() {
                rgba[index] = (byte as char).to_digit(16)? as u8 * 17;
            }
        }
        6 | 8 => {
            for (index, slot) in rgba.iter_mut().enumerate().take(hex.len() / 2) {
                *slot = u8::from_str_radix(&hex[index * 2..index * 2 + 2], 16).ok()?;
            }
        }
        _ => return None,
    }
    Some(Color(rgba))
}
