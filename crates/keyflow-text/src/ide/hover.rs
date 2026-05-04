//! Hover info for the token under the cursor.
//!
//! Today this is rule-based and works against the token slice. Future
//! revisions should consult the parsed `Chart` to enrich hovers (e.g.
//! resolve scale degrees against the active key, show measure / beat
//! position, look up melody-variable definitions).

use crate::chart::Chart;
use crate::parsing::TextSpan;

#[derive(Debug, Clone)]
pub struct HoverInfo {
    /// Multi-line markdown body. Renderers should treat the leading line as
    /// a one-liner summary.
    pub markdown: String,
    /// Range of the token the hover applies to.
    pub range: TextSpan,
}

/// Compute hover info at `byte_offset`. Returns `None` if no recognized
/// token sits under the cursor.
#[must_use]
pub fn hover(text: &str, byte_offset: usize, chart: &Chart) -> Option<HoverInfo> {
    let offset = byte_offset.min(text.len());
    let bytes = text.as_bytes();

    // Find the whitespace-bounded token under the cursor.
    let mut start = offset;
    while start > 0 {
        let b = bytes[start - 1];
        if b.is_ascii_whitespace() || b == b'|' {
            break;
        }
        start -= 1;
    }
    let mut end = offset;
    while end < bytes.len() {
        let b = bytes[end];
        if b.is_ascii_whitespace() || b == b'|' {
            break;
        }
        end += 1;
    }
    if start == end {
        return None;
    }

    let token = &text[start..end];
    let range = TextSpan::new(start, end - start);

    if let Some(name) = token.strip_prefix('$') {
        let body = if let Some(melody) = chart.melody_variables.get(name) {
            format!(
                "**`${}`** — recall stored melody\n\n```\n{}\n```",
                name, melody
            )
        } else {
            format!(
                "**`${}`** — melody-variable recall\n\n_(unknown variable; \
                 define it earlier with `{} = m{{ … }}`)_",
                name, name
            )
        };
        return Some(HoverInfo {
            markdown: body,
            range,
        });
    }

    if let Some(name) = token.strip_prefix('/') {
        return Some(HoverInfo {
            markdown: format!("**`/{}`** — chart command / config directive", name),
            range,
        });
    }

    // Scale-degree number (1..=7) or roman numeral.
    if let Ok(deg) = token.parse::<u8>() {
        if (1..=7).contains(&deg) {
            let key_str = chart
                .current_key
                .as_ref()
                .map(|k| format!("{}", k))
                .unwrap_or_else(|| "C major".into());
            return Some(HoverInfo {
                markdown: format!("**`{}`** — scale degree {} of **{}**", deg, deg, key_str),
                range,
            });
        }
    }

    // Looks like a chord token (starts with a note letter or accidental).
    let first = token.chars().next()?;
    if first.is_ascii_alphabetic() && first.is_ascii_uppercase() {
        return Some(HoverInfo {
            markdown: format!("**`{}`** — chord", token),
            range,
        });
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hover_on_dollar_token() {
        let chart = Chart::new();
        let h = hover("| 1 $mainRiff |", 7, &chart).expect("hover for $name");
        assert!(h.markdown.contains("$main"));
    }

    #[test]
    fn hover_on_scale_degree() {
        let chart = Chart::new();
        let h = hover("| 4 |", 2, &chart).expect("hover for digit");
        assert!(h.markdown.contains("scale degree"));
    }

    #[test]
    fn hover_returns_none_on_whitespace() {
        let chart = Chart::new();
        assert!(hover("   ", 1, &chart).is_none());
    }
}
