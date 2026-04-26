.PHONY: install-all test-all demo build-cli build-extension lint clean help

PYTHON ?= python
PIP    ?= $(PYTHON) -m pip

help:
	@echo "MetaTree make targets:"
	@echo "  install-all      install Python deps for action, build Rust CLI, npm install for extension"
	@echo "  test-all         run pytest, cargo test, npm test across all branches"
	@echo "  demo             run the GitHub Action locally with `act` against the sandbox"
	@echo "  build-cli        cargo build --release for the CLI (output: cli/target/release/ometa)"
	@echo "  build-extension  vsce package for the VS Code extension"
	@echo "  lint             ruff (Python), cargo clippy (Rust), eslint (TypeScript)"
	@echo "  clean            remove build artifacts"

install-all:
	$(PIP) install -r action/requirements.txt
	cd cli && cargo build
	cd vscode-extension && npm install

test-all:
	cd action && $(PYTHON) -m pytest -q
	cd cli && cargo test --quiet
	cd vscode-extension && npm test

demo:
	@command -v act >/dev/null 2>&1 || { echo "act not installed -- see https://github.com/nektos/act"; exit 1; }
	act pull_request -W .github/workflows/demo.yml \
		-s OPENMETADATA_HOST=$${OPENMETADATA_HOST:-https://sandbox.open-metadata.org/api} \
		-s OPENMETADATA_JWT_TOKEN=$${OPENMETADATA_JWT_TOKEN}

build-cli:
	cd cli && cargo build --release

build-extension:
	cd vscode-extension && npx --yes @vscode/vsce package --out metatree-vscode.vsix

lint:
	-ruff check action/
	-cd cli && cargo clippy --all-targets -- -D warnings
	-cd vscode-extension && npm run lint

clean:
	rm -rf cli/target vscode-extension/node_modules vscode-extension/out vscode-extension/*.vsix
	find . -type d -name __pycache__ -exec rm -rf {} + 2>/dev/null || true
	find . -type d -name .pytest_cache -exec rm -rf {} + 2>/dev/null || true
