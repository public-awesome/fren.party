use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Coin, Decimal, Event, Uint128};

use crate::state::Config;

#[cw_serde]
pub struct InstantiateMsg {
    pub protocol_fee_destination: String,
    pub protocol_fee_bps: u64,
    pub subject_fee_bps: u64,
    pub curve_coefficient: Decimal,
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
    SellPrice { subject: String, amount: Uint128 },
    #[returns(Coin)]
    BuyPriceAfterFee { subject: String, amount: Uint128 },
    #[returns(Coin)]
    SellPriceAfterFee { subject: String, amount: Uint128 },
}

pub struct TradeEvent {
    pub trader: String,
    pub subject: String,
    pub is_buy: bool,
    pub share_amount: Uint128,
    pub stars_amount: Uint128,
    pub protocol_stars_amount: Uint128,
    pub subject_stars_amount: Uint128,
    pub supply: Uint128,
}
impl TradeEvent {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        trader: impl Into<String>,
        subject: impl Into<String>,
        is_buy: bool,
        share_amount: Uint128,
        stars_amount: Uint128,
        protocol_stars_amount: Uint128,
        subject_stars_amount: Uint128,
        supply: Uint128,
    ) -> Self {
        Self {
            trader: trader.into(),
            subject: subject.into(),
            is_buy,
            share_amount,
            stars_amount,
            protocol_stars_amount,
            subject_stars_amount,
            supply,
        }
    }
}

impl From<TradeEvent> for Event {
    fn from(val: TradeEvent) -> Self {
        let mut event = Event::new("Trade".to_string());

        event = event.add_attribute("trader", val.trader);
        event = event.add_attribute("subject", val.subject);
        event = event.add_attribute("is_buy", val.is_buy.to_string());
        event = event.add_attribute("share_amount", val.share_amount.to_string());
        event = event.add_attribute("stars_amount", val.stars_amount.to_string());
        event = event.add_attribute(
            "protocol_stars_amount",
            val.protocol_stars_amount.to_string(),
        );
        event = event.add_attribute("subject_stars_amount", val.subject_stars_amount.to_string());
        event = event.add_attribute("supply", val.supply.to_string());

        event
    }
}
