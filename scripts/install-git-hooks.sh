#!/bin/sh
set -eu

repo_root=$(git rev-parse --show-toplevel)
git -C "$repo_root" config --local core.hooksPath .githooks
printf 'Installed CivCraft Git hooks from %s/.githooks\n' "$repo_root"
