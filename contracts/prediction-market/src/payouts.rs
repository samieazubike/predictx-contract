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
use predictx_shared::{
    Poll, PollStatus, PredictXError, Stake, StakeSide, UserStats, BPS_DENOMINATOR,
};
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

    let mut poll: Poll = env
        .storage()
        .persistent()
        .get(&DataKey::Poll(poll_id))
        .ok_or(PredictXError::PollNotFound)?;
    if poll.outcome.is_some() {
        return Err(PredictXError::PollAlreadyResolved);
    }

    poll.outcome = Some(outcome);
    poll.resolution_time = env.ledger().timestamp();
    // Validates the edge, persists the new status, and publishes
    // `PollStatusChanged`.  A rejected transition leaves the poll untouched.
    transition_poll_status(env, &mut poll, PollStatus::Resolved)?;

    let total_pool = poll
        .yes_pool
        .checked_add(poll.no_pool)
        .ok_or(PredictXError::ArithmeticOverflow)?;
    let fee = total_pool
        .checked_mul(token_utils::get_platform_fee_bps(env) as i128)
        .and_then(|value| value.checked_div(BPS_DENOMINATOR as i128))
        .ok_or(PredictXError::ArithmeticOverflow)?;
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
    let total_pool = poll
        .yes_pool
        .checked_add(poll.no_pool)
        .ok_or(PredictXError::ArithmeticOverflow)?;
    let fee_bps = token_utils::get_platform_fee_bps(env);
    let fee_factor = BPS_DENOMINATOR
        .checked_sub(fee_bps)
        .ok_or(PredictXError::ArithmeticOverflow)? as i128;
    let bps = BPS_DENOMINATOR as i128;

    let gross = stake
        .amount
        .checked_mul(total_pool)
        .and_then(|value| value.checked_div(winning_pool))
        .ok_or(PredictXError::ArithmeticOverflow)?;
    let net = gross
        .checked_mul(fee_factor)
        .and_then(|value| value.checked_div(bps))
        .ok_or(PredictXError::ArithmeticOverflow)?;
    if net == 0 {
        // The staker is on the winning side, so `NotOnWinningSide` would be a
        // lie. Report the truncation instead and leave the stake unclaimed —
        // the pool is frozen, so retrying cannot change the result.
        return Err(PredictXError::PayoutRoundsToZero);
    }

    // Derived, never a second division: net + fee == gross.
    Ok(Payout {
        net,
        fee: gross
            .checked_sub(net)
            .ok_or(PredictXError::ArithmeticOverflow)?,
    })
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
            polls_participated: 0,
            polls_won: 0,
            polls_lost: 0,
            votes_cast: 0,
            voting_rewards_earned: 0,
        })
}

fn save_user_stats(env: &Env, user: &Address, stats: &UserStats) {
    env.storage()
        .persistent()
        .set(&DataKey::UserStats(user.clone()), stats);
}

/// Record a winning claim. `net_profit` is payout minus original stake —
/// `total_won` tracks net profit, never gross payout.
fn record_win(env: &Env, user: &Address, net_profit: i128) {
    let mut stats = load_user_stats(env, user);
    stats.polls_won = stats.polls_won.saturating_add(1);
    stats.total_won = stats.total_won.saturating_add(net_profit);
    save_user_stats(env, user, &stats);
}

/// Record a losing position: the stake is forfeit and counts as lost.
fn record_loss(env: &Env, user: &Address, stake_amount: i128) {
    let mut stats = load_user_stats(env, user);
    stats.polls_lost = stats.polls_lost.saturating_add(1);
    stats.total_lost = stats.total_lost.saturating_add(stake_amount);
    save_user_stats(env, user, &stats);
}

/// Claim winnings (or a full stake refund) after a poll resolves.
///
/// ## Normal path
/// The caller must have staked on the winning side.  Their proportional share
/// of the total pool — minus the platform fee — is transferred to them and the
/// fee is sent to the treasury.
///
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
/// ## Payouts that truncate to zero
/// Returns [`PredictXError::PayoutRoundsToZero`] rather than transferring a
/// worthless amount, and leaves the stake unclaimed.
///
/// ## Stats accounting (issues #150, #151)
/// `total_won` records net profit (payout minus original stake), not gross
/// payout. Refunds (empty winning pool or no-contest, where `fee == 0` and
/// `net == stake.amount`) count as neither a win nor a loss. Losing stakes
/// record a loss before `NotOnWinningSide` is returned.
pub fn claim_winnings(env: &Env, claimant: Address, poll_id: u64) -> Result<i128, PredictXError> {
    claimant.require_auth();

    // ── Load & validate the poll and the stake ───────────────────────────────

    let poll = load_resolved_poll(env, poll_id)?;

    let mut stake: Stake = env
        .storage()
        .persistent()
        .get(&DataKey::Stake(poll_id, claimant.clone()))
        .ok_or(PredictXError::NotStaker)?;

    if stake.claimed {
        return Err(PredictXError::AlreadyClaimed);
    }

    // A stake that was already refunded through the emergency path is spent;
    // claiming on top of that would pay the same money out twice.
    if has_emergency_claimed(env, poll_id, &claimant) {
        return Err(PredictXError::AlreadyClaimed);
    }

    // ── Determine the payout — identical maths to the quote ──────────────────

    let payout = match payout_for(env, &poll, &stake) {
        Ok(payout) => payout,
        Err(PredictXError::NotOnWinningSide) => {
            record_loss(env, &claimant, stake.amount);
            return Err(PredictXError::NotOnWinningSide);
        }
        Err(other) => return Err(other),
    };

    // ── Mark claimed & persist ───────────────────────────────────────────────

    stake.claimed = true;
    env.storage()
        .persistent()
        .set(&DataKey::Stake(poll_id, claimant.clone()), &stake);

    // ── Transfer the platform fee, then the payout ──────────────────────────

    if payout.fee > 0 {
        token_utils::transfer_to_treasury(env, payout.fee)?;
    }
    token_utils::transfer_from_contract(env, &claimant, payout.net)?;

    // ── Update user stats ─────────────────────────────────────────────────────
    //
    // Refunds (empty winning pool or no-contest) return the stake whole with
    // no fee, so they count as neither a win nor a loss.
    if payout.fee > 0 {
        let net_profit = payout
            .net
            .checked_sub(stake.amount)
            .ok_or(PredictXError::ArithmeticOverflow)?;
        record_win(env, &claimant, net_profit);
    }

    // ── Update platform stats ─────────────────────────────────────────────────

    let mut stats = get_platform_stats(env);
    stats.total_value_locked = stats.total_value_locked.saturating_sub(payout.net);
    stats.total_payouts = stats.total_payouts.saturating_add(payout.net);
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

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod test {
    extern crate std;

    use crate::{DataKey, PredictionMarket, PredictionMarketClient};
    use predictx_shared::{
        Poll, PollCategory, PollStatus, PredictXError, Stake, StakeSide, MAX_STAKE_AMOUNT,
        MIN_STAKE_AMOUNT,
    };
    use soroban_sdk::{
        testutils::{Address as _, Ledger},
        token, Address, Env, String, Vec,
    };

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

    fn mint_tokens(s: &TestSetup, to: &Address, amount: i128) {
        let sac = token::StellarAssetClient::new(&s.env, &s.token_addr);
        sac.mint(to, &amount);
    }

    fn token_balance(s: &TestSetup, addr: &Address) -> i128 {
        token::Client::new(&s.env, &s.token_addr).balance(addr)
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
                resolution_time: 1_000_000,
                created_at: 900_000,
            };
            s.env
                .storage()
                .persistent()
                .set(&DataKey::Poll(poll_id), &poll);
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

    /// Mint, then stake through the real entry point.  Returns the staker.
    fn stake_user(s: &TestSetup, poll_id: u64, side: StakeSide, amount: i128) -> Address {
        let user = Address::generate(&s.env);
        mint_tokens(s, &user, amount);
        s.client.stake(&user, &poll_id, &amount, &side);
        user
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

        inject_resolved_poll(&s, poll_id, true, amount, amount);
        inject_stake(&s, poll_id, &user, amount, StakeSide::Yes);

        s.client.claim_winnings(&user, &poll_id);

        let err = s
            .client
            .try_claim_winnings(&user, &poll_id)
            .expect_err("second claim should fail");
        assert_eq!(err, Ok(PredictXError::AlreadyClaimed));
    }

    #[test]
    fn one_sided_poll_refunds_the_winner_at_par() {
        let s = setup();
        let poll_id = create_poll(&s, 2_000_000);
        let winner = stake_user(&s, poll_id, StakeSide::Yes, 100_000_000);
        resolve_via_state_machine(&s, poll_id, true);

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
        resolve_via_state_machine(&s, poll_id, false);

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
        let s = setup();
        let poll_id: u64 = 500;
        let yes_pool: i128 = 33_333_331;
        let no_pool: i128 = 66_666_667;
        let amounts: [i128; 3] = [11_111_111, 11_111_110, 11_111_110];
        let pot: i128 = yes_pool + no_pool;

        let treasury = s.client.get_treasury_address();
        mint_tokens(&s, &s.contract_id, pot);
        inject_resolved_poll(&s, poll_id, true, yes_pool, no_pool);

        let mut winners: Vec<Address> = Vec::new(&s.env);
        for amount in amounts {
            let user = Address::generate(&s.env);
            inject_stake(&s, poll_id, &user, amount, StakeSide::Yes);
            winners.push_back(user);
        }

        for i in 0..amounts.len() as u32 {
            let claimed = s.client.claim_winnings(&winners.get(i).unwrap(), &poll_id);
            assert_eq!(
                claimed,
                s.client
                    .calculate_winnings(&poll_id, &winners.get(i).unwrap())
            );
        }

        let paid = token_balance(&s, &winners.get(0).unwrap())
            + token_balance(&s, &winners.get(1).unwrap())
            + token_balance(&s, &winners.get(2).unwrap())
            + token_balance(&s, &treasury);
        assert!(paid <= pot, "payouts plus fees must stay inside the pot");
        assert_eq!(
            token_balance(&s, &s.contract_id) + paid,
            pot,
            "every base unit is either paid out or still held"
        );
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
    }
}
