PY_TEST_DIR := tests
APP_AND_TEST_DIRS := $(PY_TEST_DIR)
INSTALL_STAMP := .install.stamp
TEST_DIR := tests
UV := $(shell command -v uv 2> /dev/null)
CARGO := $(shell command -v cargo 2> /dev/null)

.PHONY: install
install: $(INSTALL_STAMP)  ##  Install dependencies with uv
$(INSTALL_STAMP): pyproject.toml uv.lock
	@if [ -z $(UV) ]; then echo "uv could not be found."; exit 2; fi
	$(UV) sync --all-groups
	touch $(INSTALL_STAMP)

.PHONY: ruff-lint
ruff-lint: $(INSTALL_STAMP)  ##  Run ruff linting
	$(UV) run ruff check $(APP_AND_TEST_DIRS)

.PHONY: ruff-fmt
ruff-fmt: $(INSTALL_STAMP)  ##  Run ruff format checker
	$(UV) run ruff format --check $(APP_AND_TEST_DIRS)

.PHONY: ruff-format
ruff-format: $(INSTALL_STAMP)  ##  Run ruff format
	$(UV) run ruff format $(APP_AND_TEST_DIRS)

.PHONY: cargo-fmt
cargo-fmt: $(INSTALL_STAMP)  ##  Run cargo fmt checker
	$(CARGO) fmt --all --check

.PHONY: lint
lint: $(INSTALL_STAMP) ruff-lint ruff-fmt cargo-fmt ##  Run various linters

.PHONY: dev
dev: $(INSTALL_STAMP)  ## Run maturin develop
	$(UV) run maturin develop --uv

.PHONY: format
format: ruff-format cargo-fmt  ##  Format the entire codebase

.PHONY: pytest
pytest: $(INSTALL_STAMP)  ##  Run Python tests
	$(UV) run pytest $(UNIT_TEST_DIR)

.PHONY: dev-test  ## Run `maturin develop` and then pytest
dev-test: $(INSTALL_STAMP) dev pytest

.PHONY: test  ## Run both `cargo test` and `pytest`
test: pytest

.PHONY: help
help:
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*?## "}; {printf "\033[36m%-20s\033[0m %s\n", $$1, $$2}'
