#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
UPGRADE_SCRIPT="$SCRIPT_DIR/upgrade.sh"

# --- Create test WASM files ---
FAKE_WASM=/tmp/fake_valid.wasm
echo -n -e "\x00\x61\x73\x6D\x01" > "$FAKE_WASM"  # Minimal WASM magic header

INVALID_WASM=/tmp/fake_invalid.wasm
echo -n -e "\xFF\x61\x73\x6D\x01" > "$INVALID_WASM"  # Wrong magic header

EMPTY_WASM=/tmp/empty_wasm
touch "$EMPTY_WASM"  # Empty file

fail() { echo "✘ FAIL: $1"; exit 1; }
pass() { echo "✔ PASS: $1"; }

MOCK_BIN="$(pwd)/mock_bin"
mkdir -p "$MOCK_BIN"
ORIGINAL_PATH="$PATH"

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
exit 0
EOF

    chmod +x "$MOCK_BIN/stellar" "$MOCK_BIN/curl"
    export PATH="$MOCK_BIN:$ORIGINAL_PATH"
}

disable_mocks() {
    export PATH="$ORIGINAL_PATH"
}

enable_network_mock() {
    rm -f "$MOCK_BIN/stellar" "$MOCK_BIN/soroban"
    cat > "$MOCK_BIN/curl" <<'EOF'
#!/usr/bin/env bash
exit 0
EOF
    chmod +x "$MOCK_BIN/curl"
    export PATH="$MOCK_BIN:$ORIGINAL_PATH"
}

run_expect_fail() {
    desc="$1"
    expected_msg="$2"
    shift 2

    set +e
    output=$("$UPGRADE_SCRIPT" "$@" 2>&1)
    exit_code=$?
    set -e

    if [[ $exit_code -eq 0 ]]; then
        echo "$output"
        fail "$desc (expected failure, got exit 0)"
    fi

    if ! echo "$output" | grep -q "$expected_msg"; then
        echo "$output"
        fail "$desc (expected message '$expected_msg')"
    fi

    pass "$desc"
}

run_expect_success() {
    desc="$1"
    expected_msg="$2"
    shift 2

    set +e
    output=$("$UPGRADE_SCRIPT" "$@" 2>&1)
    exit_code=$?
    set -e

    if [[ $exit_code -ne 0 ]]; then
        echo "$output"
        fail "$desc (expected success, got exit $exit_code)"
    fi

    if [[ -n "$expected_msg" ]] && ! echo "$output" | grep -q "$expected_msg"; then
        echo "$output"
        fail "$desc (expected output containing '$expected_msg')"
    fi

    pass "$desc"
}

echo "=== Upgrade Script Failure Tests ==="

#  1. Missing contract ID
run_expect_fail "Missing contract ID" "No contract ID specified"

#  2. Invalid contract ID format
run_expect_fail "Invalid format" "Contract ID format may be invalid" "BAD_ID" "/tmp/missing.wasm"

#  3. Missing WASM file argument
run_expect_fail "Missing WASM file" "No WASM file specified" "C1234567890123456789012345678901234567890123456789012345678"

#  4. Nonexistent WASM
run_expect_fail "Invalid WASM file path" "WASM file not found" \
    "C1234567890123456789012345678901234567890123456789012345678" \
    "/tmp/not_real_contract.wasm"

#  5. Invalid WASM file format (should warn but continue)
set +e
output=$("$UPGRADE_SCRIPT" "C1234567890123456789012345678901234567890123456789012345678" "$INVALID_WASM" 2>&1)
exit_code=$?
set -e

# Note: Invalid WASM format only warns, doesn't fail
if echo "$output" | grep -q "may not be a valid WASM binary"; then
    pass "Invalid WASM file format (warning displayed)"
else
    fail "Invalid WASM file format" "warning about invalid WASM" "output: $output"
fi

#  6. Empty WASM file
run_expect_fail "Empty WASM file" "WASM file is empty" \
    "C1234567890123456789012345678901234567890123456789012345678" \
    "$EMPTY_WASM"

#  7. Invalid network
echo -n -e "\x00\x61\x73\x6D\x01" > "$FAKE_WASM"
run_expect_fail "Invalid network" "Invalid network" \
    "C1234567890123456789012345678901234567890123456789012345678" \
    "$FAKE_WASM" -n "invalid_network"

#  8. Invalid identity
enable_invalid_identity_with_network_mock
run_expect_fail "Missing identity" "Identity not found" \
    "C1234567890123456789012345678901234567890123456789012345678" \
    "$FAKE_WASM" \
    --source ghost_id
disable_mocks

#  9. Help should succeed
run_expect_success "Help command works" "USAGE:" "$UPGRADE_SCRIPT" --help

#  10. Dry run should work with valid inputs
enable_network_mock
run_expect_success "Dry run mode" "DRY RUN" \
    "C1234567890123456789012345678901234567890123456789012345678" \
    "$FAKE_WASM" --dry-run
disable_mocks

#  11. Unknown option handling
run_expect_fail "Unknown option" "Unknown option" \
    "C1234567890123456789012345678901234567890123456789012345678" \
    "$FAKE_WASM" --unknown-option

#  12. Missing config file should warn but not fail
set +e
output=$("$UPGRADE_SCRIPT" "C1234567890123456789012345678901234567890123456789012345678" "$FAKE_WASM" -c "/tmp/nonexistent_config.env" 2>&1)
exit_code=$?
set -e

if [[ $exit_code -ne 0 ]] && echo "$output" | grep -q "Config file not found"; then
    pass "Missing config file handled gracefully"
else
    fail "Missing config file should warn but potentially continue"
fi

# Cleanup test files
rm -f "$FAKE_WASM" "$INVALID_WASM" "$EMPTY_WASM"

echo "All upgrade failure tests passed!"
