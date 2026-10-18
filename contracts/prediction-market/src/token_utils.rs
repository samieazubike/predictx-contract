use soroban_sdk::{token, Address, Env};
use predictx_shared::PredictXError;
use crate::DataKey;

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Retrieve the stored token address or error if not initialized.
pub fn get_token_address(env: &Env) -> Result<Address, PredictXError> {
    env.storage()
        .instance()
        .get(&DataKey::TokenAddress)
        .ok_or(PredictXError::NotInitialized)
}

/// Retrieve the stored treasury address or error if not initialized.
pub fn get_treasury_address(env: &Env) -> Result<Address, PredictXError> {
    env.storage()
        .instance()
        .get(&DataKey::TreasuryAddress)
        .ok_or(PredictXError::NotInitialized)
}

/// Retrieve the platform fee in basis points.
pub fn get_platform_fee_bps(env: &Env) -> u32 {
    env.storage()
        .instance()
        .get(&DataKey::PlatformFeeBps)
        .unwrap_or(predictx_shared::PLATFORM_FEE_BPS)
}

// ── Token operations ──────────────────────────────────────────────────────────

/// Transfer tokens **from** a user **to** this contract.
///
/// The caller (`from`) must have authorised the transfer before invoking.
pub fn transfer_to_contract(env: &Env, from: &Address, amount: i128) -> Result<(), PredictXError> {
    let token_addr = get_token_address(env)?;
    let client = token::Client::new(env, &token_addr);
    client.transfer(from, &env.current_contract_address(), &amount);
    Ok(())
}

/// Transfer tokens **from** this contract **to** a recipient.
///
/// Used for payouts, emergency withdrawals, and treasury distributions.
pub fn transfer_from_contract(env: &Env, to: &Address, amount: i128) -> Result<(), PredictXError> {
    let token_addr = get_token_address(env)?;
    let client = token::Client::new(env, &token_addr);
    client.transfer(&env.current_contract_address(), to, &amount);
    Ok(())
}

/// Transfer tokens **from** this contract **to** the treasury address.
pub fn transfer_to_treasury(env: &Env, amount: i128) -> Result<(), PredictXError> {
    let treasury = get_treasury_address(env)?;
    transfer_from_contract(env, &treasury, amount)
}

/// Get the contract's token balance.
pub fn get_balance(env: &Env) -> Result<i128, PredictXError> {
    let token_addr = get_token_address(env)?;
    let client = token::Client::new(env, &token_addr);
    Ok(client.balance(&env.current_contract_address()))
}
