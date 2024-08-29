use cosmwasm_std::StdError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ContractError {
    #[error("{0}")] Std(#[from] StdError),

    #[error("Unauthorized")] Unauthorized {},

    #[error("Invalid funds were provided")] InvalidFunds {},

    #[error("Invalid coin passed in funds")] InvalidCoin {},

    #[error("No tokens were staked")] CannotUnstake {},

    #[error("Insufficient balance")] CannotUnstakeAmount {},

    #[error("No claims")] CannotClaim {},

    #[error("Insufficient contract balance")] InsufficientContractBalance {},

    #[error("Invalid APR")] InvalidApr {},

    #[error("Only admin")] OnlyAdmin {},
}
