# Contributing

All contributions are welcome!

## What you can do

There are many ways to contribute to httpress:

- reporting bugs
- suggesting new features
- writing documentation
- submitting code changes

## Development setup

Before making any changes, make sure you have the following installed:

- [Rust](https://www.rust-lang.org/tools/install) (stable toolchain)
- [cargo](https://doc.rust-lang.org/cargo/) (included with Rust)

Clone the repo and build the project:

```bash
git clone https://github.com/GabrielTecuceanu/httpress.git
cd httpress
cargo build
```

Verify everything is working before making any changes:

```bash
cargo test --all-features
```

## Creating issues

- before creating a new issue make sure one doesn't already exist
- use one of the provided templates depending on the issue type:
  - [Bug report](ISSUE_TEMPLATE/bug_report.md)
  - [Feature request](ISSUE_TEMPLATE/feature_request.md)

## Making a patch

1. Fork the repository
2. Create a new branch [e.g `git checkout -b feat/your-feature-name`]
3. Write code
4. Use `cargo fmt --all` to format the code
5. Run `cargo clippy --all-targets --all-features -- -D warnings` and fix any warnings
6. Run `cargo test --all-features` and make sure all tests pass
7. Commit your changes (the commit messages should follow [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/)),
   also if your commit targets a specific issue you should reference that in the
   description
8. Push to your fork
9. Create a pull request using the [pull request template](PULL_REQUEST_TEMPLATE.md)