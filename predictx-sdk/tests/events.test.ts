import { parseEvent } from "../src/events/parser";
import { PollCategory, StakeSide } from "../src/types/index";

function makeEvent(category: string, action: string, data: Record<string, unknown>) {
  return {
    type: "contract",
    topic: [
      { type: "symbol", value: category },
      { type: "symbol", value: action },
    ],
    value: { type: "map", value: data },
  };
}

describe("parseEvent", () => {
  it("parses poll:created events", () => {
    const raw = makeEvent("poll", "created", {
      poll_id: 1,
      match_id: 2,
      creator: "GABCDE",
      question: "Will Chelsea win?",
      category: PollCategory.TeamEvent,
      lock_time: 1_700_000_000,
    });
    const event = parseEvent(raw as any);
    expect(event?.type).toBe("poll:created");
    if (event?.type === "poll:created") {
      expect(event.pollId).toBe(1);
      expect(event.question).toBe("Will Chelsea win?");
      expect(event.lockTime).toBeInstanceOf(Date);
    }
  });

  it("parses stake:placed events", () => {
    const raw = makeEvent("stake", "placed", {
      poll_id: 5,
      staker: "GXXX",
      amount: "100000000",
      side: StakeSide.Yes,
    });
    const event = parseEvent(raw as any);
    expect(event?.type).toBe("stake:placed");
    if (event?.type === "stake:placed") {
      expect(event.pollId).toBe(5);
      expect(event.amount).toBe(100_000_000n);
      expect(event.side).toBe(StakeSide.Yes);
    }
  });

  it("parses poll:cancelled events", () => {
    const raw = makeEvent("poll", "cancelled", {
      poll_id: 3,
      admin: "GADMIN",
    });
    const event = parseEvent(raw as any);
    expect(event?.type).toBe("poll:cancelled");
    if (event?.type === "poll:cancelled") {
      expect(event.pollId).toBe(3);
      expect(event.admin).toBe("GADMIN");
    }
  });

  it("parses match:created events", () => {
    const raw = makeEvent("match", "created", {
      match_id: 10,
      home_team: "Arsenal",
      away_team: "Chelsea",
    });
    const event = parseEvent(raw as any);
    expect(event?.type).toBe("match:created");
    if (event?.type === "match:created") {
      expect(event.matchId).toBe(10);
      expect(event.homeTeam).toBe("Arsenal");
    }
  });

  it("parses stake:emergency_withdrawal events", () => {
    const raw = makeEvent("stake", "emergency_withdrawal", {
      poll_id: 7,
      user: "GUSER",
      amount: "50000000",
    });
    const event = parseEvent(raw as any);
    expect(event?.type).toBe("stake:emergency_withdrawal");
    if (event?.type === "stake:emergency_withdrawal") {
      expect(event.amount).toBe(50_000_000n);
    }
  });

  it("parses contract:paused events", () => {
    const raw = makeEvent("contract", "paused", { admin: "GADMIN2" });
    const event = parseEvent(raw as any);
    expect(event?.type).toBe("contract:paused");
  });

  it("parses contract:unpaused events", () => {
    const raw = makeEvent("contract", "unpaused", { admin: "GADMIN2" });
    const event = parseEvent(raw as any);
    expect(event?.type).toBe("contract:unpaused");
  });

  it("returns null for unknown event types", () => {
    const raw = makeEvent("unknown", "action", {});
    expect(parseEvent(raw as any)).toBeNull();
  });

  it("returns null for events with too few topics", () => {
    const raw = {
      type: "contract",
      topic: [{ type: "symbol", value: "poll" }],
      value: { type: "map", value: {} },
    };
    expect(parseEvent(raw as any)).toBeNull();
  });
});
