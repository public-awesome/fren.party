#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    to_binary, Binary, Decimal, Deps, DepsMut, Env, MessageInfo, Response, StdResult,
};
use cw2::set_contract_version;
use cw_utils::nonpayable;

use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};
use crate::state::{Config, CONFIG};

use self::execute::buy_shares;

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:shares";
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
        ExecuteMsg::SellShares { subject, amount } => todo!(),
    }
}

pub mod execute {
    use cosmwasm_std::{coins, ensure, BankMsg, Uint128};
    use cw_utils::must_pay;
    use sg_std::NATIVE_DENOM;

    use crate::state::{Config, CONFIG, SHARES_BALANCE, SHARES_SUPPLY};

    use super::*;

    pub fn buy_shares(
        deps: DepsMut,
        info: MessageInfo,
        subject: String,
        amount: Uint128,
    ) -> Result<Response, ContractError> {
        let subject = deps.api.addr_validate(&subject)?;

        let payment = must_pay(&info, NATIVE_DENOM)?;

        let supply = SHARES_SUPPLY
            .may_load(deps.storage, subject.clone())?
            .unwrap_or_default();

        ensure!(
            supply.u128() > 0 || subject == info.sender,
            ContractError::NotSubject {
                subject: subject.into()
            }
        );

        let Config {
            protocol_fee_destination,
            protocol_fee_percent,
            subject_fee_percent,
            ..
        } = CONFIG.load(deps.storage)?;

        let price = Uint128::from(price(supply.u128(), amount.u128()));

        let protocol_fee = price * protocol_fee_percent;
        let subject_fee = price * subject_fee_percent;

        let expected_payment = (price + protocol_fee + subject_fee).into();
        ensure!(
            payment.u128() >= expected_payment,
            ContractError::NotEnoughFunds {
                expected: expected_payment,
                actual: payment.u128(),
            }
        );

        SHARES_BALANCE.update(
            deps.storage,
            (subject.clone(), info.sender),
            |balance| -> Result<_, ContractError> { Ok(balance.unwrap_or_default() + amount) },
        )?;

        SHARES_SUPPLY.update(
            deps.storage,
            subject.clone(),
            |supply| -> Result<_, ContractError> { Ok(supply.unwrap_or_default() + amount) },
        )?;

        let mut res = Response::new();

        let protocol_fee_msg = BankMsg::Send {
            to_address: protocol_fee_destination.to_string(),
            amount: coins(protocol_fee.u128(), NATIVE_DENOM),
        };

        let subject_fee_msg = BankMsg::Send {
            to_address: subject.to_string(),
            amount: coins(subject_fee.u128(), NATIVE_DENOM),
        };

        if !protocol_fee.is_zero() {
            res = res
                .add_message(protocol_fee_msg)
                .add_message(subject_fee_msg);
        }

        Ok(res.add_attribute("action", "buy_shares"))
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
    }
}

pub mod query {
    use cosmwasm_std::{coin, Coin, Uint128};
    use sg_std::NATIVE_DENOM;

    use crate::state::{SHARES_BALANCE, SHARES_SUPPLY};

    use super::*;

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
        let supply = SHARES_SUPPLY.load(deps.storage, deps.api.addr_validate(&subject)?)?;

        Ok(coin(price(supply.u128(), amount.u128()), NATIVE_DENOM))
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

        Ok(coin(
            (price.amount + protocol_fee + subject_fee).into(),
            NATIVE_DENOM,
        ))
    }
}

// This first share can only be bought by the subject.
// Price in STARS.
//
// NOTE: This can panic in debug mode if the supply and amount are both 0.
// Panic will never occurr in production since the parameters are ensured to be valid.
fn price(supply: u128, amount: u128) -> u128 {
    let sum1 = if supply == 0 {
        0
    } else {
        (supply - 1) * (supply) * (2 * (supply - 1) + 1) / 6
    };

    let sum2 = if supply == 0 && amount == 1 {
        0
    } else {
        (supply - 1 + amount) * (supply + amount) * (2 * (supply - 1 + amount) + 1) / 6
    };

    let summation = sum2 - sum1;
    println!("Summation: {summation}");

    let star = 1_000_000;
    (summation * star) / 8
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
    use cosmwasm_std::{coins, from_binary, Uint128};

    #[test]
    #[should_panic]
    fn invalid_price_arguments() {
        let supply = 0;
        let amount = 0;
        let price = price(supply, amount);
        assert_eq!(price, 0);
    }

    #[test]
    fn correct_price_for_first_share() {
        let supply = 0;
        let amount = 1;
        let price = price(supply, amount);
        assert_eq!(price, 0);
    }

    #[test]
    fn correct_price_for_second_share() {
        let supply = 1;
        let amount = 1;
        let price = price(supply, amount);
        assert_eq!(price, 125_000);
    }

    #[test]
    fn correct_price_for_third_share() {
        let supply = 2;
        let amount = 3;
        let price = price(supply, amount);
        assert_eq!(price, 3_625_000);
    }

    #[test]
    fn proper_initialization() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg {
            protocol_fee_destination: String::from("protocol_fee_destination"),
            protocol_fee_bps: 500,
            subject_fee_bps: 500,
        };
        let info = mock_info("creator", &[]);

        let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(0, res.messages.len());

        let res = query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap();
        let value: Config = from_binary(&res).unwrap();
        assert_eq!(Decimal::bps(500), value.protocol_fee_percent);
    }

    #[test]
    fn buy_shares() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg {
            protocol_fee_destination: String::from("protocol_fee_destination"),
            protocol_fee_bps: 500,
            subject_fee_bps: 500,
        };
        let subject = String::from("subject");

        let info = mock_info(&subject, &[]);
        let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        let info = mock_info("anyone", &coins(2, "ustars"));
        let msg = ExecuteMsg::BuyShares {
            subject: String::from("subject"),
            amount: Uint128::from(1u128),
        };
        let err = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
        assert_eq!(
            err,
            ContractError::NotSubject {
                subject: subject.clone()
            }
        );

        let info = mock_info(&subject, &coins(2, "ustars"));
        let msg = ExecuteMsg::BuyShares {
            subject: String::from("subject"),
            amount: Uint128::from(1u128),
        };
        let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        // the subject is the holder and should have 1 share
        let res = query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::SharesBalance {
                subject: subject.clone(),
                holder: subject.clone(),
            },
        )
        .unwrap();
        let value: Uint128 = from_binary(&res).unwrap();
        assert_eq!(Uint128::from(1u128), value);

        // the subject should also have a supply for 1 now
        let res = query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::SharesSupply { subject },
        )
        .unwrap();
        let value: Uint128 = from_binary(&res).unwrap();
        assert_eq!(Uint128::from(1u128), value);
    }
}
