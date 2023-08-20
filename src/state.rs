use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Addr, Coin, Uint128};
use cw_storage_plus::{Item, Map};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct State {
    pub count: i32,
    pub owner: Addr,
}

pub const STATE: Item<State> = Item::new("state");

// ((subject, holder), balance)
pub const SHARES_BALANCE: Map<(Addr, Addr), Uint128> = Map::new("sb");

// (subject, supply)
pub const SHARES_SUPPLY: Map<Addr, Uint128> = Map::new("ss");
