/**
 * Re-export all event types from the main types module so callers can
 * import from either `@predictx/sdk` or `@predictx/sdk/events`.
 */
export type {
  PredictXEvent,
  PollCreatedEvent,
  PollCancelledEvent,
  StakePlacedEvent,
  EmergencyWithdrawalEvent,
  MatchCreatedEvent,
  ContractPausedEvent,
  ContractUnpausedEvent,
  EventCallback,
  Unsubscribe,
} from "../types/index";
