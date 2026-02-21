#![no_std]

use predictx_shared::{PlatformStats, PredictXError, PollStatus, Stake};
use soroban_sdk::{contract, contractimpl, contracttype, Address, Env, Symbol};

mod voting_oracle {
    soroban_sdk::contractimport!(file = "wasm/voting_oracle.wasm");
}

fn map_oracle_poll_status(status: voting_oracle::PollStatus) -> PollStatus {
    match status {
		voting_oracle::PollStatus::Active      => PollStatus::Active,
		voting_oracle::PollStatus::Locked      => PollStatus::Locked,
		voting_oracle::PollStatus::Voting      => PollStatus::Voting,
		voting_oracle::PollStatus::AdminReview => PollStatus::AdminReview,
		voting_oracle::PollStatus::Disputed    => PollStatus::Disputed,
		voting_oracle::PollStatus::Resolved    => PollStatus::Resolved,
		voting_oracle::PollStatus::Cancelled   => PollStatus::Cancelled,
    }
}

#[contract]
pub struct PredictionMarket;

#[contracttype]
#[derive(Clone)]
enum DataKey {
    Admin,
    VotingOracle,
    Paused,
    Stake(u64, Address),
    EmergencyClaimed(u64, Address),
    PlatformStats,
}

fn get_admin(env: &Env) -> Result<Address, PredictXError> {
    env.storage()
        .instance()
        .get(&DataKey::Admin)
        .ok_or(PredictXError::NotInitialized)
}

fn get_oracle(env: &Env) -> Result<Address, PredictXError> {
    env.storage()
        .instance()
        .get(&DataKey::VotingOracle)
        .ok_or(PredictXError::NotInitialized)
}

fn is_paused(env: &Env) -> bool {
    env.storage()
        .instance()
        .get(&DataKey::Paused)
        .unwrap_or(false)
}

fn ensure_not_paused(env: &Env) -> Result<(), PredictXError> {
    if is_paused(env) {
        return Err(PredictXError::ContractPaused);
    }
    Ok(())
}

fn get_platform_stats(env: &Env) -> PlatformStats {
    env.storage()
        .instance()
        .get(&DataKey::PlatformStats)
        .unwrap_or(PlatformStats {
            total_value_locked: 0,
            total_polls_created: 0,
            total_stakes_placed: 0,
            total_payouts: 0,
            total_users: 0,
        })
}

fn set_platform_stats(env: &Env, stats: &PlatformStats) {
    env.storage().instance().set(&DataKey::PlatformStats, stats);
}

fn get_stake(env: &Env, poll_id: u64, user: &Address) -> Option<Stake> {
    env.storage()
        .persistent()
        .get(&DataKey::Stake(poll_id, user.clone()))
}

fn has_emergency_claimed(env: &Env, poll_id: u64, user: &Address) -> bool {
    env.storage()
        .persistent()
        .get(&DataKey::EmergencyClaimed(poll_id, user.clone()))
        .unwrap_or(false)
}

fn set_emergency_claimed(env: &Env, poll_id: u64, user: &Address) {
    env.storage()
        .persistent()
        .set(&DataKey::EmergencyClaimed(poll_id, user.clone()), &true);
}

const EMERGENCY_TIMEOUT_SECS: u64 = 7 * 24 * 60 * 60;

#[contractimpl]
impl PredictionMarket {
    pub fn initialize(env: Env, admin: Address, voting_oracle: Address) -> Result<(), PredictXError> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(PredictXError::AlreadyInitialized);
        }

        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage()
            .instance()
            .set(&DataKey::VotingOracle, &voting_oracle);

        Ok(())
    }

    pub fn admin(env: Env) -> Result<Address, PredictXError> {
        get_admin(&env)
    }

    pub fn oracle(env: Env) -> Result<Address, PredictXError> {
        get_oracle(&env)
    }

    pub fn set_oracle(env: Env, voting_oracle: Address) -> Result<(), PredictXError> {
        ensure_not_paused(&env)?;
        let admin = get_admin(&env)?;
        admin.require_auth();

        env.storage()
            .instance()
            .set(&DataKey::VotingOracle, &voting_oracle);
        Ok(())
    }

    pub fn pause(env: Env, admin: Address) -> Result<(), PredictXError> {
        let stored_admin = get_admin(&env)?;
        if admin != stored_admin {
            return Err(PredictXError::Unauthorized);
        }
        admin.require_auth();

        env.storage().instance().set(&DataKey::Paused, &true);

        let topic = Symbol::new(&env, "ContractPaused");
        env.events().publish((topic,), true);

        Ok(())
    }

    pub fn unpause(env: Env, admin: Address) -> Result<(), PredictXError> {
        let stored_admin = get_admin(&env)?;
        if admin != stored_admin {
            return Err(PredictXError::Unauthorized);
        }
        admin.require_auth();

        env.storage().instance().set(&DataKey::Paused, &false);

        let topic = Symbol::new(&env, "ContractUnpaused");
        env.events().publish((topic,), true);

        Ok(())
    }

    pub fn is_paused(env: Env) -> bool {
        is_paused(&env)
    }

    /// Minimal cross-contract invocation example.
    ///
    /// Calls into the `VotingOracle` contract to fetch the current status for a poll.
    pub fn oracle_poll_status(env: Env, poll_id: u64) -> Result<PollStatus, PredictXError> {
        let oracle_id = get_oracle(&env)?;
        let client = voting_oracle::Client::new(&env, &oracle_id);
        Ok(map_oracle_poll_status(client.get_poll_status(&poll_id)))
    }

    pub fn cancel_poll(env: Env, admin: Address, poll_id: u64) -> Result<(), PredictXError> {
        ensure_not_paused(&env)?;

        let stored_admin = get_admin(&env)?;
        if admin != stored_admin {
            return Err(PredictXError::Unauthorized);
        }
        admin.require_auth();

        let oracle_id = get_oracle(&env)?;
        let client = voting_oracle::Client::new(&env, &oracle_id);
        client.set_poll_status(&poll_id, &voting_oracle::PollStatus::Cancelled);

        let topic = Symbol::new(&env, "PollCancelled");
        env.events().publish((topic,), poll_id);

        Ok(())
    }

    pub fn check_emergency_eligible(env: Env, poll_id: u64) -> bool {
        let oracle_id = match get_oracle(&env) {
            Ok(id) => id,
            Err(_) => return false,
        };

        let client = voting_oracle::Client::new(&env, &oracle_id);
        let status = map_oracle_poll_status(client.get_poll_status(&poll_id));

        if status == PollStatus::Cancelled {
            return true;
        }

        if status != PollStatus::Disputed && status != PollStatus::Locked {
            return false;
        }

        let updated_at = client.get_poll_status_updated_at(&poll_id);
        if updated_at == 0 {
            return false;
        }

        let now = env.ledger().timestamp();
        now.saturating_sub(updated_at) >= EMERGENCY_TIMEOUT_SECS
    }

    pub fn emergency_withdraw(env: Env, user: Address, poll_id: u64) -> Result<i128, PredictXError> {
        user.require_auth();

        if has_emergency_claimed(&env, poll_id, &user) {
            return Err(PredictXError::AlreadyClaimed);
        }

        let oracle_id = get_oracle(&env)?;
        let client = voting_oracle::Client::new(&env, &oracle_id);
        let status = map_oracle_poll_status(client.get_poll_status(&poll_id));

        let eligible = if status == PollStatus::Cancelled {
            true
        } else if status == PollStatus::Disputed || status == PollStatus::Locked {
            let updated_at = client.get_poll_status_updated_at(&poll_id);
            if updated_at == 0 {
                false
            } else {
                let now = env.ledger().timestamp();
                now.saturating_sub(updated_at) >= EMERGENCY_TIMEOUT_SECS
            }
        } else {
            false
        };

        if !eligible {
            return Err(PredictXError::EmergencyWithdrawNotAllowed);
        }

        let stake = get_stake(&env, poll_id, &user).ok_or(PredictXError::NotStaker)?;

        set_emergency_claimed(&env, poll_id, &user);

        let mut stats = get_platform_stats(&env);
        stats.total_value_locked -= stake.amount;
        set_platform_stats(&env, &stats);

        let topic = Symbol::new(&env, "EmergencyWithdrawal");
        env.events()
            .publish((topic, poll_id, user.clone()), stake.amount);

        Ok(stake.amount)
    }
}

#[cfg(test)]
extern crate std;

#[cfg(test)]
mod test {
    use super::*;
    use predictx_shared::StakeSide;
    use soroban_sdk::testutils::{Address as _, Ledger};

    #[test]
    fn initialize_sets_admin_and_oracle() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(PredictionMarket, ());
        let client = PredictionMarketClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let oracle = Address::generate(&env);

        client.initialize(&admin, &oracle);
        assert_eq!(client.admin(), admin);
        assert_eq!(client.oracle(), oracle);
    }

    #[test]
    fn initialize_is_one_time_only() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(PredictionMarket, ());
        let client = PredictionMarketClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let oracle = Address::generate(&env);

        client.initialize(&admin, &oracle);
        let err = client
            .try_initialize(&admin, &oracle)
            .expect_err("should fail");
        assert_eq!(err, Ok(PredictXError::AlreadyInitialized));
    }

    #[test]
    fn cross_contract_oracle_call_works() {
        let env = Env::default();
        env.mock_all_auths();

        let admin = Address::generate(&env);

        // Register VotingOracle from WASM (imported via `contractimport!`).
        let oracle_id = env.register(voting_oracle::WASM, ());
        let oracle_client = voting_oracle::Client::new(&env, &oracle_id);
        oracle_client.initialize(&admin);
        oracle_client.set_poll_status(&7_u64, &voting_oracle::PollStatus::Resolved);

        // Register PredictionMarket natively and point it at the oracle contract.
        let contract_id = env.register(PredictionMarket, ());
        let client = PredictionMarketClient::new(&env, &contract_id);
        client.initialize(&admin, &oracle_id);

        let status = client.oracle_poll_status(&7_u64);
        assert_eq!(status, PollStatus::Resolved);
    }

    #[test]
    fn pause_and_unpause_toggle_contract_state() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(PredictionMarket, ());
        let client = PredictionMarketClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let oracle = Address::generate(&env);
        client.initialize(&admin, &oracle);

        assert_eq!(client.is_paused(), false);
        client.pause(&admin);
        assert_eq!(client.is_paused(), true);

        let err = client
            .try_set_oracle(&oracle)
            .expect_err("set_oracle should be blocked when paused");
        assert_eq!(err, Ok(PredictXError::ContractPaused));

        client.unpause(&admin);
        assert_eq!(client.is_paused(), false);
    }

    #[test]
    fn cancel_poll_sets_cancelled_status_and_emits_event() {
        let env = Env::default();
        env.mock_all_auths();

        let admin = Address::generate(&env);

        let oracle_id = env.register(voting_oracle::WASM, ());
        let oracle_client = voting_oracle::Client::new(&env, &oracle_id);
        oracle_client.initialize(&admin);

        let contract_id = env.register(PredictionMarket, ());
        let client = PredictionMarketClient::new(&env, &contract_id);
        client.initialize(&admin, &oracle_id);

        client.cancel_poll(&admin, &1_u64);

        assert_eq!(
            oracle_client.get_poll_status(&1_u64),
            voting_oracle::PollStatus::Cancelled
        );
    }

    #[test]
    fn emergency_withdraw_on_cancelled_poll_refunds_stake() {
        let env = Env::default();
        env.mock_all_auths();

        let admin = Address::generate(&env);

        let oracle_id = env.register(voting_oracle::WASM, ());
        let oracle_client = voting_oracle::Client::new(&env, &oracle_id);
        oracle_client.initialize(&admin);

        let contract_id = env.register(PredictionMarket, ());
        let client = PredictionMarketClient::new(&env, &contract_id);
        client.initialize(&admin, &oracle_id);

        let user = Address::generate(&env);
        let stake = Stake {
            user: user.clone(),
            poll_id: 10,
            amount: 50,
            side: StakeSide::Yes,
            claimed: false,
            staked_at: env.ledger().timestamp(),
        };

        env.as_contract(&contract_id, || {
            env.storage()
                .persistent()
                .set(&DataKey::Stake(10, user.clone()), &stake);
        });

        client.cancel_poll(&admin, &10_u64);

        let refunded = client.emergency_withdraw(&user, &10_u64);
        assert_eq!(refunded, 50);
    }

    #[test]
    fn emergency_withdraw_after_dispute_timeout() {
        let env = Env::default();
        env.mock_all_auths();

        let admin = Address::generate(&env);

        let oracle_id = env.register(voting_oracle::WASM, ());
        let oracle_client = voting_oracle::Client::new(&env, &oracle_id);
        oracle_client.initialize(&admin);

        env.ledger().set_timestamp(100);
        oracle_client.set_poll_status(&5_u64, &voting_oracle::PollStatus::Disputed);

        let contract_id = env.register(PredictionMarket, ());
        let client = PredictionMarketClient::new(&env, &contract_id);
        client.initialize(&admin, &oracle_id);

        let user = Address::generate(&env);
        let stake = Stake {
            user: user.clone(),
            poll_id: 5,
            amount: 25,
            side: StakeSide::No,
            claimed: false,
            staked_at: env.ledger().timestamp(),
        };

        env.as_contract(&contract_id, || {
            env.storage()
                .persistent()
                .set(&DataKey::Stake(5, user.clone()), &stake);
        });

        env.ledger()
            .set_timestamp(100 + EMERGENCY_TIMEOUT_SECS + 1);

        assert!(client.check_emergency_eligible(&5_u64));

        let refunded = client.emergency_withdraw(&user, &5_u64);
        assert_eq!(refunded, 25);
    }

    #[test]
    fn emergency_withdraw_rejected_before_timeout() {
        let env = Env::default();
        env.mock_all_auths();

        let admin = Address::generate(&env);

        let oracle_id = env.register(voting_oracle::WASM, ());
        let oracle_client = voting_oracle::Client::new(&env, &oracle_id);
        oracle_client.initialize(&admin);

        env.ledger().set_timestamp(200);
        oracle_client.set_poll_status(&2_u64, &voting_oracle::PollStatus::Locked);

        let contract_id = env.register(PredictionMarket, ());
        let client = PredictionMarketClient::new(&env, &contract_id);
        client.initialize(&admin, &oracle_id);

        let user = Address::generate(&env);
        let stake = Stake {
            user: user.clone(),
            poll_id: 2,
            amount: 30,
            side: StakeSide::Yes,
            claimed: false,
            staked_at: env.ledger().timestamp(),
        };

        env.as_contract(&contract_id, || {
            env.storage()
                .persistent()
                .set(&DataKey::Stake(2, user.clone()), &stake);
        });

        env.ledger()
            .set_timestamp(200 + EMERGENCY_TIMEOUT_SECS - 1);

        assert!(!client.check_emergency_eligible(&2_u64));

        let err = client
            .try_emergency_withdraw(&user, &2_u64)
            .expect_err("should reject before timeout");
        assert_eq!(err, Ok(PredictXError::EmergencyWithdrawNotAllowed));
    }

    #[test]
    fn emergency_withdraw_prevents_double_withdrawal() {
        let env = Env::default();
        env.mock_all_auths();

        let admin = Address::generate(&env);

        let oracle_id = env.register(voting_oracle::WASM, ());
        let oracle_client = voting_oracle::Client::new(&env, &oracle_id);
        oracle_client.initialize(&admin);

        env.ledger().set_timestamp(300);
        oracle_client.set_poll_status(&3_u64, &voting_oracle::PollStatus::Disputed);

        let contract_id = env.register(PredictionMarket, ());
        let client = PredictionMarketClient::new(&env, &contract_id);
        client.initialize(&admin, &oracle_id);

        let user = Address::generate(&env);
        let stake = Stake {
            user: user.clone(),
            poll_id: 3,
            amount: 40,
            side: StakeSide::No,
            claimed: false,
            staked_at: env.ledger().timestamp(),
        };

        env.as_contract(&contract_id, || {
            env.storage()
                .persistent()
                .set(&DataKey::Stake(3, user.clone()), &stake);
        });

        env.ledger()
            .set_timestamp(300 + EMERGENCY_TIMEOUT_SECS + 1);

        let refunded = client.emergency_withdraw(&user, &3_u64);
        assert_eq!(refunded, 40);

        let err = client
            .try_emergency_withdraw(&user, &3_u64)
            .expect_err("second withdrawal should fail");
        assert_eq!(err, Ok(PredictXError::AlreadyClaimed));
    }
}
