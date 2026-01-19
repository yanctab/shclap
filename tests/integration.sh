#!/usr/bin/env bash
#
# shclap Integration Tests
#
# This script tests shclap's end-to-end behavior and serves as an example
# of what's possible with shclap. Run with: ./tests/integration.sh
#
# Exit codes:
#   0 - All tests passed
#   1 - One or more tests failed
#
set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Test counters
TESTS_RUN=0
TESTS_PASSED=0
TESTS_FAILED=0

# Find the shclap binary
if [[ -x "./target/release/shclap" ]]; then
    SHCLAP="./target/release/shclap"
elif [[ -x "./target/debug/shclap" ]]; then
    SHCLAP="./target/debug/shclap"
elif command -v shclap &> /dev/null; then
    SHCLAP="shclap"
else
    echo -e "${RED}Error: shclap binary not found. Run 'cargo build' first.${NC}"
    exit 1
fi

echo "Using shclap: $SHCLAP"
echo "========================================"
echo ""

#
# Test helper functions
#

pass() {
    TESTS_PASSED=$((TESTS_PASSED + 1))
    echo -e "  ${GREEN}PASS${NC}: $1"
}

fail() {
    TESTS_FAILED=$((TESTS_FAILED + 1))
    echo -e "  ${RED}FAIL${NC}: $1"
    if [[ -n "${2:-}" ]]; then
        echo -e "        Expected: $2"
    fi
    if [[ -n "${3:-}" ]]; then
        echo -e "        Got: $3"
    fi
}

run_test() {
    TESTS_RUN=$((TESTS_RUN + 1))
}

section() {
    echo ""
    echo -e "${YELLOW}$1${NC}"
    echo "----------------------------------------"
}

#
# Test cases
#

section "1. Basic Flag Parsing"

# Test: Short flag
run_test
unset SHCLAP_DEBUG 2>/dev/null || true
source "$("$SHCLAP" parse --config '{"name":"test","args":[{"name":"debug","short":"d","type":"flag"}]}' -- -d)"
if [[ "${SHCLAP_DEBUG:-}" == "true" ]]; then
    pass "Short flag (-d) sets SHCLAP_DEBUG=true"
else
    fail "Short flag (-d)" "SHCLAP_DEBUG=true" "SHCLAP_DEBUG=${SHCLAP_DEBUG:-unset}"
fi

# Test: Long flag
run_test
unset SHCLAP_VERBOSE 2>/dev/null || true
source "$("$SHCLAP" parse --config '{"name":"test","args":[{"name":"verbose","long":"verbose","type":"flag"}]}' -- --verbose)"
if [[ "${SHCLAP_VERBOSE:-}" == "true" ]]; then
    pass "Long flag (--verbose) sets SHCLAP_VERBOSE=true"
else
    fail "Long flag (--verbose)" "SHCLAP_VERBOSE=true" "SHCLAP_VERBOSE=${SHCLAP_VERBOSE:-unset}"
fi

# Test: Flag defaults to false
run_test
unset SHCLAP_DEBUG 2>/dev/null || true
source "$("$SHCLAP" parse --config '{"name":"test","args":[{"name":"debug","short":"d","type":"flag"}]}' -- )"
if [[ "${SHCLAP_DEBUG:-}" == "false" ]]; then
    pass "Unset flag defaults to SHCLAP_DEBUG=false"
else
    fail "Unset flag default" "SHCLAP_DEBUG=false" "SHCLAP_DEBUG=${SHCLAP_DEBUG:-unset}"
fi

# Test: Combined short flags
run_test
unset SHCLAP_A SHCLAP_B SHCLAP_C 2>/dev/null || true
source "$("$SHCLAP" parse --config '{"name":"test","args":[
    {"name":"a","short":"a","type":"flag"},
    {"name":"b","short":"b","type":"flag"},
    {"name":"c","short":"c","type":"flag"}
]}' -- -abc)"
if [[ "${SHCLAP_A:-}" == "true" && "${SHCLAP_B:-}" == "true" && "${SHCLAP_C:-}" == "true" ]]; then
    pass "Combined short flags (-abc) sets all flags to true"
else
    fail "Combined short flags (-abc)" "A=true, B=true, C=true" "A=${SHCLAP_A:-unset}, B=${SHCLAP_B:-unset}, C=${SHCLAP_C:-unset}"
fi


section "2. Option Parsing"

# Test: Long option with space
run_test
unset SHCLAP_OUTPUT 2>/dev/null || true
source "$("$SHCLAP" parse --config '{"name":"test","args":[{"name":"output","long":"output","type":"option"}]}' -- --output file.txt)"
if [[ "${SHCLAP_OUTPUT:-}" == "file.txt" ]]; then
    pass "Long option with space (--output file.txt)"
else
    fail "Long option with space" "SHCLAP_OUTPUT=file.txt" "SHCLAP_OUTPUT=${SHCLAP_OUTPUT:-unset}"
fi

# Test: Long option with equals
run_test
unset SHCLAP_OUTPUT 2>/dev/null || true
source "$("$SHCLAP" parse --config '{"name":"test","args":[{"name":"output","long":"output","type":"option"}]}' -- --output=file.txt)"
if [[ "${SHCLAP_OUTPUT:-}" == "file.txt" ]]; then
    pass "Long option with equals (--output=file.txt)"
else
    fail "Long option with equals" "SHCLAP_OUTPUT=file.txt" "SHCLAP_OUTPUT=${SHCLAP_OUTPUT:-unset}"
fi

# Test: Short option with space
run_test
unset SHCLAP_OUTPUT 2>/dev/null || true
source "$("$SHCLAP" parse --config '{"name":"test","args":[{"name":"output","short":"o","type":"option"}]}' -- -o file.txt)"
if [[ "${SHCLAP_OUTPUT:-}" == "file.txt" ]]; then
    pass "Short option with space (-o file.txt)"
else
    fail "Short option with space" "SHCLAP_OUTPUT=file.txt" "SHCLAP_OUTPUT=${SHCLAP_OUTPUT:-unset}"
fi

# Test: Short option attached
run_test
unset SHCLAP_OUTPUT 2>/dev/null || true
source "$("$SHCLAP" parse --config '{"name":"test","args":[{"name":"output","short":"o","type":"option"}]}' -- -ofile.txt)"
if [[ "${SHCLAP_OUTPUT:-}" == "file.txt" ]]; then
    pass "Short option attached (-ofile.txt)"
else
    fail "Short option attached" "SHCLAP_OUTPUT=file.txt" "SHCLAP_OUTPUT=${SHCLAP_OUTPUT:-unset}"
fi

# Test: Option with default value
run_test
unset SHCLAP_OUTPUT 2>/dev/null || true
source "$("$SHCLAP" parse --config '{"name":"test","args":[{"name":"output","long":"output","type":"option","default":"default.txt"}]}' -- )"
if [[ "${SHCLAP_OUTPUT:-}" == "default.txt" ]]; then
    pass "Option default value (--output defaults to default.txt)"
else
    fail "Option default value" "SHCLAP_OUTPUT=default.txt" "SHCLAP_OUTPUT=${SHCLAP_OUTPUT:-unset}"
fi


section "3. Positional Arguments"

# Test: Single positional
run_test
unset SHCLAP_INPUT 2>/dev/null || true
source "$("$SHCLAP" parse --config '{"name":"test","args":[{"name":"input","type":"positional"}]}' -- myfile.txt)"
if [[ "${SHCLAP_INPUT:-}" == "myfile.txt" ]]; then
    pass "Single positional argument"
else
    fail "Single positional argument" "SHCLAP_INPUT=myfile.txt" "SHCLAP_INPUT=${SHCLAP_INPUT:-unset}"
fi

# Test: Multiple positionals
run_test
unset SHCLAP_INPUT SHCLAP_OUTPUT 2>/dev/null || true
source "$("$SHCLAP" parse --config '{"name":"test","args":[
    {"name":"input","type":"positional"},
    {"name":"output","type":"positional"}
]}' -- input.txt output.txt)"
if [[ "${SHCLAP_INPUT:-}" == "input.txt" && "${SHCLAP_OUTPUT:-}" == "output.txt" ]]; then
    pass "Multiple positional arguments"
else
    fail "Multiple positional arguments" "INPUT=input.txt, OUTPUT=output.txt" "INPUT=${SHCLAP_INPUT:-unset}, OUTPUT=${SHCLAP_OUTPUT:-unset}"
fi


section "4. Mixed Arguments"

# Test: Flags, options, and positionals together
run_test
unset SHCLAP_VERBOSE SHCLAP_OUTPUT SHCLAP_INPUT 2>/dev/null || true
source "$("$SHCLAP" parse --config '{"name":"test","args":[
    {"name":"verbose","short":"v","long":"verbose","type":"flag"},
    {"name":"output","short":"o","long":"output","type":"option"},
    {"name":"input","type":"positional"}
]}' -- -v --output result.txt myfile.txt)"
if [[ "${SHCLAP_VERBOSE:-}" == "true" && "${SHCLAP_OUTPUT:-}" == "result.txt" && "${SHCLAP_INPUT:-}" == "myfile.txt" ]]; then
    pass "Mixed arguments (flag + option + positional)"
else
    fail "Mixed arguments" "VERBOSE=true, OUTPUT=result.txt, INPUT=myfile.txt" "VERBOSE=${SHCLAP_VERBOSE:-unset}, OUTPUT=${SHCLAP_OUTPUT:-unset}, INPUT=${SHCLAP_INPUT:-unset}"
fi


section "5. Custom Prefix"

# Test: Custom prefix from config
run_test
unset MYAPP_DEBUG 2>/dev/null || true
source "$("$SHCLAP" parse --config '{"name":"test","prefix":"MYAPP_","args":[{"name":"debug","short":"d","type":"flag"}]}' -- -d)"
if [[ "${MYAPP_DEBUG:-}" == "true" ]]; then
    pass "Custom prefix from config (MYAPP_DEBUG)"
else
    fail "Custom prefix from config" "MYAPP_DEBUG=true" "MYAPP_DEBUG=${MYAPP_DEBUG:-unset}"
fi

# Test: CLI prefix overrides config
run_test
unset CLI_DEBUG 2>/dev/null || true
source "$("$SHCLAP" parse --config '{"name":"test","prefix":"CONFIG_","args":[{"name":"debug","short":"d","type":"flag"}]}' --prefix CLI_ -- -d)"
if [[ "${CLI_DEBUG:-}" == "true" ]]; then
    pass "CLI --prefix overrides config prefix (CLI_DEBUG)"
else
    fail "CLI --prefix overrides config prefix" "CLI_DEBUG=true" "CLI_DEBUG=${CLI_DEBUG:-unset}"
fi


section "6. Help Flag Detection"

# Test: --help flag (run in subshell since source will exit)
run_test
OUTPUT=$("$SHCLAP" parse --config '{"name":"myapp","description":"My awesome app","version":"1.0.0","args":[
    {"name":"verbose","short":"v","type":"flag","help":"Enable verbose output"}
]}' -- --help)
# Run source in subshell to capture output without exiting main script
HELP_OUTPUT=$(bash -c "source '$OUTPUT'" 2>&1) || true
if echo "$HELP_OUTPUT" | grep -q "myapp"; then
    pass "--help displays help text and exits 0"
else
    fail "--help flag" "Should display help text" "$HELP_OUTPUT"
fi

# Test: -h flag
run_test
OUTPUT=$("$SHCLAP" parse --config '{"name":"myapp","description":"Test app"}' -- -h)
HELP_OUTPUT=$(bash -c "source '$OUTPUT'" 2>&1) || true
if echo "$HELP_OUTPUT" | grep -q "myapp"; then
    pass "-h displays help text and exits 0"
else
    fail "-h flag" "Should display help text" "$HELP_OUTPUT"
fi

# Test: Help takes precedence over other args
run_test
OUTPUT=$("$SHCLAP" parse --config '{"name":"myapp","args":[{"name":"verbose","short":"v","type":"flag"}]}' -- -v --help)
HELP_OUTPUT=$(bash -c "source '$OUTPUT'" 2>&1) || true
if echo "$HELP_OUTPUT" | grep -q "USAGE"; then
    pass "--help takes precedence over other flags"
else
    fail "--help precedence" "Should display help even with other flags" "$HELP_OUTPUT"
fi


section "7. Version Flag Detection"

# Test: --version flag (run in subshell since source will exit)
run_test
OUTPUT=$("$SHCLAP" parse --config '{"name":"myapp","version":"2.5.0"}' -- --version)
VERSION_OUTPUT=$(bash -c "source '$OUTPUT'" 2>&1) || true
if echo "$VERSION_OUTPUT" | grep -q "2.5.0"; then
    pass "--version displays version and exits 0"
else
    fail "--version flag" "Should display version 2.5.0" "$VERSION_OUTPUT"
fi

# Test: -V flag
run_test
OUTPUT=$("$SHCLAP" parse --config '{"name":"myapp","version":"1.0.0"}' -- -V)
VERSION_OUTPUT=$(bash -c "source '$OUTPUT'" 2>&1) || true
if echo "$VERSION_OUTPUT" | grep -q "1.0.0"; then
    pass "-V displays version and exits 0"
else
    fail "-V flag" "Should display version 1.0.0" "$VERSION_OUTPUT"
fi


section "8. Error Handling"

# Test: Unknown option (run in subshell since source will exit 1)
run_test
OUTPUT=$("$SHCLAP" parse --config '{"name":"test"}' -- --unknown)
ERROR_OUTPUT=$(bash -c "source '$OUTPUT'" 2>&1) || true
if echo "$ERROR_OUTPUT" | grep -q "unknown option"; then
    pass "Unknown option produces error"
else
    fail "Unknown option error" "Should report unknown option" "$ERROR_OUTPUT"
fi

# Test: Invalid JSON config
run_test
OUTPUT=$("$SHCLAP" parse --config 'not valid json' -- )
ERROR_OUTPUT=$(bash -c "source '$OUTPUT'" 2>&1) || true
if echo "$ERROR_OUTPUT" | grep -q "failed to parse"; then
    pass "Invalid JSON config produces error"
else
    fail "Invalid JSON error" "Should report parse error" "$ERROR_OUTPUT"
fi

# Test: Missing required argument
run_test
OUTPUT=$("$SHCLAP" parse --config '{"name":"test","args":[{"name":"input","type":"positional","required":true}]}' -- )
ERROR_OUTPUT=$(bash -c "source '$OUTPUT'" 2>&1) || true
if echo "$ERROR_OUTPUT" | grep -q "missing required"; then
    pass "Missing required argument produces error"
else
    fail "Missing required error" "Should report missing required argument" "$ERROR_OUTPUT"
fi

# Test: Unsupported schema version
run_test
OUTPUT=$("$SHCLAP" parse --config '{"schema_version":99,"name":"test"}' -- )
ERROR_OUTPUT=$(bash -c "source '$OUTPUT'" 2>&1) || true
if echo "$ERROR_OUTPUT" | grep -q "unsupported schema version"; then
    pass "Unsupported schema version produces error"
else
    fail "Unsupported schema version error" "Should report unsupported schema version" "$ERROR_OUTPUT"
fi


section "9. Special Characters in Values"

# Test: Value with spaces
run_test
unset SHCLAP_MSG 2>/dev/null || true
source "$("$SHCLAP" parse --config '{"name":"test","args":[{"name":"msg","long":"msg","type":"option"}]}' -- --msg "hello world")"
if [[ "${SHCLAP_MSG:-}" == "hello world" ]]; then
    pass "Value with spaces preserved"
else
    fail "Value with spaces" "SHCLAP_MSG='hello world'" "SHCLAP_MSG=${SHCLAP_MSG:-unset}"
fi

# Test: Value with special characters (should be escaped)
run_test
unset SHCLAP_MSG 2>/dev/null || true
source "$("$SHCLAP" parse --config '{"name":"test","args":[{"name":"msg","long":"msg","type":"option"}]}' -- --msg 'say "hello"')"
if [[ "${SHCLAP_MSG:-}" == 'say "hello"' ]]; then
    pass "Value with quotes preserved"
else
    fail "Value with quotes" "SHCLAP_MSG='say \"hello\"'" "SHCLAP_MSG=${SHCLAP_MSG:-unset}"
fi


section "10. Double-Dash Separator"

# Test: Arguments after -- treated as positional
run_test
unset SHCLAP_VERBOSE SHCLAP_INPUT 2>/dev/null || true
source "$("$SHCLAP" parse --config '{"name":"test","args":[
    {"name":"verbose","short":"v","type":"flag"},
    {"name":"input","type":"positional"}
]}' -- -- -v)"
if [[ "${SHCLAP_VERBOSE:-}" == "false" && "${SHCLAP_INPUT:-}" == "-v" ]]; then
    pass "-- separator treats -v as positional"
else
    fail "-- separator" "VERBOSE=false, INPUT=-v" "VERBOSE=${SHCLAP_VERBOSE:-unset}, INPUT=${SHCLAP_INPUT:-unset}"
fi


#
# Summary
#

echo ""
echo "========================================"
echo "Test Results"
echo "========================================"
echo -e "  Total:  ${TESTS_RUN}"
echo -e "  ${GREEN}Passed${NC}: ${TESTS_PASSED}"
echo -e "  ${RED}Failed${NC}: ${TESTS_FAILED}"
echo ""

if [[ ${TESTS_FAILED} -gt 0 ]]; then
    echo -e "${RED}Some tests failed!${NC}"
    exit 1
else
    echo -e "${GREEN}All tests passed!${NC}"
    exit 0
fi
