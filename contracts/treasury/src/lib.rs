#![no_std]

use predictx_shared::PredictXError;
use soroban_sdk::{contract, contractimpl, contracttype, token, Address, Env};

#[contract]
pub struct Treasury;

#[contracttype]
#[derive(Clone)]
enum DataKey {
    Admin,
    Market,
    Oracle,
    TokenAddress,
    Balance(Address),
    VoterRewardsFunded(u64),
}

fn get_admin(env: &Env) -> Result<Address, PredictXError> {
    env.storage()
        .instance()
        .get(&DataKey::Admin)
        .ok_or(PredictXError::NotInitialized)
}

fn get_market(env: &Env) -> Result<Address, PredictXError> {
    env.storage()
        .instance()
        .get(&DataKey::Market)
        .ok_or(PredictXError::NotInitialized)
}

fn get_oracle(env: &Env) -> Result<Address, PredictXError> {
    env.storage()
        .instance()
        .get(&DataKey::Oracle)
        .ok_or(PredictXError::NotInitialized)
}

fn get_balance(env: &Env, who: &Address) -> i128 {
    env.storage()
        .persistent()
        .get(&DataKey::Balance(who.clone()))
        .unwrap_or(0_i128)
}

#[contractimpl]
impl Treasury {
    pub fn initialize(env: Env, admin: Address) -> Result<(), PredictXError> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(PredictXError::AlreadyInitialized);
        }

        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
        Ok(())
    }

    pub fn admin(env: Env) -> Result<Address, PredictXError> {
        get_admin(&env)
    }

    /// Returns the registered market address, if set.
    pub fn market(env: Env) -> Result<Address, PredictXError> {
        get_market(&env)
    }

    /// Admin-gated setter for the registered market address.
    pub fn set_market(env: Env, admin: Address, market: Address) -> Result<(), PredictXError> {
        let stored_admin = get_admin(&env)?;
        if admin != stored_admin {
            return Err(PredictXError::Unauthorized);
        }
        admin.require_auth();
        env.storage().instance().set(&DataKey::Market, &market);
        Ok(())
    }

    /// Admin-gated setter for the registered VotingOracle address.
    pub fn set_oracle(env: Env, admin: Address, oracle: Address) -> Result<(), PredictXError> {
        let stored_admin = get_admin(&env)?;
        if admin != stored_admin {
            return Err(PredictXError::Unauthorized);
        }
        admin.require_auth();
        env.storage().instance().set(&DataKey::Oracle, &oracle);
        Ok(())
    }

    /// Admin-gated setter for the token used to fund voter rewards.
    pub fn set_token(
        env: Env,
        admin: Address,
        token_address: Address,
    ) -> Result<(), PredictXError> {
        let stored_admin = get_admin(&env)?;
        if admin != stored_admin {
            return Err(PredictXError::Unauthorized);
        }
        admin.require_auth();
        env.storage()
            .instance()
            .set(&DataKey::TokenAddress, &token_address);
        Ok(())
    }

    /// Placeholder accounting method.
    ///
    /// Real token transfers are integrated in later issues.
    pub fn deposit(env: Env, from: Address, amount: i128) -> Result<i128, PredictXError> {
        if amount <= 0 {
            return Err(PredictXError::StakeAmountZero);
        }
        if !env.storage().instance().has(&DataKey::Admin) {
            return Err(PredictXError::NotInitialized);
        }
        from.require_auth();

        let new_balance = get_balance(&env, &from) + amount;
        env.storage()
            .persistent()
            .set(&DataKey::Balance(from), &new_balance);
        Ok(new_balance)
    }

    /// Deposit fees — only callable by the registered PredictionMarket contract.
    ///
    /// Any address other than the registered market receives `Unauthorized`.
    pub fn deposit_fees(env: Env, from: Address, amount: i128) -> Result<i128, PredictXError> {
        if amount <= 0 {
            return Err(PredictXError::StakeAmountZero);
        }
        if !env.storage().instance().has(&DataKey::Admin) {
            return Err(PredictXError::NotInitialized);
        }

        let registered_market = get_market(&env)?;
        if from != registered_market {
            return Err(PredictXError::Unauthorized);
        }

        from.require_auth();

        let new_balance = get_balance(&env, &from) + amount;
        env.storage()
            .persistent()
            .set(&DataKey::Balance(from), &new_balance);
        Ok(new_balance)
    }

    /// Release voter rewards to the registered VotingOracle, once per poll.
    pub fn fund_voter_rewards(
        env: Env,
        caller: Address,
        poll_id: u64,
        amount: i128,
    ) -> Result<i128, PredictXError> {
        if amount <= 0 {
            return Err(PredictXError::StakeAmountZero);
        }

        let registered_oracle = get_oracle(&env)?;
        if caller != registered_oracle {
            return Err(PredictXError::Unauthorized);
        }
        caller.require_auth();

        let funding_key = DataKey::VoterRewardsFunded(poll_id);
        if env.storage().persistent().has(&funding_key) {
            return Err(PredictXError::VoterRewardsAlreadyFunded);
        }

        let token_address: Address = env
            .storage()
            .instance()
            .get(&DataKey::TokenAddress)
            .ok_or(PredictXError::NotInitialized)?;
        let token_client = token::Client::new(&env, &token_address);
        let treasury_address = env.current_contract_address();
        if token_client.balance(&treasury_address) < amount {
            return Err(PredictXError::InsufficientBalance);
        }

        token_client.transfer(&treasury_address, &registered_oracle, &amount);
        env.storage().persistent().set(&funding_key, &amount);
        Ok(amount)
    }

    /// Return the amount released to the VotingOracle for a poll, or zero.
    pub fn funded_voter_rewards(env: Env, poll_id: u64) -> i128 {
        env.storage()
            .persistent()
            .get(&DataKey::VoterRewardsFunded(poll_id))
            .unwrap_or(0_i128)
    }

    pub fn balance(env: Env, who: Address) -> Result<i128, PredictXError> {
        if !env.storage().instance().has(&DataKey::Admin) {
            return Err(PredictXError::NotInitialized);
        }
        Ok(get_balance(&env, &who))
    }
}

#[cfg(test)]
extern crate std;

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::testutils::Address as _;

    fn setup() -> (
        Env,
        Address,
        TreasuryClient<'static>,
        Address,
        Address,
        Address,
    ) {
        let env = Env::default();
        env.mock_all_auths();

        let admin = Address::generate(&env);
        let oracle = Address::generate(&env);
        let token_admin = Address::generate(&env);
        let token_contract = env.register_stellar_asset_contract_v2(token_admin);
        let contract_id = env.register(Treasury, ());
        let client = TreasuryClient::new(&env, &contract_id);
        client.initialize(&admin);
        client.set_oracle(&admin, &oracle);
        client.set_token(&admin, &token_contract.address());

        (
            env,
            contract_id,
            client,
            admin,
            oracle,
            token_contract.address(),
        )
    }

    #[test]
    fn deposit_tracks_balance() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(Treasury, ());
        let client = TreasuryClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        client.initialize(&admin);

        let user = Address::generate(&env);
        assert_eq!(client.deposit(&user, &10_i128), 10_i128);
        assert_eq!(client.deposit(&user, &5_i128), 15_i128);
        assert_eq!(client.balance(&user), 15_i128);
    }

    // ── deposit_fees access control tests ──────────────────────────────────

    #[test]
    fn deposit_fees_fails_for_unregistered_address() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(Treasury, ());
        let client = TreasuryClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        client.initialize(&admin);

        // Register a market address
        let market = Address::generate(&env);
        client.set_market(&admin, &market);

        // A different address that is NOT the registered market
        let unauthorized = Address::generate(&env);
        let err = client
            .try_deposit_fees(&unauthorized, &100_i128)
            .expect_err("should be unauthorized");
        assert_eq!(err, Ok(PredictXError::Unauthorized));
    }

    #[test]
    fn deposit_fees_succeeds_for_registered_market() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(Treasury, ());
        let client = TreasuryClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        client.initialize(&admin);

        // Register the market address
        let market = Address::generate(&env);
        client.set_market(&admin, &market);

        // The registered market can deposit fees
        let result = client.deposit_fees(&market, &500_i128);
        assert_eq!(result, 500_i128);
        assert_eq!(client.balance(&market), 500_i128);
    }

    #[test]
    fn set_market_rejects_non_admin() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(Treasury, ());
        let client = TreasuryClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        client.initialize(&admin);

        let non_admin = Address::generate(&env);
        let new_market = Address::generate(&env);
        let err = client
            .try_set_market(&non_admin, &new_market)
            .expect_err("should be unauthorized");
        assert_eq!(err, Ok(PredictXError::Unauthorized));
    }

    #[test]
    fn fund_voter_rewards_rejects_unregistered_caller() {
        let (env, _, client, _, _, _) = setup();
        let unauthorized = Address::generate(&env);

        let err = client
            .try_fund_voter_rewards(&unauthorized, &42_u64, &100_i128)
            .expect_err("unregistered callers must be rejected");
        assert_eq!(err, Ok(PredictXError::Unauthorized));
    }

    #[test]
    fn fund_voter_rewards_transfers_and_rejects_double_funding() {
        let (env, contract_id, client, _, oracle, token_address) = setup();
        let token_admin = token::StellarAssetClient::new(&env, &token_address);
        let token_client = token::Client::new(&env, &token_address);
        token_admin.mint(&contract_id, &500_i128);

        assert_eq!(
            client.fund_voter_rewards(&oracle, &42_u64, &200_i128),
            200_i128
        );
        assert_eq!(client.funded_voter_rewards(&42_u64), 200_i128);
        assert_eq!(token_client.balance(&oracle), 200_i128);
        assert_eq!(token_client.balance(&contract_id), 300_i128);

        let err = client
            .try_fund_voter_rewards(&oracle, &42_u64, &100_i128)
            .expect_err("a poll can only be funded once");
        assert_eq!(err, Ok(PredictXError::VoterRewardsAlreadyFunded));
        assert_eq!(client.funded_voter_rewards(&42_u64), 200_i128);
    }

    #[test]
    fn fund_voter_rewards_rejects_amount_above_treasury_balance() {
        let (_, _, client, _, oracle, _) = setup();

        let err = client
            .try_fund_voter_rewards(&oracle, &42_u64, &100_i128)
            .expect_err("funding must not exceed the actual token balance");
        assert_eq!(err, Ok(PredictXError::InsufficientBalance));
        assert_eq!(client.funded_voter_rewards(&42_u64), 0_i128);
    }
}
