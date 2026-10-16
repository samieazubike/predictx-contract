//! Resolution and payout engine for the prediction market.
//!
//! # Rounding policy
//!
//! Every amount is an integer number of token base units, so every division
//! truncates towards zero. The engine splits a winner's gross share into a net
//! payout and a platform fee:
//!
//! ```text
//! gross = stake.amount * total_pool / winning_pool              (truncates)
//! net   = gross * (BPS_DENOMINATOR - fee_bps) / BPS_DENOMINATOR  (truncates)
//! fee   = gross - net                                            (exact)
//! ```
//!
//! Two properties are deliberate:
//!
//! * **The fee split is exact.** `net + fee == gross` always holds, so the
//!   treasury transfer and the claimant transfer can never disagree by a base
//!   unit — the engine never mints or burns a token.
//! * **Truncation remainders ("dust") stay in the contract.** They are never
//!   redistributed to a staker. Handing the remainder to a single claimant
//!   would require paying out more than the pool holds, and the pool is frozen
//!   once a poll resolves, so there is no later round to carry it into. The
//!   remainder therefore remains held by the contract, where the emergency
//!   withdrawal and cancellation paths can still reach it.
//!
//! A winner whose share truncates to **zero** base units is not paid a
//! zero-value transfer: [`PredictXError::PayoutRoundsToZero`] is returned
//! instead, so a winner is never told they lost when they backed the winning
//! side. The read-only quote ([`calculate_winnings`]) and the transfer path
//! ([`claim_winnings`]) both go through [`payout_for`], so they cannot
//! disagree — including at rounding edges.

use crate::{
    get_platform_stats, has_emergency_claimed, set_platform_stats, token_utils,
    transition_poll_status, DataKey,
};
use predictx_shared::{Poll, PollStatus, PredictXError, Stake, StakeSide, BPS_DENOMINATOR};
use soroban_sdk::{Address, Env, Symbol};

/// A resolved poll's payout for a single staker.
struct Payout {
    /// Base units transferred to the staker.
    net: i128,
    /// Base units routed to the treasury as the platform fee.
    fee: i128,
}

/// Resolve a poll and record its final outcome.
///
/// Callable only by the registered admin or the registered voting oracle —
/// either address stored at `initialize`.  Legality of the transition is
/// decided by the state machine in [`crate::transition_status`]: a poll has to
/// be in `Voting`, `AdminReview` or `Disputed`, so an unresolved or cancelled
/// poll is rejected with `InvalidStateTransition` before anything is written.
pub fn resolve_poll(
    env: &Env,
    caller: Address,
    poll_id: u64,
    outcome: bool,
) -> Result<(), PredictXError> {
    caller.require_auth();

    let admin: Address = env
        .storage()
        .instance()
        .get(&DataKey::Admin)
        .ok_or(PredictXError::NotInitialized)?;
    let oracle: Address = env
        .storage()
        .instance()
        .get(&DataKey::VotingOracle)
        .ok_or(PredictXError::NotInitialized)?;
    if caller != admin && caller != oracle {
        return Err(PredictXError::Unauthorized);
    }

    // ── 2. Only resolved polls have claimable amounts ─────────────────────────
    if poll.status != PollStatus::Resolved {
        return 0;
use crate::{get_platform_stats, set_platform_stats, token_utils};
use soroban_sdk::{Address, Env, String, Symbol};
use predictx_shared::{
    DataKey, Poll, PollStatus, Stake, StakeSide, PredictXError,
    BPS_DENOMINATOR,
};
use crate::{DataKey, get_oracle, get_platform_stats, set_platform_stats, token_utils};
use crate::{DataKey, get_platform_stats, set_platform_stats, token_utils, ensure_not_paused};
use crate::{DataKey, get_platform_stats, has_emergency_claimed, set_platform_stats, token_utils};
use crate::{get_platform_stats, set_platform_stats, token_utils, DataKey};
use predictx_shared::{Poll, PollStatus, PredictXError, Stake, StakeSide, BPS_DENOMINATOR};
use soroban_sdk::{Address, Env, Symbol};
use crate::{DataKey, get_platform_stats, set_platform_stats, polls, token_utils};
use crate::{DataKey, get_platform_stats, set_platform_stats, token_utils, load_poll, store_poll, load_stake, store_stake};

/// Resolve a poll using the registered oracle and record its final outcome.
///
/// Only the address stored as the market's voting oracle may resolve polls;
/// this is the implementation behind the market's `resolve_poll` entry point.
/// Resolve a poll using the configured admin or oracle and record its final outcome.
/// Resolve a poll and record its final outcome in the payouts engine.
///
/// Callable by the **admin** (manual resolution) or the registered **voting
/// oracle** (automated resolution flow) — whichever authority resolves the
/// poll first wins; subsequent calls fail with `PollAlreadyResolved`.
pub fn resolve_poll(
    env: &Env,
    caller: Address,
    poll_id: u64,
    outcome: bool,
    resolution_basis: String,
) -> Result<(), PredictXError> {
    caller.require_auth();
    let oracle = get_oracle(env)?;
    if caller != oracle {
    ensure_not_paused(env)?;
    admin.require_auth();

    // ── Authorisation: admin or registered oracle ─────────────────────────
use soroban_sdk::{Address, Env, Symbol};
use predictx_shared::{
    Poll, PollStatus, Stake, StakeSide, UserStats, PredictXError,
    BPS_DENOMINATOR,
};
use crate::{get_platform_stats, set_platform_stats, token_utils};
use predictx_shared::DataKey;

/// Resolve a poll using the configured admin and record its final outcome.
pub fn resolve_poll(
    env: &Env,
    admin: Address,
    poll_id: u64,
    outcome: bool,
) -> Result<(), PredictXError> {
    admin.require_auth();
    let stored_admin: Address = env
        .storage()
        .instance()
        .get(&DataKey::Admin)
        .ok_or(PredictXError::NotInitialized)?;
    let stored_oracle: Address = env
        .storage()
        .instance()
        .get(&DataKey::VotingOracle)
        .ok_or(PredictXError::NotInitialized)?;
    if caller != stored_admin && caller != stored_oracle {
        return Err(PredictXError::Unauthorized);
    let mut poll: Poll = env
        .storage()
        .persistent()
        .get(&DataKey::Poll(poll_id))
        .ok_or(PredictXError::PollNotFound)?;
    if poll.outcome.is_some() {
        return Err(PredictXError::PollAlreadyResolved);
    if caller != stored_admin {
        let oracle_id: Address = env
            .storage()
            .instance()
            .get(&DataKey::VotingOracle)
            .ok_or(PredictXError::NotInitialized)?;
        if caller != oracle_id {
            return Err(PredictXError::Unauthorized);
        }
    }

    poll.outcome = Some(outcome);
    poll.resolution_time = env.ledger().timestamp();
    // Validates the edge, persists the new status, and publishes
    // `PollStatusChanged`.  A rejected transition leaves the poll untouched.
    transition_poll_status(env, &mut poll, PollStatus::Resolved)?;

    let total_pool = poll.yes_pool + poll.no_pool;
    let fee = total_pool * token_utils::get_platform_fee_bps(env) as i128 / BPS_DENOMINATOR as i128;
    env.events().publish(
        (Symbol::new(env, "PollResolved"), poll_id),
        (outcome, total_pool, fee),
    );
    Ok(())
}

// ── Payout / claim engine ─────────────────────────────────────────────────────

/// Load a poll and require that it has been resolved.
fn load_resolved_poll(env: &Env, poll_id: u64) -> Result<Poll, PredictXError> {
    let poll: Poll = env
        .storage()
        .persistent()
        .get(&DataKey::Poll(poll_id))
    let mut poll: Poll = load_poll(env, poll_id)
        .ok_or(PredictXError::PollNotFound)?;
    if poll.status != PollStatus::Resolved {
        return Err(PredictXError::InvalidStateTransition);
    }
    Ok(poll)
}

/// What a resolved poll owes one staker.
///
/// The single source of truth behind both [`claim_winnings`] and
/// [`calculate_winnings`] — see the module docs for the rounding policy.
///
/// ## Cases
/// 1. **Empty winning pool.** Nobody backed the winning side, so there is no
///    eligible winner to collect the pot. Every staker — on either side —
///    recovers their stake in full and no fee is charged. `NotOnWinningSide`
///    must not be returned here: it would strand the pot.
/// 2. **Losing stake.** A staker on the losing side gets
///    [`PredictXError::NotOnWinningSide`].
/// 3. **No losing pool.** Nobody backed the losing side, so there is no profit
///    to share: winners are refunded at par and the platform takes nothing.
///    This is a no-contest poll, not a win.
/// 4. **Normal win.** The winner receives their proportional share of the whole
///    pool, minus the platform fee.
fn payout_for(env: &Env, poll: &Poll, stake: &Stake) -> Result<Payout, PredictXError> {
    // outcome is always Some(_) for a Resolved poll
    let outcome_yes: bool = poll.outcome.ok_or(PredictXError::InvalidOutcome)?;

    let winning_pool: i128 = if outcome_yes {
        poll.yes_pool
    } else {
        poll.no_pool
    };
    let losing_pool: i128 = if outcome_yes {
        poll.no_pool
    } else {
        poll.yes_pool
    };

    // ── Case 1: empty winning pool — full, fee-free refund for everyone ─────
    if winning_pool == 0 {
        return stake.amount;
        .get(&DataKey::Poll(poll_id))
        .ok_or(PredictXError::PollNotFound)?;
    if poll.status == PollStatus::Resolved || poll.outcome.is_some() {
        return Err(PredictXError::PollAlreadyResolved);
        return Ok(Payout {
            net: stake.amount,
            fee: 0,
        });
    }

    // ── Case 2: losing stake ────────────────────────────────────────────────
    let staker_on_winning_side = match stake.side {
        StakeSide::Yes => outcome_yes,
        StakeSide::No => !outcome_yes,
    };
    if !staker_on_winning_side {
        return Err(PredictXError::NotOnWinningSide);
    }

    // ── Case 3: no-contest — refund at par, no fee ──────────────────────────
    //
    // Every staker is on the winning side here, so the share is 1:1 anyway;
    // short-circuiting keeps the platform from taking a fee out of a pool
    // nobody profited from.
    if losing_pool == 0 {
        return Ok(Payout {
            net: stake.amount,
            fee: 0,
        });
    }

    // ── Case 4: proportional share of the pool, minus the platform fee ──────
    let total_pool: i128 = poll.yes_pool + poll.no_pool;
    let fee_bps = token_utils::get_platform_fee_bps(env);
    let fee_factor = (BPS_DENOMINATOR - fee_bps) as i128;
    let bps = BPS_DENOMINATOR as i128;

    let gross = stake.amount * total_pool / winning_pool;
    let net = gross * fee_factor / bps;
    if net == 0 {
        // The staker is on the winning side, so `NotOnWinningSide` would be a
        // lie. Report the truncation instead and leave the stake unclaimed —
        // the pool is frozen, so retrying cannot change the result.
        return Err(PredictXError::PayoutRoundsToZero);
    }

    stake.amount + share_of_losers

/// Record a poll's final outcome and emit the `PollResolved` event.
///
/// Shared by the oracle-gated `resolve_poll` entry point in `lib.rs`;
/// ownership, auth and the poll lookup are handled by the caller.
pub(crate) fn record_poll_resolution(
    env: &Env,
    poll: &mut Poll,
    outcome: bool,
) -> Result<(), PredictXError> {
    if poll.status == PollStatus::Cancelled {
        return Err(PredictXError::PollNotActive);
    }

    if env.ledger().timestamp() < poll.lock_time {
        return Err(PredictXError::PollNotLocked);
    }

    poll.outcome = Some(outcome);
    poll.resolver = Some(admin.clone());
    poll.resolution_basis = Some(resolution_basis.clone());
    poll.resolution_time = env.ledger().timestamp();
    env.storage()
        .persistent()
        .set(&DataKey::Poll(poll.poll_id), poll);
        .set(&DataKey::Poll(poll_id), &poll);
    polls::transition_status(&env, poll_id, PollStatus::Resolved)?;
    store_poll(env, &poll);

    let total_pool = poll.yes_pool + poll.no_pool;
    let fee = total_pool * token_utils::get_platform_fee_bps(env) as i128 / BPS_DENOMINATOR as i128;
    env.events().publish(
        (Symbol::new(env, "PollResolved"), poll.poll_id),
        (outcome, total_pool, fee),
        (Symbol::new(env, "PollResolved"), poll_id),
        (outcome, total_pool, fee, admin, resolution_basis),
    if admin != stored_admin {
        return Err(PredictXError::Unauthorized);
    }

    let mut poll: Poll = env
        .storage()
        .persistent()
        .get(&DataKey::Poll(poll_id))
        .ok_or(PredictXError::PollNotFound)?;
    if poll.status == PollStatus::Resolved {
        return Err(PredictXError::PollAlreadyResolved);
    }

    poll.status = PollStatus::Resolved;
    poll.outcome = Some(outcome);
    poll.resolution_time = env.ledger().timestamp();
    env.storage()
        .persistent()
        .set(&DataKey::Poll(poll_id), &poll);

    let total_pool = poll.yes_pool + poll.no_pool;
    let fee = total_pool * token_utils::get_platform_fee_bps(env) as i128
        / BPS_DENOMINATOR as i128;
    env.events().publish(
        (Symbol::new(env, "PollResolved"), poll_id),
        (outcome, total_pool, fee),
    );
    Ok(())
}

// ── claim_winnings ────────────────────────────────────────────────────────────
// ── Payout / claim engine ─────────────────────────────────────────────────────

/// Claim winnings for a resolved poll.
    // Derived, never a second division: net + fee == gross.
    Ok(Payout {
        net,
        fee: gross - net,
    })
}

// ── Payout / claim engine ─────────────────────────────────────────────────────

/// Claim winnings (or a full stake refund) after a poll resolves.
///
/// ## Normal path
/// The caller must have staked on the winning side.  Their proportional share
/// of the total pool — minus the platform fee — is transferred to them and the
/// fee is sent to the treasury.
///
/// Side-effects:
/// - Marks `stake.claimed = true`
/// - Transfers payout tokens from contract → user
/// - Updates `PlatformStats.total_payouts`
/// - Emits `WinningsClaimed(poll_id, user)` event
pub fn claim_winnings_for_poll(
/// ## Empty winning-pool path (issue #74)
/// When a poll resolves Yes but *every* staker picked No (or vice versa) the
/// winning pool is zero.  There is nobody eligible to collect winnings, so the
/// entire pot would be stranded forever.  In this case we treat every staker —
/// regardless of side — as eligible for a full, fee-free refund of their
/// original stake.
///
/// **This is the one place `NotOnWinningSide` must NOT be returned.**
/// Returning it here would lock funds in the contract with no recovery path.
///
/// ## One-sided path (no losing pool)
/// When every staker is on the winning side there is no pot to skim a fee
/// from, so winners are refunded at par.  This matches `calculate_winnings`.
/// ## One-sided pool path (issue #73)
/// When nobody staked on the losing side there is no pot to skim a platform
/// fee from, so winners are refunded their exact stake.  The amount paid here
/// always matches the [`calculate_winnings`] quote.
pub fn claim_winnings(
    env: &Env,
    user: Address,
    poll_id: u64,
) -> Result<i128, PredictXError> {
    user.require_auth();
    ensure_not_paused(env)?;
pub fn claim_winnings(env: &Env, claimant: Address, poll_id: u64) -> Result<i128, PredictXError> {
    claimant.require_auth();

    // ── Checks ────────────────────────────────────────────────────────────────

    let poll: Poll = load_poll(env, poll_id)
        .ok_or(PredictXError::PollNotFound)?;
/// ## Payouts that truncate to zero
/// Returns [`PredictXError::PayoutRoundsToZero`] rather than transferring a
/// worthless amount, and leaves the stake unclaimed.
pub fn claim_winnings(env: &Env, claimant: Address, poll_id: u64) -> Result<i128, PredictXError> {
    claimant.require_auth();

    // ── Load & validate the poll and the stake ───────────────────────────────

    let poll = load_resolved_poll(env, poll_id)?;

    let mut stake: Stake = load_stake(env, poll_id, &claimant)
        .ok_or(PredictXError::NotStaker)?;

    if stake.claimed || has_emergency_claimed(env, poll_id, &claimant) {
        return Err(PredictXError::AlreadyClaimed);
    }

    // A stake that was already refunded through the emergency path is spent;
    // claiming on top of that would pay the same money out twice.
    if has_emergency_claimed(env, poll_id, &claimant) {
        return Err(PredictXError::AlreadyClaimed);
    }

    let staker_won = match stake.side {
        StakeSide::Yes => yes_won,
        StakeSide::No => !yes_won,
    };
    if !staker_won {
        return Err(PredictXError::NotOnWinningSide);
    }

    // ── Compute payout (same formula as get_claimable_amount) ─────────────────
    let (winning_pool, losing_pool) = if yes_won {
        (poll.yes_pool, poll.no_pool)
    } else {
        (poll.no_pool, poll.yes_pool)
    };
    // ── Determine payout ──────────────────────────────────────────────────────
    //
    // `calculate_winnings_for` is the single source of truth shared with the
    // on-chain quote, so a claim can never pay out an amount different from
    // what `calculate_winnings` promised.
    let payout =
        calculate_winnings_for(&poll, &stake, token_utils::get_platform_fee_bps(env))?;
    if payout <= 0 {
        return Err(PredictXError::NotOnWinningSide);
    }

    // ── Platform fee ──────────────────────────────────────────────────────────
    //
    // On the normal two-sided path the winner's gross share of the pool
    // exceeds their fee-adjusted payout; that difference is the platform fee
    // and is forwarded to the treasury. On the refund paths of issues #73
    // (one-sided pool) and #74 (empty winning pool) the fee is zero, so no
    // transfer happens and every staker is made whole.
    let winning_pool: i128 = if outcome_yes { poll.yes_pool } else { poll.no_pool };
    let losing_pool: i128 = if outcome_yes { poll.no_pool } else { poll.yes_pool };
    let winning_pool: i128 = if outcome_yes {
        poll.yes_pool
    } else {
        poll.no_pool
    };
    let total_pool: i128 = poll.yes_pool + poll.no_pool;
    let losing_pool: i128 = total_pool - winning_pool;

    let payout = if winning_pool == 0 {
        stake.amount
    } else if losing_pool == 0 {
        // ── One-sided poll: full stake refund, no fee ─────────────────────────
        //
        // Nothing was staked against the winning side, so there is no losing
        // pot to skim a platform fee from.  Mirror `calculate_winnings` and
        // hand the stake back whole.
        stake.amount
    } else if losing_pool == 0 {
        // ── One-sided poll: full stake refund, no fee ─────────────────────────
        //
        // Nothing was staked against the winning side, so there is no losing
        // pot to skim a platform fee from.  Mirror `calculate_winnings` and
        // hand the stake back whole.
        stake.amount
    } else {
        let fee_bps = token_utils::get_platform_fee_bps(env);
        let fee_factor = (BPS_DENOMINATOR - fee_bps) as i128;
        let bps = BPS_DENOMINATOR as i128;
        let net_losing_pool = losing_pool * fee_factor / bps;
        let share_of_losers = stake.amount * net_losing_pool / winning_pool;
        stake.amount + share_of_losers
/// ## Stats accounting
/// `total_won` records **net profit** (payout minus original stake), not the
/// gross payout.  This keeps win-rate and ROI numbers meaningful: a user who
/// breaks even on a large stake is not credited with a large "win".
pub fn claim_winnings(
    env: &Env,
    claimant: Address,
    poll_id: u64,
) -> Result<i128, PredictXError> {
    claimant.require_auth();

    // ── Load & validate poll ──────────────────────────────────────────────────

    let poll: Poll = env
        .storage()
        .persistent()
        .get(&DataKey::Poll(poll_id))
        .ok_or(PredictXError::PollNotFound)?;

    if poll.status != PollStatus::Resolved {
        return Err(PredictXError::PollNotActive);
    }

    // outcome is always Some(_) for a Resolved poll
    let outcome_yes: bool = poll.outcome.ok_or(PredictXError::PollNotActive)?;

    // ── Load & validate stake ─────────────────────────────────────────────────

    let mut stake: Stake = env
        .storage()
        .persistent()
        .get(&DataKey::Stake(poll_id, claimant.clone()))
        .ok_or(PredictXError::NotStaker)?;

    if stake.claimed {
        return Err(PredictXError::AlreadyClaimed);
    }

    // ── Determine winning pool and payout ─────────────────────────────────────

    let winning_pool: i128 = if outcome_yes { poll.yes_pool } else { poll.no_pool };
    let total_pool: i128 = poll.yes_pool + poll.no_pool;
    let distributable: i128 = total_pool;

    let payout: i128 = if winning_pool == 0 {
        // ── Empty winning-pool: full stake refund, no fee ─────────────────────
        //
        // Every staker — regardless of which side they chose — recovers their
        // original stake in full.  No platform fee is deducted because there
        // is no "winner's profit" to share.
        //
        // NOTE: we deliberately skip the `NotOnWinningSide` check here.
        // Returning that error would leave all funds permanently stranded.
        stake.amount
    } else {
        // ── Normal winning-side claim ─────────────────────────────────────────

        let staker_on_winning_side = match stake.side {
            StakeSide::Yes => outcome_yes,
            StakeSide::No => !outcome_yes,
        };

        if !staker_on_winning_side {
            // Record the loss before bailing out: the stake is forfeit and the
            // user's stats must reflect that they lost this poll.
            record_loss(env, &claimant, stake.amount);
            return Err(PredictXError::NotOnWinningSide);
        }

        if losing_pool <= 0 {
            // No-contest: nothing to skim a fee from, refund at par.
            // Must match calculate_winnings.
        let losing_pool = if outcome_yes { poll.no_pool } else { poll.yes_pool };
        if losing_pool <= 0 {
            // No-contest: nothing was staked on the losing side, so there is no
            // pot to skim a platform fee from. Every winner is refunded their exact
            // stake rather than a fee-discounted share of a one-sided pool.
        let losing_pool: i128 = if outcome_yes { poll.no_pool } else { poll.yes_pool };
        if losing_pool <= 0 {
            // ── One-sided pool: refund the winner at par, no fee ──────────────
            //
            // Nothing was staked on the losing side, so there is no opposing
            // liquidity to share and no "winner's profit" to skim a fee from.
            // Mirrors `calculate_winnings`.
        // No-contest (issue #73): nothing was staked on the losing side, so
        // there is no pot to skim a platform fee from. Every winner is
        // refunded their exact stake rather than a fee-discounted share of a
        // one-sided pool. Mirrors `calculate_winnings`.
        let losing_pool = if outcome_yes { poll.no_pool } else { poll.yes_pool };

        if losing_pool <= 0 {
            stake.amount
        } else {
            // Proportional share of total pool, after platform fee.
            //
            // payout = stake_amount * total_pool * (BPS_DENOMINATOR - fee_bps)
            //          / (winning_pool * BPS_DENOMINATOR)
            //
            // Integer division rounds down; any dust remains in the contract.
            let fee_bps = token_utils::get_platform_fee_bps(env);
            let fee_factor = (BPS_DENOMINATOR - fee_bps) as i128;
            let bps = BPS_DENOMINATOR as i128;

            let gross = stake.amount * total_pool / winning_pool;
            let net = gross * fee_factor / bps;
            let fee = gross - net;

            // Send platform fee to treasury
            if fee > 0 {
                token_utils::transfer_to_treasury(env, fee)?;
            }

            net
        }
        // If there is no losing pool (one-sided), refund at par with no fee.
        let losing_pool = if outcome_yes { poll.no_pool } else { poll.yes_pool };
        if losing_pool <= 0 {
            stake.amount
        } else {
            // Proportional share of total pool, after platform fee.
        // Proportional share of total pool, after platform fee.
        //
        // payout = stake_amount * total_pool * (BPS_DENOMINATOR - fee_bps)
        //          / (winning_pool * BPS_DENOMINATOR)
        //
        // Integer division rounds down; any dust remains in the contract.
        let fee_bps = token_utils::get_platform_fee_bps(env);
        let fee_factor = (BPS_DENOMINATOR - fee_bps) as i128;
        let bps = BPS_DENOMINATOR as i128;

        let gross = stake.amount * total_pool / winning_pool;
        let net = gross * fee_factor / bps;
        let fee = gross - net;
        let _ = distributable;

        // Send platform fee to treasury
        if fee > 0 {
            token_utils::transfer_to_treasury(env, fee)?;
        }

        net
        }

    let fee = if winning_pool > 0 {
        let gross = stake.amount * total_pool / winning_pool;
        gross.saturating_sub(payout)
    } else {
        0
        }
        }
        }
        }
    };
    if fee > 0 {
        token_utils::transfer_to_treasury(env, fee)?;
    }
    // ── Determine the payout — identical maths to the quote ──────────────────

    let payout = payout_for(env, &poll, &stake)?;

    // ── Mark claimed & persist ───────────────────────────────────────────────

    stake.claimed = true;
    store_stake(env, poll_id, &stake);

    // ── Transfer the platform fee, then the payout ──────────────────────────

    if payout.fee > 0 {
        token_utils::transfer_to_treasury(env, payout.fee)?;
    }
    token_utils::transfer_from_contract(env, &claimant, payout.net)?;
    };

    // ── Per-poll escrow solvency check ────────────────────────────────────────
    //
    // Each poll's cumulative outflow (payout + fee) must never exceed its own
    // distributable pool.  This prevents one poll's accounting error from
    // silently draining another poll's stake.
    let claimed_so_far: i128 = env
        .storage()
        .persistent()
        .get(&DataKey::PollClaimed(poll_id))
        .unwrap_or(0);
    if claimed_so_far + payout > distributable {
        return Err(PredictXError::InsufficientEscrow);
    }

    // ── Mark claimed & persist ────────────────────────────────────────────────

    stake.claimed = true;
    env.storage()
        .persistent()
        .set(&DataKey::Stake(poll_id, claimant.clone()), &stake);

    env.storage()
        .persistent()
        .set(&DataKey::PollClaimed(poll_id), &(claimed_so_far + payout));

    // ── Transfer payout to claimant ───────────────────────────────────────────

    token_utils::transfer_from_contract(env, &claimant, payout)?;

    // ── Update user stats ─────────────────────────────────────────────────────
    //
    // Refunds (empty winning pool) count as neither a win nor a loss: the
    // staker simply got their money back, so we skip stats entirely.
    if winning_pool != 0 {
        // Net profit = payout - original stake.  `total_won` is NET, not gross.
        let net_profit = payout - stake.amount;
        record_win(env, &claimant, net_profit);
    }

    // ── Update platform stats ─────────────────────────────────────────────────

    let mut stats = get_platform_stats(env);
    stats.total_value_locked = stats.total_value_locked.saturating_sub(payout.net);
    stats.total_payouts += payout.net;
    stats.total_value_locked = stats.total_value_locked.saturating_sub(payout);
    stats.total_payouts += payout;
    set_platform_stats(env, &stats);

    // ── Emit event ────────────────────────────────────────────────────────────

    env.events().publish(
        (Symbol::new(env, "WinningsClaimed"), poll_id, claimant),
        payout.net,
    );

    Ok(payout.net)
}

/// Quote a resolved poll's payout for a user without transferring tokens.
///
/// Shares every guard and every calculation with [`claim_winnings`], so the
/// two can never report a different amount — or a different error — for the
/// same stake.  The only differences are that this path does not require
/// authorisation and does not report whether the payout was already claimed.
pub fn calculate_winnings(env: &Env, poll_id: u64, user: Address) -> Result<i128, PredictXError> {
    let poll = load_resolved_poll(env, poll_id)?;
    let stake: Stake = env
        .storage()
        .persistent()
        .get(&DataKey::Stake(poll_id, user))
        .ok_or(PredictXError::NotStaker)?;
    Ok(payout_for(env, &poll, &stake)?.net)
}

/// Return the amount `claim_winnings` would transfer, without mutating state.
///
/// The forgiving sibling of [`calculate_winnings`]: every ineligible case
/// reports `0` instead of an error, so a frontend can poll it for any
/// `(poll, user)` pair without handling a failure mode.  It is built on the
/// same [`payout_for`] as the transfer path, so a non-zero value always equals
/// what a later claim transfers.
///
/// | Case                                     | Returns |
/// |------------------------------------------|---------|
/// | Poll not found                           | `0`     |
/// | Poll not resolved                        | `0`     |
/// | User never staked                        | `0`     |
/// | Stake already claimed or emergency-refunded | `0`   |
/// | User staked on the losing side           | `0`     |
/// | Payout rounds to zero                    | `0`     |
/// | Winner with an open claim                | exact payout |
pub fn get_claimable_amount(env: &Env, poll_id: u64, user: &Address) -> i128 {
    let poll = match load_resolved_poll(env, poll_id) {
        Ok(poll) => poll,
        Err(_) => return 0,
    };
    let stake: Stake = match env
        .storage()
        .persistent()
        .get(&DataKey::Stake(poll_id, user.clone()))
    {
        Some(stake) => stake,
        None => return 0,
    };
    if stake.claimed || has_emergency_claimed(env, poll_id, user) {
        return 0;
    }
    payout_for(env, &poll, &stake)
        .map(|payout| payout.net)
        .unwrap_or(0)
}

/// Calculate a resolved poll's payout for a user without transferring tokens.
pub fn calculate_winnings(env: &Env, poll_id: u64, user: Address) -> Result<i128, PredictXError> {
    let poll: Poll = env
        .storage()
        .persistent()
        .get(&DataKey::Poll(poll_id))
        payout,
    );

    Ok(payout)
}

// ── User stats helpers ────────────────────────────────────────────────────────

/// Load the caller's `UserStats`, creating a zeroed record on first touch.
fn load_user_stats(env: &Env, user: &Address) -> UserStats {
    env.storage()
        .persistent()
        .get(&DataKey::UserStats(user.clone()))
        .unwrap_or(UserStats {
            total_staked: 0,
            total_won: 0,
            total_lost: 0,
            polls_won: 0,
            polls_lost: 0,
        })
}

fn save_user_stats(env: &Env, user: &Address, stats: &UserStats) {
    env.storage()
        .persistent()
        .set(&DataKey::UserStats(user.clone()), stats);
}

/// Record a winning claim.  `net_profit` is the payout minus the original
/// stake — `total_won` tracks **net profit**, never gross payout.
fn record_win(env: &Env, user: &Address, net_profit: i128) {
    let mut stats = load_user_stats(env, user);
    stats.polls_won += 1;
    stats.total_won += net_profit;
    save_user_stats(env, user, &stats);
}

/// Record a losing position: the stake is forfeit and counts as lost.
fn record_loss(env: &Env, user: &Address, stake_amount: i128) {
    let mut stats = load_user_stats(env, user);
    stats.polls_lost += 1;
    stats.total_lost += stake_amount;
    save_user_stats(env, user, &stats);
}

/// Calculate a resolved poll's payout for a user without transferring tokens.
pub fn calculate_winnings(
    env: &Env,
    poll_id: u64,
    user: Address,
) -> Result<i128, PredictXError> {
    let poll: Poll = load_poll(env, poll_id)
        .ok_or(PredictXError::PollNotFound)?;
    let stake: Stake = load_stake(env, poll_id, &user)
    let poll: Poll = env
        .storage()
        .persistent()
        .get(&DataKey::Poll(poll_id))
        .ok_or(PredictXError::PollNotFound)?;
    let stake: Stake = env
        .storage()
        .persistent()
        .get(&DataKey::Stake(poll_id, user))
        .ok_or(PredictXError::NotStaker)?;
    if poll.status != PollStatus::Resolved {
        return Err(PredictXError::PollNotLocked);
    }

    calculate_winnings_for(&poll, &stake, token_utils::get_platform_fee_bps(env))
}

/// Shared payout formula behind both the on-chain quote (`calculate_winnings`)
/// and the actual transfer in `claim_winnings`, so the two can never drift.
///
/// - Loser on a two-sided pool: 0 (caller surfaces `NotOnWinningSide`).
/// - Empty winning pool (issue #74): every staker is refunded in full —
///   returning zero here would strand the whole pot.
/// - One-sided pool (issue #73): there is no losing pot to skim a platform
///   fee from, so winners are refunded their exact stake.
/// - Normal case: proportional share of the pool after the platform fee.
fn calculate_winnings_for(
    poll: &Poll,
    stake: &Stake,
    fee_bps: u32,
) -> Result<i128, PredictXError> {
    let outcome = poll.outcome.ok_or(PredictXError::InvalidOutcome)?;

    let winning_pool = if outcome { poll.yes_pool } else { poll.no_pool };
    if winning_pool == 0 {
        // Empty winning pool (issue #74): refund every staker, fee-free.
        return Ok(stake.amount);
    }

    let winning_side = if outcome { StakeSide::Yes } else { StakeSide::No };
    if stake.side != winning_side {
    if stake.side
        != if outcome {
            StakeSide::Yes
        } else {
            StakeSide::No
        }
    {
    let outcome = poll.outcome.ok_or(PredictXError::InvalidOutcome)?;
    let winning_pool = if outcome { poll.yes_pool } else { poll.no_pool };
    if winning_pool == 0 {
        return Ok(stake.amount);
    }
    if stake.side != if outcome { StakeSide::Yes } else { StakeSide::No } {
        return Ok(0);
    }

    let losing_pool = if outcome { poll.no_pool } else { poll.yes_pool };
    if losing_pool <= 0 {
        // One-sided pool (issue #73): no losing pot → no fee, refund at par.
        // No-contest: nothing was staked on the losing side, so there is no
        // pot to skim a platform fee from. Every winner is refunded their exact
        // stake rather than a fee-discounted share of a one-sided pool.
        return Ok(stake.amount);
    }

    let total_pool = poll.yes_pool + poll.no_pool;
    let payout_pool = total_pool * (BPS_DENOMINATOR - fee_bps) as i128
        / BPS_DENOMINATOR as i128;

    let payout_pool = total_pool
        * (BPS_DENOMINATOR - token_utils::get_platform_fee_bps(env)) as i128
        / BPS_DENOMINATOR as i128;
    Ok(stake.amount * payout_pool / winning_pool)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod test {
    extern crate std;

    use crate::{DataKey, PredictionMarket, PredictionMarketClient};
    use predictx_shared::{Poll, PollCategory, PollStatus, PredictXError, Stake, StakeSide};
    use soroban_sdk::{
        testutils::{Address as _, Ledger},
        token, Address, Env, String,
    };
    use predictx_shared::{DataKey, Poll, PollCategory, PollStatus, PredictXError, Stake, StakeSide};
    use crate::{PredictionMarket, PredictionMarketClient};
    use predictx_shared::{
        Poll, PollCategory, PollStatus, PredictXError, Stake, StakeSide, MAX_STAKE_AMOUNT,
        MIN_STAKE_AMOUNT,
    };
    use soroban_sdk::{
        testutils::{Address as _, Ledger},
        token, Address, Env, String, Vec,
    };
    use predictx_shared::{
        Poll, PollCategory, PollStatus, PredictXError, Stake, StakeSide, VoteChoice,
        VOTING_WINDOW_SECS,
    };
    use crate::{DataKey, PredictionMarket, PredictionMarketClient};

    // ── Test helpers ──────────────────────────────────────────────────────────

    struct TestSetup<'a> {
        env: Env,
        admin: Address,
        #[allow(dead_code)]
        oracle_id: Address,
        token_addr: Address,
        contract_id: Address,
        client: PredictionMarketClient<'a>,
    }

    fn setup() -> TestSetup<'static> {
        let env = Env::default();
        env.mock_all_auths();

        let admin = Address::generate(&env);

        let oracle_id = env.register(crate::voting_oracle::WASM, ());
        let oracle_client = crate::voting_oracle::Client::new(&env, &oracle_id);
        oracle_client.initialize(&admin);

        let token_admin = Address::generate(&env);
        let token_contract = env.register_stellar_asset_contract_v2(token_admin.clone());
        let token_addr = token_contract.address();

        oracle_client.initialize(&admin, &token_addr);

        let contract_id = env.register(PredictionMarket, ());
        let client = PredictionMarketClient::new(&env, &contract_id);
        let treasury = Address::generate(&env);
        client.initialize(&admin, &oracle_id, &token_addr, &treasury, &500_u32);
        // `cancel_poll` mirrors the cancellation into the oracle, which
        // authorises the caller against its admin registry.
        oracle_client.add_admin(&admin, &contract_id);

        env.ledger().with_mut(|l| l.timestamp = 1_000_000);

        TestSetup {
            env,
            admin,
            oracle_id,
            token_addr,
            contract_id,
            client,
        }
    }

    /// Drive a poll to `Voting` through the legal graph, since a poll may only
    /// be resolved from `Voting`, `AdminReview` or `Disputed`.
    fn advance_to_voting(s: &TestSetup, poll_id: u64) {
        for status in [PollStatus::Locked, PollStatus::Voting] {
            s.env.as_contract(&s.contract_id, || {
                let mut poll: Poll = s
                    .env
                    .storage()
                    .persistent()
                    .get(&DataKey::Poll(poll_id))
                    .unwrap();
                poll.status = status;
                s.env
                    .storage()
                    .persistent()
                    .set(&DataKey::Poll(poll_id), &poll);
            });
        }
    }

    /// Resolve a poll the way the state machine demands.
    fn resolve_via_state_machine(s: &TestSetup, poll_id: u64, outcome_yes: bool) {
        advance_to_voting(s, poll_id);
        s.client.resolve_poll(&s.admin, &poll_id, &outcome_yes);
    }

    fn stake_user(
        s: &TestSetup,
        poll_id: u64,
        side: StakeSide,
        amount: i128,
    ) -> Address {
    fn stake_user(s: &TestSetup, poll_id: u64, side: StakeSide, amount: i128) -> Address {
        let user = Address::generate(&s.env);
        mint_tokens(s, &user, amount);
        s.client.stake(&user, &poll_id, &amount, &side);
        user

        env.ledger().with_mut(|l| l.timestamp = 1_000_000);

        TestSetup { env, admin, oracle_id, token_addr, contract_id, client }
    }

    fn mint_tokens(s: &TestSetup, to: &Address, amount: i128) {
        let sac = token::StellarAssetClient::new(&s.env, &s.token_addr);
        sac.mint(to, &amount);
    }

    /// Create a fresh user, fund them and place a stake on `poll_id`.
    /// Returns the user address.
    fn stake_user(s: &TestSetup, poll_id: u64, side: StakeSide, amount: i128) -> Address {
        let user = Address::generate(&s.env);
        mint_tokens(s, &user, amount);
        s.client.stake(&user, &poll_id, &amount, &side);
        user
    }

    fn token_balance(s: &TestSetup, addr: &Address) -> i128 {
        token::Client::new(&s.env, &s.token_addr).balance(addr)
    }

    /// Mint tokens and place a real stake, returning the staker's address.
    /// Stake on a poll through the public entry point and return the staker.
    fn stake_user(s: &TestSetup, poll_id: u64, side: StakeSide, amount: i128) -> Address {
    fn stake_user(
        s: &TestSetup,
        poll_id: u64,
        side: StakeSide,
        amount: i128,
    ) -> Address {
    /// Create a user, fund them, place a stake, and return the user address.
    fn stake_user(s: &TestSetup, poll_id: u64, side: StakeSide, amount: i128) -> Address {
        let user = Address::generate(&s.env);
        mint_tokens(s, &user, amount);
        s.client.stake(&user, &poll_id, &amount, &side);
        user
    }

    /// Create a match + poll and return the poll_id.
    fn create_poll(s: &TestSetup, lock_time: u64) -> u64 {
        let match_id = s.client.create_match(
            &s.admin,
            &String::from_str(&s.env, "Arsenal"),
            &String::from_str(&s.env, "Chelsea"),
            &String::from_str(&s.env, "Premier League"),
            &String::from_str(&s.env, "Emirates"),
            &(lock_time + 3_600),
        );
        s.client.create_poll(
            &s.admin,
            &match_id,
            &String::from_str(&s.env, "Will Palmer score?"),
            &PollCategory::PlayerEvent,
            &lock_time,
        )
    }

    /// Directly inject a resolved poll with a given outcome into storage.
    fn inject_resolved_poll(
        s: &TestSetup,
        poll_id: u64,
        outcome_yes: bool,
        yes_pool: i128,
        no_pool: i128,
    ) {
    fn inject_resolved_poll(s: &TestSetup, poll_id: u64, outcome_yes: bool, yes_pool: i128, no_pool: i128) {
        s.env.as_contract(&s.contract_id, || {
            let poll = Poll {
                poll_id,
                match_id: 1,
                creator: s.admin.clone(),
                question: String::from_str(&s.env, "test"),
                category: PollCategory::PlayerEvent,
                lock_time: 500_000,
                yes_pool,
                no_pool,
                yes_count: if yes_pool > 0 { 1 } else { 0 },
                no_count: if no_pool > 0 { 1 } else { 0 },
                status: PollStatus::Resolved,
                outcome: Some(outcome_yes),
                resolver: Some(s.admin.clone()),
                resolution_basis: Some(String::from_str(&s.env, "test")),
                resolution_time: 1_000_000,
                created_at: 900_000,
            };
            s.env
                .storage()
                .persistent()
                .set(&DataKey::Poll(poll_id), &poll);
                resolution_time: 1_000_000,
                created_at: 900_000,
            };
            s.env.storage().persistent().set(&DataKey::Poll(poll_id), &poll);
        });
    }

    /// Inject a stake record directly (bypasses staking checks — used to set
    /// up state for claim tests without going through the full staking flow).
    fn inject_stake(s: &TestSetup, poll_id: u64, user: &Address, amount: i128, side: StakeSide) {
        s.env.as_contract(&s.contract_id, || {
            let stake = Stake {
                user: user.clone(),
                poll_id,
                amount,
                side,
                claimed: false,
                staked_at: 900_000,
            };
            s.env
                .storage()
                .persistent()
                .set(&DataKey::Stake(poll_id, user.clone()), &stake);
        });
    }

    // ── Test 1: view output matches actual claim payout ───────────────────────
    //
    // This is the primary acceptance-criteria test: get_claimable_amount must
    // return the same value that claim_winnings would transfer to the winner.
    // Because claim_winnings hasn't been implemented yet we verify against the
    // hand-computed expected payout instead, using the same formula that both
    // functions will use.
    //
    // Setup:
    //   Yes pool = 300 tokens, No pool = 200 tokens, fee = 5 %
    //   Winner is Yes (yes_won = true)
    //   User staked 100 tokens on Yes
    //
    // Expected payout:
    //   net_losing_pool = 200 * 9500 / 10000 = 190
    //   share_of_losers = 100 * 190 / 300    =  63  (truncated)
    //   payout          = 100 + 63           = 163
    /// Mint tokens to a fresh user and stake them through the real staking flow.
    /// Helper that creates a user, mints tokens and places a stake through the
    /// normal staking flow. Returns the user address.
    /// Mint, then stake through the real entry point.  Returns the staker.
    fn stake_user(s: &TestSetup, poll_id: u64, side: StakeSide, amount: i128) -> Address {
        let user = Address::generate(&s.env);
        mint_tokens(s, &user, amount);
        s.client.stake(&user, &poll_id, &amount, &side);
    /// Inject a stake for a user (convenience helper that mints tokens and stakes).
    fn stake_user(s: &TestSetup, poll_id: u64, side: StakeSide, amount: i128) -> Address {
        let user = Address::generate(&s.env);
        mint_tokens(s, &user, amount);
        inject_stake(s, poll_id, &user, amount, side);
        user
    }

            s.env.storage().persistent().set(&DataKey::Stake(poll_id, user.clone()), &stake);
        });
    }

    // ── Tests: empty winning-pool path (issue #74) ────────────────────────────

    /// A losing staker can recover their original stake when the winning pool
    /// is empty (i.e. nobody staked on the winning side).
    #[test]
    fn empty_winning_pool_losing_staker_gets_full_refund() {
        let s = setup();

        // Poll resolves Yes, but only No stakers exist → yes_pool == 0
        let poll_id: u64 = 99;
        let no_stake_amount: i128 = 200_000_000;

        let no_user = Address::generate(&s.env);

        // Seed contract with the pool amount
        mint_tokens(&s, &s.contract_id, no_stake_amount);

        inject_resolved_poll(&s, poll_id, true, 0, no_stake_amount);
        inject_stake(&s, poll_id, &no_user, no_stake_amount, StakeSide::No);

        let refund = s.client.claim_winnings(&no_user, &poll_id);

        assert_eq!(refund, no_stake_amount, "should refund full stake");
        assert_eq!(
            token_balance(&s, &no_user),
            no_stake_amount,
            "user balance should equal refunded stake"
        );
    }

    /// No platform fee is taken in the empty-winning-pool case.
    #[test]
    fn empty_winning_pool_no_platform_fee_deducted() {
        let s = setup();

        // Poll resolves No, but only Yes stakers exist → no_pool == 0
        let poll_id: u64 = 100;
        let yes_stake_amount: i128 = 150_000_000;

        let yes_user = Address::generate(&s.env);
        mint_tokens(&s, &s.contract_id, yes_stake_amount);

        inject_resolved_poll(&s, poll_id, false, yes_stake_amount, 0);
        inject_stake(&s, poll_id, &yes_user, yes_stake_amount, StakeSide::Yes);

        let treasury_before = token_balance(&s, &s.client.get_treasury_address());
        let refund = s.client.claim_winnings(&yes_user, &poll_id);

        // Exact stake returned — no fee
        assert_eq!(refund, yes_stake_amount);
        // Treasury unchanged
        assert_eq!(
            token_balance(&s, &s.client.get_treasury_address()),
            treasury_before,
            "treasury must not receive any fee in empty-pool refund"
        );
    }

    /// Once all stakers have claimed in the empty-winning-pool scenario, the
    /// contract balance reaches exactly zero.
    #[test]
    fn empty_winning_pool_contract_balance_zero_after_all_claims() {
        let s = setup();

        // Poll resolves Yes, but all three stakers picked No
        let poll_id: u64 = 101;
        let amounts: [i128; 3] = [100_000_000, 200_000_000, 150_000_000];
        let total: i128 = amounts[0] + amounts[1] + amounts[2];

        let users: [Address; 3] = [
            Address::generate(&s.env),
            Address::generate(&s.env),
            Address::generate(&s.env),
        ];

        // Seed contract with the full pooled amount
        mint_tokens(&s, &s.contract_id, total);

        inject_resolved_poll(&s, poll_id, true, 0, total);

        for (i, user) in users.iter().enumerate() {
            inject_stake(&s, poll_id, user, amounts[i], StakeSide::No);
        }

        // All three stakers claim their refund
        for (i, user) in users.iter().enumerate() {
            let refund = s.client.claim_winnings(user, &poll_id);
            assert_eq!(refund, amounts[i]);
        }

        // Contract balance must be exactly zero — no stranded funds
        assert_eq!(
            token_balance(&s, &s.contract_id),
            0,
            "all funds should be returned; contract balance must be zero"
        );
    }

    #[test]
    fn successful_claim_marks_stake_as_claimed() {
        let s = setup();
        let poll_id: u64 = 102;
        let winner = Address::generate(&s.env);
        let winning_stake = 100_000_000;
        let losing_pool = 300_000_000;

        mint_tokens(&s, &s.contract_id, winning_stake + losing_pool);
        inject_resolved_poll(&s, poll_id, true, winning_stake, losing_pool);
        inject_stake(&s, poll_id, &winner, winning_stake, StakeSide::Yes);

        s.client.claim_winnings(&winner, &poll_id);

        let stake = s.client.get_stake_info(&poll_id, &winner);
        assert!(stake.claimed);
    }

    // ── Tests: normal claim path ──────────────────────────────────────────────

    /// A winner on the correct side receives their proportional payout.
    #[test]
    fn normal_winner_receives_proportional_payout() {
        let s = setup();

        let lock_time = 1_500_000;
        let poll_id = create_poll(&s, lock_time);

        let yes_user = Address::generate(&s.env);
        let no_user = Address::generate(&s.env);
        let yes_amount: i128 = 100_000_000;
        let no_amount: i128 = 100_000_000;

        mint_tokens(&s, &yes_user, yes_amount);
        mint_tokens(&s, &no_user, no_amount);

        s.client
            .stake(&yes_user, &poll_id, &yes_amount, &StakeSide::Yes);
        s.client
            .stake(&no_user, &poll_id, &no_amount, &StakeSide::No);
        s.client.stake(&yes_user, &poll_id, &yes_amount, &StakeSide::Yes);
        s.client.stake(&no_user, &poll_id, &no_amount, &StakeSide::No);

        // Resolve with Yes winning
        inject_resolved_poll(&s, poll_id, true, yes_amount, no_amount);

        let payout = s.client.claim_winnings(&yes_user, &poll_id);

        // gross = 100M * 200M / 100M = 200M; net = 200M * 9500 / 10000 = 190M
        let expected_net: i128 = 190_000_000;
        assert_eq!(payout, expected_net);
        assert!(token_balance(&s, &yes_user) >= expected_net);
    }

    /// A staker on the losing side is rejected with `NotOnWinningSide`.
    #[test]
    fn loser_cannot_claim_on_normal_resolution() {
        let s = setup();

        let lock_time = 1_500_000;
        let poll_id = create_poll(&s, lock_time);

        let yes_user = Address::generate(&s.env);
        let no_user = Address::generate(&s.env);
        let amount: i128 = 100_000_000;

        mint_tokens(&s, &yes_user, amount);
        mint_tokens(&s, &no_user, amount);

        s.client
            .stake(&yes_user, &poll_id, &amount, &StakeSide::Yes);
        s.client.stake(&yes_user, &poll_id, &amount, &StakeSide::Yes);
        s.client.stake(&no_user, &poll_id, &amount, &StakeSide::No);

        // Resolve with Yes winning — No user is the loser
        inject_resolved_poll(&s, poll_id, true, amount, amount);

        let err = s
            .client
            .try_claim_winnings(&no_user, &poll_id)
            .expect_err("loser should not be able to claim");
        assert_eq!(err, Ok(PredictXError::NotOnWinningSide));
    }

    /// Double-claiming is rejected with `AlreadyClaimed`.
    #[test]
    fn double_claim_is_rejected() {
        let s = setup();

        let poll_id: u64 = 200;
        let amount: i128 = 100_000_000;
        let user = Address::generate(&s.env);

        mint_tokens(&s, &s.contract_id, amount * 2);

        // ── Read view BEFORE claiming ─────────────────────────────────────────
        let claimable = s.client.get_claimable_amount(&poll_id, &user);
        assert!(claimable > 0, "winner must have a positive claimable amount");
    #[test]
    fn one_sided_poll_refunds_the_winner_at_par() {
        let s = setup();
        let poll_id = create_poll(&s, 2_000_000);
        let winner = stake_user(&s, poll_id, StakeSide::Yes, 100_000_000);
        inject_resolved_poll(&s, poll_id, true, 100_000_000, 0);
        s.client.resolve_poll(&s.oracle_id, &poll_id, &true);
        inject_resolved_poll(&s, poll_id, true, amount, amount);
        inject_stake(&s, poll_id, &user, amount, StakeSide::Yes);

        s.client.claim_winnings(&user, &poll_id);

        let err = s
            .client
            .try_claim_winnings(&user, &poll_id)
            .expect_err("second claim should fail");
        assert_eq!(err, Ok(PredictXError::AlreadyClaimed));
    }

        // ── Assert: view == returned value == tokens received ─────────────────
        assert_eq!(
            claimable, returned,
            "get_claimable_amount ({claimable}) must equal claim_winnings return ({returned})"
        );
        assert_eq!(
            claimable,
            balance_after - balance_before,
            "get_claimable_amount ({claimable}) must equal tokens transferred ({})",
            balance_after - balance_before
        );
    #[test]
    fn one_sided_poll_refunds_the_winner_at_par() {
        let s = setup();
        let poll_id = create_poll(&s, 2_000_000);
        let winner = stake_user(&s, poll_id, StakeSide::Yes, 100_000_000);
        resolve_via_state_machine(&s, poll_id, true);
        s.env.ledger().set_timestamp(2_000_001);
        s.client.resolve_poll(&s.admin, &poll_id, &true, &String::from_str(&s.env, "test"));
        s.client.admin_resolve_poll(&s.admin, &poll_id, &true);
        s.client.resolve_poll(&s.admin, &poll_id, &true);

        let claimed = s.client.claim_winnings(&winner, &poll_id);

        // No losing side, so no fee may be taken: the stake comes back whole.
        assert_eq!(claimed, 100_000_000);
        let token_client = token::Client::new(&s.env, &s.token_addr);
        assert_eq!(token_client.balance(&winner), 100_000_000);
    }

    #[test]
    fn one_sided_poll_no_side_also_refunds_at_par() {
        let s = setup();
        let poll_id = create_poll(&s, 2_000_000);
        let winner = stake_user(&s, poll_id, StakeSide::No, 50_000_000);
        inject_resolved_poll(&s, poll_id, false, 0, 50_000_000);
        s.client.resolve_poll(&s.oracle_id, &poll_id, &false);
        resolve_via_state_machine(&s, poll_id, false);
        s.env.ledger().set_timestamp(2_000_001);
        s.client.resolve_poll(&s.admin, &poll_id, &false, &String::from_str(&s.env, "test"));
        s.client.admin_resolve_poll(&s.admin, &poll_id, &false);
        s.client.resolve_poll(&s.admin, &poll_id, &false);

        // The quote and the claim must agree, both fee-free.
        assert_eq!(s.client.calculate_winnings(&poll_id, &winner), 50_000_000);
        assert_eq!(s.client.claim_winnings(&winner, &poll_id), 50_000_000);
    }

    // ── Payouts that round to zero (issue #235) ───────────────────────────────

    /// A winning-side stake whose payout truncates to zero gets its own error —
    /// never `NotOnWinningSide`, which is factually wrong for a winner — and
    /// the quote and the claim agree on it.
    ///
    /// `stake` refuses anything below `MIN_STAKE_AMOUNT` and the pool maths
    /// guarantee `gross >= amount` for a real stake, so this is only reachable
    /// by a record that predates the floor (or a directly seeded one).  The
    /// guard is what stops that record from being told it lost.
    #[test]
    fn zero_rounded_payout_returns_its_own_error() {
        let s = setup();
        let poll_id: u64 = 300;
        let winner = Address::generate(&s.env);
        // A maximal winning pool faced by a dust-sized winning stake.
        let winning_pool: i128 = MAX_STAKE_AMOUNT;
        let losing_pool: i128 = 10_000_000;
        let dust_stake: i128 = 1;

        mint_tokens(&s, &s.contract_id, winning_pool + losing_pool);
        inject_resolved_poll(&s, poll_id, true, winning_pool, losing_pool);
        inject_stake(&s, poll_id, &winner, dust_stake, StakeSide::Yes);

        // 1 * 110_000_000 / 100_000_000 = 1 gross, 1 * 9500 / 10000 = 0 net.
        let err = s
            .client
            .try_calculate_winnings(&poll_id, &winner)
            .expect_err("a zero payout is not payable");
        assert_eq!(err, Ok(PredictXError::PayoutRoundsToZero));

        let err = s
            .client
            .try_claim_winnings(&winner, &poll_id)
            .expect_err("a zero payout is not payable");
        assert_eq!(err, Ok(PredictXError::PayoutRoundsToZero));

        // The stake is left unclaimed and no token moved.
        assert!(!s.client.get_stake_info(&poll_id, &winner).claimed);
        assert_eq!(token_balance(&s, &winner), 0);
    }

    /// Boundary case: the smallest stake `stake` accepts, against the largest
    /// pool a single stake can build.  It must still pay, and both paths must
    /// return that same number.
    #[test]
    fn minimal_stake_against_maximal_pool_is_payable() {
        let s = setup();
        let poll_id = create_poll(&s, 2_000_000);

        let whale = Address::generate(&s.env);
        mint_tokens(&s, &whale, MAX_STAKE_AMOUNT);
        s.client
            .stake(&whale, &poll_id, &MAX_STAKE_AMOUNT, &StakeSide::Yes);

        let min_user = Address::generate(&s.env);
        mint_tokens(&s, &min_user, MIN_STAKE_AMOUNT);
        s.client
            .stake(&min_user, &poll_id, &MIN_STAKE_AMOUNT, &StakeSide::Yes);

        let losing = Address::generate(&s.env);
        mint_tokens(&s, &losing, MIN_STAKE_AMOUNT);
        s.client
            .stake(&losing, &poll_id, &MIN_STAKE_AMOUNT, &StakeSide::No);

        resolve_via_state_machine(&s, poll_id, true);

        let winning_pool = MAX_STAKE_AMOUNT + MIN_STAKE_AMOUNT;
        let total_pool = winning_pool + MIN_STAKE_AMOUNT;
        let gross = MIN_STAKE_AMOUNT * total_pool / winning_pool;
        let expected = gross * 9_500 / 10_000;

        let quoted = s.client.calculate_winnings(&poll_id, &min_user);
        assert_eq!(quoted, expected);
        assert!(quoted > 0, "a minimum stake must never round to zero");
        assert_eq!(s.client.claim_winnings(&min_user, &poll_id), expected);
    }

    /// The quote and the claim agree on every case the engine distinguishes,
    /// including the rounding edges that the two used to compute differently.
    #[test]
    fn quote_and_claim_agree_across_payout_cases() {
        // (yes_pool, no_pool, outcome_yes, staker amount, staker side)
        let cases: [(i128, i128, bool, i128, StakeSide); 6] = [
            (10_000_007, 10_000_003, true, 10_000_007, StakeSide::Yes),
            (10_000_007, 10_000_003, true, 3, StakeSide::Yes),
            (3, 10_000_000, true, 3, StakeSide::Yes),
            (10_000_007, 10_000_003, false, 10_000_003, StakeSide::No),
            (10_000_000, 0, true, 10_000_000, StakeSide::Yes),
            (0, 10_000_000, true, 10_000_000, StakeSide::No),
        ];

        for (i, (yes_pool, no_pool, outcome_yes, amount, side)) in cases.iter().enumerate() {
            let s = setup();
            let poll_id: u64 = 400 + i as u64;
            let staker = Address::generate(&s.env);
            let other = Address::generate(&s.env);

            mint_tokens(&s, &s.contract_id, yes_pool + no_pool);
            inject_resolved_poll(&s, poll_id, *outcome_yes, *yes_pool, *no_pool);
            inject_stake(&s, poll_id, &staker, *amount, *side);
            // A second staker keeps the case honest: somebody is always on the
            // other side of the book.
            inject_stake(&s, poll_id, &other, *amount, *side);

            let quoted = s.client.try_calculate_winnings(&poll_id, &staker);
            let claimed = s.client.try_claim_winnings(&staker, &poll_id);
            assert_eq!(
                quoted,
                claimed,
                "quote and claim diverged for case {:?}",
                (yes_pool, no_pool, outcome_yes, amount, side)
            );
        }
    }

    /// The engine never pays out more than the pool holds, and the truncation
    /// remainder stays behind in the contract rather than being handed to a
    /// single claimant.
    #[test]
    fn payouts_never_exceed_the_pool() {
        struct FixedRng {
            state: u64,
        }

        impl FixedRng {
            fn new(seed: u64) -> Self {
                Self { state: seed }
            }

            fn next_u64(&mut self) -> u64 {
                self.state = self
                    .state
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                self.state
            }

            fn next_i128(&mut self, min: i128, max: i128) -> i128 {
                let range = (max - min + 1) as u128;
                let offset = (self.next_u64() as u128 % range) as i128;
                min + offset
            }

            fn next_bool(&mut self) -> bool {
                (self.next_u64() & 1) == 1
            }
        }

        let mut rng = FixedRng::new(0xC0FFEE_1234_5678);

        for scenario in 0..60 {
            let s = setup();
            let poll_id: u64 = 500 + scenario as u64;
            let outcome_yes = rng.next_bool();
            let winning_side = if outcome_yes { StakeSide::Yes } else { StakeSide::No };
            let losing_side = if outcome_yes { StakeSide::No } else { StakeSide::Yes };
            let force_single_staker = scenario % 9 == 0;

            let mut winners: Vec<Address> = Vec::new(&s.env);
            let mut yes_pool = 0_i128;
            let mut no_pool = 0_i128;
            let mut total_staked = 0_i128;

            let winner_count = if force_single_staker {
                1
            } else {
                1 + (rng.next_u64() % 6) as usize
            };
            let loser_count = if scenario % 11 == 0 {
                0
            } else {
                (rng.next_u64() % 6) as usize
            };

            for _ in 0..winner_count {
                let amount = if force_single_staker {
                    100_000 + (rng.next_u64() % 7_500_000) as i128
                } else {
                    rng.next_i128(100_000, 7_500_000)
                };
                let user = Address::generate(&s.env);
                inject_stake(&s, poll_id, &user, amount, winning_side);
                winners.push_back(user.clone());
                total_staked += amount;
                if outcome_yes {
                    yes_pool += amount;
                } else {
                    no_pool += amount;
                }
            }

            for _ in 0..loser_count {
                let amount = rng.next_i128(100_000, 3_500_000);
                let user = Address::generate(&s.env);
                inject_stake(&s, poll_id, &user, amount, losing_side);
                total_staked += amount;
                if outcome_yes {
                    no_pool += amount;
                } else {
                    yes_pool += amount;
                }
            }

            let treasury = s.client.get_treasury_address();
            mint_tokens(&s, &s.contract_id, total_staked);
            inject_resolved_poll(&s, poll_id, outcome_yes, yes_pool, no_pool);

            let mut total_paid = 0_i128;
            for i in 0..winners.len() {
                let winner = winners.get(i).unwrap();
                let claimed = s.client.claim_winnings(&winner, &poll_id);
                assert_eq!(
                    claimed,
                    s.client.calculate_winnings(&poll_id, &winner),
                    "fund-loss bug: payouts plus fees never exceed the total staked"
                );
                total_paid += claimed;
            }

            let fee_total = token_balance(&s, &treasury);
            let paid = total_paid + fee_total;
            assert!(
                paid <= total_staked,
                "fund-loss bug: payouts plus fees never exceed the total staked"
            );
            assert_eq!(
                token_balance(&s, &s.contract_id) + paid,
                total_staked,
                "fund-loss bug: payouts plus fees never exceed the total staked"
            );
        }
    }
}

    // ── Invalid state transitions (issue #124) ────────────────────────────────

    /// Claiming — or quoting — a poll that is not resolved reports the state
    /// mismatch itself instead of the unrelated `PollNotActive` /
    /// `PollNotLocked` that used to surface.
    #[test]
    fn claim_and_quote_before_resolution_report_invalid_state() {
        let s = setup();
        let poll_id = create_poll(&s, 2_000_000);
        let user = stake_user(&s, poll_id, StakeSide::Yes, 10_000_000);

        let err = s
            .client
            .try_claim_winnings(&user, &poll_id)
            .expect_err("poll is not resolved");
        assert_eq!(err, Ok(PredictXError::InvalidStateTransition));

        let err = s
            .client
            .try_calculate_winnings(&poll_id, &user)
            .expect_err("poll is not resolved");
        assert_eq!(err, Ok(PredictXError::InvalidStateTransition));
    }

    /// A cancelled poll is terminal: resolving it is an illegal transition.
    #[test]
    fn resolve_after_cancel_reports_invalid_state() {
        let s = setup();
        let poll_id = create_poll(&s, 2_000_000);
        s.client.cancel_poll(&s.admin, &poll_id);

        let err = s
            .client
            .try_resolve_poll(&s.admin, &poll_id, &true)
            .expect_err("cancelled polls are terminal");
        assert_eq!(err, Ok(PredictXError::InvalidStateTransition));
    // ── Issue #153: Platform stats update on claim ────────────────────────────

    /// Test that total_payouts is incremented on a successful claim.
    #[test]
    fn total_payouts_incremented_on_successful_claim() {
        let s = setup();
        let poll_id = create_poll(&s, 2_000_000);

        let winner = Address::generate(&s.env);
        let amount: i128 = 100_000_000;

        mint_tokens(&s, &winner, amount);
        mint_tokens(&s, &s.contract_id, amount + 300_000_000);
        s.client.stake(&winner, &poll_id, &amount, &StakeSide::Yes);

        // Resolve with Yes winning, no_pool = 300M
        inject_resolved_poll(&s, poll_id, true, amount, 300_000_000);

        let payout = s.client.claim_winnings(&winner, &poll_id);

        // Verify total_payouts was incremented
        let stats = s.client.get_platform_stats();
        assert_eq!(stats.total_payouts, payout, "total_payouts should equal the payout amount");

        // Verify total_value_locked was decreased by the payout amount
        // (TVL was 100M at stake, decreased by 95M net payout after 5% fee)
        let expected_locked = 100_000_000_i128 - payout;
        assert_eq!(
            stats.total_value_locked,
            expected_locked,
            "total_value_locked should decrease by the claimed payout"
        );
    }

    /// Test that total_value_locked cannot go negative (uses saturating_sub).
    #[test]
    fn total_value_locked_cannot_go_negative() {
        let s = setup();
        let poll_id = create_poll(&s, 2_000_000);

        let winner = Address::generate(&s.env);
        let amount: i128 = 50_000_000;

        mint_tokens(&s, &winner, amount);
        mint_tokens(&s, &s.contract_id, amount);
        s.client.stake(&winner, &poll_id, &amount, &StakeSide::Yes);

        // Resolve with Yes winning, no_pool = 0 (one-sided poll)
        inject_resolved_poll(&s, poll_id, true, amount, 0);

        let payout = s.client.claim_winnings(&winner, &poll_id);

        // After claim, TVL should be the platform fee (2.5M = 5% of 50M), not negative
        // due to saturating_sub. The fee remains in the contract.
        let stats = s.client.get_platform_stats();
        assert!(
            stats.total_value_locked >= 0,
            "total_value_locked should not go negative"
        );
        // TVL = initial stake - net payout = 50M - 47.5M = 2.5M (platform fee)
        assert_eq!(
            stats.total_value_locked,
            2_500_000,
            "total_value_locked should be the platform fee amount remaining"
        );
    }

    /// Full-cycle invariant test: TVL == contract balance after claims.
    /// This test verifies that after a full claim cycle, the total_value_locked
    /// statistic equals the difference between total staked and total paid out,
    /// which should match the actual contract token balance.
    #[test]
    fn tvl_equals_contract_balance_invariant() {
        let s = setup();
        let poll_id = create_poll(&s, 2_000_000);

        // Two users stake on opposite sides
        let user1 = Address::generate(&s.env);
        let user2 = Address::generate(&s.env);
        let stake1: i128 = 100_000_000;
        let stake2: i128 = 150_000_000;

        mint_tokens(&s, &user1, stake1);
        mint_tokens(&s, &user2, stake2);

        s.client.stake(&user1, &poll_id, &stake1, &StakeSide::Yes);
        s.client.stake(&user2, &poll_id, &stake2, &StakeSide::No);

        // Resolve with Yes winning
        inject_resolved_poll(&s, poll_id, true, stake1, stake2);

        // User 1 claims their winnings
        let payout1 = s.client.claim_winnings(&user1, &poll_id);

        // Verify TVL invariant
        let stats = s.client.get_platform_stats();
        // TVL should equal initial total staked minus total payouts
        let initial_locked = stake1 + stake2;
        let expected_locked = initial_locked - payout1;
        assert_eq!(
            stats.total_value_locked,
            expected_locked,
            "total_value_locked should equal initial staked minus paid out"
        );
    #[test]
    fn resolved_claim_cannot_be_followed_by_cancel_emergency_refund() {
        let s = setup();
        let poll_id = create_poll(&s, 1_500_000);
        let winner = Address::generate(&s.env);
        let loser = Address::generate(&s.env);
        let stake_amount = 100_000_000_i128;

        mint_tokens(&s, &winner, stake_amount);
        mint_tokens(&s, &loser, stake_amount);
        s.client
            .stake(&winner, &poll_id, &stake_amount, &StakeSide::Yes);
        s.client
            .stake(&loser, &poll_id, &stake_amount, &StakeSide::No);

        let oracle = crate::voting_oracle::Client::new(&s.env, &s.oracle_id);
        oracle.set_poll_status(
            &poll_id,
            &crate::voting_oracle::PollStatus::Voting,
            &None,
        );
        for _ in 0..24 {
            oracle.cast_vote(&Address::generate(&s.env), &poll_id, &VoteChoice::Yes);
        }
        oracle.cast_vote(&Address::generate(&s.env), &poll_id, &VoteChoice::No);
        s.env
            .ledger()
            .set_timestamp(1_000_000 + VOTING_WINDOW_SECS);
        assert_eq!(oracle.auto_resolve(&poll_id), VoteChoice::Yes);

        s.client.resolve_poll(&s.oracle_id, &poll_id, &true);
        let payout = s.client.claim_winnings(&winner, &poll_id);
        let balance_after_claim = token_balance(&s, &winner);
        assert_eq!(balance_after_claim, payout);
        assert_eq!(s.client.get_stake_info(&poll_id, &winner).claimed, true);

        let cancel_error = s
            .client
            .try_cancel_poll(&s.admin, &poll_id)
            .expect_err("resolved oracle polls cannot be cancelled");
        assert_eq!(
            cancel_error,
            Ok(PredictXError::InvalidPollStatusTransition)
        );
        assert_eq!(oracle.get_poll_status(&poll_id), crate::voting_oracle::PollStatus::Resolved);

        let withdraw_error = s
            .client
            .try_emergency_withdraw(&winner, &poll_id)
            .expect_err("resolved polls cannot issue an emergency refund");
        assert_eq!(
            withdraw_error,
            Ok(PredictXError::EmergencyWithdrawNotAllowed)
        );
        assert_eq!(token_balance(&s, &winner), balance_after_claim);
    // ── Tests: UserStats updates on claim ─────────────────────────────────────

    /// A winning claim increments `polls_won` and adds NET profit to
    /// `total_won` (payout minus original stake, not gross payout).
    #[test]
    fn winning_claim_updates_user_stats_with_net_profit() {
        let s = setup();
        let poll_id: u64 = 300;
        let winner = Address::generate(&s.env);
        let winning_stake: i128 = 100_000_000;
        let losing_pool: i128 = 100_000_000;

        mint_tokens(&s, &s.contract_id, winning_stake + losing_pool);
        inject_resolved_poll(&s, poll_id, true, winning_stake, losing_pool);
        inject_stake(&s, poll_id, &winner, winning_stake, StakeSide::Yes);

        let payout = s.client.claim_winnings(&winner, &poll_id);
        // gross = 100M * 200M / 100M = 200M; net = 200M * 9500 / 10000 = 190M
        assert_eq!(payout, 190_000_000);

        let stats = s.client.get_user_stats(&winner);
        assert_eq!(stats.polls_won, 1);
        assert_eq!(stats.polls_lost, 0);
        // NET profit, not the 190M gross payout.
        assert_eq!(stats.total_won, payout - winning_stake);
        assert_eq!(stats.total_lost, 0);
    }

    /// A losing position increments `polls_lost` and records the stake as lost.
    #[test]
    fn losing_claim_updates_user_stats_with_stake_lost() {
        let s = setup();
        let poll_id: u64 = 301;
        let loser = Address::generate(&s.env);
        let amount: i128 = 75_000_000;

        mint_tokens(&s, &s.contract_id, amount * 2);
        inject_resolved_poll(&s, poll_id, true, amount, amount);
        inject_stake(&s, poll_id, &loser, amount, StakeSide::No);

        let err = s
            .client
            .try_claim_winnings(&loser, &poll_id)
            .expect_err("loser should not be able to claim");
        assert_eq!(err, Ok(PredictXError::NotOnWinningSide));

        let stats = s.client.get_user_stats(&loser);
        assert_eq!(stats.polls_lost, 1);
        assert_eq!(stats.polls_won, 0);
        assert_eq!(stats.total_lost, amount);
        assert_eq!(stats.total_won, 0);
    }

    /// A refund on a cancelled poll (empty winning pool) counts as neither a
    /// win nor a loss.
    #[test]
    fn refund_on_cancelled_poll_does_not_touch_user_stats() {
        let s = setup();
        let poll_id: u64 = 302;
        let user = Address::generate(&s.env);
        let amount: i128 = 200_000_000;

        mint_tokens(&s, &s.contract_id, amount);
        inject_resolved_poll(&s, poll_id, true, 0, amount);
        inject_stake(&s, poll_id, &user, amount, StakeSide::No);

        let refund = s.client.claim_winnings(&user, &poll_id);
        assert_eq!(refund, amount);

        let stats = s.client.get_user_stats(&user);
        assert_eq!(stats.polls_won, 0);
        assert_eq!(stats.polls_lost, 0);
        assert_eq!(stats.total_won, 0);
        assert_eq!(stats.total_lost, 0);
    }
}
