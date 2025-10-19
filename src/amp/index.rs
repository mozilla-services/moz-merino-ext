use crate::amp::domain::{
    AmpIndexer, AmpResult, FullKeyword, OriginalAmp, collapse_keywords, dictionary_encode,
    extract_template,
};
use std::collections::{BTreeMap, HashMap};
use std::ops::Bound::{Included, Unbounded};

/// Optimized AMP suggestion for storage
#[derive(Clone)]
struct AmpSuggestion {
    title: String,
    url_tid: u32,
    url_suf: String,
    click_tid: u32,
    click_suf: String,
    imp_tid: u32,
    imp_suf: String,
    advertiser_id: u32,
    block_id: i32,
    iab: String,
    icon_id: u32,
    serp_categories: Vec<i32>,
}

pub struct BTreeAmpIndex {
    /// collapsed prefix → (suggestion_idx, unused_min_pref, full_keyword)
    pub keyword_index: BTreeMap<String, (usize, usize, FullKeyword)>,
    suggestions: Vec<AmpSuggestion>,
    advertisers: HashMap<u32, String>,
    url_templates: HashMap<u32, String>,
    click_templates: HashMap<u32, String>,
    imp_templates: HashMap<u32, String>,
    icons: HashMap<u32, String>,
}

impl AmpIndexer for BTreeAmpIndex {
    fn new() -> Self {
        BTreeAmpIndex {
            keyword_index: BTreeMap::new(),
            suggestions: Vec::new(),
            advertisers: HashMap::new(),
            url_templates: HashMap::new(),
            click_templates: HashMap::new(),
            imp_templates: HashMap::new(),
            icons: HashMap::new(),
        }
    }

    fn build(&mut self, amps: &[OriginalAmp]) -> Result<(), Box<dyn std::error::Error>> {
        let mut adv_lookup = HashMap::new();
        let mut url_lookup = HashMap::new();
        let mut click_lookup = HashMap::new();
        let mut imp_lookup = HashMap::new();
        let mut icon_lookup = HashMap::new();

        for amp in amps {
            // Internal advertiser ID.
            let adv_id = dictionary_encode(&amp.advertiser, &mut adv_lookup, &mut self.advertisers);
            // Internal icon ID.
            let icon_id = dictionary_encode(&amp.icon, &mut icon_lookup, &mut self.icons);

            // Templatize URLs
            let (url_tid, url_suf) =
                extract_template(&amp.url, &mut url_lookup, &mut self.url_templates);
            let (click_tid, click_suf) =
                extract_template(&amp.click_url, &mut click_lookup, &mut self.click_templates);
            let (imp_tid, imp_suf) = extract_template(
                &amp.impression_url,
                &mut imp_lookup,
                &mut self.imp_templates,
            );

            // Store suggestion
            let idx = self.suggestions.len();
            self.suggestions.push(AmpSuggestion {
                title: amp.title.clone(),
                url_tid,
                url_suf,
                click_tid,
                click_suf,
                imp_tid,
                imp_suf,
                advertiser_id: adv_id,
                block_id: amp.block_id,
                iab: amp.iab_category.clone(),
                icon_id,
                serp_categories: amp.serp_categories.clone(),
            });

            // Collapse each chain on keyword partials
            for (kw, min_pref, fw) in
                collapse_keywords(&amp.keywords, &amp.full_keywords).into_iter()
            {
                self.keyword_index.insert(kw, (idx, min_pref, fw));
            }
        }

        self.suggestions.shrink_to_fit();

        Ok(())
    }

    fn query(&self, query: &str) -> Result<Vec<AmpResult>, Box<dyn std::error::Error>> {
        let qlen = query.chars().count();
        let range = (Included(query), Unbounded);
        let mut best: Option<(&String, &(usize, usize, FullKeyword))> = None;

        // scan collapsed keys in order, picking the shortest key that meets min_pref
        for (key, val) in self.keyword_index.range::<str, _>(range) {
            match (key, val) {
                (key, _) if !key.starts_with(query) => break,
                (_, &(_, min_pref, _)) if qlen < min_pref => continue,
                (_, _) => {
                    best = Some((key, val));
                    break;
                }
            }
        }

        // if we found a match, build and return it
        if let Some((key, &(sidx, _, ref fk))) = best {
            let mut out = Vec::new();
            self.build_result(key, sidx, fk, &mut out)?;
            return Ok(out);
        }
        Ok(Vec::new())
    }

    fn stats(&self) -> HashMap<String, usize> {
        let mut m = HashMap::new();
        m.insert("keyword_index_size".into(), self.keyword_index.len());
        m.insert("suggestions_count".into(), self.suggestions.len());
        m.insert("advertisers_count".into(), self.advertisers.len());
        m.insert("url_templates_count".into(), self.url_templates.len());
        m.insert("icons_count".into(), self.icons.len());
        m
    }

    fn list_icons(&self) -> Vec<String> {
        self.icons.values().cloned().collect::<_>()
    }
}

impl BTreeAmpIndex {
    fn build_result(
        &self,
        keyword: &str,
        sidx: usize,
        full_keyword: &FullKeyword,
        results: &mut Vec<AmpResult>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let sugg = &self.suggestions[sidx];

        let url = Self::reconstruct(&self.url_templates, sugg.url_tid, &sugg.url_suf);
        let click = Self::reconstruct(&self.click_templates, sugg.click_tid, &sugg.click_suf);
        let imp = Self::reconstruct(&self.imp_templates, sugg.imp_tid, &sugg.imp_suf);
        let adv = self
            .advertisers
            .get(&sugg.advertiser_id)
            .cloned()
            .unwrap_or_default();
        let icon = self.icons.get(&sugg.icon_id).cloned().unwrap_or_default();

        results.push(AmpResult {
            title: sugg.title.clone(),
            url,
            click_url: click,
            impression_url: imp,
            advertiser: adv,
            block_id: sugg.block_id,
            iab_category: sugg.iab.clone(),
            icon,
            full_keyword: full_keyword.full_keyword(keyword),
            serp_categories: sugg.serp_categories.clone(),
        });
        Ok(())
    }

    fn reconstruct(dict: &HashMap<u32, String>, tid: u32, suffix: &str) -> String {
        dict.get(&tid)
            .map_or_else(|| suffix.to_string(), |t| format!("{}{}", t, suffix))
    }
}

#[cfg(test)]
mod test {
    use crate::amp::{
        domain::{AmpIndexer, AmpResult, FullKeyword, OriginalAmp},
        index::BTreeAmpIndex,
    };

    fn test_amp_fixture() -> OriginalAmp {
        OriginalAmp {
            keywords: vec![
                "los".to_string(),
                "los p".to_string(),
                "los pollos".to_string(),
                "los pollos h".to_string(),
                "los pollos hermanos".to_string(),
            ],
            title: "Los Pollos Hermanos - Albuquerque".to_string(),
            url: "https://www.lph-nm.biz".to_string(),
            score: Some(0.3),
            full_keywords: vec![
                ("los pollos".to_string(), 4),
                ("los pollos hermanos".to_string(), 2),
            ],
            advertiser: "Los Pollos Hermanos".to_string(),
            block_id: 1,
            iab_category: "8 - Food & Drink".to_string(),
            click_url: "https://example.com/click_url".to_string(),
            impression_url: "https://example.com/impression_url".to_string(),
            icon: "https://example.com/icon".to_string(),
            serp_categories: vec![0],
        }
    }

    #[test]
    fn test_build_and_query() {
        let amp = test_amp_fixture();
        let mut index = BTreeAmpIndex::new();
        index.build(&[amp.clone()]).unwrap();

        let result = index.query("los pollos hermanos").unwrap();
        assert_eq!(result.len(), 1);
        let suggestion = &result[0];
        assert_eq!(suggestion.title, amp.title);
        assert_eq!(suggestion.advertiser, amp.advertiser);
        assert_eq!(suggestion.url, amp.url);
        assert_eq!(suggestion.block_id, amp.block_id);
        assert_eq!(suggestion.iab_category, amp.iab_category);
        assert_eq!(suggestion.click_url, amp.click_url);
        assert_eq!(suggestion.impression_url, amp.impression_url);
        assert_eq!(suggestion.icon, amp.icon);
        assert_eq!(suggestion.serp_categories, amp.serp_categories);
    }

    #[test]
    fn test_query_with_no_matches() {
        let amp = test_amp_fixture();
        let mut index = BTreeAmpIndex::new();
        index.build(&[amp]).unwrap();

        let results = index.query("abc").unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_stats() {
        let amp = test_amp_fixture();
        let mut index = BTreeAmpIndex::new();
        index.build(&[amp]).unwrap();

        let stats = index.stats();
        assert_eq!(stats["keyword_index_size"], 5);
        assert_eq!(stats["suggestions_count"], 1);
        assert_eq!(stats["advertisers_count"], 1);
        assert_eq!(stats["url_templates_count"], 1);
        assert_eq!(stats["icons_count"], 1);
    }

    #[test]
    fn test_list_icons() {
        let amp = test_amp_fixture();
        let mut index = BTreeAmpIndex::new();
        index.build(&[amp.clone()]).unwrap();

        let icons = index.list_icons();
        assert_eq!(icons.len(), 1);
        assert_eq!(icons[0], amp.icon);
    }

    #[test]
    fn test_build_result_format() {
        let amp = test_amp_fixture();
        let mut index = BTreeAmpIndex::new();
        index.build(&[amp.clone()]).unwrap();

        let mut results = Vec::new();
        index
            .build_result(
                "los",
                0,
                &FullKeyword::Different("los pollos".to_string()),
                &mut results,
            )
            .unwrap();

        assert_eq!(results.len(), 1);
        let expected = AmpResult {
            title: amp.title.clone(),
            url: amp.url.clone(),
            click_url: amp.click_url.clone(),
            impression_url: amp.impression_url.clone(),
            advertiser: amp.advertiser.clone(),
            block_id: amp.block_id,
            iab_category: amp.iab_category.clone(),
            icon: amp.icon.clone(),
            full_keyword: "los pollos".to_string(),
            serp_categories: vec![0],
        };

        assert_eq!(results[0], expected);
    }
}
