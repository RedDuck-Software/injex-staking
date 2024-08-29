use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Addr, Timestamp, Uint256};
use cw_storage_plus::{Item, Map};

// 100%
pub const PERCENTS: Uint256 = Uint256::from_u128(10_000_u128);

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    pub apr: Uint256,
    pub injex_token: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct State {
    pub total_staked: Uint256,
    pub total_withdrawn: Uint256,
    pub ci_current: Uint256,
    pub ci_time_current: Timestamp
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct StakerInfo {
    pub staked: Uint256,
    pub block_time: Timestamp,
    pub ci_0: Uint256,
    pub reward: Uint256,
}


pub const CONFIG: Item<Config> = Item::new("config");

pub const STATE: Item<State> = Item::new("state");

pub const ADMIN: Item<Addr> = Item::new("admin");

pub const USER_STAKINGS: Map<Addr, StakerInfo> = Map::new("user_stakings");
