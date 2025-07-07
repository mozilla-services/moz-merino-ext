"""Test the top-level `moz-merino-ext` module."""

import tomllib

import moz_merino_ext


def test_doc() -> None:
    """Test the top-level module doc."""
    assert (
        moz_merino_ext.__doc__
        == "Python extensions for Mozilla/Merino implemented in Rust using PyO3."
    )


def test_version() -> None:
    """Test the top-level module version."""
    with open("pyproject.toml", "rb") as f:
        data = tomllib.load(f)
        version = data["project"]["version"]

    assert moz_merino_ext.__version__ == version
