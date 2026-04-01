#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
DEPLOY_SCRIPT="$SCRIPT_DIR/deploy.sh"

# --- Create test WASM files ---
FAKE_WASM=/tmp/fake_valid.wasm
echo -n -e "\x00\x61\x73\x6D\x01" > "$FAKE_WASM"  # Minimal WASM magic header

INVALID_WASM=/tmp/fake_invalid.wasm
echo -n -e "\xFF\x61\x73\x6D\x01" > "$INVALID_WASM"  # Wrong magic header

EMPTY_WASM=/tmp/empty_wasm
touch "$EMPTY_WASM"  # Empty file

fail() { echo "✘ FAIL: $1"; exit 1; }
pass() { echo "✔ PASS: $1"; }

# --------------------- MOCKING HELPERS ----------------------

MOCK_BIN="$(pwd)/mock_bin"
mkdir -p "$MOCK_BIN"

enable_identity_mock() {
    echo '#!/usr/bin/env bash
if [[ "$1" = "keys" && "$2" = "address" ]]; then
    echo FAKE_ADDRESS
    exit 0
fi
echo "Mock stellar call"
exit 0' > "$MOCK_BIN/stellar"

    chmod +x "$MOCK_BIN/stellar"
    export PATH="$MOCK_BIN:$ORIGINAL_PATH"
}

disable_identity_mock() {
    export PATH="$ORIGINAL_PATH"
}

enable_invalid_identity_with_network_mock() {
    cat > "$MOCK_BIN/stellar" <<'EOF'
#!/usr/bin/env bash
if [[ "$1" = "keys" && "$2" = "address" ]]; then
    echo "Identity not found" >&2
    exit 1
fi
echo "Mock stellar call"
exit 0
EOF

    cat > "$MOCK_BIN/curl" <<'EOF'
#!/usr/bin/env bash
# Keep connectivity checks deterministic in offline CI/sandbox runs.
exit 0
EOF

    chmod +x "$MOCK_BIN/stellar" "$MOCK_BIN/curl"
    export PATH="$MOCK_BIN:$ORIGINAL_PATH"
}

ORIGINAL_PATH="$PATH"

# --------------------- TEST RUNNER --------------------------

run_expect_fail() {
    desc="$1"
    expected="$2"
    shift 2

    set +e
    output=$("$DEPLOY_SCRIPT" "$@" 2>&1)
    exit_code=$?
    set -e

    if [[ $exit_code -eq 0 ]]; then
        echo "$output"
        fail "$desc (expected failure, got exit 0)"
    fi

    if ! echo "$output" | grep -q "$expected"; then
        echo "$output"
        fail "$desc (expected message '$expected')"
    fi

    pass "$desc"
}

run_expect_success() {
    desc="$1"
    expected="$2"
    shift 2

    set +e
    output=$("$DEPLOY_SCRIPT" "$@" 2>&1)
    exit_code=$?
    set -e

    if [[ $exit_code -ne 0 ]]; then
        echo "$output"
        fail "$desc (expected success, got exit $exit_code)"
    fi

    if [[ -n "$expected" ]] && ! echo "$output" | grep -q "$expected"; then
        echo "$output"
        fail "$desc (expected output containing '$expected')"
    fi

    pass "$desc"
}

echo "=== Deployment Script Failure Tests ==="

# ------------------------------------------------------------
# 1. Missing WASM file (NO mocking)
# ------------------------------------------------------------
disable_identity_mock
run_expect_fail "Missing WASM file" "No WASM file specified"

# ------------------------------------------------------------
# 2. Invalid WASM path (NO mocking)
# ------------------------------------------------------------
run_expect_fail "Invalid WASM file path" "WASM file not found" "/tmp/this_file_does_not_exist.wasm"

# ------------------------------------------------------------
# 3. Invalid WASM file format (should warn but continue)
# ------------------------------------------------------------
set +e
output=$("$DEPLOY_SCRIPT" "$INVALID_WASM" 2>&1)
exit_code=$?
set -e

# Note: Invalid WASM format only warns, doesn't fail
if echo "$output" | grep -q "may not be a valid WASM binary"; then
    pass "Invalid WASM file format (warning displayed)"
else
    fail "Invalid WASM file format" "warning about invalid WASM" "output: $output"
fi

# ------------------------------------------------------------
# 4. Empty WASM file
# ------------------------------------------------------------
run_expect_fail "Empty WASM file" "WASM file is empty" "$EMPTY_WASM"

# ------------------------------------------------------------
# 5. Invalid network
# ------------------------------------------------------------
run_expect_fail "Invalid network" "Invalid network" "$FAKE_WASM" -n "invalid_network"

# ------------------------------------------------------------
# 6. Invalid identity should FAIL identity check (deterministic mock)
# ------------------------------------------------------------
enable_invalid_identity_with_network_mock
run_expect_fail "Invalid identity" "Identity not found" "$FAKE_WASM" --identity "ghost_id"

# ------------------------------------------------------------
# 7. Missing CLI dependency (requires identity mock)
# ------------------------------------------------------------
enable_identity_mock
PATH="/usr/bin:/bin" run_expect_fail \
  "Missing soroban CLI" \
  "Neither 'stellar' nor 'soroban' CLI found" \
  "$FAKE_WASM"

# ------------------------------------------------------------
# 8. Help should succeed even without dependencies
# ------------------------------------------------------------
run_expect_success "Help command works" "USAGE:" "$DEPLOY_SCRIPT" --help

# ------------------------------------------------------------
# 9. Dry run should work with valid inputs
# ------------------------------------------------------------
enable_identity_mock
run_expect_success "Dry run mode" "DRY RUN" "$FAKE_WASM" --dry-run

# ------------------------------------------------------------
# 10. Multiple arguments error handling
# ------------------------------------------------------------
disable_identity_mock
run_expect_fail "Multiple WASM arguments" "Unexpected argument" "$FAKE_WASM" "$FAKE_WASM"

# ------------------------------------------------------------
# 11. Unknown option handling
# ------------------------------------------------------------
run_expect_fail "Unknown option" "Unknown option" "$FAKE_WASM" --unknown-option

# ------------------------------------------------------------
# 12. Missing config file should warn but not fail
# ------------------------------------------------------------
set +e
output=$("$DEPLOY_SCRIPT" "$FAKE_WASM" -c "/tmp/nonexistent_config.env" 2>&1)
exit_code=$?
set -e

if [[ $exit_code -ne 0 ]] && echo "$output" | grep -q "Config file not found"; then
    pass "Missing config file handled gracefully"
else
    fail "Missing config file should warn but potentially continue"
fi

# Cleanup test files
rm -f "$FAKE_WASM" "$INVALID_WASM" "$EMPTY_WASM"

echo "All deployment failure tests passed!"
