use crate::cli::Args;
use std::cell::RefCell;
use std::fs::File;
use std::io::Read;
use std::path::Path;

const INLINE_ROW: usize = 129;

thread_local! {
    static CONTENT_BUFFER: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}

pub struct PatternMatcher {
    patterns: Vec<Box<str>>,
    pattern_chars: Vec<Box<[char]>>,
    case_sensitive: bool,
    fuzzy: bool,
    threshold: usize,
    content_search: bool,
    max_content_size: u64,
}

impl PatternMatcher {
    pub fn from_args(args: &Args) -> Self {
        let case_sensitive = !args.case_insensitive;

        let patterns: Vec<Box<str>> = args
            .patterns
            .iter()
            .map(|pattern| {
                if case_sensitive {
                    pattern.as_str().into()
                } else {
                    pattern.to_lowercase().into_boxed_str()
                }
            })
            .collect();

        let pattern_chars = if args.fuzzy {
            patterns
                .iter()
                .map(|pattern| pattern.chars().collect())
                .collect()
        } else {
            Vec::new()
        };

        Self {
            patterns,
            pattern_chars,
            case_sensitive,
            fuzzy: args.fuzzy,
            threshold: args.fuzzy_threshold,
            content_search: args.content_search,
            max_content_size: args.max_content_size,
        }
    }

    pub fn matches(&self, name: &str) -> bool {
        if self.case_sensitive || name.is_ascii() {
            if self.contains_any(name.as_bytes()) {
                return true;
            }
            return self.fuzzy && self.any_word_matches(name);
        }

        let lowered = name.to_lowercase();
        if self.patterns.iter().any(|p| lowered.contains(&**p)) {
            return true;
        }

        self.fuzzy && self.any_word_matches(&lowered)
    }

    pub fn matches_content(&self, path: &Path) -> bool {
        if !self.content_search {
            return false;
        }

        CONTENT_BUFFER.with(|cell| {
            let mut buffer = cell.borrow_mut();
            if !read_capped(path, self.max_content_size, &mut buffer) {
                return false;
            }

            if !self.case_sensitive {
                buffer.make_ascii_lowercase();
            }

            if self.contains_any_folded(&buffer) {
                return true;
            }

            if !self.fuzzy {
                return false;
            }

            buffer
                .split(|byte| !byte.is_ascii_alphanumeric())
                .filter(|word| !word.is_empty())
                .any(|word| match std::str::from_utf8(word) {
                    Ok(word) => self.word_matches(word),
                    Err(_) => false,
                })
        })
    }

    fn contains_any_folded(&self, data: &[u8]) -> bool {
        if let Ok(text) = std::str::from_utf8(data) {
            return self
                .patterns
                .iter()
                .any(|pattern| text.contains(&**pattern));
        }

        self.patterns
            .iter()
            .any(|pattern| contains_bytes(data, pattern.as_bytes()))
    }

    fn contains_any(&self, haystack: &[u8]) -> bool {
        if self.case_sensitive {
            self.patterns
                .iter()
                .any(|pattern| contains_bytes(haystack, pattern.as_bytes()))
        } else {
            self.patterns
                .iter()
                .any(|pattern| contains_bytes_ignore_case(haystack, pattern.as_bytes()))
        }
    }

    fn any_word_matches(&self, haystack: &str) -> bool {
        haystack
            .split(|c: char| !c.is_alphanumeric())
            .filter(|word| !word.is_empty())
            .any(|word| self.word_matches(word))
    }

    fn word_matches(&self, word: &str) -> bool {
        self.pattern_chars
            .iter()
            .any(|pattern| within_distance(pattern, word, self.threshold, !self.case_sensitive))
    }
}

fn read_capped(path: &Path, max_size: u64, buffer: &mut Vec<u8>) -> bool {
    let Ok(file) = File::open(path) else {
        return false;
    };

    let Ok(metadata) = file.metadata() else {
        return false;
    };

    let size = metadata.len();
    if size == 0 || size > max_size {
        return false;
    }

    let Ok(capacity) = usize::try_from(size) else {
        return false;
    };

    buffer.clear();
    buffer.reserve(capacity);
    file.take(size).read_to_end(buffer).is_ok()
}

const SWAR_LOW: u64 = 0x0101_0101_0101_0101;
const SWAR_HIGH: u64 = 0x8080_8080_8080_8080;
const SWAR_BLOCK: usize = 64;

#[inline]
fn zero_byte_mask(word: u64) -> u64 {
    word.wrapping_sub(SWAR_LOW) & !word & SWAR_HIGH
}

fn find_byte(haystack: &[u8], needle: u8) -> Option<usize> {
    let broadcast = SWAR_LOW.wrapping_mul(u64::from(needle));
    let (blocks, remainder) = haystack.as_chunks::<SWAR_BLOCK>();

    for (index, block) in blocks.iter().enumerate() {
        let mut hits = 0;

        for word in block.as_chunks::<8>().0 {
            hits |= zero_byte_mask(u64::from_ne_bytes(*word) ^ broadcast);
        }

        if hits != 0 {
            return block
                .iter()
                .position(|&byte| byte == needle)
                .map(|found| index * SWAR_BLOCK + found);
        }
    }

    remainder
        .iter()
        .position(|&byte| byte == needle)
        .map(|found| blocks.len() * SWAR_BLOCK + found)
}

fn find_byte_pair(haystack: &[u8], first: u8, second: u8) -> Option<usize> {
    let lower = SWAR_LOW.wrapping_mul(u64::from(first));
    let upper = SWAR_LOW.wrapping_mul(u64::from(second));
    let (blocks, remainder) = haystack.as_chunks::<SWAR_BLOCK>();

    for (index, block) in blocks.iter().enumerate() {
        let mut hits = 0;

        for word in block.as_chunks::<8>().0 {
            let value = u64::from_ne_bytes(*word);
            hits |= zero_byte_mask(value ^ lower) | zero_byte_mask(value ^ upper);
        }

        if hits != 0 {
            return block
                .iter()
                .position(|&byte| byte == first || byte == second)
                .map(|found| index * SWAR_BLOCK + found);
        }
    }

    remainder
        .iter()
        .position(|&byte| byte == first || byte == second)
        .map(|found| blocks.len() * SWAR_BLOCK + found)
}

fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
    let Some((&first, rest)) = needle.split_first() else {
        return true;
    };

    if needle.len() > haystack.len() {
        return false;
    }

    let mut offset = 0;
    let limit = haystack.len() - rest.len();

    while let Some(position) = find_byte(&haystack[offset..limit], first) {
        let start = offset + position + 1;
        if haystack[start..start + rest.len()] == *rest {
            return true;
        }
        offset = start;
    }

    false
}

fn contains_bytes_ignore_case(haystack: &[u8], needle: &[u8]) -> bool {
    let Some((&first, rest)) = needle.split_first() else {
        return true;
    };

    if needle.len() > haystack.len() {
        return false;
    }

    let upper = first.to_ascii_uppercase();
    let mut offset = 0;
    let limit = haystack.len() - rest.len();

    while let Some(position) = if upper == first {
        find_byte(&haystack[offset..limit], first)
    } else {
        find_byte_pair(&haystack[offset..limit], first, upper)
    } {
        let start = offset + position + 1;
        if haystack[start..start + rest.len()].eq_ignore_ascii_case(rest) {
            return true;
        }
        offset = start;
    }

    false
}

fn within_distance(pattern: &[char], word: &str, max: usize, fold: bool) -> bool {
    let pattern_len = pattern.len();
    let word_len = word.chars().count();

    if pattern_len.abs_diff(word_len) > max {
        return false;
    }

    if pattern_len == 0 || word_len == 0 {
        return true;
    }

    if pattern_len < INLINE_ROW {
        let mut row = [0u32; INLINE_ROW];
        distance_row(pattern, word, max, fold, &mut row[..=pattern_len])
    } else {
        let mut row = vec![0u32; pattern_len + 1];
        distance_row(pattern, word, max, fold, &mut row)
    }
}

fn distance_row(pattern: &[char], word: &str, max: usize, fold: bool, row: &mut [u32]) -> bool {
    let ceiling = u32::try_from(max).unwrap_or(u32::MAX).min(u32::MAX / 2);

    for (index, slot) in row.iter_mut().enumerate() {
        *slot = u32::try_from(index).unwrap_or(u32::MAX);
    }

    let mut last = u32::MAX;

    for (index, character) in word.chars().enumerate() {
        let character = if fold {
            character.to_ascii_lowercase()
        } else {
            character
        };

        let mut diagonal = row[0];
        let mut left = u32::try_from(index).unwrap_or(u32::MAX).saturating_add(1);
        let mut row_minimum = left;
        row[0] = left;

        for (slot, &pattern_character) in row[1..].iter_mut().zip(pattern) {
            let above = *slot;
            let value = (left + 1)
                .min(above + 1)
                .min(diagonal + u32::from(pattern_character != character));

            *slot = value;
            diagonal = above;
            left = value;
            row_minimum = row_minimum.min(value);
        }

        if row_minimum > ceiling {
            return false;
        }

        last = left;
    }

    last <= ceiling
}
