// SPDX-License-Identifier: MIT
use num_bigint::BigUint;
use num_traits::ToPrimitive;
use serde::{Deserialize, Serialize};

/// JSON-safe representation: JavaScript parseInt can overflow to +Infinity.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Width {
    Unwrap,
    Finite(f64),
    Infinity,
}
pub const INVALID_NUMBER: &str =
    "Please enter a valid number (or leave blank for no text wrapping)";
pub const NON_POSITIVE: &str =
    "Width must be a positive number (or leave blank for no text wrapping)";

/// ECMAScript WhiteSpace + LineTerminator (not Rust Unicode White_Space).
pub fn js_space(c: char) -> bool {
    matches!(c, '\u{0009}'..='\u{000d}' | '\u{0020}' | '\u{00a0}' | '\u{1680}' |
        '\u{2000}'..='\u{200a}' | '\u{2028}' | '\u{2029}' | '\u{202f}' |
        '\u{205f}' | '\u{3000}' | '\u{feff}')
}

pub fn validate_width(input: &str) -> Result<Width, &'static str> {
    let input = input.trim_matches(js_space);
    if input.is_empty() {
        return Ok(Width::Unwrap);
    }
    let (negative, digits) = if let Some(s) = input.strip_prefix('-') {
        (true, s)
    } else {
        (false, input.strip_prefix('+').unwrap_or(input))
    };
    let (radix, digits) = if digits.starts_with("0x") || digits.starts_with("0X") {
        (16, &digits[2..])
    } else {
        (10, digits)
    };
    let count = digits
        .bytes()
        .take_while(|c| match radix {
            16 => c.is_ascii_hexdigit(),
            _ => c.is_ascii_digit(),
        })
        .count();
    if count == 0 {
        return Err(INVALID_NUMBER);
    }
    // Decimal conversion uses the platform's correctly rounded binary64 parser.
    // Hexadecimal uses arbitrary precision then nearest-even binary64 conversion.
    let value = if radix == 10 {
        digits[..count].parse::<f64>().unwrap_or(f64::INFINITY)
    } else {
        BigUint::parse_bytes(&digits.as_bytes()[..count], 16)
            .and_then(|n| n.to_f64())
            .unwrap_or(f64::INFINITY)
    };
    if negative || value == 0.0 {
        return Err(NON_POSITIVE);
    }
    Ok(if value.is_infinite() {
        Width::Infinity
    } else {
        Width::Finite(value)
    })
}

pub fn format_message(width: Width) -> String {
    match width {
        Width::Unwrap => "No text wrapping applied".into(),
        Width::Infinity => "Text re-wrapped at Infinity characters".into(),
        Width::Finite(n) if n == 0.0 || n.is_nan() => "No text wrapping applied".into(),
        Width::Finite(n) => format!(
            "Text re-wrapped at {} characters",
            ryu_js::Buffer::new().format(n)
        ),
    }
}

pub fn rewrap_text(text: &str, width: Option<f64>) -> String {
    if text.is_empty() || width.is_some_and(|w| w <= 0.0) {
        return text.into();
    }
    rewrap_paragraphs(text, width.unwrap_or(9_007_199_254_740_991.0))
}

pub fn wrap(text: &str, width: Width) -> String {
    rewrap_text(
        text,
        match width {
            Width::Unwrap => None,
            Width::Finite(n) => Some(n),
            Width::Infinity => Some(f64::INFINITY),
        },
    )
}

pub fn rewrap_paragraphs(text: &str, width: f64) -> String {
    // Exact greedy /\n\s*\n/ splitting: CR alone never starts a separator.
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let mut paragraphs = Vec::new();
    let (mut start, mut i) = (0, 0);
    while i < chars.len() {
        if chars[i].1 == '\n' {
            let mut j = i + 1;
            let mut last_lf = None;
            while j < chars.len() && js_space(chars[j].1) {
                if chars[j].1 == '\n' {
                    last_lf = Some(j);
                }
                j += 1;
            }
            if let Some(end) = last_lf {
                paragraphs.push(&text[start..chars[i].0]);
                start = chars[end].0 + 1;
                i = end + 1;
                continue;
            }
        }
        i += 1;
    }
    paragraphs.push(&text[start..]);
    paragraphs
        .into_iter()
        .filter_map(|p| {
            let words: Vec<&str> = p.split(js_space).filter(|w| !w.is_empty()).collect();
            let mut lines = Vec::new();
            let mut line = String::new();
            let mut len = 0;
            for word in words {
                let word_len = word.encode_utf16().count();
                let candidate = len + usize::from(!line.is_empty()) + word_len;
                if candidate as f64 > width {
                    if !line.is_empty() {
                        lines.push(std::mem::take(&mut line));
                    }
                    line.push_str(word);
                    len = word_len;
                } else {
                    if !line.is_empty() {
                        line.push(' ');
                    }
                    line.push_str(word);
                    len = candidate;
                }
            }
            if !line.is_empty() {
                lines.push(line);
            }
            if lines.is_empty() {
                None
            } else {
                Some(lines.join("\n"))
            }
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}
