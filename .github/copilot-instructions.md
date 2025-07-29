# GitHub Copilot Instructions for `Alirexaa/chico` Repository

To ensure code quality and consistency, please follow these instructions when using GitHub Copilot or making any commits and pull requests:

## 🚦 Required CI Steps: Lint, Format, Build, & Test

Our Continuous Integration (CI) pipeline **requires** that your code passes the following checks before merging:

### 1. **Lint Check**

Before committing, **run the following lint command** to check for warnings and errors:

```sh
cargo clippy --all-targets --all-features -- -D warnings
```

- **All warnings are treated as errors.**
- Fix any issues reported by Clippy before committing.

### 2. **Format Check**

Ensure your code is properly formatted by running:

```sh
cargo fmt
```

- This will format all Rust files according to the project's style guide.
- Commit any formatting changes if necessary.

### 3. **Build**

Verify that the code builds successfully by running:

```sh
cargo build --all-targets
```

- Resolve any build errors before pushing your code.

### 4. **Test**

Run all tests to ensure correctness:

```sh
cargo test --all-features
```

- All tests must pass before opening a pull request.

### 5. **Doc Test**

Run all doc tests to ensure correctness:

```sh
cargo test --doc
```


- All doc tests must pass before opening a pull request.

## ✅ Final Checklist Before Committing

- [ ] Code passes **lint**: `cargo clippy --all-targets --all-features -- -D warnings`
- [ ] Code passes **format**: `cargo fmt`
- [ ] Code **builds**: `cargo build --all-targets`
- [ ] All **tests pass**: `cargo test --all-features`
- [ ] All **doc tests pass**: `cargo test --doc`
