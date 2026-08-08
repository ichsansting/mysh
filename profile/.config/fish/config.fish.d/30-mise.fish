# pin uv's python resolution to whatever mise currently has active, so a
# bare `uv venv`/`uv pip` never silently picks a different interpreter
set -gx UV_PYTHON (mise which python 2>/dev/null)
