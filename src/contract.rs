use self::execute::{buy_shares, sell_shares};
use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};
use crate::state::{Config, CONFIG};
#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    coin, coins, to_binary, Addr, BankMsg, Binary, Coin, Decimal, Deps, DepsMut, Env, MessageInfo,
    Response, StdResult, Uint128,
};
use cw2::set_contract_version;
use cw_utils::nonpayable;
use sg_std::NATIVE_DENOM;

const CONTRACT_NAME: &str = "crates.io:fren-party";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    nonpayable(&info)?;

    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    let config = Config {
        protocol_fee_destination: deps.api.addr_validate(&msg.protocol_fee_destination)?,
        protocol_fee_percent: Decimal::bps(msg.protocol_fee_bps),
        subject_fee_percent: Decimal::bps(msg.subject_fee_bps),
        curve_coefficient: msg.curve_coefficient,
    };
    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new()
        .add_attribute("method", "instantiate")
        .add_attribute("owner", info.sender))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::BuyShares { subject, amount } => buy_shares(deps, info, subject, amount),
        ExecuteMsg::SellShares { subject, amount } => {
            sell_shares(deps, info, subject, amount.into())
        }
    }
}

pub mod execute {
    use super::*;
    use crate::{
        msg::TradeEvent,
        state::{decrement_shares, increment_shares, load_supply, Config, CONFIG, SHARES_BALANCE},
    };
    use cosmwasm_std::{ensure, Uint128};
    use cw_utils::must_pay;
    use sg_std::NATIVE_DENOM;

    pub fn buy_shares(
        deps: DepsMut,
        info: MessageInfo,
        subject: String,
        amount: Uint128,
    ) -> Result<Response, ContractError> {
        let subject = deps.api.addr_validate(&subject)?;
        let payment = must_pay(&info, NATIVE_DENOM)?.into();
        let supply = load_supply(deps.storage, subject.clone())?;

        ensure!(
            supply > 0 || subject == info.sender,
            ContractError::NotSubject {
                subject: subject.to_string()
            }
        );

        let Config {
            protocol_fee_destination,
            protocol_fee_percent,
            subject_fee_percent,
            curve_coefficient,
        } = CONFIG.load(deps.storage)?;

        let price = price(supply, amount, curve_coefficient);

        let protocol_fee = price * protocol_fee_percent;
        let subject_fee = price * subject_fee_percent;

        let expected_payment = (price + protocol_fee + subject_fee).into();
        ensure!(
            payment >= expected_payment,
            ContractError::NotEnoughFunds {
                expected: expected_payment,
                actual: payment,
            }
        );

        increment_shares(deps.storage, subject.clone(), info.sender.clone(), amount)?;

        let mut res = Response::new();

        if !protocol_fee.is_zero() {
            let protocol_fee_msg = send_msg(&protocol_fee_destination, protocol_fee);
            let subject_fee_msg = send_msg(&subject, subject_fee);
            res = res.add_messages(vec![protocol_fee_msg, subject_fee_msg]);
        }

        let event = TradeEvent::new(
            info.sender,
            subject,
            true,
            amount,
            price,
            protocol_fee,
            subject_fee,
            supply + amount.u128(),
        );

        Ok(res.add_event(event.into()))
    }

    pub fn sell_shares(
        deps: DepsMut,
        info: MessageInfo,
        subject: String,
        amount: u128,
    ) -> Result<Response, ContractError> {
        nonpayable(&info)?;

        let subject = deps.api.addr_validate(&subject)?;
        let supply = load_supply(deps.storage, subject.clone())?;

        ensure!(supply > amount, ContractError::LastShare {});

        let Config {
            protocol_fee_destination,
            protocol_fee_percent,
            subject_fee_percent,
            curve_coefficient,
        } = CONFIG.load(deps.storage)?;

        let price = price(supply - amount, amount, curve_coefficient);

        let protocol_fee = price * protocol_fee_percent;
        let subject_fee = price * subject_fee_percent;

        ensure!(
            SHARES_BALANCE.load(
                deps.as_ref().storage,
                (subject.clone(), info.sender.clone()),
            )? >= amount.into(),
            ContractError::NotEnoughShares {}
        );

        decrement_shares(deps.storage, subject.clone(), info.sender.clone(), amount)?;

        let sender_fee_msg = send_msg(&info.sender, price - protocol_fee - subject_fee);
        let protocol_fee_msg = send_msg(&protocol_fee_destination, protocol_fee);
        let subject_fee_msg = send_msg(&subject, subject_fee);

        let event = TradeEvent::new(
            info.sender,
            subject,
            false,
            amount,
            price,
            protocol_fee,
            subject_fee,
            supply - amount,
        );

        Ok(Response::new().add_event(event.into()).add_messages(vec![
            sender_fee_msg,
            protocol_fee_msg,
            subject_fee_msg,
        ]))
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&CONFIG.load(deps.storage)?),
        QueryMsg::SharesBalance { subject, holder } => {
            to_binary(&query::shares_balance(deps, subject, holder)?)
        }
        QueryMsg::SharesSupply { subject } => to_binary(&query::shares_supply(deps, subject)?),
        QueryMsg::BuyPrice { subject, amount } => {
            to_binary(&query::buy_price(deps, subject, amount)?)
        }
        QueryMsg::BuyPriceAfterFee { subject, amount } => {
            to_binary(&query::buy_price_after_fee(deps, subject, amount)?)
        }
        QueryMsg::SellPrice { subject, amount } => {
            to_binary(&query::sell_price(deps, subject, amount)?)
        }
        QueryMsg::SellPriceAfterFee { subject, amount } => {
            to_binary(&query::sell_price_after_fee(deps, subject, amount)?)
        }
    }
}

pub mod query {
    use super::*;
    use crate::state::{SHARES_BALANCE, SHARES_SUPPLY};
    use cosmwasm_std::{Coin, Uint128};

    pub fn shares_balance(deps: Deps, subject: String, holder: String) -> StdResult<Uint128> {
        let balance = SHARES_BALANCE
            .may_load(
                deps.storage,
                (
                    deps.api.addr_validate(&subject)?,
                    deps.api.addr_validate(&holder)?,
                ),
            )?
            .unwrap_or_default();

        Ok(balance)
    }

    pub fn shares_supply(deps: Deps, subject: String) -> StdResult<Uint128> {
        let supply = SHARES_SUPPLY
            .may_load(deps.storage, deps.api.addr_validate(&subject)?)?
            .unwrap_or_default();

        Ok(supply)
    }

    pub fn buy_price(deps: Deps, subject: String, amount: Uint128) -> StdResult<Coin> {
        let coefficient = CONFIG.load(deps.storage)?.curve_coefficient;
        let supply = SHARES_SUPPLY.load(deps.storage, deps.api.addr_validate(&subject)?)?;

        Ok(star(price(supply, amount, coefficient)))
    }

    pub fn sell_price(deps: Deps, subject: String, amount: Uint128) -> StdResult<Coin> {
        let coefficient = CONFIG.load(deps.storage)?.curve_coefficient;
        let supply = SHARES_SUPPLY.load(deps.storage, deps.api.addr_validate(&subject)?)?;

        Ok(star(price(supply - amount, amount, coefficient)))
    }

    pub fn buy_price_after_fee(deps: Deps, subject: String, amount: Uint128) -> StdResult<Coin> {
        let Config {
            protocol_fee_percent,
            subject_fee_percent,
            ..
        } = CONFIG.load(deps.storage)?;

        let price = buy_price(deps, subject, amount)?;

        let protocol_fee = price.amount * protocol_fee_percent;
        let subject_fee = price.amount * subject_fee_percent;

        Ok(star(price.amount + protocol_fee + subject_fee))
    }

    pub fn sell_price_after_fee(deps: Deps, subject: String, amount: Uint128) -> StdResult<Coin> {
        let Config {
            protocol_fee_percent,
            subject_fee_percent,
            ..
        } = CONFIG.load(deps.storage)?;

        let price = sell_price(deps, subject, amount)?;

        let protocol_fee = price.amount * protocol_fee_percent;
        let subject_fee = price.amount * subject_fee_percent;

        Ok(star(price.amount - protocol_fee - subject_fee))
    }
}

/// Price of shares is based on a cubic polynomial function with a fixed coefficient.
/// The first share can only be bought by the subject.
fn price(supply: impl Into<u128>, amount: impl Into<u128>, coefficient: Decimal) -> Uint128 {
    let (supply, amount) = (supply.into(), amount.into());

    let sum = |x: u128| x * (x + 1) * (2 * x + 1) / 6;

    let sum1 = if supply == 0 { 0 } else { sum(supply - 1) };

    let sum2 = if supply == 0 && amount == 1 {
        0
    } else {
        sum(supply + amount - 1)
    };

    let summation = sum2.wrapping_sub(sum1);
    let star = 1_000_000u128;

    Uint128::from(summation.wrapping_mul(star)) * coefficient
}

fn star(amount: impl Into<u128>) -> Coin {
    coin(amount.into(), NATIVE_DENOM)
}

fn stars(amount: impl Into<u128>) -> Vec<Coin> {
    coins(amount.into(), NATIVE_DENOM)
}

fn send_msg(to_address: &Addr, amount: impl Into<u128>) -> BankMsg {
    BankMsg::Send {
        to_address: to_address.to_string(),
        amount: stars(amount),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
    use cosmwasm_std::{from_binary, BankMsg, CosmosMsg, Uint128};

    fn coefficient() -> Decimal {
        Decimal::from_ratio(1u128, 8u128)
    }

    #[test]
    #[should_panic]
    fn invalid_price_arguments() {
        let supply = 0u128;
        let amount = 0u128;
        let price = price(supply, amount, coefficient()).u128();
        assert_eq!(price, 0u128);
    }

    #[test]
    fn correct_price_for_first_share() {
        let supply = 0u128;
        let amount = 1u128;
        let price = price(supply, amount, coefficient()).u128();
        assert_eq!(price, 0);
    }

    #[test]
    fn correct_price_for_second_share() {
        let supply = 1u128;
        let amount = 1u128;
        let price = price(supply, amount, coefficient()).u128();
        assert_eq!(price, 125_000);
    }

    #[test]
    fn correct_price_for_third_share() {
        let supply = 2u128;
        let amount = 3u128;
        let price = price(supply, amount, coefficient()).u128();
        assert_eq!(price, 3_625_000);
    }

    #[test]
    fn proper_initialization() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg {
            protocol_fee_destination: "protocol_fee_destination".to_string(),
            protocol_fee_bps: 500,
            subject_fee_bps: 500,
            curve_coefficient: coefficient(),
        };
        let info = mock_info("creator", &[]);

        let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(0, res.messages.len());

        let res = query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap();
        let value: Config = from_binary(&res).unwrap();
        assert_eq!(Decimal::bps(500), value.protocol_fee_percent);
    }

    #[test]
    fn buy_and_sell_shares() {
        let mut deps = mock_dependencies();
        let protocol_fee_destination = "protocol_fee_destination";

        let msg = InstantiateMsg {
            protocol_fee_destination: protocol_fee_destination.to_string(),
            protocol_fee_bps: 500,
            subject_fee_bps: 500,
            curve_coefficient: coefficient(),
        };

        let subject = "subject";

        let info = mock_info(subject, &[]);
        let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        let info = mock_info("anyone", &stars(2u128));
        let msg = ExecuteMsg::BuyShares {
            subject: subject.to_string(),
            amount: Uint128::from(1u128),
        };
        let err = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
        assert_eq!(
            err,
            ContractError::NotSubject {
                subject: subject.to_string()
            }
        );

        let info = mock_info(subject, &stars(2u128));
        let msg = ExecuteMsg::BuyShares {
            subject: subject.to_string(),
            amount: Uint128::from(1u128),
        };
        let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        // the subject is the holder and should have 1 share
        let res = query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::SharesBalance {
                subject: subject.to_string(),
                holder: subject.to_string(),
            },
        )
        .unwrap();
        let value: Uint128 = from_binary(&res).unwrap();
        assert_eq!(Uint128::from(1u128), value);

        // the subject should also have a supply for 1 now
        let res = query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::SharesSupply {
                subject: subject.to_string(),
            },
        )
        .unwrap();
        let value: Uint128 = from_binary(&res).unwrap();
        assert_eq!(Uint128::from(1u128), value);

        // check the buy price for friend to buy shares
        let res = query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::BuyPrice {
                subject: subject.to_string(),
                amount: Uint128::from(10u128),
            },
        )
        .unwrap();
        let value: Coin = from_binary(&res).unwrap();
        let friend_buy_price = value.amount.u128();
        assert_eq!(friend_buy_price, 48_125_000u128);

        // check the buy price with fees
        let res = query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::BuyPriceAfterFee {
                subject: subject.to_string(),
                amount: Uint128::from(10u128),
            },
        )
        .unwrap();
        let value: Coin = from_binary(&res).unwrap();
        assert_eq!(value.amount.u128(), 52_937_500u128);

        // buy the same subject's shares as another friend
        let friend = "friend";
        let info = mock_info(friend, &stars(52_937_500u128));
        let msg = ExecuteMsg::BuyShares {
            subject: subject.to_string(),
            amount: Uint128::from(10u128),
        };
        let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(2, res.messages.len());
        assert_eq!(
            CosmosMsg::Bank(BankMsg::Send {
                to_address: protocol_fee_destination.to_string(),
                amount: stars(2_406_250u128)
            }),
            res.messages[0].msg
        );

        // friend should now have a balance of subject's shares
        let res = query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::SharesBalance {
                subject: subject.to_string(),
                holder: friend.to_string(),
            },
        )
        .unwrap();
        let value: Uint128 = from_binary(&res).unwrap();
        assert_eq!(Uint128::from(10u128), value);

        // the subject should have increased supply since friend bought shares
        let res = query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::SharesSupply {
                subject: subject.to_string(),
            },
        )
        .unwrap();
        let value: Uint128 = from_binary(&res).unwrap();
        assert_eq!(Uint128::from(11u128), value);

        // check the sell price for the friend
        let res = query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::SellPrice {
                subject: subject.to_string(),
                amount: Uint128::from(10u128),
            },
        )
        .unwrap();
        let value: Coin = from_binary(&res).unwrap();
        assert_eq!(value.amount.u128(), friend_buy_price);

        // check the sell price after fees
        // this is what the shares seller actually gets
        let res = query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::SellPriceAfterFee {
                subject: subject.to_string(),
                amount: Uint128::from(10u128),
            },
        )
        .unwrap();
        let value: Coin = from_binary(&res).unwrap();
        let sell_price_after_fees = value.amount.u128();
        assert_eq!(sell_price_after_fees, 43_312_500u128);

        // friend sells shares to be back at the previous state
        let info = mock_info(friend, &[]);
        let msg = ExecuteMsg::SellShares {
            subject: subject.to_string(),
            amount: Uint128::from(10u128),
        };
        let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(3, res.messages.len());
        // friend lost money on their trade due to fees
        assert_eq!(
            CosmosMsg::Bank(BankMsg::Send {
                to_address: friend.to_string(),
                amount: stars(sell_price_after_fees)
            }),
            res.messages[0].msg
        );
        assert_eq!(
            CosmosMsg::Bank(BankMsg::Send {
                to_address: protocol_fee_destination.to_string(),
                amount: stars(2_406_250u128)
            }),
            res.messages[1].msg
        );

        // friend should now have reset their shares of subject
        let res = query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::SharesBalance {
                subject: subject.to_string(),
                holder: friend.to_string(),
            },
        )
        .unwrap();
        let value: Uint128 = from_binary(&res).unwrap();
        assert_eq!(Uint128::from(0u128), value);

        // the subject should have gone back to the previous supply
        let res = query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::SharesSupply {
                subject: subject.to_string(),
            },
        )
        .unwrap();
        let value: Uint128 = from_binary(&res).unwrap();
        assert_eq!(Uint128::from(1u128), value);
    }
}
