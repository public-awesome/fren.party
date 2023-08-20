use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Addr, Coin, Decimal, Uint128};
use cw_storage_plus::{Item, Map};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct State {
    pub count: i32,
    pub owner: Addr,
}

pub const STATE: Item<State> = Item::new("state");

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct Config {
    pub protocol_fee_destination: Addr,
    pub protocol_fee_percent: Decimal,
    pub subject_fee_percent: Decimal,
}

pub const CONFIG: Item<Config> = Item::new("config");

// ((subject, holder), balance)
pub const SHARES_BALANCE: Map<(Addr, Addr), Uint128> = Map::new("sb");

// (subject, supply)
pub const SHARES_SUPPLY: Map<Addr, Uint128> = Map::new("ss");
