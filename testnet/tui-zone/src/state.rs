use std::{collections::HashMap, ops::AddAssign as _};

use lb_core::{
    mantle::Value,
    utils::{display_hex_bytes_newtype, serde_bytes_newtype},
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StateTransition {
    Mint { amount: Value, address: Address },
    Burn { amount: Value, address: Address },
}

#[derive(Debug, Clone)]
pub struct State {
    accounts: HashMap<Address, Value>,
}

impl State {
    pub fn new() -> Self {
        Self {
            accounts: HashMap::new(),
        }
    }

    pub fn apply(&mut self, transition: &StateTransition) {
        match transition {
            StateTransition::Mint { amount, address } => {
                self.mint(*address, *amount);
            }
            StateTransition::Burn { amount, address } => {
                self.burn(*address, *amount);
            }
        }
    }

    fn mint(&mut self, address: Address, amount: Value) -> Value {
        *self
            .accounts
            .entry(address)
            .and_modify(|balance| balance.add_assign(amount))
            .or_insert(amount)
    }

    fn burn(&mut self, address: Address, amount: Value) {
        if let Some(balance) = self.accounts.get_mut(&address) {
            balance
                .checked_sub(amount)
                .expect("can't burn more than balance");
        }
    }

    pub fn balance(&self, address: &Address) -> Option<Value> {
        self.accounts.get(address).copied()
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct Address([u8; 32]);
serde_bytes_newtype!(Address, 32);
display_hex_bytes_newtype!(Address);

impl Address {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl From<[u8; 32]> for Address {
    fn from(addr: [u8; 32]) -> Self {
        Self(addr)
    }
}

impl TryFrom<&[u8]> for Address {
    type Error = ();

    fn try_from(addr: &[u8]) -> Result<Self, Self::Error> {
        if addr.len() != 32 {
            return Err(());
        }
        let mut array = [0u8; 32];
        array.copy_from_slice(addr);
        Ok(Self(array))
    }
}
