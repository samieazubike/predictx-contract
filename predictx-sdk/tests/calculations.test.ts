import {
  calculatePotentialWinnings,
  calculateClaimableWinnings,
  calculatePlatformFee,
  calculateVoterRewardPool,
  formatTokenAmount,
  parseTokenAmount,
} from "../src/utils/calculations";
import { PLATFORM_FEE_BPS, VOTER_REWARD_BPS } from "../src/utils/constants";

// ---------------------------------------------------------------------------
// calculatePotentialWinnings
// ---------------------------------------------------------------------------

describe("calculatePotentialWinnings", () => {
  const yesPool = 1_000_000_000n; // 100 tokens
  const noPool  = 1_000_000_000n; // 100 tokens

  it("returns zero values for zero stake", () => {
    const result = calculatePotentialWinnings(0n, "yes", yesPool, noPool);
    expect(result.winnings).toBe(0n);
    expect(result.profit).toBe(0n);
    expect(result.roi).toBe(0);
  });

  it("returns zero values for negative stake", () => {
    const result = calculatePotentialWinnings(-1n, "yes", yesPool, noPool);
    expect(result.winnings).toBe(0n);
  });

  it("winnings >= stake for YES stake on equal pools", () => {
    const stake = 100_000_000n; // 10 tokens
    const result = calculatePotentialWinnings(stake, "yes", yesPool, noPool);
    expect(result.winnings).toBeGreaterThan(stake);
    expect(result.profit).toBeGreaterThan(0n);
    expect(result.roi).toBeGreaterThan(0);
  });

  it("winnings >= stake for NO stake on equal pools", () => {
    const stake = 100_000_000n;
    const result = calculatePotentialWinnings(stake, "no", yesPool, noPool);
    expect(result.winnings).toBeGreaterThan(stake);
  });

  it("deducts platform fee and voter reward from distributable", () => {
    const stake = 1_000_000_000n; // 100 tokens
    const empty = 0n;
    const totalPool = yesPool + noPool + stake;
    const result = calculatePotentialWinnings(stake, "yes", yesPool, empty);

    // With empty noPool, the staker staking on YES gets almost all distributable
    const totalDeductions = BigInt(PLATFORM_FEE_BPS + VOTER_REWARD_BPS);
    const distributable = totalPool - (totalPool * totalDeductions) / 10_000n;
    // Winnings should be approximately distributable * stake / (yesPool + stake)
    expect(result.winnings).toBeLessThan(totalPool);
    expect(result.winnings).toBeGreaterThan(0n);
  });

  it("roi matches profit/stake ratio", () => {
    const stake = 500_000_000n;
    const result = calculatePotentialWinnings(stake, "yes", yesPool, noPool);
    const expectedRoi = Number((result.profit * 10_000n) / stake) / 100;
    expect(result.roi).toBeCloseTo(expectedRoi, 5);
  });

  it("respects custom feeBps", () => {
    const stake = 100_000_000n;
    const zeroFee = calculatePotentialWinnings(stake, "yes", yesPool, noPool, 0, 0);
    const withFee = calculatePotentialWinnings(stake, "yes", yesPool, noPool, 1000, 0);
    expect(zeroFee.winnings).toBeGreaterThan(withFee.winnings);
  });
});

// ---------------------------------------------------------------------------
// calculateClaimableWinnings
// ---------------------------------------------------------------------------

describe("calculateClaimableWinnings", () => {
  it("returns 0 when stakePool is 0", () => {
    expect(calculateClaimableWinnings(100n, 0n, 500n)).toBe(0n);
  });

  it("proportional share of distributable pool", () => {
    const stakeAmount = 500_000_000n;
    const stakePool   = 1_000_000_000n;
    const totalPool   = 2_000_000_000n;
    const feeBps  = 500;
    const voterBps = 100;
    const totalDeductions = BigInt(feeBps + voterBps);
    const distributable = totalPool - (totalPool * totalDeductions) / 10_000n;
    const expected = (stakeAmount * distributable) / stakePool;

    expect(calculateClaimableWinnings(stakeAmount, stakePool, totalPool, feeBps, voterBps)).toBe(expected);
  });

  it("full stake pool returns full distributable", () => {
    const stakeAmount = 1_000_000_000n;
    const stakePool   = 1_000_000_000n;
    const totalPool   = 2_000_000_000n;
    const result = calculateClaimableWinnings(stakeAmount, stakePool, totalPool, 0, 0);
    expect(result).toBe(totalPool);
  });
});

// ---------------------------------------------------------------------------
// calculatePlatformFee
// ---------------------------------------------------------------------------

describe("calculatePlatformFee", () => {
  it("computes 5% fee correctly", () => {
    expect(calculatePlatformFee(10_000_000n, 500)).toBe(500_000n);
  });

  it("returns 0 for zero pool", () => {
    expect(calculatePlatformFee(0n)).toBe(0n);
  });

  it("uses default PLATFORM_FEE_BPS", () => {
    const pool = 20_000_000n;
    const expected = (pool * BigInt(PLATFORM_FEE_BPS)) / 10_000n;
    expect(calculatePlatformFee(pool)).toBe(expected);
  });
});

// ---------------------------------------------------------------------------
// calculateVoterRewardPool
// ---------------------------------------------------------------------------

describe("calculateVoterRewardPool", () => {
  it("computes 1% voter reward", () => {
    expect(calculateVoterRewardPool(10_000_000n, 100)).toBe(100_000n);
  });

  it("uses default VOTER_REWARD_BPS", () => {
    const pool = 30_000_000n;
    const expected = (pool * BigInt(VOTER_REWARD_BPS)) / 10_000n;
    expect(calculateVoterRewardPool(pool)).toBe(expected);
  });
});

// ---------------------------------------------------------------------------
// formatTokenAmount / parseTokenAmount
// ---------------------------------------------------------------------------

describe("formatTokenAmount", () => {
  it("formats whole tokens", () => {
    expect(formatTokenAmount(10_000_000n)).toBe("1.0000000");
  });

  it("formats fractional tokens", () => {
    expect(formatTokenAmount(10_000_001n)).toBe("1.0000001");
  });

  it("formats zero", () => {
    expect(formatTokenAmount(0n)).toBe("0.0000000");
  });

  it("formats large amounts", () => {
    expect(formatTokenAmount(1_000_000_000_000n)).toBe("100000.0000000");
  });
});

describe("parseTokenAmount", () => {
  it("parses whole number strings", () => {
    expect(parseTokenAmount("1")).toBe(10_000_000n);
  });

  it("parses decimal strings", () => {
    expect(parseTokenAmount("1.5")).toBe(15_000_000n);
  });

  it("parses zero", () => {
    expect(parseTokenAmount("0")).toBe(0n);
  });

  it("round-trips with formatTokenAmount", () => {
    const amount = 123_456_789n;
    const formatted = formatTokenAmount(amount);
    const parsed = parseTokenAmount(formatted);
    expect(parsed).toBe(amount);
  });
});
