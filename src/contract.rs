#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    coins, to_binary, Binary, Coin, Decimal, Deps, DepsMut, Env, MessageInfo, Response, StdResult,
};
use cw2::set_contract_version;
use cw_utils::nonpayable;
use sg_std::NATIVE_DENOM;

use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};
use crate::state::{Config, CONFIG};

use self::execute::{buy_shares, sell_shares};

const CONTRACT_NAME: &str = "crates.io:stargaze-shares";
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
        ExecuteMsg::SellShares { subject, amount } => {
            sell_shares(deps, info, subject, amount.into())
        }
    }
}

pub mod execute {
    use cosmwasm_std::{ensure, BankMsg, Uint128};
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

        let payment = must_pay(&info, NATIVE_DENOM)?.u128();

        let supply = SHARES_SUPPLY
            .may_load(deps.storage, subject.clone())?
            .unwrap_or_default()
            .u128();

        ensure!(
            supply > 0 || subject == info.sender,
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

        let price: Uint128 = price(supply, amount.u128()).into();

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
            amount: stars(protocol_fee),
        };

        let subject_fee_msg = BankMsg::Send {
            to_address: subject.to_string(),
            amount: stars(subject_fee),
        };

        if !protocol_fee.is_zero() {
            res = res
                .add_message(protocol_fee_msg)
                .add_message(subject_fee_msg);
        }

        Ok(res.add_attribute("action", "buy_shares"))
    }

    pub fn sell_shares(
        deps: DepsMut,
        info: MessageInfo,
        subject: String,
        amount: u128,
    ) -> Result<Response, ContractError> {
        nonpayable(&info)?;

        let subject = deps.api.addr_validate(&subject)?;

        let supply = SHARES_SUPPLY
            .may_load(deps.storage, subject.clone())?
            .unwrap_or_default()
            .u128();

        ensure!(supply > amount, ContractError::LastShare {});

        let Config {
            protocol_fee_destination,
            protocol_fee_percent,
            subject_fee_percent,
            ..
        } = CONFIG.load(deps.storage)?;

        let price = Uint128::from(price(supply - amount, amount));

        let protocol_fee = price * protocol_fee_percent;
        let subject_fee = price * subject_fee_percent;

        ensure!(
            SHARES_BALANCE.load(
                deps.as_ref().storage,
                (subject.clone(), info.sender.clone()),
            )? >= amount.into(),
            ContractError::NotEnoughShares {}
        );

        SHARES_BALANCE.update(
            deps.storage,
            (subject.clone(), info.sender.clone()),
            |balance| -> Result<_, ContractError> {
                Ok(balance.unwrap_or_default() - Uint128::from(amount))
            },
        )?;

        SHARES_SUPPLY.update(
            deps.storage,
            subject.clone(),
            |supply| -> Result<_, ContractError> {
                Ok(supply.unwrap_or_default() - Uint128::from(amount))
            },
        )?;

        let sender_fee_msg = BankMsg::Send {
            to_address: info.sender.to_string(),
            amount: stars(price - protocol_fee - subject_fee),
        };

        let protocol_fee_msg = BankMsg::Send {
            to_address: protocol_fee_destination.to_string(),
            amount: stars(protocol_fee),
        };

        let subject_fee_msg = BankMsg::Send {
            to_address: subject.to_string(),
            amount: stars(subject_fee),
        };

        Ok(Response::new()
            .add_attribute("action", "sell_shares")
            .add_messages(vec![sender_fee_msg, protocol_fee_msg, subject_fee_msg]))
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

    pub fn sell_price(deps: Deps, subject: String, amount: Uint128) -> StdResult<Coin> {
        let supply = SHARES_SUPPLY.load(deps.storage, deps.api.addr_validate(&subject)?)?;

        Ok(coin(
            price(supply.u128() - amount.u128(), amount.u128()),
            NATIVE_DENOM,
        ))
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

    pub fn sell_price_after_fee(deps: Deps, subject: String, amount: Uint128) -> StdResult<Coin> {
        let Config {
            protocol_fee_percent,
            subject_fee_percent,
            ..
        } = CONFIG.load(deps.storage)?;

        let price = sell_price(deps, subject, amount)?;

        let protocol_fee = price.amount * protocol_fee_percent;
        let subject_fee = price.amount * subject_fee_percent;

        Ok(coin(
            (price.amount - protocol_fee - subject_fee).into(),
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

fn stars(amount: impl Into<u128>) -> Vec<Coin> {
    coins(amount.into(), NATIVE_DENOM)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
    use cosmwasm_std::{from_binary, BankMsg, CosmosMsg, Uint128};

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
    fn buy_and_sell_shares() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg {
            protocol_fee_destination: String::from("protocol_fee_destination"),
            protocol_fee_bps: 500,
            subject_fee_bps: 500,
        };
        let subject = String::from("subject");

        let info = mock_info(&subject, &[]);
        let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        let info = mock_info("anyone", &stars(2u128));
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

        let info = mock_info(&subject, &stars(2u128));
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
            QueryMsg::SharesSupply {
                subject: subject.clone(),
            },
        )
        .unwrap();
        let value: Uint128 = from_binary(&res).unwrap();
        assert_eq!(Uint128::from(1u128), value);

        // buy the same subject's shares as another friend
        let friend = String::from("friend");
        let info = mock_info(&friend, &stars(52_937_500u128));
        let msg = ExecuteMsg::BuyShares {
            subject: String::from("subject"),
            amount: Uint128::from(10u128),
        };
        let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(2, res.messages.len());
        assert_eq!(
            CosmosMsg::Bank(BankMsg::Send {
                to_address: String::from("protocol_fee_destination"),
                amount: stars(2_406_250u128)
            }),
            res.messages[0].msg
        );

        // friend should now have a balance of subject's shares
        let res = query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::SharesBalance {
                subject: subject.clone(),
                holder: friend.clone(),
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
                subject: subject.clone(),
            },
        )
        .unwrap();
        let value: Uint128 = from_binary(&res).unwrap();
        assert_eq!(Uint128::from(11u128), value);

        // friend sells shares to be back at the previous state
        let info = mock_info(&friend, &[]);
        let msg = ExecuteMsg::SellShares {
            subject: String::from("subject"),
            amount: Uint128::from(10u128),
        };
        let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(3, res.messages.len());
        // friend lost money on their trade due to fees
        // TODO: this is not the correct amount, but it's close
        // let expected_amount = 52_937_500u128 - 2 * 2_406_250u128;
        let expected_amount = 43_312_500u128;
        assert_eq!(
            CosmosMsg::Bank(BankMsg::Send {
                to_address: String::from("friend"),
                amount: stars(expected_amount)
            }),
            res.messages[0].msg
        );
        assert_eq!(
            CosmosMsg::Bank(BankMsg::Send {
                to_address: String::from("protocol_fee_destination"),
                amount: stars(2_406_250u128)
            }),
            res.messages[1].msg
        );

        // friend should now have reset their shares of subject
        let res = query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::SharesBalance {
                subject: subject.clone(),
                holder: friend,
            },
        )
        .unwrap();
        let value: Uint128 = from_binary(&res).unwrap();
        assert_eq!(Uint128::from(0u128), value);

        // the subject should have gone back to the previous supply
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
