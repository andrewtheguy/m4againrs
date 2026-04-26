# m4againrs

Minimal Rust library for **fixed-step gain adjustment of AAC audio in M4A/MP4
files**. Based on https://github.com/M-Igashi/mp3rgain, narrowed to AAC gain
rewriting only: no MP3 support, loudness analysis, replaygain tags, or undo
metadata.

The library finds AAC `global_gain` fields in the bitstream and adds or
subtracts the requested number of native AAC gain steps. Unlike the original
implementation, the file API streams source audio to a separate destination
file instead of modifying the input in place; the output is written forward and
does not need to be seekable. It patches only the gain bits and
records the applied step in a custom MP4 metadata tag.

## Workspace layout

This is a Cargo workspace:

| Crate | Path | Purpose |
| --- | --- | --- |
| `m4againrs` | `./` | The primary Rust library — what other crates depend on. |
| `m4againrs-cli` | `cli/` | Standalone CLI binary (`m4againrs`). |
| `m4againrs-py` | `python/` | PyO3 bindings, built into a Python wheel via maturin. |

## Library

```toml
[dependencies]
m4againrs = { git = "https://github.com/andrewtheguy/m4againrs" }
```

```rust
use std::path::Path;

m4againrs::aac_apply_gain_file(
    Path::new("track.m4a"),
    Path::new("track_louder.m4a"),
    2,
)?;
```

For non-file outputs, pass a seekable input and any `std::io::Write` output:

```rust
use std::fs::File;

let mut input = File::open("track.m4a")?;
let mut output = Vec::new();
m4againrs::aac_apply_gain_to_writer(&mut input, &mut output, 2)?;
```

`gain_steps == 0` returns an `Error`, as does passing the same source and
destination path. `m4againrs::GAIN_STEP_DB` exposes the AAC step size (1.5 dB).

## CLI

Build and use the standalone binary:

```bash
cargo build --release -p m4againrs-cli
./target/release/m4againrs <input.m4a> <output.m4a|-> <gain_steps>
```

Three positional arguments — input path, output path, signed integer step
count. One step is 1.5 dB. The source file is never overwritten, and a
custom `M4AG` MP4 metadata tag is written to the destination. Use `-` as the
output path to stream the modified M4A to stdout.

```bash
m4againrs track.m4a track_louder.m4a 2     # +3.0 dB
m4againrs track.m4a track_softer.m4a -2    # -3.0 dB
```

## Python bindings

Prebuilt wheels (Linux x86_64/arm64, macOS arm64, Windows x86_64) are published
via GitHub Pages as a PEP 503 index:

```bash
pip install m4againrs --extra-index-url https://andrewtheguy.github.io/m4againrs/simple/
```

Or with [uv](https://docs.astral.sh/uv/):

```bash
uv pip install m4againrs --extra-index-url https://andrewtheguy.github.io/m4againrs/simple/
```

Requires Python ≥ 3.9 (abi3 wheels).

```python
import m4againrs

m4againrs.aac_apply_gain_file("track.m4a", "track_louder_file.m4a", 2)
with open("track_louder_writer.m4a", "wb") as output:
    m4againrs.aac_apply_gain_to_writer("track.m4a", output, 2)
m4againrs.GAIN_STEP_DB  # 1.5
```

Build from source (needs Rust + [maturin](https://www.maturin.rs/)):

```bash
git clone https://github.com/andrewtheguy/m4againrs.git
cd m4againrs/python
uv venv
uv pip install maturin
uv run maturin develop --release
```

## Units

`gain_steps` is the native AAC `global_gain` unit (an 8-bit integer in the
bitstream). One step is 1.5 dB. To think in dB:
`steps = round(db / m4againrs.GAIN_STEP_DB)`.

Zero steps is rejected; gain locations are saturating-clamped to `0..=255`;
locations with `global_gain == 0` are skipped (silence).

The file API writes custom MP4 metadata to the destination:
`TAG:M4AG=m4againrs version=1 gain_steps=<n> gain_step_db=1.5`.
Use `ffprobe -export_all 1` to show the custom tag.

## Development

```bash
cargo test --workspace
cargo clippy --workspace -- -D warnings

# Python bindings
cd python
uv venv
uv run --no-project --with 'maturin>=1.9.4,<2.0' maturin develop --skip-install
uv run --no-sync python -m unittest tests.test_python_bindings -v
```

The Python binding tests load the built extension from the workspace
`target/debug` or `target/release`; they do not import an installed
`site-packages` copy.

The `python/testdata/tagged_tone.m4a` fixture is committed; to regenerate it
with ffmpeg, run `python/testdata/regenerate.sh`.
