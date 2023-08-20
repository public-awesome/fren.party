use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Decimal, Uint128};
use cw_storage_plus::{Item, Map};

#[cw_serde]
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
