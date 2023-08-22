use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Decimal, StdResult, Storage, Uint128};
use cw_storage_plus::{Item, Map};

#[cw_serde]
pub struct Config {
    pub protocol_fee_destination: Addr,
    pub protocol_fee_percent: Decimal,
    pub subject_fee_percent: Decimal,
    pub curve_coefficient: Decimal,
}

pub const CONFIG: Item<Config> = Item::new("config");

// ((subject, holder), balance)
pub const SHARES_BALANCE: Map<(Addr, Addr), Uint128> = Map::new("sb");

// (subject, supply)
pub const SHARES_SUPPLY: Map<Addr, Uint128> = Map::new("ss");

pub fn load_supply(storage: &dyn Storage, subject: Addr) -> StdResult<u128> {
    Ok(SHARES_SUPPLY
        .may_load(storage, subject)?
        .unwrap_or_default()
        .u128())
}

pub fn increment_shares(
    storage: &mut dyn Storage,
    subject: Addr,
    sender: Addr,
    amount: impl Into<Uint128>,
) -> StdResult<()> {
    let amount = amount.into();

    SHARES_BALANCE.update(
        storage,
        (subject.clone(), sender),
        |balance| -> StdResult<_> { Ok(balance.unwrap_or_default() + amount) },
    )?;

    SHARES_SUPPLY.update(storage, subject, |supply| -> StdResult<_> {
        Ok(supply.unwrap_or_default() + amount)
    })?;

    Ok(())
}

pub fn decrement_shares(
    storage: &mut dyn Storage,
    subject: Addr,
    sender: Addr,
    amount: impl Into<Uint128>,
) -> StdResult<()> {
    let amount = amount.into();

    SHARES_BALANCE.update(
        storage,
        (subject.clone(), sender),
        |balance| -> StdResult<_> { Ok(balance.unwrap_or_default().checked_sub(amount)?) },
    )?;

    SHARES_SUPPLY.update(storage, subject, |supply| -> StdResult<_> {
        Ok(supply.unwrap_or_default().checked_sub(amount)?)
    })?;

    Ok(())
}
