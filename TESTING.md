# Testing

## Unit Tests

```bash
# All crates
nix develop -c cargo test --workspace

# Individual crates
nix develop -c bash -c "cd hydra-mail && cargo test"
nix develop -c bash -c "cd hydra-wt && cargo test"
nix develop -c bash -c "cd hydra-orchestrator && cargo test"
nix develop -c bash -c "cd hydra-cli && cargo test"
```

## Integration Tests

### hydra-mail pub/sub

```bash
cd /tmp && mkdir test-mail && cd test-mail

hydra-mail init --daemon
hydra-mail emit --channel repo:delta --type delta --data '{"action":"test"}'
hydra-mail status
```

### hydra-wt worktree lifecycle

```bash
cd /tmp && mkdir test-wt && cd test-wt && git init
echo "# test" > README.md && git add . && git commit -m "initial"

hydra-mail init
hydra-wt init
hydra-wt create feature-test
hydra-wt list
hydra-wt remove feature-test
```

### hydra-cli session management

```bash
cd /tmp && mkdir test-hydra && cd test-hydra && git init

hydra-mail init
hydra-mail start &
hydra init
hydra ls
```

## CI

```yaml
# .github/workflows/test.yml
name: Test
on: [push, pull_request]
jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: cachix/install-nix-action@v27
      - run: nix develop -c cargo test --workspace
      - run: nix develop -c cargo clippy --workspace
```
