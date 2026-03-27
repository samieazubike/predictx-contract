/**
 * PredictX SDK — Soroban event parser.
 *
 * Transforms raw `SorobanEvent` objects returned by the RPC API into
 * strongly-typed `PredictXEvent` objects.
 */

import type {
  PredictXEvent,
  PollCategory,
  StakeSide,
} from "../types/index";

/** Minimal interface that mirrors the Stellar SDK's SorobanEvent shape. */
interface RawSorobanEvent {
  type: string;
  /** Array of ScVal topics (encoded as objects with `value` properties). */
  topic: Array<{ type: string; value: unknown }>;
  /** Event data as a ScVal. */
  value: { type: string; value: unknown };
}

/**
 * Parses a raw Soroban contract event into a typed PredictXEvent.
 *
 * @param rawEvent - Raw event from the Stellar SDK / RPC streaming.
 * @returns Typed PredictXEvent, or `null` if the event is unrecognised.
 */
export function parseEvent(rawEvent: RawSorobanEvent): PredictXEvent | null {
  try {
    const topics = rawEvent.topic.map((t) => String(t.value));
    if (topics.length < 2) return null;

    const [category, action] = topics;
    const key = `${category}:${action}`;

    switch (key) {
      case "poll:created":
        return parsePollCreatedEvent(rawEvent);
      case "poll:cancelled":
        return parsePollCancelledEvent(rawEvent);
      case "stake:placed":
        return parseStakePlacedEvent(rawEvent);
      case "stake:emergency_withdrawal":
        return parseEmergencyWithdrawalEvent(rawEvent);
      case "match:created":
        return parseMatchCreatedEvent(rawEvent);
      case "contract:paused":
        return parseContractPausedEvent(rawEvent);
      case "contract:unpaused":
        return parseContractUnpausedEvent(rawEvent);
      default:
        return null;
    }
  } catch {
    return null;
  }
}

// ---------------------------------------------------------------------------
// Individual event parsers
// ---------------------------------------------------------------------------

function parsePollCreatedEvent(ev: RawSorobanEvent): PredictXEvent {
  const data = ev.value.value as Record<string, unknown>;
  return {
    type: "poll:created",
    pollId: Number(data["poll_id"]),
    matchId: Number(data["match_id"]),
    creator: String(data["creator"]),
    question: String(data["question"]),
    category: Number(data["category"]) as PollCategory,
    lockTime: new Date(Number(data["lock_time"]) * 1000),
  };
}

function parsePollCancelledEvent(ev: RawSorobanEvent): PredictXEvent {
  const data = ev.value.value as Record<string, unknown>;
  return {
    type: "poll:cancelled",
    pollId: Number(data["poll_id"]),
    admin: String(data["admin"]),
  };
}

function parseStakePlacedEvent(ev: RawSorobanEvent): PredictXEvent {
  const data = ev.value.value as Record<string, unknown>;
  return {
    type: "stake:placed",
    pollId: Number(data["poll_id"]),
    staker: String(data["staker"]),
    amount: BigInt(String(data["amount"])),
    side: Number(data["side"]) as StakeSide,
  };
}

function parseEmergencyWithdrawalEvent(ev: RawSorobanEvent): PredictXEvent {
  const data = ev.value.value as Record<string, unknown>;
  return {
    type: "stake:emergency_withdrawal",
    pollId: Number(data["poll_id"]),
    user: String(data["user"]),
    amount: BigInt(String(data["amount"])),
  };
}

function parseMatchCreatedEvent(ev: RawSorobanEvent): PredictXEvent {
  const data = ev.value.value as Record<string, unknown>;
  return {
    type: "match:created",
    matchId: Number(data["match_id"]),
    homeTeam: String(data["home_team"]),
    awayTeam: String(data["away_team"]),
  };
}

function parseContractPausedEvent(ev: RawSorobanEvent): PredictXEvent {
  const data = ev.value.value as Record<string, unknown>;
  return {
    type: "contract:paused",
    admin: String(data["admin"]),
  };
}

function parseContractUnpausedEvent(ev: RawSorobanEvent): PredictXEvent {
  const data = ev.value.value as Record<string, unknown>;
  return {
    type: "contract:unpaused",
    admin: String(data["admin"]),
  };
}
