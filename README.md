# Prequel

A Solana launchpad for event-conditional memecoins. Traders commit SOL to unborn coins tied to future real-world events. A coin is minted only if its event happens, everyone who committed to a born coin pays the same price, and coins tied to the outcome that didn't happen refund 100%.

**Status:** Phase 0 (setup and spikes) is done; Phase 1 waits on the spec questions in [docs/DECISIONS.md](docs/DECISIONS.md). Localnet and devnet only; there is no mainnet deployment and none is planned for v0.

| Read this | For |
| --- | --- |
| [SPEC.md](SPEC.md) | the build spec and source of truth |
| [PROGRESS.md](PROGRESS.md) | phase gates, what is done, what needs a decision |
| [docs/DECISIONS.md](docs/DECISIONS.md) | every decision, evidence, and open spec questions |
| [CLAUDE.md](CLAUDE.md) | working rules and verified commands for coding sessions |

## Quick start

```bash
scripts/bootstrap.sh                      # Agave 4.3.2, Anchor CLI 1.2.2, solana-verify (pinned, idempotent)
export PATH="$HOME/.local/share/solana/install/active_release/bin:$HOME/.cargo/bin:$PATH"
pnpm install
pnpm turbo run lint typecheck test build  # JS packages and the web app
scripts/rust-checks.sh                    # fmt, anchor build, clippy, LiteSVM tests
```

Spikes (Phase 0, throwaway): `pnpm spike:kalshi`, `pnpm spike:media`, `pnpm spike:amm`.
