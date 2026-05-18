use super::constants::MAX_FINGERPRINT_WORDS;

pub(super) fn normalized_text_fingerprint(text: &str) -> String {
    // Allocate space assuming average word length is ~5 chars + space, bounded by MAX_FINGERPRINT_WORDS
    let mut result = String::with_capacity(text.len().min(MAX_FINGERPRINT_WORDS * 6));
    let mut words = 0;
    let mut in_word = false;

    for ch in text.chars() {
        let mapped = if ch.is_ascii_alphabetic() {
            ch.to_ascii_lowercase()
        } else if ch.is_ascii_digit() {
            '#'
        } else {
            ' '
        };

        if mapped == ' ' {
            if in_word {
                in_word = false;
                words += 1;
                if words == MAX_FINGERPRINT_WORDS {
                    break;
                }
            }
        } else {
            if !in_word {
                if words > 0 {
                    result.push(' ');
                }
                in_word = true;
            }
            result.push(mapped);
        }
    }

    result
}

pub(super) fn normalize_whitespace(value: &str) -> String {
    let mut result = String::with_capacity(value.len());
    for (i, word) in value.split_whitespace().enumerate() {
        if i > 0 {
            result.push(' ');
        }
        result.push_str(word);
    }
    result
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
