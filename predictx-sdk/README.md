# @predictx/sdk

TypeScript SDK for the **PredictX** decentralised football prediction market on Stellar / Soroban.

## Installation

```bash
npm install @predictx/sdk
# or
yarn add @predictx/sdk
```

## Quick Start

```ts
import {
  PredictXClient,
  StakeSide,
  VoteChoice,
  formatTokenAmount,
} from "@predictx/sdk";

const client = new PredictXClient({
  network: "testnet",
  contractIds: {
    predictionMarket: "CXXX...",
    votingOracle:     "CYYY...",
    treasury:         "CZZZ...",
    pollFactory:      "CWWW...",
  },
});

// Connect a wallet (e.g. Freighter)
await client.connect({
  publicKey: "GABC...",
  signTransaction: (xdr, network) => freighter.signTransaction(xdr, { network }),
});

// Browse polls for a match
const polls = await client.getPollsByMatch(1);

// Stake 10 tokens on YES
const result = await client.stake(polls[0].pollId, 100_000_000n, StakeSide.Yes);
console.log("Staked! tx:", result.hash);

// Preview potential winnings before staking
const winnings = await client.calculatePotentialWinnings(
  polls[0].pollId,
  StakeSide.Yes,
  100_000_000n,
);
console.log("If you win:", formatTokenAmount(winnings));
```

## API Reference

### `PredictXClient`

The main entry point. Aggregates all contract clients.

#### Connection

| Method | Description |
|--------|-------------|
| `connect(wallet)` | Connect a Stellar wallet |
| `connectedAddress` | Public key of connected wallet |

#### Matches

| Method | Description |
|--------|-------------|
| `getMatch(matchId)` | Fetch a single match |
| `getUpcomingMatches()` | All non-finished matches |
| `createMatch(params)` | Create a match (admin) |

#### Polls

| Method | Description |
|--------|-------------|
| `getPoll(pollId)` | Fetch a single poll |
| `getPollsByMatch(matchId)` | All polls for a match |
| `getTrendingPolls(limit?)` | Top polls by pool size |
| `createPoll(params)` | Create a new poll |

#### Staking

| Method | Description |
|--------|-------------|
| `stake(pollId, amount, side)` | Place a stake |
| `getStake(pollId, user?)` | Get user's stake on a poll |
| `getUserStakes(user?)` | All poll IDs user has staked on |
| `calculatePotentialWinnings(pollId, side, amount)` | Preview winnings |

#### Claims

| Method | Description |
|--------|-------------|
| `claimWinnings(pollId)` | Claim winnings (resolved polls) |
| `calculateWinnings(pollId, user?)` | View claimable amount |

#### Voting

| Method | Description |
|--------|-------------|
| `castVote(pollId, choice)` | Vote on a locked poll |
| `getVoteTally(pollId)` | Current vote tally |
| `getVotingOpportunities(user?)` | Polls open for voting (non-stakers) |
| `claimVotingReward(pollId)` | Claim voter incentive |

#### Stats

| Method | Description |
|--------|-------------|
| `getPlatformStats()` | Aggregate platform statistics |
| `getUserStats(user?)` | Per-user statistics |
| `getUserHistory(user?)` | Full prediction history |

#### Events

| Method | Description |
|--------|-------------|
| `subscribeToEvents(callback)` | Subscribe to real-time events; returns unsubscribe fn |

---

### Utility Functions

```ts
import {
  calculatePotentialWinnings,
  formatTokenAmount,
  parseTokenAmount,
  truncateAddress,
  formatCountdown,
} from "@predictx/sdk";

// Client-side winnings preview (no RPC needed)
const { winnings, profit, roi } = calculatePotentialWinnings(
  100_000_000n,  // your stake
  "yes",
  1_000_000_000n, // current YES pool
  1_000_000_000n, // current NO pool
);

// Format raw base units → "10.0000000"
formatTokenAmount(100_000_000n);

// Parse "10.5" → 105_000_000n
parseTokenAmount("10.5");

// "GABCDE…WXYZ"
truncateAddress("GABCDEFGHIJKLMNOPQRSTUVWXYZ...");

// "2h 15m"
formatCountdown(new Date(Date.now() + 2 * 3600 * 1000));
```

---

### Error Handling

All SDK errors are instances of `PredictXError` with a typed `code` property:

```ts
import { PredictXError, PredictXErrorCode } from "@predictx/sdk";

try {
  await client.stake(pollId, amount, StakeSide.Yes);
} catch (err) {
  if (err instanceof PredictXError) {
    switch (err.code) {
      case PredictXErrorCode.AlreadyStaked:
        // User already staked on this poll
        break;
      case PredictXErrorCode.StakeBelowMinimum:
        // Stake is below the 10-token minimum
        break;
      case PredictXErrorCode.ContractPaused:
        // Protocol is paused
        break;
    }
  }
}
```

---

### Real-time Events

```ts
const unsubscribe = client.subscribeToEvents((event) => {
  if (event.type === "stake:placed") {
    console.log(`New stake on poll ${event.pollId}: ${event.amount}`);
  }
  if (event.type === "poll:created") {
    console.log(`New poll: "${event.question}"`);
  }
});

// Stop listening
unsubscribe();
```

---

## Token Amounts

All token amounts use `bigint` with **7 decimal places** (like Stellar's XLM):

- 1 token = `10_000_000n`
- Minimum stake = `10_000_000n` (10 tokens)

Use `formatTokenAmount` / `parseTokenAmount` to convert between raw and display values.

## Browser & Node.js

The SDK ships as both CJS and ESM bundles and has no DOM dependencies, so it works in:
- Node.js ≥ 18
- Modern browsers (Chrome, Firefox, Safari, Edge)
- React, Vue, Next.js, etc.

## License

MIT
