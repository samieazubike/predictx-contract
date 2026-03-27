# PredictX Documentation

Comprehensive documentation for the PredictX smart contract system — a decentralized football prediction market built on Stellar.

## Quick Links

| Document | Description |
|----------|-------------|
| [Architecture](../architecture.md) | System overview, data flows, contract interactions |
| [Getting Started](./guides/getting-started.md) | Developer setup, building, testing |
| [Deployment](./guides/deployment.md) | Deploying to testnet/mainnet |
| [Contributing](./guides/contributing.md) | Code standards, PR process |

## Contract API Reference

| Contract | Description |
|----------|-------------|
| [PredictionMarket](./contracts/prediction-market.md) | Core contract — polls, staking, matches |
| [VotingOracle](./contracts/voting-oracle.md) | Poll status tracking |
| [Treasury](./contracts/treasury.md) | Fee management |
| [PollFactory](./contracts/poll-factory.md) | Poll factory |

## Reference

| Document | Description |
|----------|-------------|
| [Events](./events.md) | Complete event reference |
| [Security Model](./security/security-model.md) | Threat model, access control |

## Diagrams

| Diagram | Description |
|---------|-------------|
| [Architecture](../diagrams/architecture.mmd) | Contract system diagram |
| [Poll Lifecycle](../diagrams/poll-lifecycle.mmd) | Poll state machine |
| [Stake Flow](../diagrams/stake-flow.mmd) | Staking flow diagram |
| [Voting Flow](../diagrams/voting-flow.mmd) | Voting resolution diagram |

## Documentation Structure

```
docs/
├── README.md                    # This index
├── architecture.md             # System architecture
├── events.md                   # Event reference
├── contracts/
│   ├── prediction-market.md    # PredictionMarket API
│   ├── voting-oracle.md        # VotingOracle API
│   ├── treasury.md             # Treasury API
│   └── poll-factory.md         # PollFactory API
├── guides/
│   ├── getting-started.md       # Dev setup guide
│   ├── deployment.md           # Deployment procedures
│   └── contributing.md         # Contribution guidelines
├── security/
│   └── security-model.md        # Security model
└── diagrams/
    ├── architecture.mmd        # System diagram
    ├── poll-lifecycle.mmd      # State machine
    ├── stake-flow.mmd          # Staking flow
    └── voting-flow.mmd         # Voting flow
```

## Key Resources

- [Product Specification](https://github.com/samieazubike/predictx-contract/blob/main/predictx-spec.md) — Full product specification
- [Shared Package](https://github.com/samieazubike/predictx-contract/blob/main/packages/shared/src/lib.rs) — Shared types and errors
- [Soroban SDK](https://soroban.stellar.org/docs) — Stellar smart contract SDK

## Development Phases

| Phase | Status | Features |
|-------|--------|----------|
| Phase 1 | Current | Core contracts, staking, basic resolution |
| Phase 2 | Planned | Voting, disputes, community features |
| Phase 3 | Planned | Mainnet launch, security audit |
| Phase 4 | Planned | Multi-sport, governance token |
