#!/usr/bin/env bash
# Fetch reference datasets for keyflow engraving + rhythm-chart development.
# Data is downloaded into ./data/ which is gitignored.
#
# Datasets:
#   groove   - Magenta Groove MIDI (drum, grid-aligned, swing). Apache-2.0. ~3 MB.
#   openscore-lieder - OpenScore Lieder MusicXML corpus. CC0. ~few MB shallow.
#   mutopia-sample - Small handpicked LilyPond + PDF set from Mutopia. PD.
#   wjd      - Weimar Jazz Database lead-sheet/solo SQLite + MusicXML.
#   bach     - music21 Bach chorale corpus (via music21 bundled data; pointer only).
#   pop909   - POP909 pop melody/lead/piano MIDI. Research license.
#
# Usage:  ./fetch.sh [name ...]   (no args = fetch all small/permissive sets)

set -euo pipefail
cd "$(dirname "$0")"
mkdir -p data
cd data

want=("$@")
if [ ${#want[@]} -eq 0 ]; then
    want=(groove openscore-lieder mutopia-sample wjd)
fi

has() { for x in "${want[@]}"; do [ "$x" = "$1" ] && return 0; done; return 1; }

fetch() {
    local url="$1" out="$2"
    if [ -e "$out" ]; then
        echo "skip $out (exists)"
        return
    fi
    echo "fetch $url -> $out"
    curl -fL --retry 3 -o "$out" "$url"
}

if has groove; then
    fetch "https://storage.googleapis.com/magentadata/datasets/groove/groove-v1.0.0-midionly.zip" \
          "groove-v1.0.0-midionly.zip"
    [ -d groove ] || (mkdir -p groove && cd groove && unzip -q ../groove-v1.0.0-midionly.zip)
fi

if has openscore-lieder; then
    if [ ! -d openscore-lieder ]; then
        git clone --depth 1 https://github.com/OpenScore/Lieder.git openscore-lieder
    fi
fi

if has mutopia-sample; then
    # Replaced w/ music21 Bach corpus MXL fixtures - small, deterministic, BSD.
    mkdir -p bach-fixtures
    base="https://raw.githubusercontent.com/cuthbertLab/music21/master/music21/corpus/bach"
    for f in bwv66.6.mxl bwv7.7.mxl bwv57.8.mxl bwv1.6.mxl bwv8.6.mxl; do
        fetch "$base/$f" "bach-fixtures/$f"
    done
fi

if has wjd; then
    # Weimar Jazz DB SQLite release. URL changes occasionally; check jazzomat.hfm-weimar.de
    fetch "https://jazzomat.hfm-weimar.de/download/downloads/wjazzd.db" \
          "wjazzd.db" || echo "wjd: manual download from https://jazzomat.hfm-weimar.de/dbformat/dbcontent.html"
fi

if has pop909; then
    if [ ! -d POP909-Dataset ]; then
        git clone --depth 1 https://github.com/music-x-lab/POP909-Dataset.git
    fi
fi

if has bach; then
    cat <<'EOF'
bach: install via Python:
    pip install music21
    python -c "from music21 import corpus; print(corpus.getComposer('bach')[:5])"
Bundled with music21 (BSD). No download needed.
EOF
fi

echo "done. data in $(pwd)"
