use super::constants::MAX_FINGERPRINT_WORDS;

pub(super) fn normalized_text_fingerprint(text: &str) -> String {
    let mut normalized = String::with_capacity(text.len());

    for ch in text.chars() {
        if ch.is_ascii_alphabetic() {
            normalized.push(ch.to_ascii_lowercase());
        } else if ch.is_ascii_digit() {
            normalized.push('#');
        } else {
            normalized.push(' ');
        }
    }

    normalize_whitespace(&normalized)
        .split_whitespace()
        .take(MAX_FINGERPRINT_WORDS)
        .collect::<Vec<_>>()
        .join(" ")
}

pub(super) fn normalize_whitespace(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fingerprint_normalizes_digits_and_punctuation() {
        assert_eq!(
            normalized_text_fingerprint("Hello, World! 123"),
            "hello world ###"
        );
    }

    #[test]
    fn fingerprint_caps_at_max_words() {
        let long_text = "word ".repeat(100);
        let fp = normalized_text_fingerprint(&long_text);
        assert_eq!(fp.split_whitespace().count(), MAX_FINGERPRINT_WORDS);
    }

    #[test]
    fn fingerprint_returns_empty_for_blank() {
        assert_eq!(normalized_text_fingerprint(""), "");
        assert_eq!(normalized_text_fingerprint("   "), "");
    }

    #[test]
    fn normalize_whitespace_collapses_runs() {
        assert_eq!(normalize_whitespace("a  b\t\nc"), "a b c");
    }
}
