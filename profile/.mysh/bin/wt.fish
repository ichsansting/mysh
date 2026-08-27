#!/bin/sh
export MISE_DATA_DIR="$HOME/.mysh/mise"
exec mise x ./wt.fish -- wt.fish "$@"
