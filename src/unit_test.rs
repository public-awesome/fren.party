use crate::contract::{execute, instantiate, price, query};
use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};
use crate::state::Config;

use super::*;
use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
use cosmwasm_std::{from_binary, BankMsg, Coin, CosmosMsg, Decimal, Uint128};
use sg_std::stars;

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
