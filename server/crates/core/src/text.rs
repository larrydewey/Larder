//! Small text helpers.

/// Collapses runs of whitespace to a single space and trims.
pub fn collapse_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Lower-cases and strips common Latin diacritics for accent-insensitive search.
pub fn fold(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars().flat_map(|c| c.to_lowercase()) {
        let mapped = match c {
            'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ā' => 'a',
            'ç' | 'č' | 'ć' => 'c',
            'è' | 'é' | 'ê' | 'ë' | 'ē' | 'ę' => 'e',
            'ì' | 'í' | 'î' | 'ï' | 'ī' => 'i',
            'ñ' | 'ń' => 'n',
            'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' | 'ō' => 'o',
            'ù' | 'ú' | 'û' | 'ü' | 'ū' => 'u',
            'ý' | 'ÿ' => 'y',
            'ś' | 'š' => 's',
            'ź' | 'ż' | 'ž' => 'z',
            'ł' => 'l',
            'ß' => {
                out.push_str("ss");
                continue;
            }
            'æ' => {
                out.push_str("ae");
                continue;
            }
            'œ' => {
                out.push_str("oe");
                continue;
            }
            c if c.is_whitespace() => ' ',
            c => c,
        };
        out.push(mapped);
    }
    collapse_ws(&out)
}

/// Creates a URL slug from a name.
pub fn slugify(s: &str) -> String {
    let folded = fold(s);
    let mut out = String::new();
    let mut dash = false;
    for c in folded.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c);
            dash = false;
        } else if !dash && !out.is_empty() {
            out.push('-');
            dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.len() > 80 {
        out.truncate(80);
        while out.ends_with('-') {
            out.pop();
        }
    }
    if out.is_empty() {
        out.push_str("recipe");
    }
    out
}

/// Strips HTML tags and decodes the most common entities. Used for imported
/// fields that sometimes contain markup.
pub fn strip_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_tag = false;
    let mut tag = String::new();
    for c in s.chars() {
        match c {
            '<' => {
                in_tag = true;
                tag.clear();
            }
            '>' if in_tag => {
                in_tag = false;
                let t = tag.trim_start_matches('/').to_ascii_lowercase();
                let name = t.split(|c: char| c.is_whitespace() || c == '/').next().unwrap_or("");
                if matches!(name, "br" | "p" | "div" | "li" | "tr" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6") {
                    out.push('\n');
                }
            }
            _ if in_tag => tag.push(c),
            _ => out.push(c),
        }
    }
    decode_entities(&out)
}

/// Decodes common HTML entities (named and numeric).
pub fn decode_entities(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(pos) = rest.find('&') {
        out.push_str(&rest[..pos]);
        let after = &rest[pos..];
        if let Some(end) = after[..after.len().min(12)].find(';') {
            let ent = &after[1..end];
            let decoded = match ent {
                "amp" => Some('&'),
                "lt" => Some('<'),
                "gt" => Some('>'),
                "quot" => Some('"'),
                "apos" | "#39" => Some('\''),
                "nbsp" => Some(' '),
                "frac12" => Some('½'),
                "frac14" => Some('¼'),
                "frac34" => Some('¾'),
                "deg" => Some('°'),
                "ndash" => Some('–'),
                "mdash" => Some('—'),
                "rsquo" | "lsquo" => Some('\''),
                "rdquo" | "ldquo" => Some('"'),
                "hellip" => Some('…'),
                "eacute" => Some('é'),
                "egrave" => Some('è'),
                "ntilde" => Some('ñ'),
                _ if ent.starts_with("#x") || ent.starts_with("#X") => u32::from_str_radix(&ent[2..], 16).ok().and_then(char::from_u32),
                _ if ent.starts_with('#') => ent[1..].parse::<u32>().ok().and_then(char::from_u32),
                _ => None,
            };
            if let Some(c) = decoded {
                out.push(c);
                rest = &after[end + 1..];
                continue;
            }
        }
        out.push('&');
        rest = &after[1..];
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugs() {
        assert_eq!(slugify("Crème Brûlée!"), "creme-brulee");
        assert_eq!(slugify("  Mom's   Best Chili "), "mom-s-best-chili");
        assert_eq!(slugify("!!!"), "recipe");
    }

    #[test]
    fn html() {
        assert_eq!(strip_html("<p>Hello&nbsp;<b>world</b> &amp; you</p>"), "\nHello world & you\n");
        assert_eq!(decode_entities("1&#189; cups &#x2F;"), "1½ cups /");
        assert_eq!(decode_entities("salt & pepper"), "salt & pepper");
    }
}
