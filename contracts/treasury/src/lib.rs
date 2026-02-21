#![no_std]

use predictx_shared::PredictXError;
use soroban_sdk::{contract, contractimpl, contracttype, Address, Env};

#[contract]
pub struct Treasury;

#[contracttype]
#[derive(Clone)]
enum DataKey {
    Admin,
    Balance(Address),
}

fn get_admin(env: &Env) -> Result<Address, PredictXError> {
    env.storage()
        .instance()
        .get(&DataKey::Admin)
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
        client.initialize(&admin);

        let user = Address::generate(&env);
        assert_eq!(client.deposit(&user, &10_i128), 10_i128);
        assert_eq!(client.deposit(&user, &5_i128), 15_i128);
        assert_eq!(client.balance(&user), 15_i128);
    }
}
