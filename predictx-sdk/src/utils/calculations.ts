/**
 * PredictX — Client-side calculation utilities.
 *
 * These functions replicate the on-chain maths so the UI can show
 * preview values before a transaction is submitted.  All amounts are
 * in the token's raw base units (7-decimal precision, like XLM stroops).
 */

import { PLATFORM_FEE_BPS, VOTER_REWARD_BPS, BPS_DENOMINATOR } from "./constants";

// ---------------------------------------------------------------------------
// Core winnings preview
// ---------------------------------------------------------------------------

/**
 * Calculates the potential winnings if `yourStake` wins.
 *
 * Formula (mirrors `calculate_potential_winnings` in the contract):
 *   totalPool        = yesPool + noPool + yourStake
 *   distributable    = totalPool × (1 − (feeBps + voterBps) / 10_000)
 *   winnings         = yourStake × distributable / (yourSidePool + yourStake)
 *
 * @param yourStake     - Amount being staked (base units, bigint).
 * @param yourSide      - "yes" or "no".
 * @param currentYesPool - Current YES pool before this stake.
 * @param currentNoPool  - Current NO pool before this stake.
 * @param feeBps        - Platform fee in basis points (default 500 = 5%).
 * @param voterRewardBps - Voter reward reservation in bps (default 100 = 1%).
 * @returns Object with `winnings`, `profit`, and `roi` (percentage as number).
 */
export function calculatePotentialWinnings(
  yourStake: bigint,
  yourSide: "yes" | "no",
  currentYesPool: bigint,
  currentNoPool: bigint,
  feeBps: number = PLATFORM_FEE_BPS,
  voterRewardBps: number = VOTER_REWARD_BPS,
): { winnings: bigint; profit: bigint; roi: number } {
  if (yourStake <= 0n) {
    return { winnings: 0n, profit: 0n, roi: 0 };
  }

  const yourSidePool =
    yourSide === "yes" ? currentYesPool : currentNoPool;

  const totalDeductions = BigInt(feeBps + voterRewardBps);
  const totalPool = currentYesPool + currentNoPool + yourStake;
  const distributable =
    totalPool - (totalPool * totalDeductions) / BigInt(BPS_DENOMINATOR);
  const poolAfterStake = yourSidePool + yourStake;

  const winnings = (yourStake * distributable) / poolAfterStake;
  const profit = winnings - yourStake;
  const roi =
    yourStake > 0n
      ? Number((profit * 10_000n) / yourStake) / 100
      : 0;

  return { winnings, profit, roi };
}

// ---------------------------------------------------------------------------
// Claim winnings (post-resolution)
// ---------------------------------------------------------------------------

/**
 * Calculates the claimable winnings for a winning stake after resolution.
 *
 * @param stakeAmount     - The user's original stake.
 * @param stakePool       - Total tokens staked on the winning side.
 * @param totalPool       - Total tokens across both sides.
 * @param feeBps          - Platform fee in basis points.
 * @param voterRewardBps  - Voter reward in basis points.
 * @returns Claimable amount in base units.
 */
export function calculateClaimableWinnings(
  stakeAmount: bigint,
  stakePool: bigint,
  totalPool: bigint,
  feeBps: number = PLATFORM_FEE_BPS,
  voterRewardBps: number = VOTER_REWARD_BPS,
): bigint {
  if (stakePool === 0n) return 0n;

  const totalDeductions = BigInt(feeBps + voterRewardBps);
  const distributable =
    totalPool - (totalPool * totalDeductions) / BigInt(BPS_DENOMINATOR);

  return (stakeAmount * distributable) / stakePool;
}

// ---------------------------------------------------------------------------
// Platform fee & voter reward breakdowns
// ---------------------------------------------------------------------------

/**
 * Returns the platform fee that would be taken from a given pool total.
 *
 * @param totalPool - Total pool amount.
 * @param feeBps    - Fee in basis points (default 500).
 */
export function calculatePlatformFee(
  totalPool: bigint,
  feeBps: number = PLATFORM_FEE_BPS,
): bigint {
  return (totalPool * BigInt(feeBps)) / BigInt(BPS_DENOMINATOR);
}

/**
 * Returns the voter reward pool carved out from a given pool total.
 *
 * @param totalPool      - Total pool amount.
 * @param voterRewardBps - Voter reward in basis points (default 100).
 */
export function calculateVoterRewardPool(
  totalPool: bigint,
  voterRewardBps: number = VOTER_REWARD_BPS,
): bigint {
  return (totalPool * BigInt(voterRewardBps)) / BigInt(BPS_DENOMINATOR);
}

// ---------------------------------------------------------------------------
// Display helpers
// ---------------------------------------------------------------------------

/**
 * Converts raw token base units to a human-readable decimal string.
 * PredictX uses 7 decimal places (like Stellar's XLM stroops format).
 *
 * @param amount   - Raw amount in base units (bigint).
 * @param decimals - Token decimal places (default 7).
 * @returns Human-readable string, e.g. "100.0000000".
 */
export function formatTokenAmount(amount: bigint, decimals = 7): string {
  const divisor = 10n ** BigInt(decimals);
  const whole = amount / divisor;
  const fraction = amount % divisor;
  const fracStr = fraction.toString().padStart(decimals, "0");
  return `${whole}.${fracStr}`;
}

/**
 * Parses a human-readable token string back to raw base units.
 *
 * @param value    - e.g. "100.5" or "100"
 * @param decimals - Token decimal places (default 7).
 * @returns Raw amount as bigint.
 */
export function parseTokenAmount(value: string, decimals = 7): bigint {
  const [whole, fraction = ""] = value.split(".");
  const paddedFraction = fraction.slice(0, decimals).padEnd(decimals, "0");
  return BigInt(whole) * 10n ** BigInt(decimals) + BigInt(paddedFraction);
}
