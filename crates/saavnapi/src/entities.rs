//! HTML entity decoding for Saavn display text.
//!
//! The API returns titles, subtitles, artist names, descriptions,
//! etc. with HTML escapes (`&amp;`, `&quot;`, `&#39;`, `&nbsp;`,
//! `&#8217;`, ...). Every response passes through
//! `SaavnClient::fetch`, which decodes entities in all string values
//! before callers map the payload, so models always carry
//! display-ready text. No new dependency — a hand-rolled decoder
//! for the entities Saavn actually emits.

use serde_json::Value;

/// Decode HTML entities in every string inside a JSON value, in place.
pub(crate) fn decode_entities(value: &mut Value) {
    match value {
        Value::String(text) => {
            let decoded = decode_str(text);
            if decoded != *text {
                *text = decoded;
            }
        }
        Value::Array(items) => items.iter_mut().for_each(decode_entities),
        Value::Object(map) => map.values_mut().for_each(decode_entities),
        _ => {}
    }
}

const NAMED: &[(&str, char)] = &[
    ("amp", '&'),
    ("lt", '<'),
    ("gt", '>'),
    ("quot", '"'),
    ("apos", '\''),
    ("nbsp", ' '),
    ("rsquo", '\u{2019}'),
    ("lsquo", '\u{2018}'),
    ("rdquo", '\u{201D}'),
    ("ldquo", '\u{201C}'),
    ("hellip", '\u{2026}'),
    ("mdash", '\u{2014}'),
    ("ndash", '\u{2013}'),
    ("copy", '\u{00A9}'),
    ("reg", '\u{00AE}'),
    ("trade", '\u{2122}'),
];

/// Decode `&name;`, `&#123;` and `&#x1F;` entities in one pass.
/// Unknown, malformed or control-character entities, and a stray
/// `&` with no `;` nearby, are left unchanged.
pub(crate) fn decode_str(input: &str) -> String {
    if !input.contains('&') {
        return input.to_string();
    }
    let bytes = input.as_bytes();
    let mut out = String::with_capacity(input.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'&' {
            let next = input[i..]
                .find('&')
                .map(|pos| i + pos)
                .unwrap_or(bytes.len());
            out.push_str(&input[i..next]);
            i = next;
            continue;
        }
        let rest = &input[i + 1..];
        let Some(semi) = rest.find(';') else {
            out.push('&');
            i += 1;
            continue;
        };
        if semi > 10 {
            // No real entity is this long; treat '&' as literal.
            out.push('&');
            i += 1;
            continue;
        }
        let body = &rest[..semi];
        // `body` is the text between '&' and ';', so a numeric
        // entity looks like "#39" / "#x27" (no leading '&').
        let decoded: Option<char> =
            if let Some(hex) = body.strip_prefix("#x").or_else(|| body.strip_prefix("#X")) {
                u32::from_str_radix(hex, 16)
                    .ok()
                    .and_then(char::from_u32)
                    .filter(|c| !c.is_control())
            } else if let Some(dec) = body.strip_prefix('#') {
                dec.parse::<u32>()
                    .ok()
                    .and_then(char::from_u32)
                    .filter(|c| !c.is_control())
            } else {
                NAMED
                    .iter()
                    .find(|(name, _)| *name == body)
                    .map(|(_, c)| *c)
            };
        match decoded {
            Some(c) => {
                out.push(c);
                i += 1 + semi + 1; // '&' + body + ';'
            }
            None => {
                out.push('&');
                i += 1;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn named_entities_decode() {
        assert_eq!(decode_str("Tom &amp; Jerry"), "Tom & Jerry");
        assert_eq!(decode_str("&quot;Hello&quot;"), "\"Hello\"");
        assert_eq!(decode_str("a &lt; b &gt; c"), "a < b > c");
        assert_eq!(decode_str("it&apos;s"), "it's");
        assert_eq!(decode_str("A&nbsp;B"), "A B");
        assert_eq!(decode_str("don&rsquo;t"), "don\u{2019}t");
        assert_eq!(decode_str("wait&hellip;"), "wait\u{2026}");
    }

    #[test]
    fn numeric_entities_decode() {
        assert_eq!(decode_str("it&#39;s"), "it's");
        assert_eq!(decode_str("it&#x27;s"), "it's");
        assert_eq!(decode_str("&#8217;"), "\u{2019}");
        assert_eq!(decode_str("&#x2019;"), "\u{2019}");
        assert_eq!(decode_str("&#169;"), "\u{00A9}");
    }

    #[test]
    fn unknown_and_stray_ampersands_are_left_alone() {
        assert_eq!(decode_str("&bogus; stays"), "&bogus; stays");
        assert_eq!(decode_str("fish & chips"), "fish & chips");
        assert_eq!(decode_str("a & b; c"), "a & b; c");
        assert_eq!(decode_str("&#xZZ;"), "&#xZZ;");
        assert_eq!(decode_str("&#0;"), "&#0;");
    }

    #[test]
    fn double_encoded_decodes_once() {
        assert_eq!(decode_str("&amp;amp;"), "&amp;");
    }

    #[test]
    fn plain_text_passes_through() {
        assert_eq!(decode_str("Levitating"), "Levitating");
        assert_eq!(decode_str(""), "");
    }

    #[test]
    fn value_decode_recurses() {
        let mut value = json!({
            "name": "Tom &amp; Jerry",
            "songs": [{"title": "Don&#39;t Stop"}],
            "count": 2,
            "ok": true,
            "nothing": null,
        });
        decode_entities(&mut value);
        assert_eq!(value["name"], "Tom & Jerry");
        assert_eq!(value["songs"][0]["title"], "Don't Stop");
        assert_eq!(value["count"], 2);
    }
}
