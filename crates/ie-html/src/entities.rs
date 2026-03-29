include!(concat!(env!("OUT_DIR"), "/entities.rs"));

/// Look up a named character reference. Key should NOT include leading '&'.
/// Returns codepoints as &[u32].
pub fn lookup(name: &str) -> Option<&'static [u32]> {
    NAMED_ENTITIES.get(name).copied()
}

/// Find the longest matching entity name in the input.
/// Input starts after '&'. Returns (matched_name, codepoints) if found.
pub fn longest_match(input: &[char]) -> Option<(usize, &'static [u32])> {
    let mut best: Option<(usize, &'static [u32])> = None;
    let mut candidate = String::new();

    for (i, &c) in input.iter().enumerate() {
        candidate.push(c);
        if let Some(codepoints) = lookup(&candidate) {
            best = Some((i + 1, codepoints));
        }
        // Stop searching if we hit ';' or a non-alphanumeric
        if c == ';' {
            break;
        }
        // The longest entity name in WHATWG is ~32 chars; cap search
        if i > 40 {
            break;
        }
    }
    best
}

// Windows-1252 replacement table for numeric character references
// See: https://html.spec.whatwg.org/multipage/parsing.html#numeric-character-reference-end-state
pub fn windows_1252_replacement(codepoint: u32) -> Option<char> {
    match codepoint {
        0x80 => Some('\u{20AC}'),
        0x82 => Some('\u{201A}'),
        0x83 => Some('\u{0192}'),
        0x84 => Some('\u{201E}'),
        0x85 => Some('\u{2026}'),
        0x86 => Some('\u{2020}'),
        0x87 => Some('\u{2021}'),
        0x88 => Some('\u{02C6}'),
        0x89 => Some('\u{2030}'),
        0x8A => Some('\u{0160}'),
        0x8B => Some('\u{2039}'),
        0x8C => Some('\u{0152}'),
        0x8E => Some('\u{017D}'),
        0x91 => Some('\u{2018}'),
        0x92 => Some('\u{2019}'),
        0x93 => Some('\u{201C}'),
        0x94 => Some('\u{201D}'),
        0x95 => Some('\u{2022}'),
        0x96 => Some('\u{2013}'),
        0x97 => Some('\u{2014}'),
        0x98 => Some('\u{02DC}'),
        0x99 => Some('\u{2122}'),
        0x9A => Some('\u{0161}'),
        0x9B => Some('\u{203A}'),
        0x9C => Some('\u{0153}'),
        0x9E => Some('\u{017E}'),
        0x9F => Some('\u{0178}'),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_amp() {
        assert_eq!(lookup("amp;"), Some(&[38u32][..]));
    }

    #[test]
    fn lookup_lt() {
        assert_eq!(lookup("lt;"), Some(&[60u32][..]));
    }

    #[test]
    fn lookup_nonexistent() {
        assert_eq!(lookup("notanentity;"), None);
    }

    #[test]
    fn lookup_without_semicolon() {
        assert_eq!(lookup("amp"), Some(&[38u32][..]));
    }

    #[test]
    fn longest_match_amp() {
        let input: Vec<char> = "amp;text".chars().collect();
        let (len, cp) = longest_match(&input).unwrap();
        assert_eq!(len, 4); // "amp;"
        assert_eq!(cp, &[38]);
    }

    #[test]
    fn longest_match_no_semicolon() {
        let input: Vec<char> = "amptext".chars().collect();
        let (len, cp) = longest_match(&input).unwrap();
        assert_eq!(len, 3); // "amp"
        assert_eq!(cp, &[38]);
    }

    #[test]
    fn longest_match_multi_codepoint() {
        // &nGt; maps to [8811, 8402] (≫⃒)
        let input: Vec<char> = "nGt;".chars().collect();
        let result = longest_match(&input);
        assert!(result.is_some());
        let (_, cp) = result.unwrap();
        assert_eq!(cp.len(), 2);
    }

    #[test]
    fn windows_1252_euro() {
        assert_eq!(windows_1252_replacement(0x80), Some('\u{20AC}'));
    }

    #[test]
    fn windows_1252_no_replacement() {
        assert_eq!(windows_1252_replacement(0x41), None);
    }
}
