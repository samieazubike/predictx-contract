# Contributing Guide

Guidelines for contributing to PredictX smart contracts.

## Code Standards

### Rust Style

Follow the [Rust Style Guide](https://rust-lang.github.io/api-guidelines/) and these project-specific rules:

**Formatting:**
- Use `cargo fmt` before committing
- 4-space indentation (no tabs)
- Maximum line length: 100 characters

```bash
cargo fmt
```

**Linting:**
```bash
cargo clippy --all --all-targets -- -D warnings
```

**Documentation:**
- All public functions must have `///` doc comments
- Module-level documentation for each `lib.rs`
- Use `#[brief]` for short descriptions when needed

```rust
/// Places a stake on a poll outcome.
///
/// Validates poll status, lock time, and user balance before
/// transferring tokens and recording the stake record.
///
/// # Arguments
/// * `staker` - The user placing the stake
/// * `poll_id` - The poll to stake on
/// * `amount` - Token amount to stake
/// * `side` - Yes or No
///
/// # Errors
/// * `PollNotActive` - Poll is not accepting stakes
/// * `PollLocked` - Lock time has passed
/// * `AlreadyStaked` - User already staked on this poll
pub fn stake(...) -> Result<Stake, PredictXError> {
    // ...
}
```

---

## Git Workflow

### Branches

Use descriptive branch names:

```
docs/description           # Documentation changes
feat/feature-name         # New features
fix/bug-description        # Bug fixes
refactor/code-improvement  # Code refactoring
```

### Commits

Follow [Conventional Commits](https://www.conventionalcommits.org/):

```
<type>(<scope>): <description>

[optional body]

[optional footer]
```

**Types:**
- `feat` — New feature
- `fix` — Bug fix
- `docs` — Documentation
- `refactor` — Code refactoring
- `test` — Adding tests
- `chore` — Maintenance

**Examples:**
```bash
git commit -m "docs(api): add staking function reference"
git commit -m "feat(voting): add consensus tracking"
git commit -m "fix(staking): prevent double-stake on same poll"
git commit -m "refactor(errors): consolidate error codes"
```

### Pull Requests

1. Fork the repository
2. Create a feature branch
3. Make changes
4. Add tests
5. Run `cargo fmt` and `cargo clippy`
6. Submit PR with clear description

**PR Template:**
```markdown
## Summary
Brief description of changes

## Type
- [ ] Bug fix
- [ ] New feature
- [ ] Documentation
- [ ] Refactoring

## Testing
How was this tested?

## Checklist
- [ ] Code follows Rust style guidelines
- [ ] Tests pass
- [ ] Documentation updated
- [ ] Breaking changes noted
```

---

## Testing Requirements

### Test Coverage

All new functionality requires tests:

| Change Type | Minimum Coverage |
|-------------|-----------------|
| Bug fix | Regression test |
| New function | Unit test per error path |
| Public API | Tests for success + all error cases |
| Security-sensitive | Fuzz tests |

### Running Tests

```bash
# All tests
cargo test --all

# With output
cargo test --all -- --nocapture

# Specific package
cargo test -p prediction-market

# With coverage
cargo tarpaulin --workspace --out html
```

### Test Naming

Use descriptive test names that explain what is tested:

```rust
#[test]
fn stake_rejects_after_lock_time() { }

#[test]
fn stake_yes_side_succeeds() { }

#[test]
fn emergency_withdraw_after_dispute_timeout() { }
```

---

## Documentation Requirements

### When Documentation is Required

- New public functions
- Changed function signatures
- New contract features
- Changed behaviors

### Documentation Location

| Type | Location |
|------|----------|
| Inline code docs | `///` comments in source |
| API reference | `docs/contracts/*.md` |
| Architecture | `docs/architecture.md` |
| Guides | `docs/guides/*.md` |

---

## Security Considerations

### Never Commit

- Private keys
- API keys
- Passwords
- Secrets in code or comments

### Use Environment Variables

```rust
// ❌ BAD
const API_KEY: &str = "secret-key-123";

// ✅ GOOD
const API_KEY: &str = env!("API_KEY");
```

### Access Control

All admin functions must use `require_auth`:

```rust
admin.require_auth();
```

### Token Handling

Follow Checks-Effects-Interactions pattern:

```rust
// 1. Checks
if amount <= 0 {
    return Err(PredictXError::StakeAmountZero);
}

// 2. Interactions (token transfers)
token_utils::transfer_to_contract(env, &staker, amount)?;

// 3. Effects (state updates)
env.storage().persistent().set(&key, &value);
```

---

## Code Review Process

### For Reviewers

1. Check code follows style guidelines
2. Verify tests cover all paths
3. Check for security issues
4. Verify documentation updated
5. Ensure no breaking changes without notice

### For Authors

1. Respond to all comments
2. Make requested changes
3. Don't take feedback personally
4. Ask for clarification if unclear

---

## File Organization

```
predictx-contract/
├── Cargo.toml              # Workspace config
├── packages/
│   └── shared/             # Shared types
│       └── src/
│           ├── lib.rs      # Module exports
│           ├── types.rs    # Data types
│           ├── errors.rs   # Error codes
│           └── constants.rs # Constants
├── contracts/
│   ├── prediction-market/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs     # Main contract
│   │       ├── staking.rs # Staking logic
│   │       └── matches.rs # Match management
│   ├── voting-oracle/
│   ├── treasury/
│   └── poll-factory/
├── tests/
│   └── integration/
└── docs/
```

---

## Issue Labels

| Label | Description |
|-------|-------------|
| `bug` | Bug report |
| `enhancement` | Feature request |
| `documentation` | Docs improvement |
| `security` | Security issue |
| `good first issue` | Beginner-friendly |
| `Stellar Wave` | Part of Stellar Wave program |

---

## Getting Help

- Open an issue for bugs
- Start a discussion for questions
- Check existing issues before creating new ones

---

## License

By contributing, you agree that your contributions will be licensed under the project's license.
