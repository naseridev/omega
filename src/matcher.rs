use crate::cli::Args;
use std::path::Path;

pub struct PatternMatcher {
    patterns: Vec<String>,
    case_sensitive: bool,
    fuzzy: bool,
    threshold: usize,
    content_search: bool,
    max_content_size: u64,
}

impl PatternMatcher {
    pub fn from_args(args: &Args) -> Self {
        let case_sensitive = !args.case_insensitive;
        let patterns = if case_sensitive {
            args.patterns.clone()
        } else {
            args.patterns.iter().map(|s| s.to_lowercase()).collect()
        };

        Self {
            patterns,
            case_sensitive,
            fuzzy: args.fuzzy,
            threshold: args.fuzzy_threshold,
            content_search: args.content_search,
            max_content_size: args.max_content_size,
        }
    }

    pub fn matches(&self, name: &str) -> bool {
        if !self.fuzzy {
            return if self.case_sensitive {
                self.patterns.iter().any(|p| name.contains(p.as_str()))
            } else {
                let target = name.to_lowercase();
                self.patterns.iter().any(|p| target.contains(p.as_str()))
            };
        }

        let target = if self.case_sensitive {
            name.to_string()
        } else {
            name.to_lowercase()
        };

        if self.patterns.iter().any(|p| target.contains(p.as_str())) {
            return true;
        }

        let words: Vec<&str> = target.split(|c: char| !c.is_alphanumeric()).collect();
        self.patterns.iter().any(|p| {
            words
                .iter()
                .any(|word| !word.is_empty() && levenshtein_distance(p, word) <= self.threshold)
        })
    }

    pub fn matches_content(&self, path: &Path, size: u64) -> bool {
        if !self.content_search || size > self.max_content_size || size == 0 {
            return false;
        }

        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => return false,
        };

        let target = if self.case_sensitive {
            content
        } else {
            content.to_lowercase()
        };

        if self.fuzzy {
            if self.patterns.iter().any(|p| target.contains(p.as_str())) {
                return true;
            }

            let words: Vec<&str> = target.split(|c: char| !c.is_alphanumeric()).collect();
            self.patterns.iter().any(|p| {
                words
                    .iter()
                    .any(|word| !word.is_empty() && levenshtein_distance(p, word) <= self.threshold)
            })
        } else {
            self.patterns.iter().any(|p| target.contains(p.as_str()))
        }
    }
}

fn levenshtein_distance(s1: &str, s2: &str) -> usize {
    let s1_chars: Vec<char> = s1.chars().collect();
    let s2_chars: Vec<char> = s2.chars().collect();
    let len1 = s1_chars.len();
    let len2 = s2_chars.len();

    if len1 == 0 {
        return len2;
    }
    if len2 == 0 {
        return len1;
    }

    let mut prev_row: Vec<usize> = (0..=len2).collect();
    let mut curr_row = vec![0; len2 + 1];

    for (i, &ch1) in s1_chars.iter().enumerate() {
        curr_row[0] = i + 1;

        for j in 0..len2 {
            let cost = if ch1 == s2_chars[j] { 0 } else { 1 };
            curr_row[j + 1] = (curr_row[j] + 1)
                .min(prev_row[j + 1] + 1)
                .min(prev_row[j] + cost);
        }

        std::mem::swap(&mut prev_row, &mut curr_row);
    }

    prev_row[len2]
}
