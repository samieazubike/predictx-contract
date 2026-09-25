#![no_std]

use predictx_shared::PredictXError;
use soroban_sdk::{contract, contractimpl, contracttype, Address, Env};

#[contract]
pub struct Treasury;

#[contracttype]
#[derive(Clone)]
enum DataKey {
    Admin,
    Token,
    Market,
    Balance(Address),
}

fn get_admin(env: &Env) -> Result<Address, PredictXError> {
    env.storage()
        .instance()
        .get(&DataKey::Admin)
        .ok_or(PredictXError::NotInitialized)
}

fn get_token(env: &Env) -> Result<Address, PredictXError> {
    env.storage()
        .instance()
        .get(&DataKey::Token)
        .ok_or(PredictXError::NotInitialized)
}

fn get_market(env: &Env) -> Result<Address, PredictXError> {
    env.storage()
        .instance()
        .get(&DataKey::Market)
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
    pub fn initialize(env: Env, admin: Address, token: Address) -> Result<(), PredictXError> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(PredictXError::AlreadyInitialized);
        }

        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::Token, &token);
        Ok(())
    }

    pub fn admin(env: Env) -> Result<Address, PredictXError> {
        get_admin(&env)
    }

    /// Returns the token address this treasury holds, if initialized.
    pub fn token(env: Env) -> Result<Address, PredictXError> {
        get_token(&env)
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

    #[test]
    fn deposit_tracks_balance() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(Treasury, ());
        let client = TreasuryClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let token = Address::generate(&env);
        client.initialize(&admin, &token);

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
        let token = Address::generate(&env);
        client.initialize(&admin, &token);

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
        let token = Address::generate(&env);
        client.initialize(&admin, &token);

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
        let token = Address::generate(&env);
        client.initialize(&admin, &token);

        let non_admin = Address::generate(&env);
        let new_market = Address::generate(&env);
        let err = client
            .try_set_market(&non_admin, &new_market)
            .expect_err("should be unauthorized");
        assert_eq!(err, Ok(PredictXError::Unauthorized));
    }

    // ── initialize / token tests ───────────────────────────────────────────

    #[test]
    fn initialize_stores_admin_and_token() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(Treasury, ());
        let client = TreasuryClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let token = Address::generate(&env);
        client.initialize(&admin, &token);

        assert_eq!(client.admin(), admin);
        assert_eq!(client.token(), token);
    }

    #[test]
    fn initialize_is_one_time_only() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(Treasury, ());
        let client = TreasuryClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let token = Address::generate(&env);
        client.initialize(&admin, &token);

        let other_token = Address::generate(&env);
        let err = client
            .try_initialize(&admin, &other_token)
            .expect_err("should be already initialized");
        assert_eq!(err, Ok(PredictXError::AlreadyInitialized));

        // The original token is preserved.
        assert_eq!(client.token(), token);
    }

    #[test]
    fn token_before_initialize_returns_not_initialized() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(Treasury, ());
        let client = TreasuryClient::new(&env, &contract_id);

        let err = client
            .try_token()
            .expect_err("should be not initialized");
        assert_eq!(err, Ok(PredictXError::NotInitialized));
    }
}
