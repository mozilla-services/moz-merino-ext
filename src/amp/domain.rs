use std::error::Error;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use serde::Deserialize;
use std::collections::HashMap;

/// Original Amp structure from JSON
#[derive(Clone, Debug, Deserialize)]
pub struct OriginalAmp {
    pub keywords: Vec<String>,
    pub title: String,
    pub url: String,
    pub score: Option<f64>,
    #[serde(default)]
    pub full_keywords: Vec<(String, usize)>,
    pub advertiser: String,
    #[serde(rename = "id")]
    pub block_id: i32,
    pub iab_category: String,
    pub click_url: String,
    pub impression_url: String,
    pub icon: String,
    pub serp_categories: Vec<i32>,
}

/// Common result structure
#[derive(Clone, Debug, PartialEq)]
pub struct AmpResult {
    pub title: String,
    pub url: String,
    pub click_url: String,
    pub impression_url: String,
    pub advertiser: String,
    pub block_id: i32,
    pub iab_category: String,
    pub icon: String,
    pub full_keyword: String,
    pub serp_categories: Vec<i32>,
}

/// Full keyword for each keyword.
#[derive(Debug, Clone)]
pub enum FullKeyword {
    /// If the full keyword is the same as the keyword.
    Same,
    /// If they're different, the full keyword is stored here.
    Different(String),
}

impl FullKeyword {
    fn new(keyword: &str, full_keyword: &str) -> Self {
        if keyword == full_keyword {
            FullKeyword::Same
        } else {
            FullKeyword::Different(full_keyword.to_string())
        }
    }

    pub fn full_keyword(&self, keyword: &str) -> String {
        match self {
            FullKeyword::Same => keyword.to_string(),
            FullKeyword::Different(fw) => fw.to_string(),
        }
    }
}

/// Interface for all AMP indexers
pub trait AmpIndexer {
    /// Create a new index
    fn new() -> Self;

    /// Build the index from raw AMP data
    fn build(&mut self, amps: &[OriginalAmp]) -> Result<(), Box<dyn std::error::Error>>;

    /// Query for suggestions matching a prefix
    fn query(&self, prefix: &str) -> Result<Vec<AmpResult>, Box<dyn std::error::Error>>;

    /// Get statistics about the index
    fn stats(&self) -> HashMap<String, usize>;

    /// List the icons of the index
    fn list_icons(&self) -> Vec<String>;
}

/// Dictionary encoding for URLs
pub fn extract_template<'a>(
    url: &'a str,
    template_lookup: &mut HashMap<&'a str, u32>,
    templates: &mut HashMap<u32, String>,
) -> (u32, String) {
    let split_idx = url.find('?').unwrap_or_else(|| url.rfind('/').unwrap_or(0));
    let (template, suffix) = url.split_at(split_idx);

    match template_lookup.get(template) {
        Some(&id) => (id, suffix.to_string()),
        None => {
            let id = template_lookup.len() as u32;
            template_lookup.insert(template, id);
            templates.insert(id, template.to_string());
            (id, suffix.to_string())
        }
    }
}

/// A dictionary encoder that inserts a `needle` into the `haystack` (i.e. the dictionary)
/// and the auxiliary lookup table. It returns an integer identifier from the lookup table
/// for the `needle`.
pub fn dictionary_encode<'a>(
    needle: &'a str,
    lookup_tbl: &mut HashMap<&'a str, u32>,
    haystack: &mut HashMap<u32, String>,
) -> u32 {
    if let Some(&id) = lookup_tbl.get(needle) {
        id
    } else {
        let id = lookup_tbl.len() as u32;
        lookup_tbl.insert(needle, id);
        haystack.insert(id, needle.to_owned());
        id
    }
}

/// Collapse each maximal chain of one-char extensions into its last element,
/// while preserving how many characters the user must type (min_prefix_len)
/// to hit that collapsed key.
/// e.g. ["fo","foo","foob","fooba","foobar"] → [("foobar", 2)]
/// It also return a `FullKeyword` for each collapsed keyword.
pub fn collapse_keywords(
    keywords: &[String],
    full_keywords: &[(String, usize)],
) -> Vec<(String, usize, FullKeyword)> {
    let mut out: Vec<(String, usize, FullKeyword)> = Vec::new();

    // Restore the pointwise full keywords sequence via the RLE encoded `full_keywords`.
    let fks = full_keywords.iter().flat_map(|(full_keyword, repeat_for)| {
        std::iter::repeat_n(full_keyword.as_str(), *repeat_for)
    });

    // Zip up the keywords with the full_keywords.
    let keywords_ext: Vec<(_, _)> = keywords.iter().map(String::as_str).zip(fks).collect();

    let mut i = 0;
    while i < keywords_ext.len() {
        let (curr, curr_fk) = keywords_ext[i];
        let curr_len = curr.chars().count();

        let mut j = i + 1;
        let mut n_collapsed = 0;
        let mut prev = curr;

        // extend the run as long as each next is curr + exactly one char
        while j < keywords_ext.len() {
            let (nxt, _) = keywords_ext[j];
            if nxt.starts_with(prev) && nxt.chars().count() == curr_len + n_collapsed + 1 {
                n_collapsed += 1;
                j += 1;
                prev = nxt;
            } else {
                break;
            }
        }

        assert_eq!(j, i + n_collapsed + 1);
        if j > i + 1 {
            // we saw a run [i .. j), so collapse to keywords_ext[j-1]
            let (kw, fk) = keywords_ext[j - 1];
            out.push((kw.to_string(), curr_len, FullKeyword::new(kw, fk)));
            i = j;
        } else {
            out.push((curr.to_string(), curr_len, FullKeyword::new(curr, curr_fk)));
            i += 1;
        }
    }

    out
}

/// Utility function to load AMP data from a JSON file
pub(crate) fn load_amp_data<P: AsRef<Path>>(path: P) -> Result<Vec<OriginalAmp>, Box<dyn Error>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let amps = serde_json::from_reader(reader)?;
    Ok(amps)
}

#[cfg(test)]
mod test {
    use std::collections::HashMap;

    use crate::amp::domain::{FullKeyword, collapse_keywords, dictionary_encode, extract_template};

    #[test]
    fn test_extract_template_with_query_param() {
        let mut template_lookup = HashMap::new();
        let mut templates = HashMap::new();
        let url = "https://example.com/path?param=hello";
        let (id, suffix) = extract_template(url, &mut template_lookup, &mut templates);

        assert_eq!(id, 0);
        assert_eq!(suffix, "?param=hello");
        assert_eq!(templates.get(&id).unwrap(), "https://example.com/path");
    }

    #[test]
    fn test_extract_template_with_slash() {
        let mut template_lookup = HashMap::new();
        let mut templates = HashMap::new();
        let url = "https://example.com/path/hello";
        let (id, suffix) = extract_template(url, &mut template_lookup, &mut templates);

        assert_eq!(id, 0);
        assert_eq!(suffix, "/hello");
        assert_eq!(templates.get(&id).unwrap(), "https://example.com/path");
    }

    #[test]
    fn test_dictionary_encode() {
        let mut lookup_tbl = HashMap::new();
        let mut haystack = HashMap::new();

        let id1 = dictionary_encode("needle", &mut lookup_tbl, &mut haystack);
        let id2 = dictionary_encode("safetypin", &mut lookup_tbl, &mut haystack);
        assert_eq!(id1, 0);
        assert_eq!(id2, 1);
        assert_eq!(haystack.get(&id1).unwrap(), "needle");
        assert_eq!(haystack.get(&id2).unwrap(), "safetypin");
    }

    #[test]
    fn test_dictionary_encode_duplicates() {
        let mut lookup_tbl = HashMap::new();
        let mut haystack = HashMap::new();

        let id1 = dictionary_encode("needle", &mut lookup_tbl, &mut haystack);
        let _ = dictionary_encode("safetypin", &mut lookup_tbl, &mut haystack);
        let dupe = dictionary_encode("needle", &mut lookup_tbl, &mut haystack);
        assert_eq!(id1, 0);
        assert_eq!(dupe, 0)
    }

    #[test]
    fn test_collapse_keywords() {
        let keywords = vec![
            "mer".to_string(),
            "meri".to_string(),
            "merin".to_string(),
            "merino".to_string(),
            "amp".to_string(),
        ];
        let full_keywords = vec![("mer".to_string(), 7), ("amp".to_string(), 4)];
        let expected = vec![
            ("merino".to_string(), 3, FullKeyword::Same),
            ("amp".to_string(), 3, FullKeyword::Same),
        ];

        let collapsed = collapse_keywords(&keywords, &full_keywords);
        assert_eq!(collapsed.len(), 2);
        for ((actual_kw, actual_len, _), (exp_kw, exp_len, _)) in collapsed.iter().zip(expected) {
            assert_eq!(*actual_kw, exp_kw);
            assert_eq!(*actual_len, exp_len);
        }
    }
}
