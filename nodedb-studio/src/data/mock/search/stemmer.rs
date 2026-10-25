//! The Snowball English (Porter2) stemmer, over lowercase ASCII words.
//!
//! The mock analyzer stems with it, so the tokens it returns are the ones
//! the English analyzer of a real index would store.

fn vowel(c: u8) -> bool {
    matches!(c, b'a' | b'e' | b'i' | b'o' | b'u' | b'y')
}

/// The index after the first non-vowel that follows a vowel, from `from`.
fn region(w: &[u8], from: usize) -> usize {
    let mut i = from + 1;
    while i < w.len() {
        if !vowel(w[i]) && vowel(w[i - 1]) {
            return i + 1;
        }
        i += 1;
    }
    w.len()
}

fn ends_with(w: &[u8], suffix: &str) -> bool {
    w.ends_with(suffix.as_bytes())
}

/// A vowel followed by a non-vowel other than `w`, `x`, `Y`, and preceded by
/// a non-vowel, or a vowel at the start followed by a non-vowel.
fn short_syllable_end(w: &[u8]) -> bool {
    let n = w.len();
    if n == 2 {
        return vowel(w[0]) && !vowel(w[1]);
    }
    n >= 3
        && !vowel(w[n - 3])
        && vowel(w[n - 2])
        && !vowel(w[n - 1])
        && !matches!(w[n - 1], b'w' | b'x' | b'Y')
}

fn is_double(w: &[u8]) -> bool {
    let n = w.len();
    n >= 2
        && w[n - 1] == w[n - 2]
        && matches!(
            w[n - 1],
            b'b' | b'd' | b'f' | b'g' | b'm' | b'n' | b'p' | b'r' | b't'
        )
}

fn exception(word: &str) -> Option<&'static str> {
    Some(match word {
        "skis" => "ski",
        "skies" => "sky",
        "dying" => "die",
        "lying" => "lie",
        "tying" => "tie",
        "idly" => "idl",
        "gently" => "gentl",
        "ugly" => "ugli",
        "early" => "earli",
        "only" => "onli",
        "singly" => "singl",
        "sky" => "sky",
        "news" => "news",
        "howe" => "howe",
        "atlas" => "atlas",
        "cosmos" => "cosmos",
        "bias" => "bias",
        "andes" => "andes",
        _ => return None,
    })
}

struct Word {
    w: Vec<u8>,
    r1: usize,
    r2: usize,
}

impl Word {
    fn in_r1(&self, suffix_len: usize) -> bool {
        self.w.len() - suffix_len >= self.r1
    }

    fn in_r2(&self, suffix_len: usize) -> bool {
        self.w.len() - suffix_len >= self.r2
    }

    fn cut(&mut self, suffix_len: usize) {
        let n = self.w.len() - suffix_len;
        self.w.truncate(n);
    }

    fn replace(&mut self, suffix_len: usize, with: &str) {
        self.cut(suffix_len);
        self.w.extend_from_slice(with.as_bytes());
    }

    fn step0(&mut self) {
        for s in ["'s'", "'s", "'"] {
            if ends_with(&self.w, s) {
                self.cut(s.len());
                return;
            }
        }
    }

    fn step1a(&mut self) {
        if ends_with(&self.w, "sses") {
            self.replace(4, "ss");
        } else if ends_with(&self.w, "ied") || ends_with(&self.w, "ies") {
            // `i` when more than one letter precedes the suffix, else `ie`.
            if self.w.len() > 4 {
                self.replace(3, "i");
            } else {
                self.replace(3, "ie");
            }
        } else if ends_with(&self.w, "us") || ends_with(&self.w, "ss") {
        } else if ends_with(&self.w, "s") {
            // Delete when a vowel precedes the `s` by at least one letter.
            let n = self.w.len();
            if n >= 3 && self.w[..n - 2].iter().any(|c| vowel(*c)) {
                self.cut(1);
            }
        }
    }

    fn step1b(&mut self) {
        for s in ["eedly", "eed"] {
            if ends_with(&self.w, s) {
                if self.in_r1(s.len()) {
                    self.replace(s.len(), "ee");
                }
                return;
            }
        }
        for s in ["ingly", "edly", "ing", "ed"] {
            if ends_with(&self.w, s) {
                let stem_len = self.w.len() - s.len();
                if !self.w[..stem_len].iter().any(|c| vowel(*c)) {
                    return;
                }
                self.cut(s.len());
                if ends_with(&self.w, "at") || ends_with(&self.w, "bl") || ends_with(&self.w, "iz")
                {
                    self.w.push(b'e');
                } else if is_double(&self.w) {
                    self.w.pop();
                } else if self.r1 == self.w.len() && short_syllable_end(&self.w) {
                    self.w.push(b'e');
                }
                return;
            }
        }
    }

    fn step1c(&mut self) {
        let n = self.w.len();
        if n > 2 && matches!(self.w[n - 1], b'y' | b'Y') && !vowel(self.w[n - 2]) {
            self.w[n - 1] = b'i';
        }
    }

    fn step2(&mut self) {
        const RULES: [(&str, &str); 24] = [
            ("ization", "ize"),
            ("ational", "ate"),
            ("fulness", "ful"),
            ("ousness", "ous"),
            ("iveness", "ive"),
            ("tional", "tion"),
            ("biliti", "ble"),
            ("lessli", "less"),
            ("entli", "ent"),
            ("ation", "ate"),
            ("alism", "al"),
            ("aliti", "al"),
            ("ousli", "ous"),
            ("iviti", "ive"),
            ("fulli", "ful"),
            ("enci", "ence"),
            ("anci", "ance"),
            ("abli", "able"),
            ("izer", "ize"),
            ("ator", "ate"),
            ("alli", "al"),
            ("bli", "ble"),
            ("ogi", "og"),
            ("li", ""),
        ];
        // The longest suffix that matches decides, even when its guard fails.
        let mut best: Option<(&str, &str)> = None;
        for (suffix, with) in RULES {
            if ends_with(&self.w, suffix) && best.is_none_or(|(b, _)| suffix.len() > b.len()) {
                best = Some((suffix, with));
            }
        }
        let Some((suffix, with)) = best else { return };
        if !self.in_r1(suffix.len()) {
            return;
        }
        match suffix {
            "ogi" => {
                let n = self.w.len();
                if n >= 4 && self.w[n - 4] == b'l' {
                    self.replace(3, "og");
                }
            }
            "li" => {
                let n = self.w.len();
                if n >= 3
                    && matches!(
                        self.w[n - 3],
                        b'c' | b'd' | b'e' | b'g' | b'h' | b'k' | b'm' | b'n' | b'r' | b't'
                    )
                {
                    self.cut(2);
                }
            }
            _ => self.replace(suffix.len(), with),
        }
    }

    fn step3(&mut self) {
        const RULES: [(&str, &str); 9] = [
            ("ational", "ate"),
            ("tional", "tion"),
            ("alize", "al"),
            ("icate", "ic"),
            ("iciti", "ic"),
            ("ative", ""),
            ("ical", "ic"),
            ("ness", ""),
            ("ful", ""),
        ];
        let mut best: Option<(&str, &str)> = None;
        for (suffix, with) in RULES {
            if ends_with(&self.w, suffix) && best.is_none_or(|(b, _)| suffix.len() > b.len()) {
                best = Some((suffix, with));
            }
        }
        let Some((suffix, with)) = best else { return };
        if !self.in_r1(suffix.len()) {
            return;
        }
        if suffix == "ative" {
            if self.in_r2(suffix.len()) {
                self.cut(suffix.len());
            }
        } else {
            self.replace(suffix.len(), with);
        }
    }

    fn step4(&mut self) {
        const SUFFIXES: [&str; 18] = [
            "ement", "able", "ible", "ance", "ence", "ment", "ant", "ent", "ism", "ate", "iti",
            "ous", "ive", "ize", "ion", "al", "er", "ic",
        ];
        let mut best: Option<&str> = None;
        for suffix in SUFFIXES {
            if ends_with(&self.w, suffix) && best.is_none_or(|b| suffix.len() > b.len()) {
                best = Some(suffix);
            }
        }
        let Some(suffix) = best else { return };
        if !self.in_r2(suffix.len()) {
            return;
        }
        if suffix == "ion" {
            let n = self.w.len();
            if n >= 4 && matches!(self.w[n - 4], b's' | b't') {
                self.cut(3);
            }
        } else {
            self.cut(suffix.len());
        }
    }

    fn step5(&mut self) {
        let n = self.w.len();
        if ends_with(&self.w, "e") {
            let before = &self.w[..n - 1];
            if self.in_r2(1) || (self.in_r1(1) && !short_syllable_end(before)) {
                self.cut(1);
            }
        } else if ends_with(&self.w, "l") && self.in_r2(1) && n >= 2 && self.w[n - 2] == b'l' {
            self.cut(1);
        }
    }
}

/// The stem of a lowercase word. A word with a character outside `a-z` and
/// the apostrophe comes back unchanged, and so does one of two letters or
/// fewer.
pub fn stem(word: &str) -> String {
    if word.len() <= 2 || !word.bytes().all(|c| c.is_ascii_lowercase() || c == b'\'') {
        return word.to_string();
    }
    let word = word.trim_start_matches('\'');
    if word.len() <= 2 {
        return word.to_string();
    }
    if let Some(e) = exception(word) {
        return e.to_string();
    }
    // `y` after a vowel, or at the start, is a consonant: mark it `Y`.
    let mut w: Vec<u8> = word.bytes().collect();
    for i in 0..w.len() {
        if w[i] == b'y' && (i == 0 || vowel(w[i - 1])) {
            w[i] = b'Y';
        }
    }
    let prefix = ["gener", "arsen"]
        .iter()
        .find(|p| word.starts_with(**p))
        .map(|p| p.len())
        .or_else(|| word.starts_with("commun").then_some(6));
    let r1 = prefix.unwrap_or_else(|| region(&w, 0));
    let r2 = region(&w, r1);
    let mut s = Word { w, r1, r2 };
    s.step0();
    s.step1a();
    let current = String::from_utf8_lossy(&s.w).into_owned();
    if !matches!(
        current.as_str(),
        "inning" | "outing" | "canning" | "herring" | "earring" | "proceed" | "exceed" | "succeed"
    ) {
        s.step1b();
    }
    s.step1c();
    s.step2();
    s.step3();
    s.step4();
    s.step5();
    String::from_utf8(s.w).unwrap_or_default().replace('Y', "y")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check(pairs: &[(&str, &str)]) {
        for (word, expected) in pairs {
            assert_eq!(stem(word), *expected, "stem of `{word}`");
        }
    }

    #[test]
    fn the_design_words_stem_as_shown() {
        check(&[
            ("distributed", "distribut"),
            ("databases", "databas"),
            ("database", "databas"),
            ("replication", "replic"),
            ("consistency", "consist"),
            ("queries", "queri"),
            ("storage", "storag"),
        ]);
    }

    #[test]
    fn step_one_handles_plurals_and_past_tense() {
        check(&[
            ("caresses", "caress"),
            ("ponies", "poni"),
            ("ties", "tie"),
            ("cries", "cri"),
            ("gas", "gas"),
            ("gaps", "gap"),
            ("agreed", "agre"),
            ("feed", "feed"),
            ("hopping", "hop"),
            ("hoping", "hope"),
            ("fluttered", "flutter"),
            ("filing", "file"),
            ("sing", "sing"),
            ("happy", "happi"),
            ("sky", "sky"),
        ]);
    }

    #[test]
    fn later_steps_trim_derivational_suffixes() {
        check(&[
            ("relational", "relat"),
            ("generously", "generous"),
            ("happiness", "happi"),
            ("running", "run"),
            ("national", "nation"),
            ("consign", "consign"),
            ("knack", "knack"),
            ("adjustment", "adjust"),
            ("communism", "communism"),
            ("generate", "generat"),
        ]);
    }

    #[test]
    fn exceptions_and_short_words_hold() {
        check(&[
            ("dying", "die"),
            ("news", "news"),
            ("skies", "sky"),
            ("be", "be"),
            ("a", "a"),
            ("proceed", "proceed"),
            ("café", "café"),
        ]);
    }
}
