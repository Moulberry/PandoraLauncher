use std::ops::Range;

use serde::Deserialize;

#[derive(Debug, Default, PartialEq, Eq, Clone)]
pub struct TextComponentStyle {
    pub colour: Option<u32>,
    pub bold: Option<bool>,
    pub italic: Option<bool>,
    pub underlined: Option<bool>,
    pub strikethrough: Option<bool>,
}

#[derive(Debug)]
pub struct TextComponentRun {
    pub range: Range<usize>,
    pub style: TextComponentStyle,
}

#[derive(Debug, Default)]
pub struct FlatTextComponent {
    pub content: String,
    pub runs: Vec<TextComponentRun>,
}

pub fn deserialize_flat_text_component_json<'de, D>(deserializer: D) -> Result<FlatTextComponent, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let json_value = serde_json::Value::deserialize(deserializer)?;

    let mut component = FlatTextComponent::default();

    append_flat(&mut component, json_value);

    Ok(component)
}

fn append_flat(component: &mut FlatTextComponent, value: serde_json::Value) {
    use std::fmt::Write;
    match value {
        serde_json::Value::Null => {
            component.content.push_str("null");
        },
        serde_json::Value::Bool(value) => {
            let value_str = if value {
                "true"
            } else {
                "false"
            };
            component.content.push_str(value_str);
        },
        serde_json::Value::Number(number) => {
            _ = write!(&mut component.content, "{}", number);
        },
        serde_json::Value::String(string) => {
            append_string(component, string);
        },
        serde_json::Value::Array(values) => {
            for value in values {
                append_flat(component, value);
            }
        },
        serde_json::Value::Object(mut map) => {
            let mut style = TextComponentStyle::default();

            if let Some(serde_json::Value::String(color)) = map.get("color") {
                if color.starts_with('#') {
                    if let Ok(color) = u32::from_str_radix(&color[1..], 16) {
                        style.colour = Some(color);
                    }
                } else {
                    style.colour = match color.as_str() {
                        "black" => Some(0x000000),
                        "dark_blue" => Some(0x0000aa),
                        "dark_green" => Some(0x00aa00),
                        "dark_aqua" => Some(0x00aaaa),
                        "dark_red" => Some(0xaa0000),
                        "dark_purple" => Some(0xaa00aa),
                        "gold" => Some(0xffaa00),
                        "gray" => Some(0xaaaaaa),
                        "dark_gray" => Some(0x555555),
                        "blue" => Some(0x5555ff),
                        "green" => Some(0x55ff55),
                        "aqua" => Some(0x55ffff),
                        "red" => Some(0xff5555),
                        "light_purple" => Some(0xff55ff),
                        "yellow" => Some(0xffff55),
                        "white" => Some(0xffffff),
                        _ => None,
                    };
                }
            }
            if let Some(serde_json::Value::Bool(value)) = map.get("bold") {
                style.bold = Some(*value);
            }
            if let Some(serde_json::Value::Bool(value)) = map.get("italic") {
                style.italic = Some(*value);
            }
            if let Some(serde_json::Value::Bool(value)) = map.get("underlined") {
                style.underlined = Some(*value);
            }
            if let Some(serde_json::Value::Bool(value)) = map.get("strikethrough") {
                style.strikethrough = Some(*value);
            }

            let index = component.runs.len();
            let start = component.content.len();

            let text = if let Some(serde_json::Value::String(string)) = map.remove("text") {
                Some(string)
            } else if let Some(serde_json::Value::String(string)) = map.remove("fallback") {
                Some(string)
            } else {
                None
            };

            if let Some(text) = text {
                append_string(component, text);
            }

            if let Some(extra) = map.remove("extra") {
                append_flat(component, extra);
            }

            let end = component.content.len();

            if end > start && style != TextComponentStyle::default() {
                component.runs.insert(index, TextComponentRun {
                    range: start..end,
                    style
                });
            }
        },
    }
}

fn append_string(component: &mut FlatTextComponent, mut text: String) {
    let mut last_legacy_color = None;
    let mut current_style = TextComponentStyle::default();
    while let Some(pos) = text.find('\u{00a7}') {
        if let Some((from, style)) = last_legacy_color.take() {
            if pos > from {
                component.runs.push(TextComponentRun {
                    range: from..pos,
                    style,
                });
            }
        }

        _ = text.remove(pos);

        if pos < text.len() {
            let next = text.remove(pos);
            match next {
                '0' => current_style.colour = Some(0x000000),
                '1' => current_style.colour = Some(0x0000aa),
                '2' => current_style.colour = Some(0x00aa00),
                '3' => current_style.colour = Some(0x00aaaa),
                '4' => current_style.colour = Some(0xaa0000),
                '5' => current_style.colour = Some(0xaa00aa),
                '6' => current_style.colour = Some(0xffaa00),
                '7' => current_style.colour = Some(0xaaaaaa),
                '8' => current_style.colour = Some(0x555555),
                '9' => current_style.colour = Some(0x5555ff),
                'a' => current_style.colour = Some(0x55ff55),
                'b' => current_style.colour = Some(0x55ffff),
                'c' => current_style.colour = Some(0xff5555),
                'd' => current_style.colour = Some(0xff55ff),
                'e' => current_style.colour = Some(0xffff55),
                'f' => current_style.colour = Some(0xffffff),
                'l' => current_style.bold = Some(true),
                'm' => current_style.strikethrough = Some(true),
                'n' => current_style.underlined = Some(true),
                'o' => current_style.italic = Some(true),
                'r' => current_style = TextComponentStyle::default(),
                _ => {}
            }
            if current_style != TextComponentStyle::default() {
                last_legacy_color = Some((pos, current_style.clone()));
            }
        }
    }

    if let Some((from, style)) = last_legacy_color.take() {
        let to = text.len();
        if to > from {
            component.runs.push(TextComponentRun {
                range: from..to,
                style,
            });
        }
    }

    component.content.push_str(&text);
}
