#!/bin/sh
set -eu
cd "$(dirname "$0")/../.."
lab=prototypes/chinese-long-form-baseline
python3 "$lab/generate.py"
if [ ! -x target/release-package/storyos-server ]; then
  make release-package
fi
for scale in ${SCALES:-30000 300000 1000000 3000000}; do
  SCALE="$scale" scripts/dev-postgres.sh run node "$lab/measure.mjs"
done
python3 "$lab/count-profiles.py" ${WORD_COUNTS:+--word} --golden "$lab/count-golden.json" "$lab"/out/corpus-*.json "$lab/public-domain.txt" > "$lab/out/word-count-evidence.json"
python3 "$lab/summarize.py"
