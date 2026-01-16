# shclap

A Rust project.

## Initial Steps for Starting a New Rust Project with Claude Code

### 1. Initialize the Project

```bash
cargo init
```

Or if creating a new directory:

```bash
cargo new project-name
```

### 2. Initialize Git Repository

```bash
git init
git add .
git commit -m "Initial commit"
```

### 3. Configure Cargo.toml

Edit `Cargo.toml` to set:
- Package name, version, and edition
- Authors and description
- License
- Dependencies

### 4. Set Up .gitignore

Cargo creates a basic `.gitignore`, but verify it includes:
```
/target
Cargo.lock  # Only for libraries, keep for binaries
```

### 5. Create Project Structure

For a typical project:
```
src/
  main.rs      # Binary entry point
  lib.rs       # Library root (if applicable)
  modules/     # Additional modules
tests/         # Integration tests
examples/      # Example code
benches/       # Benchmarks
```

### 6. Add Essential Dependencies

Common dependencies to consider:
```toml
[dependencies]
# Error handling
anyhow = "1.0"
thiserror = "1.0"

# CLI parsing (relevant for shclap!)
clap = { version = "4", features = ["derive"] }

# Serialization
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

[dev-dependencies]
# Testing
assert_cmd = "2.0"
predicates = "3.0"
```

### 7. Verify the Setup

```bash
cargo check    # Fast compilation check
cargo build    # Full build
cargo test     # Run tests
cargo clippy   # Lint code
cargo fmt      # Format code
```

### 8. Set Up CI (Optional)

Create `.github/workflows/ci.yml` for automated testing.

### 9. Document Your Project

- Update this README with project-specific details
- Add inline documentation with `///` and `//!`
- Generate docs with `cargo doc --open`

---

## Quick Reference Commands

| Command | Purpose |
|---------|---------|
| `cargo run` | Build and run |
| `cargo run -- args` | Run with arguments |
| `cargo test` | Run all tests |
| `cargo fmt` | Format code |
| `cargo clippy` | Run linter |
| `cargo doc --open` | Generate and view docs |
| `cargo add <crate>` | Add a dependency |
