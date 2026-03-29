use soroban_sdk::{Address, Env, Vec};

use crate::{DataKey, PredictXError};

pub fn get_super_admin(env: &Env) -> Result<Address, PredictXError> {
    env.storage()
        .instance()
        .get(&DataKey::SuperAdmin)
        .ok_or(PredictXError::NotInitialized)
}

pub fn get_oracle(env: &Env) -> Result<Address, PredictXError> {
    env.storage()
        .instance()
        .get(&DataKey::OracleAddress)
        .ok_or(PredictXError::NotInitialized)
}

pub fn get_admins(env: &Env) -> Vec<Address> {
    env.storage()
        .instance()
        .get(&DataKey::AdminList)
        .unwrap_or(Vec::new(env))
}

pub fn is_admin(env: &Env, address: &Address) -> Result<bool, PredictXError> {
    let super_admin = get_super_admin(env)?;
    if *address == super_admin {
        return Ok(true);
    }
    Ok(get_admins(env).contains(address))
}

pub fn require_super_admin(env: &Env, caller: &Address) -> Result<(), PredictXError> {
    caller.require_auth();
    let super_admin = get_super_admin(env)?;
    if *caller != super_admin {
        return Err(PredictXError::Unauthorized);
    }
    Ok(())
}

pub fn require_admin(env: &Env, caller: &Address) -> Result<(), PredictXError> {
    caller.require_auth();
    if !is_admin(env, caller)? {
        return Err(PredictXError::Unauthorized);
    }
    Ok(())
}

pub fn require_oracle(env: &Env, caller: &Address) -> Result<(), PredictXError> {
    caller.require_auth();
    let oracle = get_oracle(env)?;
    if *caller != oracle {
        return Err(PredictXError::Unauthorized);
    }
    Ok(())
}

pub fn add_admin(env: &Env, caller: &Address, new_admin: Address) -> Result<(), PredictXError> {
    require_super_admin(env, caller)?;
    let mut admins = get_admins(env);
    if admins.contains(&new_admin) {
        return Err(PredictXError::AdminAlreadyRegistered);
    }
    admins.push_back(new_admin);
    env.storage().instance().set(&DataKey::AdminList, &admins);
    Ok(())
}

pub fn remove_admin(
    env: &Env,
    caller: &Address,
    admin_to_remove: Address,
) -> Result<(), PredictXError> {
    require_super_admin(env, caller)?;
    let admins = get_admins(env);
    let mut next = Vec::new(env);
    let mut found = false;
    for admin in admins.iter() {
        if admin == admin_to_remove {
            found = true;
        } else {
            next.push_back(admin);
        }
    }
    if !found {
        return Err(PredictXError::Unauthorized);
    }
    env.storage().instance().set(&DataKey::AdminList, &next);
    Ok(())
}

pub fn set_oracle(env: &Env, caller: &Address, oracle: Address) -> Result<(), PredictXError> {
    require_super_admin(env, caller)?;
    env.storage().instance().set(&DataKey::OracleAddress, &oracle);
    Ok(())
}

pub fn propose_super_admin_transfer(
    env: &Env,
    caller: &Address,
    new_super_admin: Address,
) -> Result<(), PredictXError> {
    require_super_admin(env, caller)?;
    env.storage()
        .instance()
        .set(&DataKey::PendingSuperAdmin, &new_super_admin);
    Ok(())
}

pub fn accept_super_admin_transfer(env: &Env, caller: &Address) -> Result<(), PredictXError> {
    caller.require_auth();
    let pending: Address = env
        .storage()
        .instance()
        .get(&DataKey::PendingSuperAdmin)
        .ok_or(PredictXError::Unauthorized)?;
    if pending != *caller {
        return Err(PredictXError::Unauthorized);
    }
    env.storage().instance().set(&DataKey::SuperAdmin, caller);
    env.storage().instance().remove(&DataKey::PendingSuperAdmin);
    Ok(())
}
