#!/usr/bin/env bash
# Regenerate the committed test fixtures used by both the Rust integration
# tests (tests/file_api.rs) and the Python binding tests
# (python/tests/test_python_bindings.py).
#
#   tagged_tone.m4a        — short AAC tone with rich iTunes metadata; used to
#                            prove gain adjustment preserves container metadata
#                            byte-for-byte.
#   test_faststart.m4a     — faststart (moov-before-mdat) remux of test.m4a;
#                            required by the streaming-input tests.
#   he_aacv2_faststart.m4a — faststart remux of he_aacv2.m4a; used by the
#                            HE-AACv2 streaming-input tests.
#   aac_lc_51.m4a          — 5.1 AAC LC tone (six independent sine channels);
#                            exercises the ID_SCE (front center) and ID_LFE
#                            element branches in the raw_data_block parser.
#   aac_lc_transient.m4a   — mono AAC LC with periodic 1 kHz bursts; the
#                            encoder switches to EIGHT_SHORT_SEQUENCE on the
#                            attacks, exercising the short-window paths in
#                            ics_info / section_data / spectral parsing.
#
# test.m4a and he_aacv2.m4a are committed source data and are not regenerated.
# he_aacv2.m4a was sliced (with `-c copy`) from a longer real-world HE-AACv2
# (Parametric Stereo) capture to exercise the gain rewriter against an
# AAC profile that goes beyond plain LC.
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

ffmpeg -hide_banner -loglevel error \
    -i he_aacv2.m4a -c copy -movflags +faststart -y he_aacv2_faststart.m4a

ffmpeg -hide_banner -loglevel error \
    -filter_complex "\
sine=frequency=200:duration=2:sample_rate=48000[FL];\
sine=frequency=300:duration=2:sample_rate=48000[FR];\
sine=frequency=400:duration=2:sample_rate=48000[FC];\
sine=frequency=80:duration=2:sample_rate=48000[LFE];\
sine=frequency=500:duration=2:sample_rate=48000[BL];\
sine=frequency=600:duration=2:sample_rate=48000[BR];\
[FL][FR][FC][LFE][BL][BR]amerge=inputs=6,channelmap=channel_layout=5.1[a]" \
    -map "[a]" -c:a aac -b:a 192k -y aac_lc_51.m4a

ffmpeg -hide_banner -loglevel error \
    -f lavfi -i "aevalsrc=exprs='if(lt(mod(n\,4096)\,200)\,0.9*sin(2*PI*1000*n/44100)\,0)':d=2:s=44100:c=mono" \
    -c:a aac -b:a 96k -y aac_lc_transient.m4a

echo "generated: $(pwd)/tagged_tone.m4a"
echo "generated: $(pwd)/test_faststart.m4a"
echo "generated: $(pwd)/he_aacv2_faststart.m4a"
echo "generated: $(pwd)/aac_lc_51.m4a"
echo "generated: $(pwd)/aac_lc_transient.m4a"
