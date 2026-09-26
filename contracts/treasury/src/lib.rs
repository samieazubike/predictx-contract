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

    /// Admin-gated setter for the token used to collect fees.
    pub fn set_token(env: Env, admin: Address, token_address: Address) -> Result<(), PredictXError> {
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

    /// Transfer fees from an authorized address into this contract and record them.
    pub fn deposit_fees(env: Env, from: Address, amount: i128) -> Result<i128, PredictXError> {
        if amount <= 0 {
            return Err(PredictXError::StakeAmountZero);
        }
        if !env.storage().instance().has(&DataKey::Admin) {
            return Err(PredictXError::NotInitialized);
        }

        from.require_auth();

        let token_address: Address = env
            .storage()
            .instance()
            .get(&DataKey::TokenAddress)
            .ok_or(PredictXError::NotInitialized)?;
        token::Client::new(&env, &token_address).transfer(
            &from,
            &env.current_contract_address(),
            &amount,
        );

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

    fn setup() -> (Env, Address, TreasuryClient<'static>, Address) {
        let env = Env::default();
        env.mock_all_auths();

        let admin = Address::generate(&env);
        let token_admin = Address::generate(&env);
        let token_contract = env.register_stellar_asset_contract_v2(token_admin);
        let contract_id = env.register(Treasury, ());
        let client = TreasuryClient::new(&env, &contract_id);
        client.initialize(&admin);
        client.set_token(&admin, &token_contract.address());

        (env, contract_id, client, token_contract.address())
    }

    #[test]
    fn deposit_fees_transfers_tokens_and_tracks_balance() {
        let (env, contract_id, client, token_address) = setup();
        let user = Address::generate(&env);
        let treasury_address = contract_id;
        let asset = token::StellarAssetClient::new(&env, &token_address);
        let token_client = token::Client::new(&env, &token_address);
        asset.mint(&user, &500_i128);

        assert_eq!(client.deposit_fees(&user, &125_i128), 125_i128);
        assert_eq!(client.balance(&user), 125_i128);
        assert_eq!(token_client.balance(&treasury_address), 125_i128);
        assert_eq!(token_client.balance(&user), 375_i128);
    }

    #[test]
    fn deposit_fees_rejects_zero_and_negative_amounts() {
        let (env, _, client, _) = setup();
        let user = Address::generate(&env);

        assert_eq!(
            client.try_deposit_fees(&user, &0_i128),
            Err(Ok(PredictXError::StakeAmountZero))
        );
        assert_eq!(
            client.try_deposit_fees(&user, &-1_i128),
            Err(Ok(PredictXError::StakeAmountZero))
        );
    }

    #[test]
    fn failed_transfer_does_not_update_recorded_balance() {
        let (env, contract_id, client, token_address) = setup();
        let user = Address::generate(&env);
        let token_client = token::Client::new(&env, &token_address);

        assert!(client.try_deposit_fees(&user, &100_i128).is_err());
        assert_eq!(client.balance(&user), 0_i128);
        assert_eq!(token_client.balance(&contract_id), 0_i128);
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
}
