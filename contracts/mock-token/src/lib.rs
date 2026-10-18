#![no_std]

use soroban_sdk::{contract, contractimpl, Env};

#[contract]
pub struct MockToken;

#[contractimpl]
impl MockToken {
    pub fn initialize(_env: Env) {}
}

#[cfg(test)]
extern crate std;

