use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Coin, Uint128};

use crate::state::Config;

#[cw_serde]
pub struct InstantiateMsg {
    pub protocol_fee_destination: String,
    pub protocol_fee_bps: u64,
    pub subject_fee_bps: u64,
}

#[cw_serde]
pub enum ExecuteMsg {
    BuyShares { subject: String, amount: Uint128 },
    SellShares { subject: String, amount: Uint128 },
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(Config)]
    Config {},
    #[returns(Uint128)]
    SharesBalance { subject: String, holder: String },
    #[returns(Uint128)]
    SharesSupply { subject: String },
    #[returns(Coin)]
    BuyPrice { subject: String, amount: Uint128 },
    #[returns(Coin)]
    BuyPriceAfterFee { subject: String, amount: Uint128 },
}
