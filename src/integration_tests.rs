#[cfg(test)]
pub mod tests {
    use crate::{ helpers::CwTemplateContract, ContractError };
    use crate::msg::InstantiateMsg;
    use anyhow::Error;

    use cosmwasm_std::{ Addr, Coin, Empty, Uint128, Uint256 };
    use cw_multi_test::{ App, AppBuilder, AppResponse, Contract, ContractWrapper, Executor };

    pub fn contract_template() -> Box<dyn Contract<Empty>> {
        let contract = ContractWrapper::new(
            crate::contract::execute,
            crate::contract::instantiate,
            crate::contract::query
        );
        Box::new(contract)
    }

    pub const ADMIN: &str = "inj1mvnlejy385wy498z7hvmflrsdfludx8ztxsx7m";
    pub const USER: &str = "inj1mvnlejy385wy498z7hvmflrsdfludx8ztxsx7m";
    pub const INJEX_TOKEN: &str = "INJX";
    pub const USDT: &str = "USDT";

    pub fn mock_app() -> App {
        AppBuilder::new().build(|router, _, storage| {
            router.bank
                .init_balance(
                    storage,
                    &Addr::unchecked(USER),
                    vec![
                        Coin {
                            denom: "inj".to_string(),
                            amount: Uint128::new(1000000000000000000000),
                        },
                        Coin {
                            denom: USDT.to_string(),
                            amount: Uint128::new(1000000000000000000000),
                        },
                        Coin {
                            denom: INJEX_TOKEN.to_string(),
                            amount: Uint128::new(100000000000000000000000000),
                        }
                    ]
                )
                .unwrap();
        })
    }

    pub fn proper_instantiate(should_add_balance_to_contract: bool) -> (App, CwTemplateContract) {
        let mut app: App = mock_app();
        let cw_template_id = app.store_code(contract_template());

        let msg = InstantiateMsg {
            apr: Uint256::from_u128(2_000_u128), // 20%
            injex_token: INJEX_TOKEN.to_string(),
            admin: Addr::unchecked(ADMIN).to_string(),
        };

        let send_funds: &[Coin] = if should_add_balance_to_contract {
            &[
                Coin {
                    denom: INJEX_TOKEN.to_string(),
                    amount: Uint128::new(1000000000000000000000),
                },
            ]
        } else {
            &[]
        };

        let cw_template_contract_addr = app
            .instantiate_contract(
                cw_template_id,
                Addr::unchecked(ADMIN),
                &msg,
                send_funds,
                "test",
                None
            )
            .unwrap();

        let cw_template_contract = CwTemplateContract(cw_template_contract_addr);

        (app, cw_template_contract)
    }

    pub fn expect_error(res: Result<AppResponse, Error>, reason: String) -> () {
        assert!(res.is_err());

        if let Err(err) = res {
            if let Some(custom_err) = err.downcast_ref::<ContractError>() {
                assert_eq!(custom_err.to_string(), reason);
            } else {
                println!("{}", err);
                panic!("Expected a ContractError, but got a different error type");
            }
        }
    }
}
