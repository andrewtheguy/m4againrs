#!/usr/bin/env bash
# Regenerate the committed test fixtures used by both the Rust integration
# tests (tests/file_api.rs) and the Python binding tests
# (python/tests/test_python_bindings.py).
#
#   tagged_tone.m4a    — short AAC tone with rich iTunes metadata; used to
#                        prove gain adjustment preserves container metadata
#                        byte-for-byte.
#   test_faststart.m4a — faststart (moov-before-mdat) remux of test.m4a;
#                        required by the streaming-input tests.
#
# test.m4a itself is committed source data and is not regenerated.
set -euo pipefail

cd "$(dirname "$0")"

ffmpeg -hide_banner -loglevel error \
    -f lavfi -i "sine=frequency=440:duration=2:sample_rate=44100" \
    -c:a aac -b:a 128k \
    -metadata title="Gain Test Tone" \
    -metadata artist="m4againrs" \
    -metadata album="Fixtures" \
    -metadata date="2026" \
    -metadata genre="Electronic" \
    -metadata track="3/10" \
    -metadata comment="Used to verify metadata survives gain adjustment." \
    -y tagged_tone.m4a

ffmpeg -hide_banner -loglevel error \
    -i test.m4a -c copy -movflags +faststart -y test_faststart.m4a

echo "generated: $(pwd)/tagged_tone.m4a"
echo "generated: $(pwd)/test_faststart.m4a"
