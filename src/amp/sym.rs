//! SymSpell-style edit-distance-1 delete index for AMP fuzzy rescue.
//!
//! The index maps each string `delete-variant -> keyword ids`.

use std::collections::HashMap;
use std::collections::HashSet;

/// The distinct one-character-deletion variants of `word` (over chars, not bytes).
///
/// e.g. `"abc"` -> `["bc", "ac", "ab"]`. Repeated letters delete to the same
/// variant from different positions, so those are collapsed (`"aa"` -> `["a"]`)
pub fn deletes(word: &str) -> Vec<String> {
    let chars: Vec<char> = word.chars().collect();
    let n = chars.len();
    let mut out: Vec<String> = Vec::with_capacity(n);
    for i in 0..n {
        let mut s = String::with_capacity(word.len());
        s.extend(chars[..i].iter());
        s.extend(chars[i + 1..].iter());
        // this is faster than hashset since n is small
        if !out.contains(&s) {
            out.push(s);
        }
    }
    out
}

/// Whether `a` and `b` are exactly Damerau-Levenshtein distance 1 apart.
/// i.e. insertions, deletions or substitutions of a single character, or transposition
/// of two adjacent characters
///
/// Returns `false` for equal strings: exact matches are already handled by existing
/// prefix index, fuzzy match is the fallback
pub fn edit_distance_one(a: &str, b: &str) -> bool {
    if a == b {
        return false;
    }
    let ca: Vec<char> = a.chars().collect();
    let cb: Vec<char> = b.chars().collect();
    let (la, lb) = (ca.len(), cb.len());
    // if lengths differ by >1, edit distance is already greater than 1
    if la.abs_diff(lb) > 1 {
        return false;
    }

    if la == lb {
        // Same length => the only ways to be ED1 are a single substituted
        // character, or two *adjacent* characters swapped (a transposition).
        // collect character positions where there are mismatches, e.g.
        // [fragrence], [fragrance] -> [5] => this is ok
        // [swithc], [switch] -> [4, 5]    => this is also ok
        // [abcd], [abxy] -> [2, 3]        => not ok, swap test fails
        let mismatches: Vec<usize> = (0..la).filter(|&i| ca[i] != cb[i]).collect();
        return match mismatches.len() {
            // one differing position: a single substitution
            1 => true,
            // two differing positions: ED1 only if they are adjacent and swapped
            2 => {
                let (i, j) = (mismatches[0], mismatches[1]);
                j == i + 1 && ca[i] == cb[j] && ca[j] == cb[i]
            }
            // 0 (equal, handled above) or 3+ differences: not ED1
            _ => false,
        };
    }

    // Lengths differ by one: a single insertion/deletion. Walk the shorter
    // string against the longer, allowing exactly one skip in the longer.
    // swich, switch -> edit 1 passes
    // abc, axyc     -> edit 2 fails
    let (short, long) = if la < lb { (&ca, &cb) } else { (&cb, &ca) };
    let (mut i, mut j, mut edits) = (0usize, 0usize, 0u8);
    while i < short.len() && j < long.len() {
        if short[i] == long[j] {
            i += 1;
            j += 1;
        } else {
            edits += 1;
            if edits > 1 {
                return false;
            }
            j += 1;
        }
    }
    true
}

/// SymSpell delete index over full-keyword strings for ED1 fuzzy rescue.
#[derive(Default)]
pub struct SymIndex {
    /// Distinct indexed keywords; a keyword's id is its position here.
    keywords: Vec<String>,
    /// keyword or one of its delete-variants -> ids reachable in <=1 deletion.
    delete_index: HashMap<String, Vec<u32>>,
    /// Minimum char length to index a keyword AND to attempt a fuzzy query.
    /// lower char lengths tend to produce too many false positives (default is >= 5)
    min_len: usize,
}

impl SymIndex {
    /// Build from full-keyword strings. Keeps only keywords with at least
    /// `min_len` chars, deduped; assigns each a stable id and indexes the
    /// keyword itself plus each of its one-char-delete variants.
    pub fn build(keywords: impl IntoIterator<Item = String>, min_len: usize) -> Self {
        let mut idx = SymIndex {
            keywords: Vec::new(),
            delete_index: HashMap::new(),
            min_len,
        };
        let mut seen: HashSet<String> = HashSet::new();
        for kw in keywords {
            if kw.chars().count() < min_len || !seen.insert(kw.clone()) {
                continue;
            }
            // use id so we have single reference to keyword in vector
            let id = idx.keywords.len() as u32;
            // if same deletes map to different keywords, we
            // add to array.
            // multi candidates are rare (< 0.1 %); merino will
            // handle the candidate selection downstream
            idx.delete_index.entry(kw.clone()).or_default().push(id);
            for d in deletes(&kw) {
                idx.delete_index.entry(d).or_default().push(id);
            }
            idx.keywords.push(kw);
        }
        idx
    }

    /// Return the full keywords within ED1 of `query` (exact equality excluded).
    /// Empty when `query` is shorter than `min_len`.
    pub fn query_delete_index(&self, query: &str) -> Vec<String> {
        if query.chars().count() < self.min_len {
            return Vec::new();
        }
        let mut ids: Vec<u32> = Vec::new();
        if let Some(v) = self.delete_index.get(query) {
            ids.extend_from_slice(v);
        }
        for d in deletes(query) {
            if let Some(v) = self.delete_index.get(d.as_str()) {
                ids.extend_from_slice(v);
            }
        }
        // need the proper edit distance 1 filter below because sharing
        // a delete variant could mean two strings are within edit 2
        // e.g. query=world, keyword=words -> "word" in delete index and candidate
        // but edit_distance(world, words) == 2
        ids.sort_unstable();
        ids.dedup();
        ids.into_iter()
            .map(|id| &self.keywords[id as usize])
            .filter(|kw| edit_distance_one(query, kw))
            .cloned()
            .collect()
    }

    /// Number of indexed keywords.
    pub fn len(&self) -> usize {
        self.keywords.len()
    }

    /// Whether the index has no keywords.
    pub fn is_empty(&self) -> bool {
        self.keywords.is_empty()
    }

    /// Number of distinct delete-index keys (for stats/telemetry).
    pub fn delete_index_len(&self) -> usize {
        self.delete_index.len()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn set(items: &[&str]) -> HashSet<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn deletes_basic() {
        let got: HashSet<String> = deletes("abc").into_iter().collect();
        assert_eq!(got, set(&["bc", "ac", "ab"]));
    }

    #[test]
    fn deletes_is_char_safe() {
        // 'é' is a single scalar value; byte-slicing here would panic/corrupt.
        let got: HashSet<String> = deletes("café").into_iter().collect();
        assert_eq!(got, set(&["afé", "cfé", "caé", "caf"]));
    }

    #[test]
    fn deletes_dedups_repeats() {
        assert_eq!(deletes("aa"), vec!["a".to_string()]);
        let v = deletes("committee");
        let distinct: HashSet<&String> = v.iter().collect();
        assert_eq!(v.len(), distinct.len(), "deletes must return distinct variants");
    }

    #[test]
    fn ed1_substitution() {
        assert!(edit_distance_one("fragrance", "fragrence"));
    }

    #[test]
    fn ed1_insertion() {
        assert!(edit_distance_one("swich", "switch")); // insert 't'
    }

    #[test]
    fn ed1_deletion() {
        assert!(edit_distance_one("switchh", "switch")); // delete extra 'h'
    }

    #[test]
    fn ed1_transposition() {
        assert!(edit_distance_one("swithc", "switch")); // swap 'c','h'
    }

    #[test]
    fn ed1_equal_is_false() {
        assert!(!edit_distance_one("switch", "switch"));
    }

    #[test]
    fn ed1_too_far_by_length() {
        assert!(!edit_distance_one("switch", "swap"));
    }

    #[test]
    fn ed1_two_substitutions_false() {
        assert!(!edit_distance_one("abcd", "abxy"));
    }

    #[test]
    fn ed1_non_adjacent_swap_false() {
        assert!(!edit_distance_one("abcd", "adcb"));
    }

    #[test]
    fn build_and_query_all_edit_types() {
        let idx = SymIndex::build(
            ["fragrance", "switch", "camera"]
                .iter()
                .map(|s| s.to_string()),
            5,
        );
        assert_eq!(idx.query_delete_index("fragrence"), vec!["fragrance"]); // substitution
        assert_eq!(idx.query_delete_index("swithc"), vec!["switch"]); // transposition
        assert_eq!(idx.query_delete_index("cameras"), vec!["camera"]); // insertion
        // Exact match is NOT a fuzzy candidate (prefix path owns it).
        assert!(idx.query_delete_index("camera").is_empty());
        // Below min_len -> no fuzzy attempt.
        assert!(idx.query_delete_index("frag").is_empty());
        // No ED1 neighbour.
        assert!(idx.query_delete_index("zzzzz").is_empty());
    }

    #[test]
    fn build_respects_min_len() {
        let idx = SymIndex::build(
            ["cat", "cats", "camera"].iter().map(|s| s.to_string()),
            5,
        );
        assert_eq!(idx.len(), 1); // only "camera" (>= 5 chars)
    }

    #[test]
    fn build_dedups_keywords() {
        let idx = SymIndex::build(
            ["camera", "camera"].iter().map(|s| s.to_string()),
            5,
        );
        assert_eq!(idx.len(), 1);
    }
}
