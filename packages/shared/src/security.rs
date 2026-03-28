//! Security utilities for PredictX contracts.
//!
//! This module provides reentrancy guards and other security primitives
//! to protect against common smart contract attack vectors.

use soroban_sdk::{Env, Symbol};

use crate::PredictXError;

/// Storage key for reentrancy locks.
#[contracttype]
#[derive(Clone)]
pub enum ReentrancyKey {
    /// Reentrancy lock for a specific function.
    Lock(ReentrancyGuardFunction),
}

/// Functions that require reentrancy protection.
#[contracttype]
#[derive(Clone)]
pub enum ReentrancyGuardFunction {
    Stake,
    ClaimWinnings,
    EmergencyWithdraw,
    ClaimVotingReward,
    DistributeRewards,
}

impl ReentrancyKey {
    fn as_symbol(&self, env: &Env) -> Symbol {
        match self {
            ReentrancyKey::Lock(fn_key) => match fn_key {
                ReentrancyGuardFunction::Stake => Symbol::new(env, "__reentrancy_stake"),
                ReentrancyGuardFunction::ClaimWinnings => {
                    Symbol::new(env, "__reentrancy_claim_winnings")
                }
                ReentrancyGuardFunction::EmergencyWithdraw => {
                    Symbol::new(env, "__reentrancy_emergency_withdraw")
                }
                ReentrancyGuardFunction::ClaimVotingReward => {
                    Symbol::new(env, "__reentrancy_claim_voting_reward")
                }
                ReentrancyGuardFunction::DistributeRewards => {
                    Symbol::new(env, "__reentrancy_distribute_rewards")
                }
            },
        }
    }
}

/// Sets a reentrancy lock for the given function.
///
/// # Panics
/// Panics if a lock is already set (indicating a reentrancy attack).
pub fn set_reentrancy_lock(env: &Env, function: ReentrancyGuardFunction) {
    let key = ReentrancyKey::Lock(function);
    let symbol = key.as_symbol(env);

    if env.storage().instance().has(&symbol) {
        panic!("ReentrancyGuard: reentrant call detected");
    }

    env.storage().instance().set(&symbol, &true);
}

/// Clears the reentrancy lock for the given function.
///
/// This must be called after the protected function completes,
/// typically in a drop guard or at the end of the function.
pub fn clear_reentrancy_lock(env: &Env, function: ReentrancyGuardFunction) {
    let key = ReentrancyKey::Lock(function);
    let symbol = key.as_symbol(env);
    env.storage().instance().remove(&symbol);
}

/// RAII guard for reentrancy protection.
///
/// Automatically clears the lock when dropped.
pub struct ReentrancyGuard {
    env: Env,
    function: ReentrancyGuardFunction,
}

impl ReentrancyGuard {
    /// Creates a new reentrancy guard, setting the lock.
    ///
    /// # Panics
    /// Panics if a lock is already set.
    pub fn new(env: &Env, function: ReentrancyGuardFunction) -> Self {
        set_reentrancy_lock(env, function.clone());
        Self { env: env.clone(), function }
    }

    /// Releases the guard early (optional explicit release).
    pub fn release(self) {
        clear_reentrancy_lock(&self.env, self.function.clone());
    }
}

impl Drop for ReentrancyGuard {
    fn drop(&mut self) {
        clear_reentrancy_lock(&self.env, self.function.clone());
    }
}

/// Verifies that the caller is a contract (not an EOA).
///
/// This prevents certain attack vectors where an external address
/// tries to call functions that should only be accessible via contracts.
pub fn require_contract_call(env: &Env) -> Result<(), PredictXError> {
    // In Soroban, we can verify this by checking if the calling address
    // matches the invoker. For a contract call, the invoker should be
    // the contract address. This is a best-effort check.
    let invoker = env.invoker();
    let current = env.current_contract_address();
    
    if invoker != current {
        return Err(PredictXError::NotContractCall);
    }
    Ok(())
}

/// Validates that an address is not zero (empty).
///
/// A zero address could indicate an uninitialized variable or
/// a potential security issue.
pub fn validate_address(addr: &soroban_sdk::Address) -> Result<(), PredictXError> {
    // Address validation is primarily done by Soroban SDK,
    // but we can add additional checks here if needed.
    // For now, this is a placeholder for future enhancements.
    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::Env;

    #[test]
    fn reentrancy_guard_set_and_clear() {
        let env = Env::default();
        let function = ReentrancyGuardFunction::Stake;

        // Initially no lock
        let key = ReentrancyKey::Lock(function.clone());
        let symbol = key.as_symbol(&env);
        assert!(!env.storage().instance().has(&symbol));

        // Set lock
        set_reentrancy_lock(&env, function.clone());
        assert!(env.storage().instance().has(&symbol));

        // Clear lock
        clear_reentrancy_lock(&env, function);
        assert!(!env.storage().instance().has(&symbol));
    }

    #[test]
    fn reentrancy_guard_panics_on_reentry() {
        let env = Env::default();
        let function = ReentrancyGuardFunction::ClaimWinnings;

        set_reentrancy_lock(&env, function.clone());
        
        // Attempting to set again should panic
        let result = std::panic::catch_unwind(|| {
            set_reentrancy_lock(&env, function);
        });
        assert!(result.is_err());
    }

    #[test]
    fn reentrancy_guard_auto_release() {
        let env = Env::default();
        let function = ReentrancyGuardFunction::EmergencyWithdraw;

        {
            let _guard = ReentrancyGuard::new(&env, function.clone());
            let key = ReentrancyKey::Lock(function.clone());
            let symbol = key.as_symbol(&env);
            assert!(env.storage().instance().has(&symbol));
        }

        // Guard dropped, lock should be cleared
        let key = ReentrancyKey::Lock(function);
        let symbol = key.as_symbol(&env);
        assert!(!env.storage().instance().has(&symbol));
    }
}
