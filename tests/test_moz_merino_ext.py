"""Test the top-level `moz-merino-ext` module."""

import moz_merino_ext


def test_doc() -> None:
    """Test the top-level module doc."""
    assert (
        moz_merino_ext.__doc__
        == "Python extensions for Mozilla/Merino implemented in Rust using PyO3."
    )
