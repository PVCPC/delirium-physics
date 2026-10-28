#!/bin/sh
# Delirium room in Safe Mode (every graphics setting at its lowest, your saved settings left alone): the regular launcher, beside this file, with --safe.
cd "$(dirname "$0")" || exit 1
exec sh ./delirium-room.command --safe "$@"
