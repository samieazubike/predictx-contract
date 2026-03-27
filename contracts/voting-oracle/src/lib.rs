#![no_std]

use predictx_shared::{PredictXError, PollStatus, ConfigKey, ConfigValue};
use soroban_sdk::{contract, contractimpl, contracttype, Address, Env, Symbol};

#[contract]
pub struct VotingOracle;

#[contracttype]
#[derive(Clone)]
struct StoredPollStatus {
    status: PollStatus,
    updated_at: u64,
}

#[contracttype]
#[derive(Clone)]
enum DataKey {
    Admin,
    PredictionMarketAddress,
    VotingWindowSecs,
    ConsensusThresholdBps,
    AdminReviewThresholdBps,
    DisputeWindowSecs,
    DisputeFee,
    ConfigVersion,
    PollStatus(u64),
}

fn get_admin(env: &Env) -> Result<Address, PredictXError> {
    env.storage()
        .instance()
        .get(&DataKey::Admin)
        .ok_or(PredictXError::NotInitialized)
}

#[contractimpl]
impl VotingOracle {
    pub fn initialize(
        env: Env,
        admin: Address,
        prediction_market_address: Address,
        voting_window_secs: u64,
        consensus_threshold_bps: u32,
        admin_review_threshold_bps: u32,
        dispute_window_secs: u64,
        dispute_fee: i128,
    ) -> Result<(), PredictXError> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(PredictXError::AlreadyInitialized);
        }

        // Validate parameters
        if voting_window_secs < 1800 || voting_window_secs > 14400 {
            return Err(PredictXError::InvalidLockTime);
        }
        if consensus_threshold_bps < 7000 || consensus_threshold_bps > 9500 {
            return Err(PredictXError::InvalidFee);
        }
        if admin_review_threshold_bps < 5000 || admin_review_threshold_bps > 8000 {
            return Err(PredictXError::InvalidFee);
        }
        if dispute_window_secs < 43200 || dispute_window_secs > 172800 {
            return Err(PredictXError::InvalidLockTime);
        }
        if dispute_fee < 0 {
            return Err(PredictXError::StakeAmountZero);
        }

        admin.require_auth();

        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::PredictionMarketAddress, &prediction_market_address);
        env.storage().instance().set(&DataKey::VotingWindowSecs, &voting_window_secs);
        env.storage().instance().set(&DataKey::ConsensusThresholdBps, &consensus_threshold_bps);
        env.storage().instance().set(&DataKey::AdminReviewThresholdBps, &admin_review_threshold_bps);
        env.storage().instance().set(&DataKey::DisputeWindowSecs, &dispute_window_secs);
        env.storage().instance().set(&DataKey::DisputeFee, &dispute_fee);
        env.storage().instance().set(&DataKey::ConfigVersion, &1u32);

        // Emit initialization event
        env.events().publish(
            (Symbol::new(&env, "Initialized"),),
            (admin, prediction_market_address, voting_window_secs, consensus_threshold_bps, admin_review_threshold_bps, dispute_window_secs, dispute_fee)
        );

        Ok(())
    }

    pub fn admin(env: Env) -> Result<Address, PredictXError> {
        get_admin(&env)
    }

    /// Placeholder oracle state setter.
    ///
    /// This exists only to validate cross-contract invocation patterns during
    /// Phase 1 scaffolding.
    pub fn set_poll_status(env: Env, poll_id: u64, status: PollStatus) -> Result<(), PredictXError> {
        let admin = get_admin(&env)?;
        admin.require_auth();

        let stored = StoredPollStatus {
            status,
            updated_at: env.ledger().timestamp(),
        };

        env.storage()
            .persistent()
            .set(&DataKey::PollStatus(poll_id), &stored);
        Ok(())
    }

    /// Placeholder oracle query used by `PredictionMarket`.
    pub fn get_poll_status(env: Env, poll_id: u64) -> PollStatus {
        let stored: Option<StoredPollStatus> = env
            .storage()
            .persistent()
            .get(&DataKey::PollStatus(poll_id));

        stored.map(|s| s.status).unwrap_or(PollStatus::Active)
    }

    pub fn get_poll_status_updated_at(env: Env, poll_id: u64) -> u64 {
        let stored: Option<StoredPollStatus> = env
            .storage()
            .persistent()
            .get(&DataKey::PollStatus(poll_id));

        stored.map(|s| s.updated_at).unwrap_or(0)
    }

    // ── Configuration Management ───────────────────────────────────────────────

    pub fn update_config(
        env: Env,
        admin: Address,
        key: ConfigKey,
        value: ConfigValue,
    ) -> Result<(), PredictXError> {
        let stored_admin = get_admin(&env)?;
        if admin != stored_admin {
            return Err(PredictXError::Unauthorized);
        }
        admin.require_auth();

        let old_value = match key {
            ConfigKey::VotingWindowSecs => {
                let old = env.storage().instance().get(&DataKey::VotingWindowSecs)
                    .ok_or(PredictXError::NotInitialized)?;
                if let ConfigValue::U64Value(new_window) = value {
                    if new_window < 1800 || new_window > 14400 {
                        return Err(PredictXError::InvalidLockTime);
                    }
                    env.storage().instance().set(&DataKey::VotingWindowSecs, &new_window);
                    env.events().publish(
                        (Symbol::new(&env, "ConfigUpdated"),),
                        (key, ConfigValue::U64Value(old), ConfigValue::U64Value(new_window))
                    );
                }
                ConfigValue::U64Value(old)
            },
            ConfigKey::ConsensusThresholdBps => {
                let old = env.storage().instance().get(&DataKey::ConsensusThresholdBps)
                    .ok_or(PredictXError::NotInitialized)?;
                if let ConfigValue::U32Value(new_threshold) = value {
                    if new_threshold < 7000 || new_threshold > 9500 {
                        return Err(PredictXError::InvalidFee);
                    }
                    env.storage().instance().set(&DataKey::ConsensusThresholdBps, &new_threshold);
                    env.events().publish(
                        (Symbol::new(&env, "ConfigUpdated"),),
                        (key, ConfigValue::U32Value(old), ConfigValue::U32Value(new_threshold))
                    );
                }
                ConfigValue::U32Value(old)
            },
            ConfigKey::AdminReviewThresholdBps => {
                let old = env.storage().instance().get(&DataKey::AdminReviewThresholdBps)
                    .ok_or(PredictXError::NotInitialized)?;
                if let ConfigValue::U32Value(new_threshold) = value {
                    if new_threshold < 5000 || new_threshold > 8000 {
                        return Err(PredictXError::InvalidFee);
                    }
                    env.storage().instance().set(&DataKey::AdminReviewThresholdBps, &new_threshold);
                    env.events().publish(
                        (Symbol::new(&env, "ConfigUpdated"),),
                        (key, ConfigValue::U32Value(old), ConfigValue::U32Value(new_threshold))
                    );
                }
                ConfigValue::U32Value(old)
            },
            ConfigKey::DisputeWindowSecs => {
                let old = env.storage().instance().get(&DataKey::DisputeWindowSecs)
                    .ok_or(PredictXError::NotInitialized)?;
                if let ConfigValue::U64Value(new_window) = value {
                    if new_window < 43200 || new_window > 172800 {
                        return Err(PredictXError::InvalidLockTime);
                    }
                    env.storage().instance().set(&DataKey::DisputeWindowSecs, &new_window);
                    env.events().publish(
                        (Symbol::new(&env, "ConfigUpdated"),),
                        (key, ConfigValue::U64Value(old), ConfigValue::U64Value(new_window))
                    );
                }
                ConfigValue::U64Value(old)
            },
            ConfigKey::DisputeFee => {
                let old = env.storage().instance().get(&DataKey::DisputeFee)
                    .ok_or(PredictXError::NotInitialized)?;
                if let ConfigValue::I128Value(new_fee) = value {
                    if new_fee < 0 {
                        return Err(PredictXError::StakeAmountZero);
                    }
                    env.storage().instance().set(&DataKey::DisputeFee, &new_fee);
                    env.events().publish(
                        (Symbol::new(&env, "ConfigUpdated"),),
                        (key, ConfigValue::I128Value(old), ConfigValue::I128Value(new_fee))
                    );
                }
                ConfigValue::I128Value(old)
            },
            _ => return Err(PredictXError::Unauthorized), // Other keys not supported in VotingOracle
        };

        // Increment config version
        let current_version: u32 = env.storage().instance()
            .get(&DataKey::ConfigVersion)
            .ok_or(PredictXError::NotInitialized)?;
        env.storage().instance().set(&DataKey::ConfigVersion, &(current_version + 1u32));

        Ok(())
    }

    // ── Cross-Contract Address Setting ───────────────────────────────────────────

    pub fn set_market_address(env: Env, admin: Address, market: Address) -> Result<(), PredictXError> {
        let stored_admin = get_admin(&env)?;
        if admin != stored_admin {
            return Err(PredictXError::Unauthorized);
        }
        admin.require_auth();
        env.storage().instance().set(&DataKey::PredictionMarketAddress, &market);
        env.events().publish((Symbol::new(&env, "MarketAddressUpdated"),), market);
        Ok(())
    }
}

#[cfg(test)]
extern crate std;

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::testutils::Address as _;

    #[test]
    fn set_and_get_status() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(VotingOracle, ());
        let client = VotingOracleClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let market = Address::generate(&env);
        client.initialize(&admin, &market, &7200u64, &8500u32, &6000u32, &86400u64, &10000000i128);

        client.set_poll_status(&42_u64, &PollStatus::Resolved);
        assert_eq!(client.get_poll_status(&42_u64), PollStatus::Resolved);
    }

    #[test]
    fn initialization_parameter_validation() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(VotingOracle, ());
        let client = VotingOracleClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let market = Address::generate(&env);

        // Test voting window too short
        let err = client.try_initialize(&admin, &market, &1799u64, &8500u32, &6000u32, &86400u64, &10000000i128)
            .expect_err("should fail");
        assert_eq!(err, Ok(PredictXError::InvalidLockTime));

        // Test voting window too long
        let err = client.try_initialize(&admin, &market, &14401u64, &8500u32, &6000u32, &86400u64, &10000000i128)
            .expect_err("should fail");
        assert_eq!(err, Ok(PredictXError::InvalidLockTime));

        // Test consensus threshold too low
        let err = client.try_initialize(&admin, &market, &7200u64, &6999u32, &6000u32, &86400u64, &10000000i128)
            .expect_err("should fail");
        assert_eq!(err, Ok(PredictXError::InvalidFee));

        // Test consensus threshold too high
        let err = client.try_initialize(&admin, &market, &7200u64, &9501u32, &6000u32, &86400u64, &10000000i128)
            .expect_err("should fail");
        assert_eq!(err, Ok(PredictXError::InvalidFee));

        // Test negative dispute fee
        let err = client.try_initialize(&admin, &market, &7200u64, &8500u32, &6000u32, &86400u64, &-1i128)
            .expect_err("should fail");
        assert_eq!(err, Ok(PredictXError::StakeAmountZero));
    }

    #[test]
    fn update_voting_window_within_range_succeeds() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(VotingOracle, ());
        let client = VotingOracleClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let market = Address::generate(&env);
        client.initialize(&admin, &market, &7200u64, &8500u32, &6000u32, &86400u64, &10000000i128);

        client.update_config(&admin, &ConfigKey::VotingWindowSecs, &ConfigValue::U64Value(3600));

        // Verify the change took effect by checking the updated timestamp logic
        // (In a real implementation, we'd query the config)
    }

    #[test]
    fn update_voting_window_outside_range_fails() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(VotingOracle, ());
        let client = VotingOracleClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let market = Address::generate(&env);
        client.initialize(&admin, &market, &7200u64, &8500u32, &6000u32, &86400u64, &10000000i128);

        let err = client.try_update_config(&admin, &ConfigKey::VotingWindowSecs, &ConfigValue::U64Value(1799))
            .expect_err("should fail");
        assert_eq!(err, Ok(PredictXError::InvalidLockTime));
    }

    #[test]
    fn update_config_by_non_admin_fails() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(VotingOracle, ());
        let client = VotingOracleClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let non_admin = Address::generate(&env);
        let market = Address::generate(&env);
        client.initialize(&admin, &market, &7200u64, &8500u32, &6000u32, &86400u64, &10000000i128);

        let err = client.try_update_config(&non_admin, &ConfigKey::VotingWindowSecs, &ConfigValue::U64Value(3600))
            .expect_err("should fail");
        assert_eq!(err, Ok(PredictXError::Unauthorized));
    }

    #[test]
    fn set_market_address_updates_cross_contract_reference() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(VotingOracle, ());
        let client = VotingOracleClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let market = Address::generate(&env);
        let new_market = Address::generate(&env);
        client.initialize(&admin, &market, &7200u64, &8500u32, &6000u32, &86400u64, &10000000i128);

        client.set_market_address(&admin, &new_market);

        // Verify market address was updated (call succeeds)
    }
}
