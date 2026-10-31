//! Scaling free-text quantities by a recipe multiplier (MB-0005 P3).
//!
//! Quantities are free text ("2 dl", "1/2 tl", "ripaus"), so only a leading
//! number is scaled and everything after it — the unit, a note — is kept as
//! typed. The leading number may be:
//!
//! - an integer or a decimal, with `.` or `,` (`2`, `2.5`, `2,5`);
//! - a fraction `a/b` (`1/2`), or a vulgar fraction `½ ¼ ¾`, optionally after
//!   an integer (`2½`);
//! - a range of two such numbers joined by `-` or `–` (`5-6`); both ends scale.
//!
//! A quantity with no leading number ("ripaus", "maun mukaan") is returned
//! unchanged. Scaled values are written as a whole number when they are one,
//! else as a decimal with at most two places, using `,` if the original used
//! a comma.

/// Scale `quantity`'s leading number by `factor`.
#[must_use]
pub fn scale(quantity: &str, factor: f64) -> String {
    let leading_space = quantity.len() - quantity.trim_start().len();
    let (indent, rest) = quantity.split_at(leading_space);
    let Some((first, mut used)) = number(rest) else {
        return quantity.to_owned();
    };
    let comma = rest[..used].contains(',');
    let mut scaled = format_number(first * factor, comma);

    // A range: "5-6" or "5–6".
    let after = &rest[used..];
    if let Some(dash) = after.chars().next().filter(|c| matches!(c, '-' | '–'))
        && let Some((second, len)) = number(&after[dash.len_utf8()..])
    {
        scaled.push(dash);
        scaled.push_str(&format_number(second * factor, comma));
        used += dash.len_utf8() + len;
    }
    format!("{indent}{scaled}{}", &rest[used..])
}

/// The number at the start of `s` and its length in bytes, if any.
fn number(s: &str) -> Option<(f64, usize)> {
    let digits = s.bytes().take_while(u8::is_ascii_digit).count();
    let mut end = digits;
    let mut value: f64 = if digits > 0 {
        s[..digits].parse().ok()?
    } else {
        0.0
    };

    let rest = &s[end..];
    if digits > 0
        && let Some(sep) = rest.chars().next().filter(|c| matches!(c, '.' | ',' | '/'))
    {
        let tail = &rest[1..];
        let tail_digits = tail.bytes().take_while(u8::is_ascii_digit).count();
        if tail_digits > 0 {
            let tail_value: f64 = tail[..tail_digits].parse().ok()?;
            if sep == '/' {
                if tail_value == 0.0 {
                    return None;
                }
                value /= tail_value;
            } else {
                value = format!("{}.{}", &s[..digits], &tail[..tail_digits])
                    .parse()
                    .ok()?;
            }
            return Some((value, end + 1 + tail_digits));
        }
    }

    if let Some(c) = s[end..].chars().next() {
        let fraction = match c {
            '½' => Some(0.5),
            '¼' => Some(0.25),
            '¾' => Some(0.75),
            _ => None,
        };
        if let Some(f) = fraction {
            value += f;
            end += c.len_utf8();
        }
    }
    (end > 0).then_some((value, end))
}

fn format_number(value: f64, comma: bool) -> String {
    let rounded = (value * 100.0).round() / 100.0;
    let text = format!("{rounded}");
    if comma { text.replace('.', ",") } else { text }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scales_leading_numbers_and_keeps_the_rest() {
        let cases = [
            ("2 dl", 2.0, "4 dl"),
            ("2,5 dl", 2.0, "5 dl"),
            ("2,5 dl", 3.0, "7,5 dl"),
            ("1.5 kg", 2.0, "3 kg"),
            ("1/2 tl", 2.0, "1 tl"),
            ("1/2 tl", 3.0, "1.5 tl"),
            ("½ tl", 2.0, "1 tl"),
            ("2½ dl", 2.0, "5 dl"),
            ("¾ l", 0.5, "0.38 l"),
            ("5-6 kpl", 2.0, "10-12 kpl"),
            ("5–6 kpl", 0.5, "2.5–3 kpl"),
            ("3", 2.0, "6"),
            ("  2 dl", 2.0, "  4 dl"),
            ("1/3 dl", 3.0, "1 dl"),
            ("400 g (1 prk)", 2.0, "800 g (1 prk)"),
        ];
        for (quantity, factor, expected) in cases {
            assert_eq!(scale(quantity, factor), expected, "{quantity} × {factor}");
        }
    }

    #[test]
    fn passes_non_numeric_quantities_through() {
        for quantity in [
            "ripaus",
            "maun mukaan",
            "",
            "  ",
            "n. 2 dl",
            "1/0 tl",
            "-2 dl",
        ] {
            assert_eq!(scale(quantity, 2.0), quantity);
        }
    }

    #[test]
    fn a_bare_separator_is_not_part_of_the_number() {
        assert_eq!(scale("2. kerros", 2.0), "4. kerros");
        assert_eq!(scale("2/ dl", 2.0), "4/ dl");
        assert_eq!(scale("2-", 2.0), "4-");
    }
}
