use cosmwasm_std::{ Addr, Uint256 };
use cosmwasm_schema::cw_serde;

#[cw_serde]
pub struct InstantiateMsg {
    pub apr: Uint256,
    pub injex_token: String,
    pub admin: String,
}

#[cw_serde]
pub enum QueryMsg {
    GetTotalStaked {},
    GetTotalWithdrawn {},
    GetInjexToken {},
    GetApr {},
    GetState {},
    GetStakerInfo {
        user: Addr,
    },
    GetClaimableAmount {
        user: Addr,
    },
}

#[cw_serde]
pub enum ExecuteMsg {
    Stake {},
    Claim {},
    Unstake {
        amount: Uint256,
    },
    ChangeApr {
        new_apr: Uint256,
    },
    ChangeAdmin {
        address: String,
    },
    ChangeInjexToken {
        new_injex_token: String,
    },
}
