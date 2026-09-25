use crate::DataKey;
use predictx_shared::{Dispute, PollStatus, PredictXError, VoteChoice, MULTI_SIG_REQUIRED};
use soroban_sdk::{token, Address, Env, String, Symbol};

// ── Storage helpers ───────────────────────────────────────────────────────────

/// Read the dispute opened for a poll, if any.
fn read_dispute(env: &Env, poll_id: u64) -> Option<Dispute> {
    env.storage().persistent().get(&DataKey::Dispute(poll_id))
}

/// Persist a dispute for a poll.
fn write_dispute(env: &Env, dispute: &Dispute) {
    env.storage()
        .persistent()
        .set(&DataKey::Dispute(dispute.poll_id), dispute);
}

/// Read the configured payout token, if one has been set.
fn payout_token(env: &Env) -> Result<Address, PredictXError> {
    env.storage()
        .instance()
        .get(&DataKey::TokenAddress)
        .ok_or(PredictXError::NotInitialized)
}

/// Read the configured treasury address, if one has been set.
fn treasury_address(env: &Env) -> Result<Address, PredictXError> {
    env.storage()
        .instance()
        .get(&DataKey::TreasuryAddress)
        .ok_or(PredictXError::NotInitialized)
}

/// Transfer `amount` of the payout token from this contract to `to`.
fn transfer_from_contract(env: &Env, to: &Address, amount: i128) -> Result<(), PredictXError> {
    let client = token::Client::new(env, &payout_token(env)?);
    client.transfer(&env.current_contract_address(), to, &amount);
    Ok(())
}

/// Overwrite the poll's stored status.
fn write_status(env: &Env, poll_id: u64, status: PollStatus) {
    let stored = crate::StoredPollStatus {
        status,
        updated_at: env.ledger().timestamp(),
    };
    env.storage()
        .persistent()
        .set(&DataKey::PollStatus(poll_id), &stored);
}

// ── Dispute lifecycle ─────────────────────────────────────────────────────────

/// Open a dispute against a settled poll, escrowing `dispute_fee`.
///
/// The fee is what makes a challenge costly: it is refunded in full when the
/// ruling goes the challenger's way, and forfeited to the treasury when it does
/// not (see [`resolve_dispute`]).
///
/// Flow (Checks → Interactions → Effects):
/// 1. Authenticates the caller as the initiator.
/// 2. Rejects a missing or non-positive fee, else `DisputeFeeRequired`.
/// 3. Rejects polls that are not settled, else `PollNotActive`.
/// 4. Rejects a second open dispute on the same poll, else `DisputeAlreadyOpen`.
/// 5. Escrows the fee, then records the `Dispute` and moves the poll to
///    `Disputed`.
pub fn initiate_dispute(
    env: &Env,
    initiator: Address,
    poll_id: u64,
    evidence_hash: String,
    dispute_fee: i128,
) -> Result<(), PredictXError> {
    initiator.require_auth();

    // ── Checks ────────────────────────────────────────────────────────────────

    if dispute_fee <= 0 {
        return Err(PredictXError::DisputeFeeRequired);
    }

    if !env
        .storage()
        .persistent()
        .has(&DataKey::PollStatus(poll_id))
    {
        return Err(PredictXError::PollNotFound);
    }

    // Only a settled outcome can be challenged.
    if crate::read_poll_status(env, poll_id) != PollStatus::Resolved {
        return Err(PredictXError::PollNotActive);
    }

    // One poll, one open dispute.
    if let Some(existing) = read_dispute(env, poll_id) {
        if !existing.resolved {
            return Err(PredictXError::DisputeAlreadyOpen);
        }
    }

    // ── Interactions ──────────────────────────────────────────────────────────

    // Escrow the fee in this contract so it is available for either outcome.
    let client = token::Client::new(env, &payout_token(env)?);
    client.transfer(&initiator, &env.current_contract_address(), &dispute_fee);

    // ── Effects ───────────────────────────────────────────────────────────────

    let dispute = Dispute {
        poll_id,
        initiator: initiator.clone(),
        evidence_hash,
        dispute_fee,
        admin_approvals: 0,
        required_approvals: MULTI_SIG_REQUIRED,
        resolved: false,
        initiated_at: env.ledger().timestamp(),
    };
    write_dispute(env, &dispute);
    write_status(env, poll_id, PollStatus::Disputed);

    env.events().publish(
        (Symbol::new(env, "DisputeInitiated"), poll_id, initiator),
        dispute_fee,
    );

    Ok(())
}

/// Rule on an open dispute, refunding or forfeiting the escrowed fee.
///
/// The fee follows the ruling:
/// - the final outcome **differs** from the original → the dispute is upheld and
///   the fee is refunded to the initiator in full;
/// - the final outcome is **unchanged** → the dispute is rejected and the fee is
///   forfeited to the treasury.
///
/// The fee moves exactly once: the `resolved` flag is written before any
/// transfer, so a second ruling is rejected.
///
/// TODO(#94): gate on `MULTI_SIG_REQUIRED` agreeing admin approvals rather than
/// a single admin. `Dispute::required_approvals` already records the threshold.
pub fn resolve_dispute(
    env: &Env,
    admin: Address,
    poll_id: u64,
    final_outcome: VoteChoice,
) -> Result<(), PredictXError> {
    crate::storage::require_admin(env, &admin)?;
    admin.require_auth();

    // ── Checks ────────────────────────────────────────────────────────────────

    let mut dispute = read_dispute(env, poll_id).ok_or(PredictXError::PollNotFound)?;
    if dispute.resolved {
        return Err(PredictXError::PollAlreadyResolved);
    }

    let original = env
        .storage()
        .persistent()
        .get::<DataKey, VoteChoice>(&DataKey::PollOutcome(poll_id))
        .ok_or(PredictXError::PollNotFound)?;
    let upheld = final_outcome != original;

    // ── Effects ───────────────────────────────────────────────────────────────

    // Mark the dispute closed *before* moving the fee so it cannot be paid out
    // twice.
    dispute.resolved = true;
    write_dispute(env, &dispute);

    env.storage()
        .persistent()
        .set(&DataKey::PollOutcome(poll_id), &final_outcome);
    write_status(env, poll_id, PollStatus::Resolved);

    // ── Interactions ──────────────────────────────────────────────────────────

    if upheld {
        transfer_from_contract(env, &dispute.initiator, dispute.dispute_fee)?;
    } else {
        transfer_from_contract(env, &treasury_address(env)?, dispute.dispute_fee)?;
    }

    env.events().publish(
        (Symbol::new(env, "DisputeResolved"), poll_id, final_outcome),
        upheld,
    );
    env.events().publish(
        (Symbol::new(env, "DisputeFeeSettled"), poll_id, upheld),
        dispute.dispute_fee,
    );

    Ok(())
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod test {
    extern crate std;

    use predictx_shared::{PollStatus, PredictXError, VoteChoice};
    use soroban_sdk::{
        testutils::Address as _,
        token, Address, Env, String,
    };

    use crate::{VotingOracle, VotingOracleClient};

    /// Fee escrowed by the challenger in every test.
    const FEE: i128 = 500;

    struct TestSetup<'a> {
        env: Env,
        admin: Address,
        initiator: Address,
        treasury: Address,
        token_address: Address,
        client: VotingOracleClient<'a>,
    }

    /// Register the oracle, configure token + treasury, and park poll 1 as
    /// `Resolved` with an original outcome of `Yes`.
    fn setup() -> TestSetup<'static> {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(VotingOracle, ());
        let client = VotingOracleClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        client.initialize(&admin);

        let token_admin = Address::generate(&env);
        let token_contract = env.register_stellar_asset_contract_v2(token_admin);
        let token_address = token_contract.address();
        client.set_token_address(&admin, &token_address);

        let treasury = Address::generate(&env);
        client.set_treasury_address(&admin, &treasury);

        let initiator = Address::generate(&env);
        token::StellarAssetClient::new(&env, &token_address).mint(&initiator, &FEE);

        client.set_poll_status(&1_u64, &PollStatus::Resolved);
        env.as_contract(&client.address, || {
            env.storage()
                .persistent()
                .set(&crate::DataKey::PollOutcome(1), &VoteChoice::Yes);
        });

        TestSetup {
            env,
            admin,
            initiator,
            treasury,
            token_address,
            client,
        }
    }

    fn token_balance(s: &TestSetup, who: &Address) -> i128 {
        token::Client::new(&s.env, &s.token_address).balance(who)
    }

    fn raise_dispute(s: &TestSetup) {
        s.client.initiate_dispute(
            &s.initiator,
            &1_u64,
            &String::from_str(&s.env, "ipfs://evidence"),
            &FEE,
        );
    }

    #[test]
    fn upheld_dispute_refunds_the_initiator() {
        let s = setup();
        raise_dispute(&s);

        // The fee is escrowed while the dispute is open.
        assert_eq!(token_balance(&s, &s.client.address), FEE);
        assert_eq!(token_balance(&s, &s.initiator), 0);

        // Ruling `No` against an original `Yes` upholds the dispute.
        s.client.resolve_dispute(&s.admin, &1_u64, &VoteChoice::No);

        assert_eq!(token_balance(&s, &s.initiator), FEE);
        assert_eq!(token_balance(&s, &s.treasury), 0);
        assert_eq!(token_balance(&s, &s.client.address), 0);
    }

    #[test]
    fn rejected_dispute_forfeits_the_fee_to_the_treasury() {
        let s = setup();
        raise_dispute(&s);

        // Ruling the original outcome rejects the dispute.
        s.client.resolve_dispute(&s.admin, &1_u64, &VoteChoice::Yes);

        assert_eq!(token_balance(&s, &s.treasury), FEE);
        assert_eq!(token_balance(&s, &s.initiator), 0);
        assert_eq!(token_balance(&s, &s.client.address), 0);
    }

    #[test]
    fn fee_is_moved_exactly_once() {
        let s = setup();
        raise_dispute(&s);
        s.client.resolve_dispute(&s.admin, &1_u64, &VoteChoice::No);

        let initiator_after = token_balance(&s, &s.initiator);
        let treasury_after = token_balance(&s, &s.treasury);

        let err = s
            .client
            .try_resolve_dispute(&s.admin, &1_u64, &VoteChoice::Yes)
            .expect_err("a resolved dispute cannot be ruled on again");

        assert_eq!(err, Ok(PredictXError::PollAlreadyResolved));
        assert_eq!(token_balance(&s, &s.initiator), initiator_after);
        assert_eq!(token_balance(&s, &s.treasury), treasury_after);
    }

    #[test]
    fn initiate_dispute_requires_a_positive_fee() {
        let s = setup();

        let err = s
            .client
            .try_initiate_dispute(
                &s.initiator,
                &1_u64,
                &String::from_str(&s.env, "ipfs://evidence"),
                &0_i128,
            )
            .expect_err("a dispute with no fee must be rejected");

        assert_eq!(err, Ok(PredictXError::DisputeFeeRequired));
        assert_eq!(token_balance(&s, &s.client.address), 0);
    }
}
