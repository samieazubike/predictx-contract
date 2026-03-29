#![no_std]

use predictx_shared::{
    add_admin as shared_add_admin, get_admins as shared_get_admins,
    get_super_admin as shared_get_super_admin, is_admin as shared_is_admin,
    remove_admin as shared_remove_admin, DataKey as SharedDataKey, PredictXError,
};
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
    shared_get_super_admin(env)
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
        env.storage().instance().set(&SharedDataKey::SuperAdmin, &admin);
        env.storage().instance().set(&SharedDataKey::AdminList, &soroban_sdk::Vec::<Address>::new(&env));
        Ok(())
    }

    pub fn admin(env: Env) -> Result<Address, PredictXError> {
        get_admin(&env)
    }

    pub fn get_super_admin(env: Env) -> Result<Address, PredictXError> {
        shared_get_super_admin(&env)
    }

    pub fn get_admins(env: Env) -> soroban_sdk::Vec<Address> {
        shared_get_admins(&env)
    }

    pub fn is_admin(env: Env, address: Address) -> Result<bool, PredictXError> {
        shared_is_admin(&env, &address)
    }

    pub fn add_admin(env: Env, super_admin: Address, new_admin: Address) -> Result<(), PredictXError> {
        shared_add_admin(&env, &super_admin, new_admin)
    }

    pub fn remove_admin(env: Env, super_admin: Address, admin_to_remove: Address) -> Result<(), PredictXError> {
        shared_remove_admin(&env, &super_admin, admin_to_remove)
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

    #[test]
    fn only_super_admin_can_add_admin() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(Treasury, ());
        let client = TreasuryClient::new(&env, &contract_id);

        let super_admin = Address::generate(&env);
        client.initialize(&super_admin);
        let attacker = Address::generate(&env);
        let admin = Address::generate(&env);
        let err = client
            .try_add_admin(&attacker, &admin)
            .expect_err("only super admin can add");
        assert_eq!(err, Ok(PredictXError::Unauthorized));
        client.add_admin(&super_admin, &admin);
        assert!(client.is_admin(&admin));
    }
}
