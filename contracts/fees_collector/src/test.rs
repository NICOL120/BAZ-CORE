use astroport::asset::{AssetInfo, PairInfo};
use astroport::pair::{
    Cw20HookMsg as AstroportPairCw20HookMsg,
};
use astroport::factory::PairType;
use cosmwasm_std::testing::{mock_env, mock_info, MockApi, MockStorage, MOCK_CONTRACT_ADDR};
use cosmwasm_std::{
    from_binary, Addr, BankMsg, Coin, CosmosMsg, Decimal, OwnedDeps, Response, StdError, Timestamp,
    Uint128, WasmMsg, to_binary,
};
use cw20::Cw20ExecuteMsg;
use baz::fees_collector::{AssetWithLimit, CollectSimulationResponse, ExecuteMsg, InstantiateMsg, QueryMsg};

use crate::contract::{execute, instantiate, query};
use crate::error::ContractError;
use crate::mock_querier::{mock_dependencies, WasmMockQuerier};
use crate::state::{Config, CONFIG};

const OWNER: &str = "owner";
const OPERATOR_1: &str = "operator_1";
const OPERATOR_2: &str = "operator_2";
const USER_1: &str = "user_1";
const USER_2: &str = "user_2";
const USER_3: &str = "user_3";
const FACTORY_1: &str = "factory_1";
const FACTORY_2: &str = "factory_2";
const TOKEN_1: &str = "token_1";
const TOKEN_2: &str = "token_2";
const IBC_TOKEN: &str = "ibc/stablecoin";

#[test]
fn test() -> Result<(), ContractError> {
    let mut deps = mock_dependencies();
    create(&mut deps)?;
    config(&mut deps)?;
    owner(&mut deps)?;
    bridges(&mut deps)?;
    collect(&mut deps)?;
    distribute_fees(&mut deps)?;

    Ok(())
}

fn assert_error(res: Result<Response, ContractError>, expected: &str) {
    match res {
        Err(ContractError::Std(StdError::GenericErr { msg, .. })) => assert_eq!(expected, msg),
        Err(err) => assert_eq!(expected, format!("{}", err)),
        _ => panic!("Expected exception"),
    }
}

fn create(
    deps: &mut OwnedDeps<MockStorage, MockApi, WasmMockQuerier>,
) -> Result<(), ContractError> {
    let env = mock_env();
    let info = mock_info(USER_1, &[]);

    let instantiate_msg = InstantiateMsg {
        owner: USER_1.to_string(),
        factory_contract: FACTORY_1.to_string(),
        operator: OPERATOR_1.to_string(),
        stablecoin: AssetInfo::NativeToken {
            denom: IBC_TOKEN.to_string(),
        },
        target_list: vec![(USER_2.to_string(), 2), (USER_3.to_string(), 3)],
    };
    let res = instantiate(deps.as_mut(), env, info, instantiate_msg);
    assert!(res.is_ok());

    let config = CONFIG.load(deps.as_mut().storage)?;
    assert_eq!(
        config,
        Config {
            owner: Addr::unchecked(USER_1),
            operator: Addr::unchecked(OPERATOR_1),
            factory_contract: Addr::unchecked(FACTORY_1),
            target_list: vec![(Addr::unchecked(USER_2), 2), (Addr::unchecked(USER_3), 3)],
            stablecoin: AssetInfo::NativeToken {
                denom: IBC_TOKEN.to_string(),
            },
        }
    );

    Ok(())
}

fn config(
    deps: &mut OwnedDeps<MockStorage, MockApi, WasmMockQuerier>,
) -> Result<(), ContractError> {
    let env = mock_env();

    let info = mock_info(USER_2, &[]);
    let msg = ExecuteMsg::UpdateConfig {
        operator: Some(OPERATOR_2.to_string()),
        factory_contract: None,
        target_list: None,
    };
    let res = execute(deps.as_mut(), env.clone(), info.clone(), msg.clone());
    assert_error(res, "Unauthorized");

    let info = mock_info(USER_1, &[]);
    let res = execute(deps.as_mut(), env.clone(), info.clone(), msg.clone());
    assert!(res.is_ok());

    let msg = ExecuteMsg::UpdateConfig {
        operator: None,
        factory_contract: Some(FACTORY_2.to_string()),
        target_list: None,
    };
    let res = execute(deps.as_mut(), env.clone(), info.clone(), msg.clone());
    assert!(res.is_ok());

    let msg = ExecuteMsg::UpdateConfig {
        operator: None,
        factory_contract: None,
        target_list: Some(vec![(USER_1.to_string(), 1)]),
    };
    let res = execute(deps.as_mut(), env.clone(), info.clone(), msg.clone());
    assert!(res.is_ok());

    let msg = QueryMsg::Config {};
    let res: Config = from_binary(&query(deps.as_ref(), env.clone(), msg)?)?;
    assert_eq!(
        res,
        Config {
            owner: Addr::unchecked(USER_1),
            operator: Addr::unchecked(OPERATOR_2),
            factory_contract: Addr::unchecked(FACTORY_2),
            target_list: vec![(Addr::unchecked(USER_1), 1)],
            stablecoin: AssetInfo::NativeToken {
                denom: IBC_TOKEN.to_string(),
            },
        }
    );

    let msg = ExecuteMsg::UpdateConfig {
        operator: Some(OPERATOR_1.to_string()),
        factory_contract: Some(FACTORY_1.to_string()),
        target_list: Some(vec![(USER_2.to_string(), 2), (USER_3.to_string(), 3)]),
    };
    let res = execute(deps.as_mut(), env.clone(), info.clone(), msg.clone());
    assert!(res.is_ok());

    let msg = QueryMsg::Config {};
    let res: Config = from_binary(&query(deps.as_ref(), env.clone(), msg)?)?;
    assert_eq!(
        res,
        Config {
            owner: Addr::unchecked(USER_1),
            operator: Addr::unchecked(OPERATOR_1),
            factory_contract: Addr::unchecked(FACTORY_1),
            target_list: vec![(Addr::unchecked(USER_2), 2), (Addr::unchecked(USER_3), 3)],
            stablecoin: AssetInfo::NativeToken {
                denom: IBC_TOKEN.to_string(),
            },
        }
    );

    Ok(())
}

fn owner(deps: &mut OwnedDeps<MockStorage, MockApi, WasmMockQuerier>) -> Result<(), ContractError> {
    let mut env = mock_env();
    env.block.time = Timestamp::from_seconds(0);

    // new owner
    let msg = ExecuteMsg::ProposeNewOwner {
        owner: OWNER.to_string(),
        expires_in: 100,
    };

    let info = mock_info(USER_2, &[]);

    // unauthorized check
    let res = execute(deps.as_mut(), env.clone(), info, msg.clone());
    assert_error(res, "Unauthorized");

    // claim before a proposal
    let info = mock_info(USER_2, &[]);
    let res = execute(
        deps.as_mut(),
        env.clone(),
        info,
        ExecuteMsg::ClaimOwnership {},
    );
    assert_error(res, "Ownership proposal not found");

    // propose new owner
    let info = mock_info(USER_1, &[]);
    let res = execute(deps.as_mut(), env.clone(), info, msg.clone());
    assert!(res.is_ok());

    // drop ownership proposal
    let info = mock_info(USER_1, &[]);
    let res = execute(
        deps.as_mut(),
        env.clone(),
        info,
        ExecuteMsg::DropOwnershipProposal {},
    );
    assert!(res.is_ok());

    // ownership proposal dropped
    let info = mock_info(USER_2, &[]);
    let res = execute(
        deps.as_mut(),
        env.clone(),
        info,
        ExecuteMsg::ClaimOwnership {},
    );
    assert_error(res, "Ownership proposal not found");

    // propose new owner again
    let info = mock_info(USER_1, &[]);
    let res = execute(deps.as_mut(), env.clone(), info, msg);
    assert!(res.is_ok());

    // unauthorized ownership claim
    let info = mock_info(USER_3, &[]);
    let res = execute(
        deps.as_mut(),
        env.clone(),
        info,
        ExecuteMsg::ClaimOwnership {},
    );
    assert_error(res, "Unauthorized");

    env.block.time = Timestamp::from_seconds(101);

    // ownership proposal expired
    let info = mock_info(OWNER, &[]);
    let res = execute(
        deps.as_mut(),
        env.clone(),
        info,
        ExecuteMsg::ClaimOwnership {},
    );
    assert_error(res, "Ownership proposal expired");

    env.block.time = Timestamp::from_seconds(100);

    // claim ownership
    let info = mock_info(OWNER, &[]);
    let res = execute(
        deps.as_mut(),
        env.clone(),
        info,
        ExecuteMsg::ClaimOwnership {},
    )?;
    assert_eq!(0, res.messages.len());

    // query config
    let config: Config =
        from_binary(&query(deps.as_ref(), env.clone(), QueryMsg::Config {})?)?;
    assert_eq!(OWNER, config.owner);
    Ok(())
}

fn bridges(
    deps: &mut OwnedDeps<MockStorage, MockApi, WasmMockQuerier>,
) -> Result<(), ContractError> {
    let env = mock_env();

    let msg = ExecuteMsg::UpdateBridges {
        add: Some(vec![(
            AssetInfo::Token {
                contract_addr: Addr::unchecked(TOKEN_1),
            },
            AssetInfo::Token {
                contract_addr: Addr::unchecked(TOKEN_2),
            },
        )]),
        remove: None,
    };

    // update bridges unauthorized
    let info = mock_info(USER_1, &[]);
    let res = execute(deps.as_mut(), env.clone(), info.clone(), msg.clone());
    assert_error(res, "Unauthorized");

    deps.querier.set_pair(
        &[
            AssetInfo::Token {
                contract_addr: Addr::unchecked(TOKEN_1),
            },
            AssetInfo::Token {
                contract_addr: Addr::unchecked(TOKEN_2),
            },
        ],
        PairInfo {
            asset_infos: vec![
                AssetInfo::Token {
                    contract_addr: Addr::unchecked(TOKEN_1),
                },
                AssetInfo::Token {
                    contract_addr: Addr::unchecked(TOKEN_2),
                },
            ],
            contract_addr: Addr::unchecked("token1token2"),
            liquidity_token: Addr::unchecked("liquidity0000"),
            pair_type: PairType::Xyk {},
        },
    );

    deps.querier.set_pair(
        &[
            AssetInfo::Token {
                contract_addr: Addr::unchecked(TOKEN_2),
            },
            AssetInfo::NativeToken {
                denom: IBC_TOKEN.to_string(),
            },
        ],
        PairInfo {
            asset_infos: vec![
                AssetInfo::Token {
                    contract_addr: Addr::unchecked(TOKEN_2),
                },
                AssetInfo::NativeToken {
                    denom: IBC_TOKEN.to_string(),
                },
            ],
            contract_addr: Addr::unchecked("token2ibc"),
            liquidity_token: Addr::unchecked("liquidity0002"),
            pair_type: PairType::Stable {},
        },
    );

    let info = mock_info(OPERATOR_1, &[]);

    // update bridges
    let res = execute(deps.as_mut(), env.clone(), info.clone(), msg.clone());
    assert!(res.is_ok());

    // query bridges
    let bridges: Vec<(String, String)> =
        from_binary(&query(deps.as_ref(), env.clone(), QueryMsg::Bridges {})?)?;
    assert_eq!(vec![(TOKEN_1.to_string(), TOKEN_2.to_string())], bridges);

    let msg = ExecuteMsg::UpdateBridges {
        add: None,
        remove: Some(vec![AssetInfo::Token {
            contract_addr: Addr::unchecked(TOKEN_1),
        }]),
    };

    let res = execute(deps.as_mut(), env.clone(), info, msg);
    assert!(res.is_ok());

    // query bridges
    let bridges: Vec<(String, String)> =
        from_binary(&query(deps.as_ref(), env.clone(), QueryMsg::Bridges {})?)?;
    assert!(bridges.is_empty());

    Ok(())
}

fn collect(
    deps: &mut OwnedDeps<MockStorage, MockApi, WasmMockQuerier>,
) -> Result<(), ContractError> {
    let env = mock_env();

    // update bridges
    let info = mock_info(OPERATOR_1, &[]);
    let msg = ExecuteMsg::UpdateBridges {
        add: Some(vec![(
            AssetInfo::Token {
                contract_addr: Addr::unchecked(TOKEN_1),
            },
            AssetInfo::Token {
                contract_addr: Addr::unchecked(TOKEN_2),
            },
        )]),
        remove: None,
    };
    let res = execute(deps.as_mut(), env.clone(), info, msg);
    assert!(res.is_ok());

    let msg = ExecuteMsg::Collect {
        assets: vec![AssetWithLimit {
            info: AssetInfo::Token {
                contract_addr: Addr::unchecked(TOKEN_1),
            },
            limit: None,
        }],
        minimum_receive: None
    };

    let info = mock_info(USER_1, &[]);

    // unauthorized check
    let res = execute(deps.as_mut(), env.clone(), info, msg.clone());
    assert_error(res, "Unauthorized");

    // distribute fee only if no balance
    let info = mock_info(OPERATOR_1, &[]);
    let res = execute(deps.as_mut(), env.clone(), info.clone(), msg.clone())?;
    assert_eq!(
        res.messages
            .into_iter()
            .map(|it| it.msg)
            .collect::<Vec<CosmosMsg>>(),
        vec![
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: env.contract.address.to_string(),
                funds: vec![],
                msg: to_binary(&ExecuteMsg::DistributeFees { minimum_receive: None })?,
            }),
        ]
    );

    // set balance
    deps.querier.set_balance(
        TOKEN_1.to_string(),
        MOCK_CONTRACT_ADDR.to_string(),
        Uint128::from(1000000u128),
    );

    // collect success
    let res = execute(deps.as_mut(), env.clone(), info.clone(), msg)?;
    assert_eq!(
        res.messages
            .into_iter()
            .map(|it| it.msg)
            .collect::<Vec<CosmosMsg>>(),
        vec![
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: TOKEN_1.to_string(),
                funds: vec![],
                msg: to_binary(&Cw20ExecuteMsg::Send {
                    contract: "token1token2".to_string(),
                    amount: Uint128::new(1000000u128),
                    msg: to_binary(&AstroportPairCw20HookMsg::Swap {
                        ask_asset_info: None,
                        belief_price: Some(Decimal::MAX),
                        max_spread: Some(Decimal::percent(50)),
                        to: None,
                    })?
                })?,
            }),
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: env.contract.address.to_string(),
                funds: vec![],
                msg: to_binary(&ExecuteMsg::SwapBridgeAssets { assets: vec![AssetInfo::Token { contract_addr: Addr::unchecked(TOKEN_2) }], depth: 0 })?,
            }),
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: env.contract.address.to_string(),
                funds: vec![],
                msg: to_binary(&ExecuteMsg::DistributeFees { minimum_receive: None })?,
            }),
        ]
    );

    deps.querier.set_price("token1token2".to_string(), Decimal::percent(200u64));
    deps.querier.set_price("token2ibc".to_string(), Decimal::percent(25u64));

    let msg = QueryMsg::CollectSimulation {
        assets: vec![AssetWithLimit {
            info: AssetInfo::Token {
                contract_addr: Addr::unchecked(TOKEN_1),
            },
            limit: None,
        }],
    };
    let res: CollectSimulationResponse = from_binary(&query(deps.as_ref(), env.clone(), msg)?)?;
    assert_eq!(
        res,
        CollectSimulationResponse {
            return_amount: Uint128::from(500000u128),
        }
    );

    // set balance
    deps.querier.set_balance(
        TOKEN_2.to_string(),
        MOCK_CONTRACT_ADDR.to_string(),
        Uint128::from(2000000u128),
    );

    let msg = ExecuteMsg::Collect {
        assets: vec![AssetWithLimit {
            info: AssetInfo::Token {
                contract_addr: Addr::unchecked(TOKEN_2),
            },
            limit: Some(Uint128::from(1500000u128)),
        }],
        minimum_receive: None
    };

    // collect success
    let res = execute(deps.as_mut(), env.clone(), info, msg)?;
    assert_eq!(
        res.messages
            .into_iter()
            .map(|it| it.msg)
            .collect::<Vec<CosmosMsg>>(),
        vec![
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: TOKEN_2.to_string(),
                funds: vec![],
                msg: to_binary(&Cw20ExecuteMsg::Send {
                    contract: "token2ibc".to_string(),
                    amount: Uint128::new(1500000u128),
                    msg: to_binary(&AstroportPairCw20HookMsg::Swap {
                        ask_asset_info: None,
                        belief_price: Some(Decimal::MAX),
                        max_spread: Some(Decimal::percent(50)),
                        to: None,
                    })?
                })?,
            }),
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: env.contract.address.to_string(),
                funds: vec![],
                msg: to_binary(&ExecuteMsg::DistributeFees { minimum_receive: None })?,
            }),
        ]
    );


    Ok(())
}

fn distribute_fees(
    deps: &mut OwnedDeps<MockStorage, MockApi, WasmMockQuerier>,
) -> Result<(), ContractError> {
    let env = mock_env();

    // set balance
    deps.querier.set_balance(
        IBC_TOKEN.to_string(),
        MOCK_CONTRACT_ADDR.to_string(),
        Uint128::from(1000000u128),
    );

    let msg = ExecuteMsg::DistributeFees { minimum_receive: Some(Uint128::from(2000000u128)) };

    let info = mock_info(USER_1, &[]);

    // unauthorized check
    let res = execute(deps.as_mut(), env.clone(), info, msg.clone());
    assert_error(res, "Unauthorized");

    // min receive
    let info = mock_info(MOCK_CONTRACT_ADDR, &[]);
    let res = execute(deps.as_mut(), env.clone(), info.clone(), msg.clone());
    assert_error(res, "Assertion failed; minimum receive amount: 2000000, actual amount: 1000000");

    let msg = ExecuteMsg::DistributeFees { minimum_receive: None };
    let res = execute(deps.as_mut(), env.clone(), info, msg.clone())?;
    assert_eq!(
        res.messages
            .into_iter()
            .map(|it| it.msg)
            .collect::<Vec<CosmosMsg>>(),
        [
            CosmosMsg::Bank(BankMsg::Send {
                to_address: USER_2.to_string(),
                amount: vec![Coin {
                    denom: IBC_TOKEN.to_string(),
                    amount: Uint128::from(400000u128),
                }]
            }),
            CosmosMsg::Bank(BankMsg::Send {
                to_address: USER_3.to_string(),
                amount: vec![Coin {
                    denom: IBC_TOKEN.to_string(),
                    amount: Uint128::from(600000u128),
                }]
            }),
        ]
    );

    Ok(())
}
