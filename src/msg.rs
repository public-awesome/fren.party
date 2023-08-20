use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Coin, Uint128};

use crate::state::Config;

#[cw_serde]
pub struct InstantiateMsg {
    pub count: i32,
    pub protocol_fee_destination: String,
    pub protocol_fee_bps: u64,
    pub subject_fee_bps: u64,
}

#[cw_serde]
pub enum ExecuteMsg {
    Increment {},
    Reset { count: i32 },
    BuyShares { subject: String, amount: Uint128 },
    SellShares { subject: String, amount: Uint128 },
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    // GetCount returns the current count as a json-encoded number
    #[returns(GetCountResponse)]
    GetCount {},
    #[returns(Config)]
    Config {},
    #[returns(Coin)]
    BuyPrice { subject: String, amount: Uint128 },
}

// We define a custom struct for each query response
#[cw_serde]
pub struct GetCountResponse {
    pub count: i32,
}
