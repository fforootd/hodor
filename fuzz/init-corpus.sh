#!/usr/bin/env bash
# Populate per-target corpus directories from seed files.
# Run once before first fuzzing session.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
SEEDS_DIR="$SCRIPT_DIR/seeds"

# Targets that accept string input benefit from text-based seeds.
STRING_TARGETS=(
    fuzz_cookie_verify
    fuzz_authorize_params_deser
    fuzz_token_request_deser
)

for target in "${STRING_TARGETS[@]}"; do
    corpus_dir="$SCRIPT_DIR/corpus/$target"
    mkdir -p "$corpus_dir"
    i=0
    for seed_file in "$SEEDS_DIR"/*.txt; do
        while IFS= read -r line; do
            [ -z "$line" ] && continue
            echo -n "$line" > "$corpus_dir/seed_${i}"
            ((i++))
        done < "$seed_file"
    done
    echo "  $target: $i seeds"
done

echo "Corpus initialized."
