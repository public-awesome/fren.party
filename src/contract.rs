#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{to_binary, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdResult};
use cw2::set_contract_version;

use crate::error::ContractError;
use crate::msg::{ExecuteMsg, GetCountResponse, InstantiateMsg, QueryMsg};
use crate::state::{State, STATE};

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
    let state = State {
        count: msg.count,
        owner: info.sender.clone(),
    };
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    STATE.save(deps.storage, &state)?;

    Ok(Response::new()
        .add_attribute("method", "instantiate")
        .add_attribute("owner", info.sender)
        .add_attribute("count", msg.count.to_string()))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Increment {} => todo!(),
        ExecuteMsg::Reset { count } => todo!(),
        ExecuteMsg::BuyShares { subject, amount } => buy_shares(deps, info, subject, amount),
        ExecuteMsg::SellShares { subject, amount } => todo!(),
    }
}

pub mod execute {
    use cosmwasm_std::{coins, ensure, BankMsg, StdError, Uint128};
    use cw_utils::must_pay;

    use crate::state::{Config, CONFIG, SHARES_BALANCE, SHARES_SUPPLY};

    use super::*;

    pub fn increment(deps: DepsMut) -> Result<Response, ContractError> {
        STATE.update(deps.storage, |mut state| -> Result<_, ContractError> {
            state.count += 1;
            Ok(state)
        })?;

        Ok(Response::new().add_attribute("action", "increment"))
    }

    pub fn reset(deps: DepsMut, info: MessageInfo, count: i32) -> Result<Response, ContractError> {
        STATE.update(deps.storage, |mut state| -> Result<_, ContractError> {
            if info.sender != state.owner {
                return Err(ContractError::Unauthorized {});
            }
            state.count = count;
            Ok(state)
        })?;
        Ok(Response::new().add_attribute("action", "reset"))
    }

    pub fn buy_shares(
        deps: DepsMut,
        info: MessageInfo,
        subject: String,
        amount: Uint128,
    ) -> Result<Response, ContractError> {
        let subject = deps.api.addr_validate(&subject)?;

        let payment = must_pay(&info, "ustars")?;

        let supply = SHARES_SUPPLY
            .may_load(deps.storage, subject.clone())
            .unwrap_or_default();

        ensure!(
            supply.unwrap().u128() > 0 || subject == info.sender,
            StdError::generic_err("Subject must be the first to buy shares")
        );

        let Config {
            protocol_fee_destination,
            protocol_fee_percent,
            subject_fee_percent,
            ..
        } = CONFIG.load(deps.storage)?;

        let price = Uint128::from(price(supply.unwrap().u128(), amount.u128()));
        let protocol_fee = price * protocol_fee_percent;
        let subject_fee = price * subject_fee_percent;

        ensure!(
            payment.u128() >= (price + protocol_fee + subject_fee).into(),
            StdError::generic_err("Not enough funds sent")
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

        let protocol_fee_msg = BankMsg::Send {
            to_address: protocol_fee_destination.to_string(),
            amount: coins(protocol_fee.u128(), "ustars"),
        };

        let subject_fee_msg = BankMsg::Send {
            to_address: subject.to_string(),
            amount: coins(subject_fee.u128(), "ustars"),
        };

        Ok(Response::new()
            .add_attribute("action", "buy_shares")
            .add_message(protocol_fee_msg)
            .add_message(subject_fee_msg))
    }

    // This first share can only be bought by the subject.
    // Price in STARS.
    pub fn price(supply: u128, amount: u128) -> u128 {
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

        // TODO: what is this doing?
        let summation = sum2 - sum1;
        let ether = 10u128.pow(18);

        (summation * ether) / 16000
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetCount {} => to_binary(&query::count(deps)?),
    }
}

pub mod query {
    use super::*;

    pub fn count(deps: Deps) -> StdResult<GetCountResponse> {
        let state = STATE.load(deps.storage)?;
        Ok(GetCountResponse { count: state.count })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
    use cosmwasm_std::{coins, from_binary};

    // #[test]
    // fn invalid_supply() {
    //     let supply = 0;
    //     let amount = 0;
    //     let price = execute::price(supply, amount);
    //     assert_eq!(price, 0);
    // }

    #[test]
    fn correct_price_for_first_share() {
        let supply = 0;
        let amount = 1;
        let price = execute::price(supply, amount);
        assert_eq!(price, 0);
    }

    #[test]
    fn correct_price_for_second_share() {
        let supply = 1;
        let amount = 1;
        let price = execute::price(supply, amount);
        assert_eq!(price, 0);
    }

    #[test]
    fn proper_initialization() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg { count: 17 };
        let info = mock_info("creator", &coins(1000, "earth"));

        // we can just call .unwrap() to assert this was a success
        let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(0, res.messages.len());

        // it worked, let's query the state
        let res = query(deps.as_ref(), mock_env(), QueryMsg::GetCount {}).unwrap();
        let value: GetCountResponse = from_binary(&res).unwrap();
        assert_eq!(17, value.count);
    }

    #[test]
    fn increment() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg { count: 17 };
        let info = mock_info("creator", &coins(2, "token"));
        let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        // beneficiary can release it
        let info = mock_info("anyone", &coins(2, "token"));
        let msg = ExecuteMsg::Increment {};
        let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        // should increase counter by 1
        let res = query(deps.as_ref(), mock_env(), QueryMsg::GetCount {}).unwrap();
        let value: GetCountResponse = from_binary(&res).unwrap();
        assert_eq!(18, value.count);
    }

    #[test]
    fn reset() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg { count: 17 };
        let info = mock_info("creator", &coins(2, "token"));
        let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        // beneficiary can release it
        let unauth_info = mock_info("anyone", &coins(2, "token"));
        let msg = ExecuteMsg::Reset { count: 5 };
        let res = execute(deps.as_mut(), mock_env(), unauth_info, msg);
        match res {
            Err(ContractError::Unauthorized {}) => {}
            _ => panic!("Must return unauthorized error"),
        }

        // only the original creator can reset the counter
        let auth_info = mock_info("creator", &coins(2, "token"));
        let msg = ExecuteMsg::Reset { count: 5 };
        let _res = execute(deps.as_mut(), mock_env(), auth_info, msg).unwrap();

        // should now be 5
        let res = query(deps.as_ref(), mock_env(), QueryMsg::GetCount {}).unwrap();
        let value: GetCountResponse = from_binary(&res).unwrap();
        assert_eq!(5, value.count);
    }
}
