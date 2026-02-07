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
if [[ -x "./target/x86_64-unknown-linux-musl/release/shclap" ]]; then
    SHCLAP="./target/x86_64-unknown-linux-musl/release/shclap"
elif [[ -x "./target/release/shclap" ]]; then
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
if echo "$HELP_OUTPUT" | grep -qi "usage"; then
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


section "11. Schema Version 2 - Environment Variable Fallback"

# Test: Env var fallback when no CLI arg provided
run_test
unset SHCLAP_INPUT 2>/dev/null || true
export TEST_INPUT_VAR="from_environment"
source "$("$SHCLAP" parse --config '{"schema_version":2,"name":"test","args":[
    {"name":"input","long":"input","type":"option","env":"TEST_INPUT_VAR"}
]}' -- )"
if [[ "${SHCLAP_INPUT:-}" == "from_environment" ]]; then
    pass "Env var fallback (TEST_INPUT_VAR) sets SHCLAP_INPUT"
else
    fail "Env var fallback" "SHCLAP_INPUT=from_environment" "SHCLAP_INPUT=${SHCLAP_INPUT:-unset}"
fi
unset TEST_INPUT_VAR

# Test: CLI arg takes precedence over env var
run_test
unset SHCLAP_INPUT 2>/dev/null || true
export TEST_INPUT_VAR="from_environment"
source "$("$SHCLAP" parse --config '{"schema_version":2,"name":"test","args":[
    {"name":"input","long":"input","type":"option","env":"TEST_INPUT_VAR"}
]}' -- --input from_cli)"
if [[ "${SHCLAP_INPUT:-}" == "from_cli" ]]; then
    pass "CLI arg takes precedence over env var"
else
    fail "CLI precedence over env" "SHCLAP_INPUT=from_cli" "SHCLAP_INPUT=${SHCLAP_INPUT:-unset}"
fi
unset TEST_INPUT_VAR

# Test: Auto-env (no explicit env field, uses PREFIX + ARG_NAME)
run_test
unset SHCLAP_CONFIG 2>/dev/null || true
export SHCLAP_CONFIG="auto_env_value"
source "$("$SHCLAP" parse --config '{"schema_version":2,"name":"test","args":[
    {"name":"config","long":"config","type":"option"}
]}' -- )"
if [[ "${SHCLAP_CONFIG:-}" == "auto_env_value" ]]; then
    pass "Auto-env reads from PREFIX + ARG_NAME (SHCLAP_CONFIG)"
else
    fail "Auto-env" "SHCLAP_CONFIG=auto_env_value" "SHCLAP_CONFIG=${SHCLAP_CONFIG:-unset}"
fi
unset SHCLAP_CONFIG

# Test: Auto-env with custom prefix
run_test
unset MYAPP_DEBUG 2>/dev/null || true
export MYAPP_DEBUG="true"
source "$("$SHCLAP" parse --config '{"schema_version":2,"name":"test","prefix":"MYAPP_","args":[
    {"name":"debug","long":"debug","type":"option"}
]}' -- )"
if [[ "${MYAPP_DEBUG:-}" == "true" ]]; then
    pass "Auto-env with custom prefix reads from MYAPP_DEBUG"
else
    fail "Auto-env custom prefix" "MYAPP_DEBUG=true" "MYAPP_DEBUG=${MYAPP_DEBUG:-unset}"
fi
unset MYAPP_DEBUG

# Test: Auto-env CLI arg takes precedence
run_test
unset SHCLAP_MODE 2>/dev/null || true
export SHCLAP_MODE="from_env"
source "$("$SHCLAP" parse --config '{"schema_version":2,"name":"test","args":[
    {"name":"mode","long":"mode","type":"option"}
]}' -- --mode from_cli)"
if [[ "${SHCLAP_MODE:-}" == "from_cli" ]]; then
    pass "Auto-env: CLI arg takes precedence over env var"
else
    fail "Auto-env CLI precedence" "SHCLAP_MODE=from_cli" "SHCLAP_MODE=${SHCLAP_MODE:-unset}"
fi
unset SHCLAP_MODE

# Test: Opt-out with env: false (shclap should not read from env var)
run_test
unset SHCLAP_SECRET 2>/dev/null || true
export SHCLAP_SECRET="should_not_be_read"
OUTPUT_FILE=$("$SHCLAP" parse --config '{"schema_version":2,"name":"test","args":[
    {"name":"secret","long":"secret","type":"option","env":false}
]}' -- )
# The output file should NOT contain SHCLAP_SECRET since env is disabled
if ! grep -q "SHCLAP_SECRET" "$OUTPUT_FILE"; then
    pass "Opt-out (env: false) does not read from env"
else
    fail "Opt-out env:false" "No SHCLAP_SECRET in output" "$(cat $OUTPUT_FILE)"
fi
unset SHCLAP_SECRET

# Test: v1 schema does not enable auto-env
run_test
unset SHCLAP_LEGACY 2>/dev/null || true
export SHCLAP_LEGACY="should_not_be_read"
OUTPUT_FILE=$("$SHCLAP" parse --config '{"schema_version":1,"name":"test","args":[
    {"name":"legacy","long":"legacy","type":"option"}
]}' -- )
# The output file should NOT contain SHCLAP_LEGACY since v1 has no auto-env
if ! grep -q "SHCLAP_LEGACY" "$OUTPUT_FILE"; then
    pass "v1 schema does not enable auto-env"
else
    fail "v1 no auto-env" "No SHCLAP_LEGACY in output" "$(cat $OUTPUT_FILE)"
fi
unset SHCLAP_LEGACY


section "12. Schema Version 2 - Multiple Values"

# Test: Multiple option values output as bash array
run_test
unset SHCLAP_FILES 2>/dev/null || true
source "$("$SHCLAP" parse --config '{"schema_version":2,"name":"test","args":[
    {"name":"files","long":"file","type":"option","multiple":true}
]}' -- --file a.txt --file b.txt --file c.txt)"
if [[ "${#SHCLAP_FILES[@]}" -eq 3 && "${SHCLAP_FILES[0]}" == "a.txt" && "${SHCLAP_FILES[1]}" == "b.txt" && "${SHCLAP_FILES[2]}" == "c.txt" ]]; then
    pass "Multiple option values output as bash array"
else
    fail "Multiple option values" "SHCLAP_FILES=(a.txt b.txt c.txt)" "SHCLAP_FILES=(${SHCLAP_FILES[*]:-unset})"
fi

# Test: Multiple flag counts occurrences
run_test
unset SHCLAP_VERBOSE 2>/dev/null || true
source "$("$SHCLAP" parse --config '{"schema_version":2,"name":"test","args":[
    {"name":"verbose","short":"v","type":"flag","multiple":true}
]}' -- -vvv)"
if [[ "${SHCLAP_VERBOSE:-}" == "3" ]]; then
    pass "Multiple flag (-vvv) counts to 3"
else
    fail "Multiple flag count" "SHCLAP_VERBOSE=3" "SHCLAP_VERBOSE=${SHCLAP_VERBOSE:-unset}"
fi

# Test: Delimiter splits single value into array
run_test
unset SHCLAP_TAGS 2>/dev/null || true
source "$("$SHCLAP" parse --config '{"schema_version":2,"name":"test","args":[
    {"name":"tags","long":"tags","type":"option","multiple":true,"delimiter":","}
]}' -- --tags "one,two,three")"
if [[ "${#SHCLAP_TAGS[@]}" -eq 3 && "${SHCLAP_TAGS[0]}" == "one" && "${SHCLAP_TAGS[1]}" == "two" && "${SHCLAP_TAGS[2]}" == "three" ]]; then
    pass "Delimiter splits value into array"
else
    fail "Delimiter split" "SHCLAP_TAGS=(one two three)" "SHCLAP_TAGS=(${SHCLAP_TAGS[*]:-unset})"
fi

# Test: Multiple values with special characters
run_test
unset SHCLAP_FILES 2>/dev/null || true
source "$("$SHCLAP" parse --config '{"schema_version":2,"name":"test","args":[
    {"name":"files","long":"file","type":"option","multiple":true}
]}' -- --file 'file with spaces.txt' --file 'another file.txt')"
if [[ "${#SHCLAP_FILES[@]}" -eq 2 && "${SHCLAP_FILES[0]}" == "file with spaces.txt" && "${SHCLAP_FILES[1]}" == "another file.txt" ]]; then
    pass "Multiple values preserve spaces"
else
    fail "Multiple values with spaces" "SHCLAP_FILES=('file with spaces.txt' 'another file.txt')" "SHCLAP_FILES=(${SHCLAP_FILES[*]:-unset})"
fi


section "13. Schema Version 2 - Subcommands"

# Test: Basic subcommand parsing
run_test
unset SHCLAP_SUBCOMMAND 2>/dev/null || true
source "$("$SHCLAP" parse --config '{"schema_version":2,"name":"test","subcommands":[
    {"name":"init","help":"Initialize a project"},
    {"name":"run","help":"Run the project"}
]}' -- init)"
if [[ "${SHCLAP_SUBCOMMAND:-}" == "init" ]]; then
    pass "Subcommand 'init' sets SHCLAP_SUBCOMMAND=init"
else
    fail "Basic subcommand" "SHCLAP_SUBCOMMAND=init" "SHCLAP_SUBCOMMAND=${SHCLAP_SUBCOMMAND:-unset}"
fi

# Test: Subcommand with positional argument
run_test
unset SHCLAP_SUBCOMMAND SHCLAP_TEMPLATE 2>/dev/null || true
source "$("$SHCLAP" parse --config '{"schema_version":2,"name":"test","subcommands":[
    {"name":"init","args":[{"name":"template","type":"positional"}]}
]}' -- init mytemplate)"
if [[ "${SHCLAP_SUBCOMMAND:-}" == "init" && "${SHCLAP_TEMPLATE:-}" == "mytemplate" ]]; then
    pass "Subcommand with positional argument"
else
    fail "Subcommand with positional" "SUBCOMMAND=init, TEMPLATE=mytemplate" "SUBCOMMAND=${SHCLAP_SUBCOMMAND:-unset}, TEMPLATE=${SHCLAP_TEMPLATE:-unset}"
fi

# Test: Subcommand with option argument
run_test
unset SHCLAP_SUBCOMMAND SHCLAP_VERBOSE 2>/dev/null || true
source "$("$SHCLAP" parse --config '{"schema_version":2,"name":"test","subcommands":[
    {"name":"run","args":[{"name":"verbose","short":"v","type":"flag"}]}
]}' -- run -v)"
if [[ "${SHCLAP_SUBCOMMAND:-}" == "run" && "${SHCLAP_VERBOSE:-}" == "true" ]]; then
    pass "Subcommand with flag argument"
else
    fail "Subcommand with flag" "SUBCOMMAND=run, VERBOSE=true" "SUBCOMMAND=${SHCLAP_SUBCOMMAND:-unset}, VERBOSE=${SHCLAP_VERBOSE:-unset}"
fi

# Test: Main command args with subcommand
run_test
unset SHCLAP_SUBCOMMAND SHCLAP_DEBUG SHCLAP_NAME 2>/dev/null || true
source "$("$SHCLAP" parse --config '{"schema_version":2,"name":"test",
    "args":[{"name":"debug","short":"d","type":"flag"}],
    "subcommands":[{"name":"create","args":[{"name":"name","type":"positional"}]}]
}' -- -d create myproject)"
if [[ "${SHCLAP_DEBUG:-}" == "true" && "${SHCLAP_SUBCOMMAND:-}" == "create" && "${SHCLAP_NAME:-}" == "myproject" ]]; then
    pass "Main command args combined with subcommand"
else
    fail "Main args + subcommand" "DEBUG=true, SUBCOMMAND=create, NAME=myproject" "DEBUG=${SHCLAP_DEBUG:-unset}, SUBCOMMAND=${SHCLAP_SUBCOMMAND:-unset}, NAME=${SHCLAP_NAME:-unset}"
fi

# Test: Subcommand help
run_test
OUTPUT=$("$SHCLAP" parse --config '{"schema_version":2,"name":"myapp","subcommands":[
    {"name":"init","help":"Initialize a new project"}
]}' -- --help)
HELP_OUTPUT=$(bash -c "source '$OUTPUT'" 2>&1) || true
if echo "$HELP_OUTPUT" | grep -q "init" && echo "$HELP_OUTPUT" | grep -q "Initialize"; then
    pass "Subcommand appears in help output"
else
    fail "Subcommand in help" "Should show 'init' and 'Initialize'" "$HELP_OUTPUT"
fi

# Test: Missing required subcommand shows help
run_test
OUTPUT=$("$SHCLAP" parse --config '{"schema_version":2,"name":"test","subcommands":[
    {"name":"init"}
]}' -- )
HELP_OUTPUT=$(bash -c "source '$OUTPUT'" 2>&1) || true
if echo "$HELP_OUTPUT" | grep -qi "usage\|init"; then
    pass "Missing subcommand shows help/usage"
else
    fail "Missing subcommand" "Should show usage or available subcommands" "$HELP_OUTPUT"
fi


section "14. Schema Version 2 - Validation Errors"

# Test: V2 field 'env' rejected in schema v1
run_test
OUTPUT=$("$SHCLAP" parse --config '{"schema_version":1,"name":"test","args":[
    {"name":"input","long":"input","type":"option","env":"MY_VAR"}
]}' -- )
ERROR_OUTPUT=$(bash -c "source '$OUTPUT'" 2>&1) || true
if echo "$ERROR_OUTPUT" | grep -q "requires schema_version"; then
    pass "Field 'env' rejected in schema v1"
else
    fail "V2 field validation" "Should reject 'env' in v1" "$ERROR_OUTPUT"
fi

# Test: V2 field 'multiple' rejected in schema v1
run_test
OUTPUT=$("$SHCLAP" parse --config '{"schema_version":1,"name":"test","args":[
    {"name":"files","long":"file","type":"option","multiple":true}
]}' -- )
ERROR_OUTPUT=$(bash -c "source '$OUTPUT'" 2>&1) || true
if echo "$ERROR_OUTPUT" | grep -q "requires schema_version"; then
    pass "Field 'multiple' rejected in schema v1"
else
    fail "V2 field validation" "Should reject 'multiple' in v1" "$ERROR_OUTPUT"
fi

# Test: Subcommands rejected in schema v1
run_test
OUTPUT=$("$SHCLAP" parse --config '{"schema_version":1,"name":"test","subcommands":[
    {"name":"init"}
]}' -- )
ERROR_OUTPUT=$(bash -c "source '$OUTPUT'" 2>&1) || true
if echo "$ERROR_OUTPUT" | grep -q "require.*schema_version"; then
    pass "Subcommands rejected in schema v1"
else
    fail "Subcommands validation" "Should reject subcommands in v1" "$ERROR_OUTPUT"
fi


section "15. Schema Version 2 - num_args Range"

# Test: num_args accepts multiple values in single invocation
run_test
unset SHCLAP_FILES 2>/dev/null || true
source "$("$SHCLAP" parse --config '{"schema_version":2,"name":"test","args":[
    {"name":"files","long":"file","type":"option","multiple":true,"num_args":"1..3"}
]}' -- --file a.txt b.txt)"
if [[ "${#SHCLAP_FILES[@]}" -eq 2 && "${SHCLAP_FILES[0]}" == "a.txt" && "${SHCLAP_FILES[1]}" == "b.txt" ]]; then
    pass "num_args allows multiple values per invocation"
else
    fail "num_args range" "SHCLAP_FILES=(a.txt b.txt)" "SHCLAP_FILES=(${SHCLAP_FILES[*]:-unset})"
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
