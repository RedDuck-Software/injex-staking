#[cfg(test)]
mod tests {
    use cosmwasm_std::testing::mock_env;
    use cosmwasm_std::{ Addr, BlockInfo, Coin, Timestamp, Uint128, Uint256 };
    use cw_multi_test::{ App, Executor };
    use crate::contract::ONE;
    use crate::helpers::CwTemplateContract;
    use crate::integration_tests::tests::{
        expect_error,
        proper_instantiate,
        INJEX_TOKEN,
        USDT,
        USER,
    };
    use crate::msg::{ ExecuteMsg, QueryMsg };
    use crate::state::{ StakerInfo, State, PERCENTS };

    const APR: Uint256 = Uint256::from_u128(2_000_u128);
    const SECONDS_IN_YEAR: Uint256 = Uint256::from_u128(31_536_000_u128);

    fn calculate_ci(curr_ci: Uint256, apr: Uint256, time_elapsed: Uint256) -> Uint256 {
        let new_ci =
            (curr_ci * (ONE + (apr * time_elapsed * ONE) / (SECONDS_IN_YEAR * PERCENTS))) / ONE;

        new_ci
    }

    fn calculate_reward(tokens_staked: Uint256, ci_last: Uint256, ci_0: Uint256) -> Uint256 {
        let reward = (tokens_staked * (ci_last - ci_0)) / ONE;

        reward
    }

    #[test]
    fn proper_initialization() {
        let (app, contract) = proper_instantiate(true);

        let token_msg = QueryMsg::GetInjexToken {};
        let apr_msg = QueryMsg::GetApr {};

        let injex_token: Addr = app.wrap().query_wasm_smart(contract.addr(), &token_msg).unwrap();
        let apr: Uint256 = app.wrap().query_wasm_smart(contract.addr(), &apr_msg).unwrap();

        assert_eq!(INJEX_TOKEN, injex_token.to_string());
        assert_eq!(APR, apr);
    }

    #[test]
    fn stake_no_funds() {
        let (mut app, contract) = proper_instantiate(true);

        let msg = ExecuteMsg::Stake {};

        let res = app.execute_contract(Addr::unchecked(USER), contract.addr(), &msg, &[]);

        assert!(res.is_err());

        let error_message = format!("Invalid funds were provided");
        expect_error(res, error_message);
    }

    #[test]
    fn stake_invalid_token() {
        let (mut app, contract) = proper_instantiate(true);

        let msg = ExecuteMsg::Stake {};

        let res = app.execute_contract(
            Addr::unchecked(USER),
            contract.addr(),
            &msg,
            &vec![Coin {
                denom: USDT.to_string(),
                amount: Uint128::new(1_000_000),
            }]
        );

        assert!(res.is_err());

        let error_message = format!("Invalid coin passed in funds");
        expect_error(res, error_message);
    }

    #[test]
    fn stake_two_tokens() {
        let (mut app, contract) = proper_instantiate(true);

        let msg = ExecuteMsg::Stake {};

        let res = app.execute_contract(
            Addr::unchecked(USER),
            contract.addr(),
            &msg,
            &vec![
                Coin {
                    denom: USDT.to_string(),
                    amount: Uint128::new(1_000_000),
                },
                Coin {
                    denom: USDT.to_string(),
                    amount: Uint128::new(1_000_000),
                }
            ]
        );

        assert!(res.is_err());

        let error_message = format!("Invalid funds were provided");
        expect_error(res, error_message);
    }

    #[test]
    fn stake() {
        let (mut app, contract) = proper_instantiate(true);

        let stake_amount = Uint256::from_u128(1_000_000_u128);
        let state_msg = QueryMsg::GetState {};

        let balance = app.wrap().query_balance(USER.to_string(), INJEX_TOKEN.to_string()).unwrap();

        let state_before: State = app.wrap().query_wasm_smart(contract.addr(), &state_msg).unwrap();
        assert_eq!(state_before.ci_current, ONE);
        assert_eq!(state_before.total_staked, Uint256::zero());

        stake_internal(&mut app, contract, stake_amount, true);

        let balance_after = app
            .wrap()
            .query_balance(USER.to_string(), INJEX_TOKEN.to_string())
            .unwrap();

        assert_eq!(
            Uint256::from_uint128(balance_after.amount),
            Uint256::from_uint128(balance.amount) - stake_amount
        );
    }

    #[test]
    fn multiple_stake() {
        let (mut app, contract) = proper_instantiate(true);

        let stake_amount = ONE;
        let block_time = mock_env().block.time;
        let state_msg = QueryMsg::GetState {};
        let apr_msg = QueryMsg::GetApr {};

        let state_before: State = app.wrap().query_wasm_smart(contract.addr(), &state_msg).unwrap();
        let apr: Uint256 = app.wrap().query_wasm_smart(contract.addr(), &apr_msg).unwrap();
        assert_eq!(state_before.ci_current, ONE);
        assert_eq!(state_before.total_staked, Uint256::zero());
        let balance = app.wrap().query_balance(USER.to_string(), INJEX_TOKEN.to_string()).unwrap();

        let (user_staking, state) = stake_internal(&mut app, contract.clone(), stake_amount, true);

        let block_info = app.block_info();

        app.set_block(BlockInfo {
            chain_id: block_info.chain_id,
            height: block_info.height,
            time: block_info.time.plus_seconds(200),
        });

        let new_block_time: Timestamp = app.block_info().time;

        let (new_user_staking, new_state) = stake_internal(
            &mut app,
            contract.clone(),
            stake_amount,
            false
        );

        let new_ci = calculate_ci(
            state.ci_current,
            apr,
            (new_block_time.seconds() - block_time.seconds()).into()
        );
        let reward = calculate_reward(stake_amount, new_state.ci_current, user_staking.ci_0);

        assert_eq!(new_state.ci_current, new_ci);
        assert_eq!(new_state.ci_time_current, new_block_time);
        assert_eq!(new_state.total_staked, state.total_staked + stake_amount);

        assert_eq!(new_user_staking.block_time, block_time);
        assert_eq!(new_user_staking.ci_0, new_state.ci_current);
        assert_eq!(new_user_staking.staked, state.total_staked + stake_amount);
        assert_eq!(new_user_staking.reward, reward);

        let reward_calculated =
            (stake_amount * Uint256::from_u128(200_u128) * apr) / (SECONDS_IN_YEAR * PERCENTS);

        assert_eq!(reward_calculated, new_user_staking.reward);

        let balance_after = app
            .wrap()
            .query_balance(USER.to_string(), INJEX_TOKEN.to_string())
            .unwrap();

        assert_eq!(
            Uint256::from_uint128(balance_after.amount),
            Uint256::from_uint128(balance.amount) - (stake_amount + stake_amount)
        );
    }

    #[test]
    fn unstake() {
        let (mut app, contract) = proper_instantiate(true);

        let stake_amount = ONE + ONE;
        let block_time = mock_env().block.time;
        let state_msg = QueryMsg::GetState {};
        let apr_msg = QueryMsg::GetApr {};

        let amount_to_unstake: Uint256 = stake_amount * Uint256::from_u128(2_u128);

        let msg = ExecuteMsg::Unstake { amount: amount_to_unstake };
        let res = app.execute_contract(Addr::unchecked(USER), contract.addr(), &msg, &[]);

        assert!(res.is_err());

        let error_message = format!("No tokens were staked");
        expect_error(res, error_message);

        let state_before: State = app.wrap().query_wasm_smart(contract.addr(), &state_msg).unwrap();
        let apr: Uint256 = app.wrap().query_wasm_smart(contract.addr(), &apr_msg).unwrap();
        assert_eq!(state_before.ci_current, ONE);
        assert_eq!(state_before.total_staked, Uint256::zero());

        let (user_staking, state) = stake_internal(&mut app, contract.clone(), stake_amount, true);
        let balance = app.wrap().query_balance(USER.to_string(), INJEX_TOKEN.to_string()).unwrap();

        let block_info = app.block_info();

        app.set_block(BlockInfo {
            chain_id: block_info.chain_id,
            height: block_info.height,
            time: block_info.time.plus_seconds(200),
        });

        let new_block_time: Timestamp = app.block_info().time;
        let state_msg = QueryMsg::GetState {};

        let res = app.execute_contract(Addr::unchecked(USER), contract.addr(), &msg, &[]);

        assert!(res.is_err());

        let error_message = format!("Insufficient balance");
        expect_error(res, error_message);

        let amount_to_unstake: Uint256 = stake_amount / Uint256::from_u128(2_u128);
        let msg = ExecuteMsg::Unstake { amount: amount_to_unstake };
        let res = app.execute_contract(Addr::unchecked(USER), contract.addr(), &msg, &[]);

        assert!(res.is_ok());

        let user_staking_msg = QueryMsg::GetStakerInfo { user: Addr::unchecked(USER) };

        let new_user_staking: StakerInfo = app
            .wrap()
            .query_wasm_smart(contract.addr(), &user_staking_msg)
            .unwrap();
        let new_state: State = app.wrap().query_wasm_smart(contract.addr(), &state_msg).unwrap();

        let new_ci = calculate_ci(
            state.ci_current,
            apr,
            (new_block_time.seconds() - block_time.seconds()).into()
        );
        let reward = calculate_reward(stake_amount, new_state.ci_current, user_staking.ci_0);

        assert_eq!(new_state.ci_current, new_ci);
        assert_eq!(new_state.ci_time_current, new_block_time);
        assert_eq!(new_state.total_staked, state.total_staked - amount_to_unstake);

        assert_eq!(new_user_staking.block_time, block_time);
        assert_eq!(new_user_staking.ci_0, new_state.ci_current);
        assert_eq!(new_user_staking.staked, state.total_staked - amount_to_unstake);
        assert_eq!(new_user_staking.reward, reward);

        let balance_after = app
            .wrap()
            .query_balance(USER.to_string(), INJEX_TOKEN.to_string())
            .unwrap();

        assert_eq!(
            Uint256::from_uint128(balance_after.amount),
            Uint256::from_uint128(balance.amount) + amount_to_unstake
        );
    }

    #[test]
    fn claim_without_contract_balance() {
        let (mut app, contract) = proper_instantiate(false);

        let stake_amount = ONE;
        let block_time = mock_env().block.time;
        let state_msg = QueryMsg::GetState {};
        let apr_msg = QueryMsg::GetApr {};
        let msg = ExecuteMsg::Claim {};

        let res = app.execute_contract(Addr::unchecked(USER), contract.addr(), &msg, &[]);

        assert!(res.is_err());

        let error_message = format!("No claims");
        expect_error(res, error_message);

        let state_before: State = app.wrap().query_wasm_smart(contract.addr(), &state_msg).unwrap();
        let apr: Uint256 = app.wrap().query_wasm_smart(contract.addr(), &apr_msg).unwrap();
        assert_eq!(state_before.ci_current, ONE);
        assert_eq!(state_before.total_staked, Uint256::zero());

        let (user_staking, state) = stake_internal(&mut app, contract.clone(), stake_amount, true);

        let block_info = app.block_info();

        app.set_block(BlockInfo {
            chain_id: block_info.chain_id,
            height: block_info.height,
            time: block_info.time.plus_seconds(200),
        });

        let new_block_time: Timestamp = app.block_info().time;

        let (new_user_staking, new_state) = stake_internal(
            &mut app,
            contract.clone(),
            stake_amount,
            false
        );

        let new_ci = calculate_ci(
            state.ci_current,
            apr,
            (new_block_time.seconds() - block_time.seconds()).into()
        );
        let reward = calculate_reward(stake_amount, new_state.ci_current, user_staking.ci_0);

        assert_eq!(new_state.ci_current, new_ci);
        assert_eq!(new_state.ci_time_current, new_block_time);
        assert_eq!(new_state.total_staked, stake_amount + stake_amount);

        assert_eq!(new_user_staking.block_time, block_time);
        assert_eq!(new_user_staking.ci_0, new_state.ci_current);
        assert_eq!(new_user_staking.staked, stake_amount + stake_amount);
        assert_eq!(new_user_staking.reward, reward);

        let reward_calculated =
            (stake_amount * Uint256::from_u128(200_u128) * apr) / (SECONDS_IN_YEAR * PERCENTS);

        assert_eq!(reward_calculated, new_user_staking.reward);

        let block_info = app.block_info();

        app.set_block(BlockInfo {
            chain_id: block_info.chain_id,
            height: block_info.height + 1,
            time: block_info.time.plus_seconds(500),
        });

        let balance = app
            .wrap()
            .query_balance(contract.addr().to_string(), INJEX_TOKEN.to_string())
            .unwrap();

        println!("{}", balance.amount);

        let res = app.execute_contract(Addr::unchecked(USER), contract.addr(), &msg, &[]);

        assert!(res.is_err());

        let error_message = format!("Insufficient contract balance");
        expect_error(res, error_message);
    }

    #[test]
    fn claim_with_multiple_stakes() {
        let (mut app, contract) = proper_instantiate(true);

        let stake_amount = ONE;
        let block_time = mock_env().block.time;
        let state_msg = QueryMsg::GetState {};
        let apr_msg = QueryMsg::GetApr {};
        let msg = ExecuteMsg::Claim {};

        let res = app.execute_contract(Addr::unchecked(USER), contract.addr(), &msg, &[]);

        assert!(res.is_err());

        let error_message = format!("No claims");
        expect_error(res, error_message);

        let state_before: State = app.wrap().query_wasm_smart(contract.addr(), &state_msg).unwrap();
        let apr: Uint256 = app.wrap().query_wasm_smart(contract.addr(), &apr_msg).unwrap();
        assert_eq!(state_before.ci_current, ONE);
        assert_eq!(state_before.total_staked, Uint256::zero());

        let (user_staking, state) = stake_internal(&mut app, contract.clone(), stake_amount, true);

        let block_info = app.block_info();

        app.set_block(BlockInfo {
            chain_id: block_info.chain_id,
            height: block_info.height,
            time: block_info.time.plus_seconds(200),
        });

        let new_block_time: Timestamp = app.block_info().time;

        let (new_user_staking, new_state) = stake_internal(
            &mut app,
            contract.clone(),
            stake_amount,
            false
        );

        let new_ci = calculate_ci(
            state.ci_current,
            apr,
            (new_block_time.seconds() - block_time.seconds()).into()
        );
        let reward = calculate_reward(stake_amount, new_state.ci_current, user_staking.ci_0);

        assert_eq!(new_state.ci_current, new_ci);
        assert_eq!(new_state.ci_time_current, new_block_time);
        assert_eq!(new_state.total_staked, stake_amount + stake_amount);

        assert_eq!(new_user_staking.block_time, block_time);
        assert_eq!(new_user_staking.ci_0, new_state.ci_current);
        assert_eq!(new_user_staking.staked, stake_amount + stake_amount);
        assert_eq!(new_user_staking.reward, reward);

        let reward_calculated =
            (stake_amount * Uint256::from_u128(200_u128) * apr) / (SECONDS_IN_YEAR * PERCENTS);

        assert_eq!(reward_calculated, new_user_staking.reward);

        let block_info = app.block_info();

        app.set_block(BlockInfo {
            chain_id: block_info.chain_id,
            height: block_info.height + 1,
            time: block_info.time.plus_seconds(500),
        });

        let new_block_time_claim: Timestamp = app.block_info().time;

        let balance = app.wrap().query_balance(USER.to_string(), INJEX_TOKEN.to_string()).unwrap();
        let res = app.execute_contract(Addr::unchecked(USER), contract.addr(), &msg, &[]);

        assert!(res.is_ok());

        let user_staking_msg = QueryMsg::GetStakerInfo { user: Addr::unchecked(USER) };

        let new_user_staking_after_claim: StakerInfo = app
            .wrap()
            .query_wasm_smart(contract.addr(), &user_staking_msg)
            .unwrap();
        let new_state_after_claim: State = app
            .wrap()
            .query_wasm_smart(contract.addr(), &state_msg)
            .unwrap();

        let new_ci_claim = calculate_ci(
            new_state.ci_current,
            apr,
            (new_block_time_claim.seconds() - new_block_time.seconds()).into()
        );
        let reward_claim = calculate_reward(
            stake_amount + stake_amount,
            new_state_after_claim.ci_current,
            new_user_staking.ci_0
        );

        assert_eq!(new_state_after_claim.ci_current, new_ci_claim);
        assert_eq!(new_state_after_claim.ci_time_current, new_block_time_claim);
        assert_eq!(new_state_after_claim.total_withdrawn, reward_claim + reward);

        assert_eq!(new_user_staking_after_claim.block_time, block_time);
        assert_eq!(new_user_staking_after_claim.ci_0, new_state_after_claim.ci_current);
        assert_eq!(new_user_staking_after_claim.staked, new_state_after_claim.total_staked);
        assert_eq!(new_user_staking_after_claim.reward, Uint256::zero());

        let balance_after = app
            .wrap()
            .query_balance(USER.to_string(), INJEX_TOKEN.to_string())
            .unwrap();

        assert_eq!(
            Uint256::from_uint128(balance_after.amount),
            Uint256::from_uint128(balance.amount) + reward + reward_claim
        );
    }

    #[test]
    fn multiple_stake_with_apr_change() {
        let (mut app, contract) = proper_instantiate(true);

        let stake_amount = ONE;
        let block_time = mock_env().block.time;
        let state_msg = QueryMsg::GetState {};
        let apr_msg = QueryMsg::GetApr {};

        let state_before: State = app.wrap().query_wasm_smart(contract.addr(), &state_msg).unwrap();
        let apr: Uint256 = app.wrap().query_wasm_smart(contract.addr(), &apr_msg).unwrap();
        assert_eq!(state_before.ci_current, ONE);
        assert_eq!(state_before.total_staked, Uint256::zero());
        let balance = app.wrap().query_balance(USER.to_string(), INJEX_TOKEN.to_string()).unwrap();

        let (user_staking, state) = stake_internal(&mut app, contract.clone(), stake_amount, true);

        let block_info = app.block_info();

        app.set_block(BlockInfo {
            chain_id: block_info.chain_id,
            height: block_info.height,
            time: block_info.time.plus_seconds(200),
        });

        let new_block_time: Timestamp = app.block_info().time;

        let (new_user_staking, new_state) = stake_internal(
            &mut app,
            contract.clone(),
            stake_amount,
            false
        );

        let new_ci = calculate_ci(
            state.ci_current,
            apr,
            (new_block_time.seconds() - block_time.seconds()).into()
        );
        let reward = calculate_reward(stake_amount, new_state.ci_current, user_staking.ci_0);

        assert_eq!(new_state.ci_current, new_ci);
        assert_eq!(new_state.ci_time_current, new_block_time);
        assert_eq!(new_state.total_staked, state.total_staked + stake_amount);

        assert_eq!(new_user_staking.block_time, block_time);
        assert_eq!(new_user_staking.ci_0, new_state.ci_current);
        assert_eq!(new_user_staking.staked, state.total_staked + stake_amount);
        assert_eq!(new_user_staking.reward, reward);

        let reward_calculated =
            (stake_amount * Uint256::from_u128(200_u128) * apr) / (SECONDS_IN_YEAR * PERCENTS);

        assert_eq!(reward_calculated, new_user_staking.reward);

        let balance_after = app
            .wrap()
            .query_balance(USER.to_string(), INJEX_TOKEN.to_string())
            .unwrap();

        assert_eq!(
            Uint256::from_uint128(balance_after.amount),
            Uint256::from_uint128(balance.amount) - (stake_amount + stake_amount)
        );

        let block_info = app.block_info();

        app.set_block(BlockInfo {
            chain_id: block_info.chain_id,
            height: block_info.height,
            time: block_info.time.plus_seconds(500),
        });

        let new_block_time_claim: Timestamp = app.block_info().time;

        let fake_user = "inj12vpajtjf5cvmk2w737m0t8qwwkyjz0xgvxwyus";
        let change_apr_msg = ExecuteMsg::ChangeApr { new_apr: Uint256::zero() };
        let res = app.execute_contract(
            Addr::unchecked(fake_user),
            contract.addr(),
            &change_apr_msg,
            &[]
        );

        assert!(res.is_err());

        let error_message = format!("Only admin");
        expect_error(res, error_message);

        let res = app.execute_contract(
            Addr::unchecked(USER),
            contract.addr(),
            &change_apr_msg,
            &[]
        );

        assert!(res.is_err());

        let error_message = format!("Invalid APR");
        expect_error(res, error_message);

        let change_apr_msg = ExecuteMsg::ChangeApr { new_apr: Uint256::from_u128(4000_u128) };
        let res = app.execute_contract(
            Addr::unchecked(USER),
            contract.addr(),
            &change_apr_msg,
            &[]
        );

        assert!(res.is_ok());

        let new_state_after_apr_change: State = app
            .wrap()
            .query_wasm_smart(contract.addr(), &state_msg)
            .unwrap();

        let new_ci_apr_change = calculate_ci(
            new_state.ci_current,
            apr,
            (new_block_time_claim.seconds() - new_block_time.seconds()).into()
        );

        assert_eq!(new_state_after_apr_change.ci_current, new_ci_apr_change);
        assert_eq!(new_state_after_apr_change.ci_time_current, new_block_time_claim);
        assert_eq!(new_state_after_apr_change.total_withdrawn, Uint256::zero());

        let block_info = app.block_info();

        app.set_block(BlockInfo {
            chain_id: block_info.chain_id,
            height: block_info.height,
            time: block_info.time.plus_seconds(500),
        });

        let new_block_time_apr: Timestamp = app.block_info().time;

        let (new_user_staking_after, new_state) = stake_internal(
            &mut app,
            contract.clone(),
            stake_amount,
            false
        );

        let new_ci = calculate_ci(
            new_state_after_apr_change.ci_current,
            Uint256::from_u128(4000_u128),
            (new_block_time_apr.seconds() - new_block_time_claim.seconds()).into()
        );
        let reward_apr_change = calculate_reward(
            stake_amount + stake_amount,
            new_state.ci_current,
            new_user_staking.ci_0
        );

        assert_eq!(new_state.ci_current, new_ci);
        assert_eq!(new_state.ci_time_current, new_block_time_apr);
        assert_eq!(new_state.total_staked, stake_amount + stake_amount + stake_amount);

        assert_eq!(new_user_staking_after.block_time, block_time);
        assert_eq!(new_user_staking_after.ci_0, new_state.ci_current);
        assert_eq!(new_user_staking_after.reward, reward + reward_apr_change);
    }

    #[test]
    fn change_injex_token() {
        let (mut app, contract) = proper_instantiate(true);

        let new_token = "asdasd".to_string();

        let fake_user = "inj12vpajtjf5cvmk2w737m0t8qwwkyjz0xgvxwyus";
        let change_token = ExecuteMsg::ChangeInjexToken { new_injex_token: "asdasd".to_string() };
        let res = app.execute_contract(
            Addr::unchecked(fake_user),
            contract.addr(),
            &change_token,
            &[]
        );

        assert!(res.is_err());

        let error_message = format!("Only admin");
        expect_error(res, error_message);

        let change_token = ExecuteMsg::ChangeInjexToken { new_injex_token: new_token.clone() };
        let res = app.execute_contract(Addr::unchecked(USER), contract.addr(), &change_token, &[]);

        assert!(res.is_ok());

        let token_msg = QueryMsg::GetInjexToken {};

        let token: String = app.wrap().query_wasm_smart(contract.addr(), &token_msg).unwrap();

        assert_eq!(token, new_token.clone());
    }

    fn stake_internal(
        app: &mut App,
        contract: CwTemplateContract,
        stake_amount: Uint256,
        check: bool
    ) -> (StakerInfo, State) {
        let msg = ExecuteMsg::Stake {};
        let state_msg = QueryMsg::GetState {};

        let res = app.execute_contract(
            Addr::unchecked(USER),
            contract.addr(),
            &msg,
            &vec![Coin {
                denom: INJEX_TOKEN.to_string(),
                amount: Uint128::try_from(stake_amount).unwrap(),
            }]
        );

        assert!(res.is_ok());

        let user_staking_msg = QueryMsg::GetStakerInfo { user: Addr::unchecked(USER) };

        let user_staking: StakerInfo = app
            .wrap()
            .query_wasm_smart(contract.addr(), &user_staking_msg)
            .unwrap();
        let state: State = app.wrap().query_wasm_smart(contract.addr(), &state_msg).unwrap();

        if check {
            let block_time = mock_env().block.time;
            let state_msg = QueryMsg::GetState {};

            let state_before: State = app
                .wrap()
                .query_wasm_smart(contract.addr(), &state_msg)
                .unwrap();

            assert_eq!(state.ci_current, state_before.ci_current);
            assert_eq!(state.ci_time_current, block_time);
            assert_eq!(state.total_staked, stake_amount);

            assert_eq!(user_staking.block_time, block_time);
            assert_eq!(user_staking.ci_0, state.ci_current);
            assert_eq!(user_staking.staked, stake_amount);
            assert_eq!(user_staking.reward, Uint256::zero());
        }

        (user_staking, state)
    }
}
