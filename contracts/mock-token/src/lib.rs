#![no_std]

use soroban_sdk::{contract, contracterror, contractimpl, contracttype, Address, Env};

#[contract]
pub struct MockToken;

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MockTokenError {
    TransferFailed = 1,
    InsufficientBalance = 2,
}

#[contracttype]
enum DataKey {
    Balance(Address),
    FailNextTransfer,
}

fn balance(env: &Env, address: &Address) -> i128 {
    env.storage()
        .persistent()
        .get(&DataKey::Balance(address.clone()))
        .unwrap_or(0)
}

#[contractimpl]
impl MockToken {
    pub fn initialize(_env: Env) {}

    pub fn balance(env: Env, address: Address) -> i128 {
        balance(&env, &address)
    }

    pub fn transfer(
        env: Env,
        from: Address,
        to: Address,
        amount: i128,
    ) -> Result<(), MockTokenError> {
        from.require_auth();
        if amount < 0 {
            return Err(MockTokenError::InsufficientBalance);
        }

        #[cfg(feature = "testutils")]
        if env
            .storage()
            .instance()
            .get(&DataKey::FailNextTransfer)
            .unwrap_or(false)
        {
            env.storage()
                .instance()
                .set(&DataKey::FailNextTransfer, &false);
            return Err(MockTokenError::TransferFailed);
        }

        let from_balance = balance(&env, &from);
        if from_balance < amount {
            return Err(MockTokenError::InsufficientBalance);
        }

        env.storage()
            .persistent()
            .set(&DataKey::Balance(from.clone()), &(from_balance - amount));
        let to_balance = balance(&env, &to);
        env.storage()
            .persistent()
            .set(&DataKey::Balance(to), &(to_balance + amount));
        Ok(())
    }
}

#[cfg(feature = "testutils")]
#[contractimpl]
impl MockToken {
    pub fn set_fail_next_transfer(env: Env, should_fail: bool) {
        env.storage()
            .instance()
            .set(&DataKey::FailNextTransfer, &should_fail);
    }
}

#[cfg(test)]
extern crate std;

#[cfg(all(test, feature = "testutils"))]
mod test {
    use super::*;
    use soroban_sdk::testutils::Address as _;

    fn setup() -> (Env, Address, MockTokenClient<'static>) {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(MockToken, ());
        let client = MockTokenClient::new(&env, &contract_id);
        client.initialize();
        (env, contract_id, client)
    }

    #[test]
    fn transfer_succeeds_without_failure_injection() {
        let (env, contract_id, client) = setup();
        let from = Address::generate(&env);
        let to = Address::generate(&env);
        let amount = 25;

        env.as_contract(&contract_id, || {
            env.storage()
                .persistent()
                .set(&DataKey::Balance(from.clone()), &100_i128);
        });

        client.transfer(&from, &to, &amount);

        assert_eq!(client.balance(&from), 75);
        assert_eq!(client.balance(&to), amount);
    }

    #[test]
    fn failed_transfer_clears_flag_and_preserves_balances() {
        let (env, contract_id, client) = setup();
        let from = Address::generate(&env);
        let to = Address::generate(&env);

        env.as_contract(&contract_id, || {
            env.storage()
                .persistent()
                .set(&DataKey::Balance(from.clone()), &100_i128);
            env.storage()
                .persistent()
                .set(&DataKey::Balance(to.clone()), &10_i128);
        });
        client.set_fail_next_transfer(&true);

        let err = env.as_contract(&contract_id, || {
            MockToken::transfer(env.clone(), from.clone(), to.clone(), 25)
        });
        assert_eq!(err, Err(MockTokenError::TransferFailed));
        assert_eq!(client.balance(&from), 100);
        assert_eq!(client.balance(&to), 10);

        client.transfer(&from, &to, &25_i128);
        assert_eq!(client.balance(&from), 75);
        assert_eq!(client.balance(&to), 35);
    }

    #[test]
    fn disabling_failure_injection_keeps_transfers_successful() {
        let (env, contract_id, client) = setup();
        let from = Address::generate(&env);
        let to = Address::generate(&env);

        env.as_contract(&contract_id, || {
            env.storage()
                .persistent()
                .set(&DataKey::Balance(from.clone()), &50_i128);
        });
        client.set_fail_next_transfer(&false);
        client.transfer(&from, &to, &20_i128);

        assert_eq!(client.balance(&from), 30);
        assert_eq!(client.balance(&to), 20);
    }
}
