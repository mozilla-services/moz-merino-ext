<a name="0.3.2"></a>
## 0.3.2 (2026-07-08)

### Features

* Add edit-distance-1 fuzzy matching to the AMP index: `fuzzy` flag on `query`, a `matched_via` field on results, and a `full_keywords` accessor

### Maintenance

* Reduce SymIndex memory footprint: inline single keyword-id storage (`Ids` enum) + `Box<str>` delete-index keys (~10% smaller fuzzy index; exact/fuzzy latency and build time unchanged)

<a name="0.3.1"></a>
## 0.3.1 (2026-06-17)

### Maintenance

* Migrate macOS x86_64 CI runner from retired macos-13 to macos-15-intel

<a name="0.3.0"></a>
## 0.3.0 (2026-06-17)

### Features 

* Add top_pick_prefix field

### Maintenance

* Bump GHA versions and remove free-threaded wheels
* Bump python and dependencies

<a name="0.2.0"></a>
## 0.2.0 (2025-10-19)

### Features 

* Add serp_categories

<a name="0.1.0"></a>
## 0.1.0 (2025-07-07)

### Features 

* Init commit of moz-merino-ext 

