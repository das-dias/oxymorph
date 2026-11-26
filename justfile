# initialize repo
init:
  git submodule update --init --recursive

# build the Python package (release, via uv + maturin) and run its tests
test-python *args:
  uv sync --group test --reinstall-package oxymorph
  uv run --no-sync pytest crates/oxymorph-python/tests {{args}}
