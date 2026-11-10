//! Aliases, quoted staff text, and classical dynamics / hairpins written
//! as their own line or token.

use super::tokens::tokenize_with_spans;
use crate::text::chart::parser::ChartParser;
use keyflow_proto::chart::{Dynamic, DynamicLevel, Hairpin, HairpinKind, Placement, StaffText};

impl<'a> ChartParser<'a> {
    pub(super) fn parse_quoted_text_token(token: &str) -> Option<(String, Placement)> {
        let (token, placement) = if let Some(rest) = token.strip_prefix("^\"") {
            (format!("\"{rest}"), Placement::Above)
        } else if let Some(rest) = token.strip_prefix("_\"") {
            (format!("\"{rest}"), Placement::Below)
        } else {
            (token.to_string(), Placement::Below)
        };
        let inner = token.strip_prefix('"')?.strip_suffix('"')?;
        Some((unescape_quoted(inner), placement))
    }

    pub(in crate::text::chart::parser) fn parse_alias_declaration(
        line: &str,
    ) -> Option<(String, String)> {
        let (name, value) = if let Some(rest) = line.strip_prefix("\\alias ") {
            let mut parts = rest.splitn(2, char::is_whitespace);
            (parts.next()?.trim(), parts.next()?.trim())
        } else {
            let rest = line.strip_prefix("let ")?;
            let (name, value) = rest.split_once('=')?;
            (name.trim(), value.trim())
        };
        if name.is_empty() || value.is_empty() {
            return None;
        }
        let value = value
            .strip_prefix('{')
            .and_then(|inner| inner.strip_suffix('}'))
            .map(str::trim)
            .unwrap_or(value);
        Some((name.to_string(), value.to_string()))
    }

    pub(in crate::text::chart::parser) fn expand_aliases_in_line(&self, line: &str) -> String {
        let spans = tokenize_with_spans(line);
        if spans.is_empty() {
            return line.to_string();
        }

        let mut out = String::new();
        let mut cursor = 0usize;
        let mut changed = false;
        for span in spans {
            out.push_str(&line[cursor..span.start]);
            let token = &line[span.as_range()];
            if let Some(expanded) = self.expand_alias_token(token) {
                out.push_str(&expanded);
                changed = true;
            } else {
                out.push_str(token);
            }
            cursor = span.start + span.len;
        }
        out.push_str(&line[cursor..]);

        if changed { out } else { line.to_string() }
    }

    fn expand_alias_token(&self, token: &str) -> Option<String> {
        if let Some(name) = token.strip_prefix("^<").and_then(|t| t.strip_suffix('>')) {
            return self.aliases.get(name).map(|value| {
                apply_alias_placement(value, Some(Placement::Above))
                    .unwrap_or_else(|| value.to_string())
            });
        }
        if let Some(name) = token.strip_prefix("_<").and_then(|t| t.strip_suffix('>')) {
            return self.aliases.get(name).map(|value| {
                apply_alias_placement(value, Some(Placement::Below))
                    .unwrap_or_else(|| value.to_string())
            });
        }
        if let Some(name) = token.strip_prefix('<').and_then(|t| t.strip_suffix('>')) {
            return self.aliases.get(name).map(|value| {
                apply_alias_placement(value, None).unwrap_or_else(|| value.to_string())
            });
        }
        if let Some((chord, name)) = split_chord_attached_alias(token) {
            return self
                .aliases
                .get(name)
                .and_then(|value| alias_quoted_payload(value))
                .map(|payload| format!("{chord}\"{payload}\""));
        }
        None
    }

    pub(super) fn parse_standalone_staff_text_line(&self, line: &str) -> Option<Vec<StaffText>> {
        if line.starts_with("<<") {
            return None;
        }
        let expanded = self.expand_aliases_in_line(line);
        let spans = tokenize_with_spans(&expanded);
        if spans.is_empty() {
            return None;
        }

        let mut items = Vec::new();
        for span in spans {
            let token = &expanded[span.as_range()];
            let (text, placement) = Self::parse_quoted_text_token(token)?;
            items.push(StaffText {
                text,
                beat: 1,
                placement,
                source_default_x: None,
                boxed: false,
                bold: false,
                italic: false,
            });
        }

        Some(items)
    }

    pub(super) fn parse_classical_dynamic_line(line: &str) -> Option<Dynamic> {
        let value = line
            .strip_prefix("\\dyn ")
            .or_else(|| line.strip_prefix("\\dynamic "))
            .or_else(|| line.strip_prefix("dyn "))
            .or_else(|| line.strip_prefix("dynamic "))?
            .trim();
        let mut parts = value.split_whitespace();
        let token = parts.next()?;
        let (level_token, beat) = token
            .split_once('@')
            .map(|(level, beat)| (level, beat.parse::<u8>().ok().unwrap_or(1)))
            .unwrap_or((token, 1));
        let level = Self::parse_dynamic_level(level_token)?;
        let placement = parts
            .next()
            .and_then(Self::parse_placement)
            .unwrap_or(Placement::Below);
        Some(Dynamic {
            level,
            beat,
            placement,
        })
    }

    pub(super) fn parse_hairpin_line(line: &str) -> Option<Hairpin> {
        let value = line
            .strip_prefix("\\hairpin ")
            .or_else(|| line.strip_prefix("hairpin "))?
            .trim();
        let mut parts = value.split_whitespace();
        let kind = match parts.next()? {
            "<" | "crescendo" | "cresc" => HairpinKind::Crescendo,
            ">" | "decrescendo" | "decresc" | "dim" | "diminuendo" => HairpinKind::Decrescendo,
            _ => return None,
        };
        let span = parts.next().unwrap_or("1..1");
        let (start, end) = span
            .split_once("..")
            .or_else(|| span.split_once('-'))
            .unwrap_or(("1", "1"));
        let start_beat = start
            .trim_start_matches('@')
            .parse::<u8>()
            .ok()
            .unwrap_or(1);
        let (end_measure_offset, end_beat) = Self::parse_hairpin_end(end, start_beat);
        let placement = parts
            .next()
            .and_then(Self::parse_placement)
            .unwrap_or(Placement::Below);
        Some(Hairpin {
            kind,
            start_beat,
            end_measure_offset,
            end_beat,
            placement,
        })
    }

    /// A hairpin's end: `4` is beat 4 of its own bar; `+2:3` is beat 3 two
    /// bars later; `+2` the end of the bar two bars later.
    pub(super) fn parse_hairpin_end(end: &str, start_beat: u8) -> (u16, u8) {
        let end = end.trim_start_matches('@');
        if let Some(later) = end.strip_prefix('+') {
            let (bars, beat) = later.split_once(':').unwrap_or((later, ""));
            let bars = bars.parse::<u16>().unwrap_or(0);
            let beat = beat.parse::<u8>().unwrap_or(u8::MAX);
            return (bars, beat);
        }
        (0, end.parse::<u8>().unwrap_or(start_beat))
    }

    pub(super) fn parse_dynamic_level(value: &str) -> Option<DynamicLevel> {
        match value {
            "ppp" => Some(DynamicLevel::Ppp),
            "pp" => Some(DynamicLevel::Pp),
            "p" => Some(DynamicLevel::P),
            "mp" => Some(DynamicLevel::Mp),
            "mf" => Some(DynamicLevel::Mf),
            "f" => Some(DynamicLevel::F),
            "ff" => Some(DynamicLevel::Ff),
            "fff" => Some(DynamicLevel::Fff),
            "sf" => Some(DynamicLevel::Sf),
            "sfz" => Some(DynamicLevel::Sfz),
            "fp" => Some(DynamicLevel::Fp),
            _ => None,
        }
    }

    pub(super) fn parse_placement(value: &str) -> Option<Placement> {
        match value.to_ascii_lowercase().as_str() {
            "above" | "^" => Some(Placement::Above),
            "below" | "_" => Some(Placement::Below),
            _ => None,
        }
    }
}

pub(super) fn unescape_quoted(s: &str) -> String {
    let mut out = String::new();
    let mut escaped = false;
    for ch in s.chars() {
        if escaped {
            out.push(ch);
            escaped = false;
        } else if ch == '\\' {
            escaped = true;
        } else {
            out.push(ch);
        }
    }
    if escaped {
        out.push('\\');
    }
    out
}

pub(super) fn split_chord_attached_quote(token: &str) -> Option<(&str, &str)> {
    let quote_start = token.find('"')?;
    if quote_start == 0 || !token.ends_with('"') {
        return None;
    }
    Some((
        &token[..quote_start],
        &token[quote_start + 1..token.len() - 1],
    ))
}

fn split_chord_attached_alias(token: &str) -> Option<(&str, &str)> {
    let alias_start = token.find('<')?;
    if alias_start == 0 || !token.ends_with('>') {
        return None;
    }
    Some((
        &token[..alias_start],
        &token[alias_start + 1..token.len() - 1],
    ))
}

fn alias_quoted_payload(value: &str) -> Option<&str> {
    let value = value.trim();
    let value = value
        .strip_prefix("^\"")
        .or_else(|| value.strip_prefix("_\""))
        .or_else(|| value.strip_prefix('"'))?;
    value.strip_suffix('"')
}

fn apply_alias_placement(value: &str, placement: Option<Placement>) -> Option<String> {
    let value = value.trim();
    let payload = alias_quoted_payload(value)?;
    let prefix = match placement {
        Some(Placement::Below) => "_",
        Some(Placement::Above) => "^",
        None if value.starts_with("^\"") => "^",
        None if value.starts_with("_\"") => "_",
        None => "",
    };
    Some(format!("{prefix}\"{payload}\""))
}
