use cosmwasm_std::StdError;
use cw_utils::PaymentError;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("{0}")]
    Payment(#[from] PaymentError),

    #[error("subject must be the first to buy shares: {subject:?}")]
    NotSubject { subject: String },

    #[error("cannot sell last share")]
    LastShare {},

    #[error("not enough shares")]
    NotEnoughShares {},

    #[error("not enough funds: {expected} got {actual}")]
    NotEnoughFunds { expected: u128, actual: u128 },

    #[error("unauthorized")]
    Unauthorized {},
}
