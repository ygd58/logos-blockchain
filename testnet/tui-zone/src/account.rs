use std::{
    collections::HashMap,
    fmt::{self, Display, Formatter},
};

use lb_core::mantle::Value;

#[derive(Debug, Clone)]
pub struct Accounts(HashMap<Address, Account>);

impl Accounts {
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    pub fn mint(&mut self, address: Address, amount: Value) -> Value {
        self.0
            .entry(address)
            .and_modify(|account| account.balance += amount)
            .or_insert(Account {
                address,
                balance: amount,
            })
            .balance
    }
}

#[derive(Debug, Copy, Clone)]
pub struct Account {
    pub address: Address,
    pub balance: Value,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct Address([u8; 32]);

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

impl Display for Address {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", hex::encode(self.0))
    }
}
