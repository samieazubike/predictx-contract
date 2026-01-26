#![no_std]
use soroban_sdk::{contract, contractimpl, symbol_short, Env, Symbol};

const COUNTER: Symbol = symbol_short!("COUNTER");

#[contract]
pub struct PredictXContract;

#[contractimpl]
impl PredictXContract {
    /// Initialize the contract
    pub fn initialize(env: Env) -> u32 {
        env.storage().instance().set(&COUNTER, &0_u32);
        0
    }

    /// Increment the counter
    pub fn increment(env: Env) -> u32 {
        let mut count: u32 = env.storage().instance().get(&COUNTER).unwrap_or(0);
        count += 1;
        env.storage().instance().set(&COUNTER, &count);
        count
    }

    /// Get the current counter value
    pub fn get_count(env: Env) -> u32 {
        env.storage().instance().get(&COUNTER).unwrap_or(0)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::Env;

    #[test]
    fn test_initialize() {
        let env = Env::default();
        let contract_id = env.register(PredictXContract, ());
        let client = PredictXContractClient::new(&env, &contract_id);

        let count = client.initialize();
        assert_eq!(count, 0);
    }

    #[test]
    fn test_increment() {
        let env = Env::default();
        let contract_id = env.register(PredictXContract, ());
        let client = PredictXContractClient::new(&env, &contract_id);

        client.initialize();
        let count = client.increment();
        assert_eq!(count, 1);
        
        let count = client.increment();
        assert_eq!(count, 2);
    }

    #[test]
    fn test_get_count() {
        let env = Env::default();
        let contract_id = env.register(PredictXContract, ());
        let client = PredictXContractClient::new(&env, &contract_id);

        client.initialize();
        assert_eq!(client.get_count(), 0);
        
        client.increment();
        assert_eq!(client.get_count(), 1);
    }
}
