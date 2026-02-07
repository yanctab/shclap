# shclap Implementation Backlog

## Workflow Per Task
Each task follows this workflow:

### Task Status Legend
- `[ ]` - Available
- `[TAKEN]` - Being worked on by an agent
- `~~Task~~` - Completed

### Git Workflow
1. **Mark task as `[TAKEN]`** in BACKLOG.md before starting
2. Create a feature branch (e.g., `feature/config-module`)
3. Implement the changes
4. Before committing: write summary and **ask user to confirm**
5. Commit changes
6. Push branch to origin
7. **Ask user to review and approve**
8. Squash and rebase before merging (one commit per feature in main)
9. Mark completed tasks with ~~strikethrough~~ in BACKLOG.md and commit

### Code Quality
1. Implement the code
2. Write unit tests
3. Run `cargo fmt`
4. Run `cargo test`
5. Run `cargo clippy`

---

## Phase 0 - Project Setup

### Initialize Project
- ~~Run `cargo init`~~
- ~~Initialize git repository~~
- ~~Set up Cargo.toml with dependencies (locked versions)~~
- ~~Create `rust-toolchain.toml` (lock to latest stable)~~
- ~~Create README.md with project overview and usage examples~~
- ~~Commit `Cargo.lock` to repo~~
- ~~Create initial commit~~

### Create Makefile
- ~~`make help` - parse `## target - description` comments and display all targets~~
- ~~`make setup-build-env` - install Rust (locked version), musl target, cargo-deb, musl-tools~~
- ~~`make build` - cargo build~~
- ~~`make release` - cargo build --release --target x86_64-unknown-linux-musl~~
- ~~`make test` - cargo test~~
- ~~`make fmt` - cargo fmt~~
- ~~`make lint` - cargo clippy~~
- ~~`make check` - fmt + lint + test~~
- ~~`make install` - install binary to system~~
- ~~`make clean` - cargo clean + remove build artifacts~~
- ~~`make deb` - build debian package~~
- ~~`make install-deb` - install the .deb package~~
- ~~`make uninstall-deb` - remove the installed .deb package~~
- ~~Every target must have a `## target - description` comment~~

### Set Up GitHub Repository
- ~~Create repo on github.com/yanctab~~
- ~~Add remote origin~~
- ~~Push initial commit~~

---

## Phase 1 - Foundation (Parallel)

### Config Module (`src/config.rs`)
- ~~Define Rust structs matching JSON schema (ArgConfig, Config)~~
- ~~Implement serde deserialization~~
- ~~Add validation (no duplicate arg names, required fields)~~
- ~~Write unit tests for config parsing~~
- ~~Run `cargo fmt && cargo test && cargo clippy`~~

### Help Module (`src/help.rs`)
- ~~Generate usage line from config~~
- ~~Format argument descriptions~~
- ~~Handle --help flag detection~~
- ~~Write unit tests for help generation~~
- ~~Run `cargo fmt && cargo test && cargo clippy`~~

### Output Module (`src/output.rs`)
- ~~Take parsed key-value pairs + prefix~~
- ~~Generate temp file with export statements~~
- ~~Proper escaping of values for shell safety~~
- ~~Return temp file path~~
- ~~Write unit tests for output generation~~
- ~~Run `cargo fmt && cargo test && cargo clippy`~~

### CLI Interface (`src/main.rs`)
- ~~Set up clap for shclap's own args~~
  - ~~`--config=<json>` (required)~~
  - ~~`--prefix=<PREFIX>` (optional, default "SHCLAP_")~~
  - ~~`--help` flag (clap provides this)~~
  - ~~`--version` flag (clap provides this, reads from Cargo.toml)~~
  - ~~`--` separator for script args~~
- ~~Wire up module calls (stubbed initially)~~
- ~~Run `cargo fmt && cargo test && cargo clippy`~~

---

## Phase 2 - Parser

### Parser Module (`src/parser.rs`)
- ~~Parse script args according to config spec~~
- ~~Handle short flags (-v)~~
- ~~Handle long flags (--verbose)~~
- ~~Handle options with values (-o file, --output=file, --output file)~~
- ~~Handle positional arguments~~
- ~~Return parsed values as key-value pairs~~
- ~~Write unit tests for argument parsing~~
- ~~Run `cargo fmt && cargo test && cargo clippy`~~

---

## Phase 3 - Subcommand Refactor

### Refactor CLI to use subcommands
- ~~Replace flat CLI with explicit subcommands (parse, help, version)~~
- ~~`shclap parse --config=<JSON> [--prefix=<PREFIX>] -- <ARGS>...`~~
- ~~`shclap help --config=<JSON>`~~
- ~~`shclap version --config=<JSON>`~~
- ~~Update all CLI tests to use subcommand format~~
- ~~Run `cargo fmt && cargo test && cargo clippy`~~

---

## Phase 3.5 - Error Handling & Help/Version via Output File

### Config Schema Version
- ~~Add `schema_version` field to Config (defaults to 1)~~
- ~~Add validation for supported schema versions~~
- ~~Add `CURRENT_SCHEMA_VERSION` constant~~
- ~~Write unit tests for schema version handling~~

### Parser Help/Version Detection
- ~~Add `ParseOutcome` enum (Success, Help, Version)~~
- ~~Detect `-h`/`--help` flags before parsing (returns Help)~~
- ~~Detect `-V`/`--version` flags before parsing (returns Version)~~
- ~~Help takes precedence over version~~
- ~~Write unit tests for help/version detection~~

### Output File Generation for Errors/Help/Version
- ~~Add `generate_error_output()` - creates file with `echo ... >&2; exit 1`~~
- ~~Add `generate_help_output()` - creates file with heredoc + `exit 0`~~
- ~~Add `generate_version_output()` - creates file with heredoc + `exit 0`~~
- ~~Write unit tests for all new output functions~~

### Update CLI to Always Output File Path
- ~~Handle config parsing errors via output file~~
- ~~Handle validation errors via output file~~
- ~~Handle parse errors via output file~~
- ~~Handle Help outcome via output file~~
- ~~Handle Version outcome via output file~~
- ~~Fallback to stderr if temp file creation fails~~
- ~~Run `cargo fmt && cargo test && cargo clippy`~~

---

## Phase 3.6 - Integration Tests

### Shell Integration Test Script (`tests/integration.sh`)
- ~~Create test harness with pass/fail reporting~~
- ~~Test flag parsing (short, long, combined)~~
- ~~Test option parsing (space, equals, attached, defaults)~~
- ~~Test positional arguments (single, multiple)~~
- ~~Test mixed arguments (flags + options + positionals)~~
- ~~Test custom prefix (config and CLI override)~~
- ~~Test help flag detection (-h, --help, precedence)~~
- ~~Test version flag detection (-V, --version)~~
- ~~Test error handling (unknown options, invalid JSON, missing required, schema version)~~
- ~~Test special characters in values~~
- ~~Test double-dash separator~~

### Update Makefile Test Targets
- ~~`make test` - runs both unit and integration tests~~
- ~~`make unit-test` - runs cargo test only~~
- ~~`make integration-test` - runs shell integration tests~~

---

## Phase 4 - Packaging & Distribution

### Debian Package
- ~~Create `debian/` directory structure~~
- ~~Create `debian/control` with package metadata~~
- ~~Create `debian/rules` build script~~
- ~~Create `debian/changelog`~~
- ~~Create `debian/copyright`~~
- ~~Create man page (`man/shclap.1`)~~
- ~~Include man page in .deb package~~
- ~~Test package build with `cargo-deb`~~
- ~~Verify installation from .deb file~~
- ~~Verify man page accessible via `man shclap`~~

---

## Phase 5 - GitHub Actions

### CI Pipeline (`.github/workflows/ci.yml`)
- ~~Trigger on: pull_request, push to main~~
- ~~Use locked Rust version from rust-toolchain.toml~~
- ~~Job: cargo fmt --check~~
- ~~Job: cargo clippy -- -D warnings~~
- ~~Job: cargo test~~
- ~~Cache cargo registry and target directory~~

### Release Pipeline (`.github/workflows/release.yml`)
- ~~Trigger on: push tags `v*`~~
- ~~Install musl toolchain (x86_64-unknown-linux-musl)~~
- ~~Build static binary with musl~~
- ~~Build .deb package~~
- ~~Create GitHub Release (auto-generate notes from commits)~~
- ~~Upload artifacts: static binary + .deb package~~

---

## Final Verification
- ~~`make setup-build-env` installs all dependencies~~
- ~~`make check` passes (fmt + lint + test)~~
- ~~`make release` builds static musl binary~~
- ~~`make install` works~~
- ~~`make deb` produces valid .deb package~~
- ~~CI pipeline passes on PR~~
- ~~Release pipeline produces artifacts on tag push~~
- ~~Manual verification with real shell script~~

---

## Phase 6 - Extra Tasks

### More flags
- ~~We need --name to be able to set the application name as a flag to shclap that way we can avoid hard code it in the config.~~
- ~~Extend the cli-reference.md with this new flag.~~
- ~~Short and long should be optional if no short is specified then only long is accepted if long is not specified then the name should be used as the long.~~
- ~~It should be possible to specify what values are supported so like value1, value2, value3 if the flag is not set to any of these three values then it should fail listing what values are supported.~~ (Implemented as `choices` field in schema v2)
- ~~Do we need to be able to define if a value should be string, bool or int?~~ (Implemented as `value_type` field in schema v2)

### Documentation
- Clarify "Environment variable fallback for options" in the schematic v2 I don't understand it.
- Go over how errors are handled like if a flag name is wrong show by example if not already done.
- We also need to clearly cover in the documentation how env variables are managed maybe in its own md-file under docs.
- We also need to clearly cover how the --help flag is managed by the script and how shclap help can be used by script to print out help for the arguments.
- We need to clarify the different types and how they work flag, option and positional

### Print How The script was called
- We should add a shclap print that could be called by the script this should then print out all <scriptname> followed by all the flags used when calling the script. If it is a mix of flags and env convert the env varibles to matching flags.

### Default Value
- ~~Each flag should be able to define a default value so it should be possible to either supply a flag with a value or not supply the flag to the script but if there is a default value then the env in the script should be set using this value or the variable is defined as an env outside of the script.~~ (Already implemented)

### Cargo
- We should release this to cargo if it is possible to release a application using cargo also

### Make use of Make
- We are calling cargo instead of calling make targets in the github actions lets try and call make targets instead when suited.
- The instructions in CLAUDE.md should also be to call make targets instead of calling cargo directly if possible

### Environment variables
- So the idea with the environments variables is that an user should be able to define args in json source shclap and then call its script using the flags but if the user has instead defined a environmanet lets say SHCLAP_TEST and there is flag named --test then the environment variable should be used if not --test flag is specified. Make sure that this works.
---


