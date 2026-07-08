"""Test `amp` module."""

import itertools
import json

import pytest

from typing import Any

from moz_merino_ext.amp import AmpIndexManager, PyAmpResult


AMP_DATA_TYPE = list[dict[str, Any]]
IDX_NAME: str = "us/desktop"


def assert_suggestion(expect: dict[str, Any], actual: PyAmpResult) -> bool:
    """Assertion helper to compare a suggestion dict and a `PyAmpResult`"""
    for attr, key in [
        ("block_id", "id"),
        ("advertiser", None),
        ("iab_category", None),
        ("title", None),
        ("url", None),
        ("icon", None),
        ("impression_url", None),
        ("click_url", None),
    ]:
        key = key or attr
        assert expect[key] == getattr(actual, attr), (
            f"Expect `{expect[key]}` for `{attr}`, found `{getattr(actual, attr)}`"
        )


@pytest.fixture
def idxmgr() -> AmpIndexManager:
    """Test fixture for the index manager."""
    return AmpIndexManager()


@pytest.fixture
def amp_data() -> AMP_DATA_TYPE:
    """Test fixture for the AMP data."""
    return [
        {
            "id": 100,
            "advertiser": "Los Pollos Hermanos",
            "iab_category": "8 - Food & Drink",
            "serp_categories": [0],
            "keywords": [
                "lo",
                "los",
                "los p",
                "los pollos",
                "los pollos h",
                "los pollos hermanos",
            ],
            "full_keywords": [("los pollos", 4), ("los pollos hermanos", 2)],
            "title": "Los Pollos Hermanos - Albuquerque",
            "url": "https://www.lph-nm.biz",
            "icon": "los-pollos-favicon",
            "impression_url": "https://example.com/impression_url",
            "click_url": "https://example.com/click_url",
            "score": 0.3,
        },
        {
            "id": 101,
            "advertiser": "Good Place Eats",
            "iab_category": "8 - Food & Drink",
            "keywords": ["la", "las", "lasa", "lasagna", "lasagna come out tomorrow"],
            "full_keywords": [("lasagna", 3), ("lasagna come out tomorrow", 2)],
            "title": "Lasagna Come Out Tomorrow",
            "url": "https://www.lasagna.restaurant",
            "icon": "good-place-eats-favicon",
            "impression_url": "https://example.com/impression_url",
            "click_url": "https://example.com/click_url",
            "score": 0.3,
            "serp_categories": [1, 2],
        },
    ]


def test_build_from_str(idxmgr: AmpIndexManager, amp_data: AMP_DATA_TYPE) -> None:
    """Test `build` of the index manager with a JSON str."""
    idxmgr.build(IDX_NAME, json.dumps(amp_data))

    assert len(idxmgr.list()) == 1
    assert idxmgr.has(IDX_NAME)


def test_build_from_bytes(idxmgr: AmpIndexManager, amp_data: AMP_DATA_TYPE) -> None:
    """Test `build` of the index manager with a JSON byte array."""
    idxmgr.build(IDX_NAME, json.dumps(amp_data).encode("utf-8"))

    assert len(idxmgr.list()) == 1
    assert idxmgr.has(IDX_NAME)


def test_build_from_invalid_type(idxmgr: AmpIndexManager, amp_data: AMP_DATA_TYPE) -> None:
    """Test `build` of the index manager with an invalid input."""
    with pytest.raises(TypeError) as exc:
        # It can't take a Python dict yet for index building.
        idxmgr.build(IDX_NAME, amp_data)

        assert "Invalid type for the index input" in str(exc.value)


def test_build_from_invalid_value(idxmgr: AmpIndexManager, amp_data: AMP_DATA_TYPE) -> None:
    """Test `build` of the index manager with an incomplete suggestion payload."""
    with pytest.raises(ValueError) as exc:
        # Delete a required field.
        del amp_data[0]["id"]
        idxmgr.build(IDX_NAME, json.dumps(amp_data))

        assert "Invalid JSON" in str(exc.value)


def test_delete_index(idxmgr: AmpIndexManager, amp_data: AMP_DATA_TYPE) -> None:
    """Test `delete` of the index manager."""
    idxmgr.build(IDX_NAME, json.dumps(amp_data))
    idxmgr.delete(IDX_NAME)

    assert len(idxmgr.list()) == 0


def test_query_index(idxmgr: AmpIndexManager, amp_data: AMP_DATA_TYPE) -> None:
    """Test `query` of the index manager."""
    idxmgr.build(IDX_NAME, json.dumps(amp_data))

    suggestions: list[PyAmpResult] = idxmgr.query(IDX_NAME, "a missing keyword")

    assert len(suggestions) == 0

    for expected in amp_data:
        full_keywords = list(
            itertools.chain.from_iterable(
                itertools.repeat(fk, n) for fk, n in expected["full_keywords"]
            )
        )
        for i, keyword in enumerate(expected["keywords"]):
            suggestions: list[PyAmpResult] = idxmgr.query(IDX_NAME, keyword)

            assert len(suggestions) == 1
            assert_suggestion(expected, suggestions[0])
            assert suggestions[0].full_keyword == full_keywords[i]


def test_query_fuzzy_rescues_typo(idxmgr: AmpIndexManager, amp_data: AMP_DATA_TYPE) -> None:
    """A single-typo query is rescued only when `fuzzy=True`, flagged as fuzzy."""
    idxmgr.build(IDX_NAME, json.dumps(amp_data))

    # "los pollos hermanas" is one substitution from "los pollos hermanos".
    # Exact-only (default) finds nothing.
    assert idxmgr.query(IDX_NAME, "los pollos hermanas") == []

    rescued: list[PyAmpResult] = idxmgr.query(IDX_NAME, "los pollos hermanas", fuzzy=True)
    assert len(rescued) == 1
    assert rescued[0].full_keyword == "los pollos hermanos"
    assert rescued[0].advertiser == "Los Pollos Hermanos"
    assert rescued[0].matched_via == "fuzzy"


def test_query_exact_takes_precedence_over_fuzzy(
    idxmgr: AmpIndexManager, amp_data: AMP_DATA_TYPE
) -> None:
    """An exact/prefix hit is returned (flagged "exact") even when fuzzy=True."""
    idxmgr.build(IDX_NAME, json.dumps(amp_data))

    results: list[PyAmpResult] = idxmgr.query(IDX_NAME, "los pollos hermanos", fuzzy=True)
    assert len(results) == 1
    assert results[0].full_keyword == "los pollos hermanos"
    assert results[0].matched_via == "exact"  # fuzzy fallback not used


def test_query_fuzzy_no_neighbour_returns_empty(
    idxmgr: AmpIndexManager, amp_data: AMP_DATA_TYPE
) -> None:
    """A query with no edit-distance-1 neighbour returns nothing even with fuzzy=True."""
    idxmgr.build(IDX_NAME, json.dumps(amp_data))
    assert idxmgr.query(IDX_NAME, "zzzzzzzz", fuzzy=True) == []


def test_full_keywords(idxmgr: AmpIndexManager, amp_data: AMP_DATA_TYPE) -> None:
    """`full_keywords` returns the distinct full-keyword set for the index."""
    idxmgr.build(IDX_NAME, json.dumps(amp_data))
    assert set(idxmgr.full_keywords(IDX_NAME)) == {
        "los pollos",
        "los pollos hermanos",
        "lasagna",
        "lasagna come out tomorrow",
    }


def test_list_icons(idxmgr: AmpIndexManager, amp_data: AMP_DATA_TYPE) -> None:
    """Test `delete` of the index manager."""
    idxmgr.build(IDX_NAME, json.dumps(amp_data))

    assert set(idxmgr.list_icons(IDX_NAME)) == set(data["icon"] for data in amp_data)


def test_stats(idxmgr: AmpIndexManager, amp_data: AMP_DATA_TYPE) -> None:
    """Test `delete` of the index manager."""
    idxmgr.build(IDX_NAME, json.dumps(amp_data))
    stats = idxmgr.stats(IDX_NAME)

    assert stats["keyword_index_size"] > 0
    assert stats["suggestions_count"] == len(amp_data)
    assert stats["advertisers_count"] == len(set(data["advertiser"] for data in amp_data))
    assert stats["url_templates_count"] > 0
    assert stats["icons_count"] == len(set(data["icon"] for data in amp_data))
