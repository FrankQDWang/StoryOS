#!/bin/sh
set -eu

cd "$(dirname "$0")"
npm ci
npm run benchmark
python3 postgres-benchmark.py
python3 summarize.py
python3 hash-evidence.py
python3 verify-evidence.py
