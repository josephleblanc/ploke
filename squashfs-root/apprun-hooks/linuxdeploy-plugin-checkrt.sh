#! /usr/bin/env bash

if [ -z "$APPDIR" ]; then
    APPDIR="$(dirname "$(realpath "$0")")"
fi

if [ -x "$APPDIR/checkrt/checkrt" ]; then
    CHECKRT_LIBS="$($APPDIR/checkrt/checkrt)"

    # prepend to LD_LIBRARY_PATH
    if [ -n "$CHECKRT_LIBS" ]; then
        export LD_LIBRARY_PATH="${CHECKRT_LIBS}:${LD_LIBRARY_PATH}"
    fi
fi

# check for exec.so
if [ -f "$APPDIR/checkrt/exec.so" ]; then
    export LD_PRELOAD="$APPDIR/checkrt/exec.so:${LD_PRELOAD}"
fi

# debugging
if [ -n "$CHECKRT_DEBUG" ]; then
    echo "CHECKRT>> LD_LIBRARY_PATH=$LD_LIBRARY_PATH"
    echo "CHECKRT>> LD_PRELOAD=$LD_PRELOAD"
fi
