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
    TokenAddress,
    Balance(Address),
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

    /// Admin-gated setter for the token held by the treasury.
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

    /// Withdraw collected fees to an operational address.
    pub fn withdraw_fees(
        env: Env,
        admin: Address,
        to: Address,
        amount: i128,
    ) -> Result<(), PredictXError> {
        let stored_admin = get_admin(&env)?;
        if admin != stored_admin {
            return Err(PredictXError::Unauthorized);
        }
        admin.require_auth();

        if amount <= 0 {
            return Err(PredictXError::StakeAmountZero);
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

        token_client.transfer(&treasury_address, &to, &amount);
        Ok(())
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

    fn setup() -> (Env, Address, TreasuryClient<'static>, Address, Address) {
        let env = Env::default();
        env.mock_all_auths();

        let admin = Address::generate(&env);
        let token_admin = Address::generate(&env);
        let token_contract = env.register_stellar_asset_contract_v2(token_admin);
        let contract_id = env.register(Treasury, ());
        let client = TreasuryClient::new(&env, &contract_id);
        client.initialize(&admin);
        client.set_token(&admin, &token_contract.address());

        (env, contract_id, client, admin, token_contract.address())
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
    fn withdraw_fees_rejects_non_admin() {
        let (env, _, client, _, _) = setup();
        let non_admin = Address::generate(&env);
        let recipient = Address::generate(&env);

        let err = client
            .try_withdraw_fees(&non_admin, &recipient, &10_i128)
            .expect_err("non-admin withdrawal must be rejected");
        assert_eq!(err, Ok(PredictXError::Unauthorized));
    }

    #[test]
    fn withdraw_fees_checks_actual_contract_balance() {
        let (env, contract_id, client, admin, token_address) = setup();
        let recipient = Address::generate(&env);
        let token_client = token::Client::new(&env, &token_address);

        // Stored per-address accounting must not substitute for held tokens.
        client.deposit(&recipient, &100_i128);
        let err = client
            .try_withdraw_fees(&admin, &recipient, &50_i128)
            .expect_err("recorded amounts cannot exceed the real token balance");
        assert_eq!(err, Ok(PredictXError::InsufficientBalance));
        assert_eq!(token_client.balance(&contract_id), 0_i128);
        assert_eq!(token_client.balance(&recipient), 0_i128);
    }

    #[test]
    fn withdraw_fees_transfers_tokens_to_recipient() {
        let (env, contract_id, client, admin, token_address) = setup();
        let recipient = Address::generate(&env);
        let asset = token::StellarAssetClient::new(&env, &token_address);
        let token_client = token::Client::new(&env, &token_address);
        asset.mint(&contract_id, &250_i128);

        client.withdraw_fees(&admin, &recipient, &75_i128);

        assert_eq!(token_client.balance(&recipient), 75_i128);
        assert_eq!(token_client.balance(&contract_id), 175_i128);
    }
}
