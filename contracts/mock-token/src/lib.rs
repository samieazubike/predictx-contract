#![no_std]

use soroban_sdk::{contract, contracterror, contractimpl, contracttype, Address, Env, Symbol};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum MockTokenError {
    /// The token has already been initialised.
    AlreadyInitialized = 1,
    /// The account does not hold enough tokens.
    InsufficientBalance = 2,
}

#[contracttype]
enum DataKey {
    Balance(Address),
    Decimals,
    Name,
    Symbol,
}

#[contract]
pub struct MockToken;

#[contractimpl]
impl MockToken {
    /// Initialize the token with configurable metadata.
    pub fn initialize(
        env: Env,
        decimal: u32,
        name: Symbol,
        symbol: Symbol,
    ) {
        if env.storage().instance().has(&DataKey::Decimals) {
            panic!("already initialized");
        }
        env.storage().instance().set(&DataKey::Decimals, &decimal);
        env.storage().instance().set(&DataKey::Name, &name);
        env.storage().instance().set(&DataKey::Symbol, &symbol);
    }

    /// Returns the balance of `address`.
    pub fn balance(env: Env, address: Address) -> i128 {
        env.storage()
            .persistent()
            .get(&DataKey::Balance(address))
            .unwrap_or(0)
    }

    /// Transfer `amount` from `from` to `to`.
    pub fn transfer(
        env: Env,
        from: Address,
        to: Address,
        amount: i128,
    ) -> Result<(), MockTokenError> {
        from.require_auth();
        
        let from_balance = Self::balance(env.clone(), from.clone());
        if from_balance < amount {
            return Err(MockTokenError::InsufficientBalance);
        }

        env.storage()
            .persistent()
            .set(&DataKey::Balance(from.clone()), &(from_balance - amount));
        
        let to_balance = Self::balance(env.clone(), to.clone());
        env.storage()
            .persistent()
            .set(&DataKey::Balance(to), &(to_balance + amount));
        
        Ok(())
    }

    /// Returns the number of decimals.
    pub fn decimals(env: Env) -> u32 {
        env.storage().instance().get(&DataKey::Decimals).unwrap()
    }

    /// Returns the token name.
    pub fn name(env: Env) -> Symbol {
        env.storage().instance().get(&DataKey::Name).unwrap()
    }

    /// Returns the token symbol.
    pub fn symbol(env: Env) -> Symbol {
        env.storage().instance().get(&DataKey::Symbol).unwrap()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::testutils::{Address as _, Ledger as _};

    fn setup(env: &Env) -> MockTokenClient<'static> {
        let contract_id = env.register(MockToken, ());
        let client = MockTokenClient::new(env, &contract_id);
        client
    }

    #[test]
    fn initialize_sets_metadata() {
        let env = Env::default();
        let client = setup(&env);
        
        client.initialize(&7_u32, &Symbol::new(&env, "Test Token"), &Symbol::new(&env, "TST"));
        
        assert_eq!(client.decimals(), 7);
        assert_eq!(client.name(), Symbol::new(&env, "Test Token"));
        assert_eq!(client.symbol(), Symbol::new(&env, "TST"));
    }

    #[test]
    fn transfer_moves_balance_and_rejects_overdraft() {
        let env = Env::default();
        env.mock_all_auths();
        let client = setup(&env);
        
        client.initialize(&18_u32, &Symbol::new(&env, "Test"), &Symbol::new(&env, "TST"));
        
        let from = Address::generate(&env);
        let to = Address::generate(&env);
        
        // Set initial balance
        env.as_contract(&client.contract_id, || {
            env.storage()
                .persistent()
                .set(&DataKey::Balance(from.clone()), &100_i128);
        });
        
        assert_eq!(client.balance(&from), 100);
        assert_eq!(client.balance(&to), 0);
        
        // Successful transfer
        client.transfer(&from, &to, &30);
        assert_eq!(client.balance(&from), 70);
        assert_eq!(client.balance(&to), 30);
        
        // Overdraft should fail
        let err = client
            .try_transfer(&from, &to, &80)
            .expect_err("overdraft should fail");
        assert_eq!(err, Ok(MockTokenError::InsufficientBalance));
        
        // Balances unchanged after failed transfer
        assert_eq!(client.balance(&from), 70);
        assert_eq!(client.balance(&to), 30);
    }

    #[test]
    fn transfer_requires_auth() {
        let env = Env::default();
        let client = setup(&env);
        
        client.initialize(&18_u32, &Symbol::new(&env, "Test"), &Symbol::new(&env, "TST"));
        
        let from = Address::generate(&env);
        let to = Address::generate(&env);
        
        env.as_contract(&client.contract_id, || {
            env.storage()
                .persistent()
                .set(&DataKey::Balance(from.clone()), &100_i128);
        });
        
        env.set_auths(&[]);
        let err = client
            .try_transfer(&from, &to, &10)
            .expect_err("transfer must require auth");
        assert_eq!(err, Err(soroban_sdk::InvokeError::Abort));
    }

    #[test]
    fn metadata_is_configurable() {
        let env = Env::default();
        let client = setup(&env);
        
        // Test with different decimal values
        client.initialize(&6_u32, &Symbol::new(&env, "USD Coin"), &Symbol::new(&env, "USDC"));
        assert_eq!(client.decimals(), 6);
        assert_eq!(client.name(), Symbol::new(&env, "USD Coin"));
        assert_eq!(client.symbol(), Symbol::new(&env, "USDC"));
        
        // Re-initialize should panic
        env.as_contract(&client.contract_id, || {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                MockToken::initialize(env.clone(), 18, Symbol::new(&env, "Other"), Symbol::new(&env, "OTH"))
            }));
            assert!(result.is_err());
        });
    }
}
