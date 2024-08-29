use std::str::FromStr;

#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    to_json_binary,
    Addr,
    BankMsg,
    Binary,
    Coin,
    CosmosMsg,
    Deps,
    DepsMut,
    Env,
    MessageInfo,
    Response,
    StdError,
    StdResult,
    Timestamp,
    Uint128,
    Uint256,
};
use cw2::set_contract_version;

use crate::error::ContractError;
use crate::msg::{ ExecuteMsg, InstantiateMsg, QueryMsg };
use crate::state::{ Config, StakerInfo, State, ADMIN, CONFIG, PERCENTS, STATE, USER_STAKINGS };

// version info for migration info
const CONTRACT_NAME: &str = "injex-staking";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");
const SECONDS_IN_YEAR: Uint256 = Uint256::from_u128(31_536_000_u128);
pub const ONE: Uint256 = Uint256::from_u128(1000000000000000000_u128);

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg
) -> Result<Response, ContractError> {
    let config = Config {
        apr: msg.apr,
        injex_token: msg.injex_token,
    };

    let ci_current = ONE;

    let state = State {
        total_withdrawn: Uint256::zero(),
        total_staked: Uint256::zero(),
        ci_current,
        ci_time_current: _env.block.time,
    };

    let admin = msg.admin;
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    CONFIG.save(deps.storage, &config)?;
    ADMIN.save(deps.storage, &deps.api.addr_validate(&admin)?)?;
    STATE.save(deps.storage, &state)?;

    Ok(
        Response::new()
            .add_attribute("method", "instantiate")
            .add_attribute("owner", info.sender)
            .add_attribute("apr", msg.apr.to_string())
    )
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: ExecuteMsg
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Stake {} => stake(deps, _env, info),
        ExecuteMsg::Claim {} => claim_rewards(deps, _env, info),
        ExecuteMsg::Unstake { amount } => unstake(deps, _env, info, amount),
        ExecuteMsg::ChangeApr { new_apr } => change_apr(deps, _env, info, new_apr),
        ExecuteMsg::ChangeAdmin { address } => change_admin(deps, info, address),
        ExecuteMsg::ChangeInjexToken { new_injex_token } =>
            change_injex_token(deps, info, new_injex_token),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetState {} => to_json_binary(&query_state(deps)?),
        QueryMsg::GetApr {} => to_json_binary(&query_apr(deps)?),
        QueryMsg::GetInjexToken {} => to_json_binary(&query_injex_token(deps)?),
        QueryMsg::GetTotalStaked {} => to_json_binary(&query_total_staked(deps)?),
        QueryMsg::GetTotalWithdrawn {} => to_json_binary(&query_total_withdrawn(deps)?),
        QueryMsg::GetStakerInfo { user } => to_json_binary(&query_staker_indo(deps, user)?),
        QueryMsg::GetClaimableAmount { user } =>
            to_json_binary(&query_claimable_tokens(deps, _env, user)?),
    }
}

pub fn stake(deps: DepsMut, env: Env, info: MessageInfo) -> Result<Response, ContractError> {
    if info.funds.len() != 1 {
        return Err(ContractError::InvalidFunds {});
    }

    let config = CONFIG.load(deps.storage).unwrap();

    let coin = &info.funds[0];

    if coin.denom != config.injex_token {
        return Err(ContractError::InvalidCoin {});
    }

    let amount = Uint256::from_uint128(coin.amount);
    let (new_ci, curr_block_time) = get_new_ci(&deps, env).unwrap();

    USER_STAKINGS.update(
        deps.storage,
        info.sender.clone(),
        |staking| -> Result<StakerInfo, StdError> {
            let mut staking = staking.unwrap_or_else(|| StakerInfo {
                block_time: curr_block_time,
                ci_0: Uint256::zero(),
                reward: Uint256::zero(),
                staked: Uint256::zero(),
            });

            if staking.staked != Uint256::zero() {
                let reward = calculate_reward(staking.staked, new_ci, staking.ci_0).unwrap();

                staking.reward += reward;
                staking.staked += amount;
            } else {
                staking.staked = amount;
            }

            staking.ci_0 = new_ci;

            Ok(staking)
        }
    ).unwrap();

    STATE.update(
        deps.storage,
        |mut state| -> Result<State, StdError> {
            state.ci_current = new_ci;
            state.ci_time_current = curr_block_time;
            state.total_staked += amount;

            Ok(state)
        }
    ).unwrap();

    Ok(
        Response::new()
            .add_attribute("user", info.sender.clone())
            .add_attribute("amount_staked", amount)
            .add_attribute("method", "execute_stake")
    )
}

pub fn unstake(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    amount: Uint256
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage).unwrap();

    let staking_info = USER_STAKINGS.load(deps.storage, info.sender.clone()).unwrap_or_else(
        |_| StakerInfo {
            block_time: Timestamp::from_seconds(0),
            ci_0: Uint256::zero(),
            reward: Uint256::zero(),
            staked: Uint256::zero(),
        }
    );

    if staking_info.staked == Uint256::zero() {
        return Err(ContractError::CannotUnstake {});
    } else if staking_info.staked < amount {
        return Err(ContractError::CannotUnstakeAmount {});
    }

    let (new_ci, curr_block_time) = get_new_ci(&deps, env).unwrap();

    USER_STAKINGS.update(
        deps.storage,
        info.sender.clone(),
        |staking| -> Result<StakerInfo, StdError> {
            let mut staking = staking.unwrap();

            let reward = calculate_reward(staking.staked, new_ci, staking.ci_0).unwrap();

            staking.reward += reward;
            staking.staked -= amount;

            staking.ci_0 = new_ci;

            Ok(staking)
        }
    ).unwrap();

    STATE.update(
        deps.storage,
        |mut state| -> Result<State, StdError> {
            state.ci_current = new_ci;
            state.ci_time_current = curr_block_time;
            state.total_staked -= amount;

            Ok(state)
        }
    ).unwrap();

    let swap_msg = BankMsg::Send {
        to_address: info.sender.clone().to_string(),
        amount: vec![Coin {
            amount: Uint128::from_str(&amount.to_string())?,
            denom: config.injex_token.to_string(),
        }],
    };

    Ok(
        Response::new()
            .add_message(CosmosMsg::Bank(swap_msg))
            .add_attribute("user", info.sender.clone())
            .add_attribute("amount_unstaked", amount)
            .add_attribute("method", "execute_unstake")
    )
}

pub fn claim_rewards(
    deps: DepsMut,
    env: Env,
    info: MessageInfo
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage).unwrap();
    let state = STATE.load(deps.storage).unwrap();

    let staking_info = USER_STAKINGS.load(deps.storage, info.sender.clone()).unwrap_or_else(
        |_| StakerInfo {
            block_time: Timestamp::from_seconds(0),
            ci_0: Uint256::zero(),
            reward: Uint256::zero(),
            staked: Uint256::zero(),
        }
    );

    let (new_ci, curr_block_time) = get_new_ci(&deps, env.clone()).unwrap();

    let reward =
        calculate_reward(staking_info.staked, new_ci, staking_info.ci_0).unwrap() +
        staking_info.reward;

    if reward == Uint256::zero() {
        return Err(ContractError::CannotClaim {});
    }

    let balance_res = deps.querier.query_balance(
        env.clone().contract.address.to_string(),
        config.injex_token.clone()
    )?;

    let balance = balance_res.amount;

    if Uint256::from_uint128(balance) < reward + state.total_staked {
        return Err(ContractError::InsufficientContractBalance {});
    }

    USER_STAKINGS.update(
        deps.storage,
        info.sender.clone(),
        |staking| -> Result<StakerInfo, StdError> {
            let mut staking = staking.unwrap();

            staking.reward = Uint256::zero();

            staking.ci_0 = new_ci;

            Ok(staking)
        }
    ).unwrap();

    STATE.update(
        deps.storage,
        |mut state| -> Result<State, StdError> {
            state.ci_current = new_ci;
            state.ci_time_current = curr_block_time;
            state.total_withdrawn += reward;

            Ok(state)
        }
    ).unwrap();

    let swap_msg = BankMsg::Send {
        to_address: info.sender.clone().to_string(),
        amount: vec![Coin {
            amount: Uint128::from_str(&reward.to_string())?,
            denom: config.injex_token.to_string(),
        }],
    };

    Ok(
        Response::new()
            .add_message(CosmosMsg::Bank(swap_msg))
            .add_attribute("user", info.sender.clone())
            .add_attribute("amount_claimed", reward)
            .add_attribute("method", "execute_claim")
    )
}

pub fn change_apr(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    new_apr: Uint256
) -> Result<Response, ContractError> {
    let admin = ADMIN.load(deps.storage).unwrap();

    if admin != info.sender.clone() {
        return Err(ContractError::OnlyAdmin {});
    }

    if new_apr == Uint256::zero() {
        return Err(ContractError::InvalidApr {});
    }

    let (new_ci, curr_block_time) = get_new_ci(&deps, env).unwrap();

    STATE.update(
        deps.storage,
        |mut state| -> Result<State, StdError> {
            state.ci_current = new_ci;
            state.ci_time_current = curr_block_time;
            Ok(state)
        }
    ).unwrap();
    CONFIG.update(
        deps.storage,
        |mut config| -> Result<Config, StdError> {
            config.apr = new_apr;

            Ok(config)
        }
    ).unwrap();

    Ok(Response::new().add_attribute("new_apr", new_apr).add_attribute("method", "execute_new_apr"))
}

pub fn change_injex_token(
    deps: DepsMut,
    info: MessageInfo,
    new_injex_token: String
) -> Result<Response, ContractError> {
    let admin = ADMIN.load(deps.storage).unwrap();

    if admin != info.sender.clone() {
        return Err(ContractError::OnlyAdmin {});
    }

    CONFIG.update(
        deps.storage,
        |mut config| -> Result<Config, StdError> {
            config.injex_token = new_injex_token;

            Ok(config)
        }
    ).unwrap();

    Ok(Response::new().add_attribute("method", "execute_new_injex_token"))
}

pub fn change_admin(
    deps: DepsMut,
    info: MessageInfo,
    new_admin: String
) -> Result<Response, ContractError> {
    let admin = ADMIN.load(deps.storage).unwrap();

    if admin != info.sender.clone() {
        return Err(ContractError::OnlyAdmin {});
    }

    ADMIN.save(deps.storage, &deps.api.addr_validate(&new_admin)?)?;

    Ok(Response::new().add_attribute("method", "execute_new_admin"))
}

fn get_new_ci(deps: &DepsMut, env: Env) -> StdResult<(Uint256, Timestamp)> {
    let config = CONFIG.load(deps.storage).unwrap();
    let state = STATE.load(deps.storage).unwrap();

    let curr_ci = state.ci_current;
    let apr = config.apr;
    let curr_block_time = env.block.time;
    let new_ci = if state.total_staked == Uint256::zero() {
        state.ci_current
    } else {
        let old_block_time = state.ci_time_current;

        calculate_ci(
            curr_ci,
            apr,
            Uint256::from_u128((curr_block_time.seconds() - old_block_time.seconds()).into())
        ).unwrap()
    };

    Ok((new_ci, curr_block_time))
}

fn calculate_ci(curr_ci: Uint256, apr: Uint256, time_elapsed: Uint256) -> StdResult<Uint256> {
    let new_ci =
        (curr_ci * (ONE + (apr * time_elapsed * ONE) / (SECONDS_IN_YEAR * PERCENTS))) / ONE;

    Ok(new_ci)
}

fn calculate_reward(tokens_staked: Uint256, ci_last: Uint256, ci_0: Uint256) -> StdResult<Uint256> {
    let reward = (tokens_staked * (ci_last - ci_0)) / ONE;

    Ok(reward)
}

pub fn query_state(deps: Deps) -> StdResult<State> {
    let state = STATE.load(deps.storage).unwrap();

    Ok(state)
}

pub fn query_total_staked(deps: Deps) -> StdResult<Uint256> {
    let state = STATE.load(deps.storage).unwrap();

    Ok(state.total_staked)
}

pub fn query_total_withdrawn(deps: Deps) -> StdResult<Uint256> {
    let state = STATE.load(deps.storage).unwrap();

    Ok(state.total_withdrawn)
}

pub fn query_injex_token(deps: Deps) -> StdResult<Addr> {
    let config = CONFIG.load(deps.storage).unwrap();

    Ok(Addr::unchecked(config.injex_token))
}

pub fn query_apr(deps: Deps) -> StdResult<Uint256> {
    let config = CONFIG.load(deps.storage).unwrap();

    Ok(config.apr)
}

pub fn query_staker_indo(deps: Deps, user: Addr) -> StdResult<StakerInfo> {
    let info = USER_STAKINGS.load(deps.storage, user).unwrap();

    Ok(info)
}

pub fn query_claimable_tokens(deps: Deps, env: Env, user: Addr) -> StdResult<Uint256> {
    let info = USER_STAKINGS.load(deps.storage, user).unwrap();
    let config = CONFIG.load(deps.storage).unwrap();
    let state = STATE.load(deps.storage).unwrap();

    let curr_ci = state.ci_current;
    let apr = config.apr;
    let curr_block_time = env.block.time;
    let new_ci = if state.total_staked == Uint256::zero() {
        state.ci_current
    } else {
        let old_block_time = state.ci_time_current;

        calculate_ci(
            curr_ci,
            apr,
            Uint256::from_u128((curr_block_time.seconds() - old_block_time.seconds()).into())
        ).unwrap()
    };

    let reward = calculate_reward(info.staked, new_ci, info.ci_0).unwrap() + info.reward;

    Ok(reward)
}
