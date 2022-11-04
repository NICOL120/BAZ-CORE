use crate::contract::{execute, instantiate, query};
use crate::error::ContractError;
use crate::mock_querier::{mock_dependencies, WasmMockQuerier};
use crate::state::{Config, State};

use astroport::asset::{Asset, AssetInfo};
use astroport::generator::{
    Cw20HookMsg as GeneratorCw20HookMsg, ExecuteMsg as GeneratorExecuteMsg,
};

use cosmwasm_std::testing::{mock_env, mock_info, MockApi, MockStorage, MOCK_CONTRACT_ADDR};
use cosmwasm_std::{
    from_binary, to_binary, Addr, Coin, CosmosMsg, Decimal, OwnedDeps, Response, StdError,
    Timestamp, Uint128, WasmMsg,
};
use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg, Expiration};
use spectrum::adapters::generator::Generator;
use spectrum::adapters::pair::Pair;
use spectrum::astroport_farm::{
    CallbackMsg, Cw20HookMsg, ExecuteMsg, InstantiateMsg, QueryMsg, RewardInfoResponse,
    RewardInfoResponseItem,
};
use spectrum::compound_proxy::{Compounder, ExecuteMsg as CompoundProxyExecuteMsg};

const ASTRO_TOKEN: &str = "astro";
const REWARD_TOKEN: &str = "reward";
const OWNER: &str = "owner";
const USER_1: &str = "user_1";
const USER_2: &str = "user_2";
const USER_3: &str = "user_3";
const GENERATOR_PROXY: &str = "generator_proxy";
const COMPOUND_PROXY: &str = "compound_proxy";
const CONTROLLER: &str = "controller";
const FEE_COLLECTOR: &str = "fee_collector";
const COMPOUND_PROXY_2: &str = "compound_proxy_2";
const CONTROLLER_2: &str = "controller_2";
const FEE_COLLECTOR_2: &str = "fee_collector_2";
const LP_TOKEN: &str = "lp_token";
const IBC_TOKEN: &str = "ibc/stablecoin";

#[test]
fn test() -> Result<(), ContractError> {
    let mut deps = mock_dependencies();
    create(&mut deps)?;
    config(&mut deps)?;
    owner(&mut deps)?;
    bond(&mut deps)?;
    deposit_time(&mut deps)?;
    compound(&mut deps)?;
    callback(&mut deps)?;

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

    // invalid fee percentage
    let info = mock_info(USER_1, &[]);
    let msg = InstantiateMsg {
        owner: USER_1.to_string(),
        staking_contract: GENERATOR_PROXY.to_string(),
        compound_proxy: COMPOUND_PROXY.to_string(),
        controller: CONTROLLER.to_string(),
        fee: Decimal::percent(101),
        fee_collector: FEE_COLLECTOR.to_string(),
        liquidity_token: LP_TOKEN.to_string(),
        base_reward_token: ASTRO_TOKEN.to_string(),
        name: "name".to_string(),
        symbol: "SYMBOL".to_string(),
        pair: "pair".to_string(),
    };
    let res = instantiate(deps.as_mut(), env.clone(), info.clone(), msg);
    assert_error(res, "fee must be 0 to 1");

    // valid init message
    let msg = InstantiateMsg {
        owner: USER_1.to_string(),
        staking_contract: GENERATOR_PROXY.to_string(),
        compound_proxy: COMPOUND_PROXY.to_string(),
        controller: CONTROLLER.to_string(),
        fee: Decimal::percent(5),
        fee_collector: FEE_COLLECTOR.to_string(),
        liquidity_token: LP_TOKEN.to_string(),
        base_reward_token: ASTRO_TOKEN.to_string(),
        name: "name".to_string(),
        symbol: "SYMBOL".to_string(),
        pair: "pair".to_string(),
    };

    let res = instantiate(deps.as_mut(), env.clone(), info.clone(), msg);
    assert!(res.is_ok());

    // query config
    let msg = QueryMsg::Config {};
    let res: Config = from_binary(&query(deps.as_ref(), env.clone(), msg)?)?;
    assert_eq!(
        res,
        Config {
            owner: Addr::unchecked(USER_1),
            controller: Addr::unchecked(CONTROLLER),
            fee_collector: Addr::unchecked(FEE_COLLECTOR),
            staking_contract: Generator(Addr::unchecked(GENERATOR_PROXY)),
            compound_proxy: Compounder(Addr::unchecked(COMPOUND_PROXY)),
            fee: Decimal::percent(5),
            liquidity_token: Addr::unchecked(LP_TOKEN.to_string()),
            base_reward_token: Addr::unchecked(ASTRO_TOKEN.to_string()),
            name: "name".to_string(),
            symbol: "SYMBOL".to_string(),
            pair: Pair(Addr::unchecked("pair")),
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
        compound_proxy: None,
        controller: None,
        fee: Some(Decimal::percent(101)),
        fee_collector: None,
    };
    let res = execute(deps.as_mut(), env.clone(), info.clone(), msg.clone());
    assert_error(res, "Unauthorized");

    let info = mock_info(USER_1, &[]);
    let res = execute(deps.as_mut(), env.clone(), info.clone(), msg);
    assert_error(res, "fee must be 0 to 1");

    let msg = ExecuteMsg::UpdateConfig {
        compound_proxy: None,
        controller: None,
        fee: Some(Decimal::percent(3)),
        fee_collector: None,
    };
    let res = execute(deps.as_mut(), env.clone(), info.clone(), msg.clone());
    assert!(res.is_ok());

    let msg = ExecuteMsg::UpdateConfig {
        compound_proxy: Some(COMPOUND_PROXY_2.to_string()),
        controller: None,
        fee: None,
        fee_collector: None,
    };
    let res = execute(deps.as_mut(), env.clone(), info.clone(), msg.clone());
    assert!(res.is_ok());

    let msg = ExecuteMsg::UpdateConfig {
        compound_proxy: None,
        controller: Some(CONTROLLER_2.to_string()),
        fee: None,
        fee_collector: None,
    };
    let res = execute(deps.as_mut(), env.clone(), info.clone(), msg.clone());
    assert!(res.is_ok());

    let msg = ExecuteMsg::UpdateConfig {
        compound_proxy: None,
        controller: None,
        fee: None,
        fee_collector: Some(FEE_COLLECTOR_2.to_string()),
    };
    let res = execute(deps.as_mut(), env.clone(), info.clone(), msg.clone());
    assert!(res.is_ok());

    let msg = QueryMsg::Config {};
    let res: Config = from_binary(&query(deps.as_ref(), env.clone(), msg)?)?;
    assert_eq!(
        res,
        Config {
            owner: Addr::unchecked(USER_1),
            controller: Addr::unchecked(CONTROLLER_2),
            fee_collector: Addr::unchecked(FEE_COLLECTOR_2),
            staking_contract: Generator(Addr::unchecked(GENERATOR_PROXY)),
            compound_proxy: Compounder(Addr::unchecked(COMPOUND_PROXY_2)),
            fee: Decimal::percent(3),
            liquidity_token: Addr::unchecked(LP_TOKEN.to_string()),
            base_reward_token: Addr::unchecked(ASTRO_TOKEN.to_string()),
            name: "name".to_string(),
            symbol: "SYMBOL".to_string(),
            pair: Pair(Addr::unchecked("pair")),
        }
    );

    let msg = ExecuteMsg::UpdateConfig {
        compound_proxy: Some(COMPOUND_PROXY.to_string()),
        controller: Some(CONTROLLER.to_string()),
        fee: Some(Decimal::percent(5)),
        fee_collector: Some(FEE_COLLECTOR.to_string()),
    };
    let res = execute(deps.as_mut(), env.clone(), info.clone(), msg.clone());
    assert!(res.is_ok());

    let msg = QueryMsg::Config {};
    let res: Config = from_binary(&query(deps.as_ref(), env.clone(), msg)?)?;
    assert_eq!(
        res,
        Config {
            owner: Addr::unchecked(USER_1),
            controller: Addr::unchecked(CONTROLLER),
            fee_collector: Addr::unchecked(FEE_COLLECTOR),
            staking_contract: Generator(Addr::unchecked(GENERATOR_PROXY)),
            compound_proxy: Compounder(Addr::unchecked(COMPOUND_PROXY)),
            fee: Decimal::percent(5),
            liquidity_token: Addr::unchecked(LP_TOKEN.to_string()),
            base_reward_token: Addr::unchecked(ASTRO_TOKEN.to_string()),
            name: "name".to_string(),
            symbol: "SYMBOL".to_string(),
            pair: Pair(Addr::unchecked("pair")),
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

fn bond(deps: &mut OwnedDeps<MockStorage, MockApi, WasmMockQuerier>) -> Result<(), ContractError> {
    let mut env = mock_env();
    env.block.time = Timestamp::from_seconds(101);

    // invalid staking token
    let info = mock_info(ASTRO_TOKEN, &[]);
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: USER_1.to_string(),
        amount: Uint128::from(100000u128),
        msg: to_binary(&Cw20HookMsg::Bond { staker_addr: None })?,
    });
    let res = execute(deps.as_mut(), env.clone(), info.clone(), msg);
    assert_error(res, "Unauthorized");

    // user_1 bond 100000 LP
    let info = mock_info(LP_TOKEN, &[]);
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: USER_1.to_string(),
        amount: Uint128::from(100000u128),
        msg: to_binary(&Cw20HookMsg::Bond { staker_addr: None })?,
    });
    let res = execute(deps.as_mut(), env.clone(), info.clone(), msg)?;
    assert_eq!(
        res.messages
            .into_iter()
            .map(|it| it.msg)
            .collect::<Vec<CosmosMsg>>(),
        [CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: LP_TOKEN.to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Send {
                contract: GENERATOR_PROXY.to_string(),
                amount: Uint128::from(100000u128),
                msg: to_binary(&GeneratorCw20HookMsg::Deposit {})?,
            })?,
            funds: vec![],
        }),]
    );

    // update generator balance
    deps.querier.set_balance(
        GENERATOR_PROXY.to_string(),
        LP_TOKEN.to_string(),
        Uint128::from(100000u128),
    );

    // query reward info
    let msg = QueryMsg::RewardInfo {
        staker_addr: USER_1.to_string(),
    };
    let res: RewardInfoResponse = from_binary(&query(deps.as_ref(), env.clone(), msg)?)?;
    assert_eq!(
        res,
        RewardInfoResponse {
            staker_addr: USER_1.to_string(),
            reward_info: {
                RewardInfoResponseItem {
                    bond_share: Uint128::from(100000u128),
                    deposit_amount: Uint128::from(100000u128),
                    deposit_time: 101,
                    staking_token: LP_TOKEN.to_string(),
                    bond_amount: Uint128::from(100000u128),
                    deposit_costs: vec![
                        Uint128::from(100000u128),
                        Uint128::from(100000u128),
                    ],
                }
            }
        }
    );

    // update time
    env.block.time = Timestamp::from_seconds(102);

    // user_1 bond 100000 LP for user_2
    let info = mock_info(LP_TOKEN, &[]);
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: USER_1.to_string(),
        amount: Uint128::from(50000u128),
        msg: to_binary(&Cw20HookMsg::Bond {
            staker_addr: Some(USER_2.to_string()),
        })?,
    });
    let res = execute(deps.as_mut(), env.clone(), info.clone(), msg)?;
    assert_eq!(
        res.messages
            .into_iter()
            .map(|it| it.msg)
            .collect::<Vec<CosmosMsg>>(),
        [CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: LP_TOKEN.to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Send {
                contract: GENERATOR_PROXY.to_string(),
                amount: Uint128::from(50000u128),
                msg: to_binary(&GeneratorCw20HookMsg::Deposit {})?,
            })?,
            funds: vec![],
        }),]
    );

    // update generator balance
    env.block.time = Timestamp::from_seconds(100102);
    deps.querier.set_balance(
        GENERATOR_PROXY.to_string(),
        LP_TOKEN.to_string(),
        Uint128::from(150000u128),
    );

    // query reward info
    let msg = QueryMsg::RewardInfo {
        staker_addr: USER_2.to_string(),
    };
    let res: RewardInfoResponse = from_binary(&query(deps.as_ref(), env.clone(), msg)?)?;
    assert_eq!(
        res,
        RewardInfoResponse {
            staker_addr: USER_2.to_string(),
            reward_info: {
                RewardInfoResponseItem {
                    bond_share: Uint128::from(50000u128),
                    deposit_amount: Uint128::from(50000u128),
                    deposit_time: 102,
                    staking_token: LP_TOKEN.to_string(),
                    bond_amount: Uint128::from(50000u128),
                    deposit_costs: vec![
                        Uint128::from(50000u128),
                        Uint128::from(50000u128),
                    ],
                }
            }
        }
    );

    // query state
    let msg = QueryMsg::State {};
    let res: State = from_binary(&query(deps.as_ref(), env.clone(), msg)?)?;
    assert_eq!(
        res,
        State {
            total_bond_share: Uint128::from(150000u128),
        }
    );

    // increase generator balance by 30000 from compound
    deps.querier.set_balance(
        GENERATOR_PROXY.to_string(),
        LP_TOKEN.to_string(),
        Uint128::from(180000u128),
    );

    // query reward info for user_1, bond amount should be 100000 + 20000 = 120000
    let msg = QueryMsg::RewardInfo {
        staker_addr: USER_1.to_string(),
    };
    let res: RewardInfoResponse = from_binary(&query(deps.as_ref(), env.clone(), msg)?)?;
    assert_eq!(
        res,
        RewardInfoResponse {
            staker_addr: USER_1.to_string(),
            reward_info: {
                RewardInfoResponseItem {
                    bond_share: Uint128::from(100000u128),
                    deposit_amount: Uint128::from(100000u128),
                    deposit_time: 101,
                    staking_token: LP_TOKEN.to_string(),
                    bond_amount: Uint128::from(120000u128),
                    deposit_costs: vec![
                        Uint128::from(100000u128),
                        Uint128::from(100000u128),
                    ],
                }
            }
        }
    );

    // query reward info for user_2, bond amount should be 50000 + 10000 = 60000
    let msg = QueryMsg::RewardInfo {
        staker_addr: USER_2.to_string(),
    };
    let res: RewardInfoResponse = from_binary(&query(deps.as_ref(), env.clone(), msg)?)?;
    assert_eq!(
        res,
        RewardInfoResponse {
            staker_addr: USER_2.to_string(),
            reward_info: {
                RewardInfoResponseItem {
                    bond_share: Uint128::from(50000u128),
                    deposit_amount: Uint128::from(50000u128),
                    deposit_time: 102,
                    staking_token: LP_TOKEN.to_string(),
                    bond_amount: Uint128::from(60000u128),
                    deposit_costs: vec![
                        Uint128::from(50000u128),
                        Uint128::from(50000u128),
                    ],
                }
            }
        }
    );

    // unbond error for new user
    let info = mock_info(USER_3, &[]);
    let msg = ExecuteMsg::Unbond {
        amount: Uint128::from(100u128),
    };
    let res = execute(deps.as_mut(), env.clone(), info.clone(), msg);
    assert_error(res, "spectrum_astroport_farm::state::RewardInfo not found");

    // unbond for user_1
    let info = mock_info(USER_1, &[]);
    let msg = ExecuteMsg::Unbond {
        amount: Uint128::from(120001u128),
    };
    let res = execute(deps.as_mut(), env.clone(), info.clone(), msg);
    assert_error(res, "Cannot unbond more than balance");

    let msg = ExecuteMsg::Unbond {
        amount: Uint128::from(60000u128),
    };
    let res = execute(deps.as_mut(), env.clone(), info.clone(), msg)?;
    assert_eq!(
        res.messages
            .into_iter()
            .map(|it| it.msg)
            .collect::<Vec<CosmosMsg>>(),
        [
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: GENERATOR_PROXY.to_string(),
                msg: to_binary(&GeneratorExecuteMsg::Withdraw {
                    lp_token: LP_TOKEN.to_string(),
                    amount: Uint128::from(60000u128)
                })?,
                funds: vec![],
            }),
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: LP_TOKEN.to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Transfer {
                    recipient: USER_1.to_string(),
                    amount: Uint128::from(60000u128)
                })?,
                funds: vec![],
            }),
        ]
    );

    // decrease generator balance by 60000 from withdrawal
    deps.querier.set_balance(
        GENERATOR_PROXY.to_string(),
        LP_TOKEN.to_string(),
        Uint128::from(120000u128),
    );

    // query reward info for user_1, bond amount should be 120000 - 60000 = 60000
    let msg = QueryMsg::RewardInfo {
        staker_addr: USER_1.to_string(),
    };
    let res: RewardInfoResponse = from_binary(&query(deps.as_ref(), env.clone(), msg)?)?;
    assert_eq!(
        res,
        RewardInfoResponse {
            staker_addr: USER_1.to_string(),
            reward_info: {
                RewardInfoResponseItem {
                    bond_share: Uint128::from(50000u128),
                    deposit_amount: Uint128::from(50000u128),
                    deposit_time: 101,
                    staking_token: LP_TOKEN.to_string(),
                    bond_amount: Uint128::from(60000u128),
                    deposit_costs: vec![
                        Uint128::from(50000u128),
                        Uint128::from(50000u128),
                    ],
                }
            }
        }
    );

    // query reward info for user_2
    let msg = QueryMsg::RewardInfo {
        staker_addr: USER_2.to_string(),
    };
    let res: RewardInfoResponse = from_binary(&query(deps.as_ref(), env.clone(), msg)?)?;
    assert_eq!(
        res,
        RewardInfoResponse {
            staker_addr: USER_2.to_string(),
            reward_info: {
                RewardInfoResponseItem {
                    bond_share: Uint128::from(50000u128),
                    deposit_amount: Uint128::from(50000u128),
                    deposit_time: 102,
                    staking_token: LP_TOKEN.to_string(),
                    bond_amount: Uint128::from(60000u128),
                    deposit_costs: vec![
                        Uint128::from(50000u128),
                        Uint128::from(50000u128),
                    ],
                }
            }
        }
    );

    // unbond for user_2
    let info = mock_info(USER_2, &[]);
    let msg = ExecuteMsg::Unbond {
        amount: Uint128::from(60000u128),
    };
    let res = execute(deps.as_mut(), env.clone(), info.clone(), msg)?;
    assert_eq!(
        res.messages
            .into_iter()
            .map(|it| it.msg)
            .collect::<Vec<CosmosMsg>>(),
        [
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: GENERATOR_PROXY.to_string(),
                msg: to_binary(&GeneratorExecuteMsg::Withdraw {
                    lp_token: LP_TOKEN.to_string(),
                    amount: Uint128::from(60000u128)
                })?,
                funds: vec![],
            }),
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: LP_TOKEN.to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Transfer {
                    recipient: USER_2.to_string(),
                    amount: Uint128::from(60000u128)
                })?,
                funds: vec![],
            }),
        ]
    );

    // decrease generator balance by 60000 from withdrawal
    deps.querier.set_balance(
        GENERATOR_PROXY.to_string(),
        LP_TOKEN.to_string(),
        Uint128::from(60000u128),
    );

    // query reward info for user_2, bond amount should be 60000 - 60000 = 0
    let msg = QueryMsg::RewardInfo {
        staker_addr: USER_2.to_string(),
    };
    let res: RewardInfoResponse = from_binary(&query(deps.as_ref(), env.clone(), msg)?)?;
    assert_eq!(
        res,
        RewardInfoResponse {
            staker_addr: USER_2.to_string(),
            reward_info: {
                RewardInfoResponseItem {
                    bond_share: Uint128::from(0u128),
                    deposit_amount: Uint128::from(0u128),
                    deposit_time: 102,
                    staking_token: LP_TOKEN.to_string(),
                    bond_amount: Uint128::from(0u128),
                    deposit_costs: vec![],
                }
            }
        }
    );

    // query reward info for user_1
    let msg = QueryMsg::RewardInfo {
        staker_addr: USER_1.to_string(),
    };
    let res: RewardInfoResponse = from_binary(&query(deps.as_ref(), env.clone(), msg)?)?;
    assert_eq!(
        res,
        RewardInfoResponse {
            staker_addr: USER_1.to_string(),
            reward_info: {
                RewardInfoResponseItem {
                    bond_share: Uint128::from(50000u128),
                    deposit_amount: Uint128::from(50000u128),
                    deposit_time: 101,
                    staking_token: LP_TOKEN.to_string(),
                    bond_amount: Uint128::from(60000u128),
                    deposit_costs: vec![
                        Uint128::from(50000u128),
                        Uint128::from(50000u128),
                    ],
                }
            }
        }
    );

    // update time
    env.block.height = 600;

    // set LP token balance of the contract
    deps.querier.set_balance(
        LP_TOKEN.to_string(),
        MOCK_CONTRACT_ADDR.to_string(),
        Uint128::from(142u128),
    );

    // deposit assets for user_1
    let info = mock_info(USER_1, &[]);
    let assets = vec![
        Asset {
            info: AssetInfo::Token {
                contract_addr: Addr::unchecked(REWARD_TOKEN),
            },
            amount: Uint128::from(20000u128),
        },
        Asset {
            info: AssetInfo::NativeToken {
                denom: IBC_TOKEN.to_string(),
            },
            amount: Uint128::from(40000u128),
        },
    ];
    let msg = ExecuteMsg::BondAssets {
        assets: assets.clone(),
        minimum_receive: Some(Uint128::from(10000u128)),
        no_swap: None,
        slippage_tolerance: Some(Decimal::percent(2)),
    };

    let res = execute(deps.as_mut(), env.clone(), info, msg.clone());
    assert_error(
        res,
        "Native token balance mismatch between the argument and the transferred",
    );

    let info = mock_info(
        USER_1,
        &[Coin {
            denom: IBC_TOKEN.to_string(),
            amount: Uint128::from(40000u128),
        }],
    );
    let res = execute(deps.as_mut(), env.clone(), info.clone(), msg.clone())?;
    assert_eq!(
        res.messages
            .into_iter()
            .map(|it| it.msg)
            .collect::<Vec<CosmosMsg>>(),
        [
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: REWARD_TOKEN.to_string(),
                msg: to_binary(&Cw20ExecuteMsg::TransferFrom {
                    owner: USER_1.to_string(),
                    recipient: MOCK_CONTRACT_ADDR.to_string(),
                    amount: Uint128::from(20000u128)
                })?,
                funds: vec![],
            }),
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: REWARD_TOKEN.to_string(),
                msg: to_binary(&Cw20ExecuteMsg::IncreaseAllowance {
                    spender: COMPOUND_PROXY.to_string(),
                    amount: Uint128::from(20000u128),
                    expires: Some(Expiration::AtHeight(601))
                })?,
                funds: vec![],
            }),
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: COMPOUND_PROXY.to_string(),
                msg: to_binary(&CompoundProxyExecuteMsg::Compound {
                    rewards: assets.clone(),
                    to: None,
                    no_swap: None,
                    slippage_tolerance: Some(Decimal::percent(2)),
                })?,
                funds: vec![Coin {
                    denom: IBC_TOKEN.to_string(),
                    amount: Uint128::from(40000u128),
                }],
            }),
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: MOCK_CONTRACT_ADDR.to_string(),
                msg: to_binary(&ExecuteMsg::Callback(CallbackMsg::BondTo {
                    to: Addr::unchecked(USER_1),
                    prev_balance: Uint128::from(142u128),
                    minimum_receive: Some(Uint128::from(10000u128)),
                }))?,
                funds: vec![],
            }),
        ]
    );

    let msg = ExecuteMsg::BondAssets {
        assets: assets.clone(),
        minimum_receive: Some(Uint128::from(10000u128)),
        no_swap: Some(true),
        slippage_tolerance: None,
    };

    let res = execute(deps.as_mut(), env.clone(), info.clone(), msg)?;
    assert_eq!(
        res.messages
            .into_iter()
            .map(|it| it.msg)
            .collect::<Vec<CosmosMsg>>(),
        [
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: REWARD_TOKEN.to_string(),
                msg: to_binary(&Cw20ExecuteMsg::TransferFrom {
                    owner: USER_1.to_string(),
                    recipient: MOCK_CONTRACT_ADDR.to_string(),
                    amount: Uint128::from(20000u128)
                })?,
                funds: vec![],
            }),
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: REWARD_TOKEN.to_string(),
                msg: to_binary(&Cw20ExecuteMsg::IncreaseAllowance {
                    spender: COMPOUND_PROXY.to_string(),
                    amount: Uint128::from(20000u128),
                    expires: Some(Expiration::AtHeight(601))
                })?,
                funds: vec![],
            }),
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: COMPOUND_PROXY.to_string(),
                msg: to_binary(&CompoundProxyExecuteMsg::Compound {
                    rewards: assets,
                    to: None,
                    no_swap: Some(true),
                    slippage_tolerance: None,
                })?,
                funds: vec![Coin {
                    denom: IBC_TOKEN.to_string(),
                    amount: Uint128::from(40000u128),
                }],
            }),
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: MOCK_CONTRACT_ADDR.to_string(),
                msg: to_binary(&ExecuteMsg::Callback(CallbackMsg::BondTo {
                    to: Addr::unchecked(USER_1),
                    prev_balance: Uint128::from(142u128),
                    minimum_receive: Some(Uint128::from(10000u128)),
                }))?,
                funds: vec![],
            }),
        ]
    );

    // update time
    env.block.time = Timestamp::from_seconds(200201);

    // set LP token balance of the contract
    deps.querier.set_balance(
        LP_TOKEN.to_string(),
        MOCK_CONTRACT_ADDR.to_string(),
        Uint128::from(10141u128),
    );

    let msg = ExecuteMsg::Callback(CallbackMsg::BondTo {
        to: Addr::unchecked(USER_1),
        prev_balance: Uint128::from(142u128),
        minimum_receive: Some(Uint128::from(10000u128)),
    });
    let info = mock_info(MOCK_CONTRACT_ADDR, &[]);
    // received less LP token than minimum_receive, received 10141 - 142 = 9999 LP
    let res = execute(deps.as_mut(), env.clone(), info.clone(), msg.clone());
    assert_error(
        res,
        "Assertion failed; minimum receive amount: 10000, actual amount: 9999",
    );

    deps.querier.set_balance(
        LP_TOKEN.to_string(),
        MOCK_CONTRACT_ADDR.to_string(),
        Uint128::from(10142u128),
    );
    let res = execute(deps.as_mut(), env.clone(), info, msg)?;
    assert_eq!(
        res.messages
            .into_iter()
            .map(|it| it.msg)
            .collect::<Vec<CosmosMsg>>(),
        [CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: LP_TOKEN.to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Send {
                contract: GENERATOR_PROXY.to_string(),
                amount: Uint128::from(10000u128),
                msg: to_binary(&GeneratorCw20HookMsg::Deposit {})?,
            })?,
            funds: vec![],
        }),]
    );

    deps.querier.set_balance(
        LP_TOKEN.to_string(),
        MOCK_CONTRACT_ADDR.to_string(),
        Uint128::from(142u128),
    );

    // increase generator balance by 10000
    deps.querier.set_balance(
        GENERATOR_PROXY.to_string(),
        LP_TOKEN.to_string(),
        Uint128::from(70000u128),
    );

    // query reward info for user_1, bond amount should be 60000 + 10000 = 70000
    let msg = QueryMsg::RewardInfo {
        staker_addr: USER_1.to_string(),
    };
    let res: RewardInfoResponse = from_binary(&query(deps.as_ref(), env.clone(), msg)?)?;
    assert_eq!(
        res,
        RewardInfoResponse {
            staker_addr: USER_1.to_string(),
            reward_info: {
                RewardInfoResponseItem {
                    bond_share: Uint128::from(58333u128),
                    deposit_amount: Uint128::from(59999u128),
                    deposit_time: 33448,
                    staking_token: LP_TOKEN.to_string(),
                    bond_amount: Uint128::from(70000u128),
                    deposit_costs: vec![
                        Uint128::from(59999u128),
                        Uint128::from(59999u128),
                    ],
                }
            }
        }
    );

    // query state
    let msg = QueryMsg::State {};
    let res: State = from_binary(&query(deps.as_ref(), env.clone(), msg)?)?;
    assert_eq!(
        res,
        State {
            total_bond_share: Uint128::from(58333u128),
        }
    );

    Ok(())
}

fn deposit_time(
    deps: &mut OwnedDeps<MockStorage, MockApi, WasmMockQuerier>,
) -> Result<(), ContractError> {
    let mut env = mock_env();
    env.block.time = Timestamp::from_seconds(300000);

    // user_3 bond 10000 LP
    let info = mock_info(LP_TOKEN, &[]);
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: USER_3.to_string(),
        amount: Uint128::from(10000u128),
        msg: to_binary(&Cw20HookMsg::Bond { staker_addr: None })?,
    });
    let res = execute(deps.as_mut(), env.clone(), info, msg);
    assert!(res.is_ok());

    // increase generator balance by 10000 + 5000 (from compound)
    deps.querier.set_balance(
        GENERATOR_PROXY.to_string(),
        LP_TOKEN.to_string(),
        Uint128::from(85000u128),
    );

    // query reward info for user_3, should get only 10000
    let msg = QueryMsg::RewardInfo {
        staker_addr: USER_3.to_string(),
    };
    let res: RewardInfoResponse = from_binary(&query(deps.as_ref(), env.clone(), msg)?)?;
    assert_eq!(
        res,
        RewardInfoResponse {
            staker_addr: USER_3.to_string(),
            reward_info: {
                RewardInfoResponseItem {
                    bond_share: Uint128::from(8333u128),
                    deposit_amount: Uint128::from(9999u128),
                    deposit_time: 300000,
                    staking_token: LP_TOKEN.to_string(),
                    bond_amount: Uint128::from(9999u128),
                    deposit_costs: vec![
                        Uint128::from(9999u128),
                        Uint128::from(9999u128),
                    ],
                }
            }
        }
    );

    env.block.time = Timestamp::from_seconds(343200);

    // query reward info for user_3, should increase to 10312 instead of 10624
    let msg = QueryMsg::RewardInfo {
        staker_addr: USER_3.to_string(),
    };
    let res: RewardInfoResponse = from_binary(&query(deps.as_ref(), env.clone(), msg)?)?;
    assert_eq!(
        res,
        RewardInfoResponse {
            staker_addr: USER_3.to_string(),
            reward_info: {
                RewardInfoResponseItem {
                    bond_share: Uint128::from(8333u128),
                    deposit_amount: Uint128::from(9999u128),
                    deposit_time: 300000,
                    staking_token: LP_TOKEN.to_string(),
                    bond_amount: Uint128::from(10311u128),
                    deposit_costs: vec![
                        Uint128::from(9999u128),
                        Uint128::from(9999u128),
                    ],
                }
            }
        }
    );

    // query reward info for user_1, should be 74375
    let msg = QueryMsg::RewardInfo {
        staker_addr: USER_1.to_string(),
    };
    let res: RewardInfoResponse = from_binary(&query(deps.as_ref(), env.clone(), msg)?)?;
    assert_eq!(
        res,
        RewardInfoResponse {
            staker_addr: USER_1.to_string(),
            reward_info: {
                RewardInfoResponseItem {
                    bond_share: Uint128::from(58333u128),
                    deposit_amount: Uint128::from(59999u128),
                    deposit_time: 33448,
                    staking_token: LP_TOKEN.to_string(),
                    bond_amount: Uint128::from(74375u128),
                    deposit_costs: vec![
                        Uint128::from(59999u128),
                        Uint128::from(59999u128),
                    ],
                }
            }
        }
    );

    // minimum time reached
    env.block.time = Timestamp::from_seconds(386400);

    // query reward info for user_3, should increase 10624
    let msg = QueryMsg::RewardInfo {
        staker_addr: USER_3.to_string(),
    };
    let res: RewardInfoResponse = from_binary(&query(deps.as_ref(), env.clone(), msg)?)?;
    assert_eq!(
        res,
        RewardInfoResponse {
            staker_addr: USER_3.to_string(),
            reward_info: {
                RewardInfoResponseItem {
                    bond_share: Uint128::from(8333u128),
                    deposit_amount: Uint128::from(9999u128),
                    deposit_time: 300000,
                    staking_token: LP_TOKEN.to_string(),
                    bond_amount: Uint128::from(10624u128),
                    deposit_costs: vec![
                        Uint128::from(9999u128),
                        Uint128::from(9999u128),
                    ],
                }
            }
        }
    );

    // rewind time
    env.block.time = Timestamp::from_seconds(343200);

    // unbond for user_3
    let info = mock_info(USER_3, &[]);
    let msg = ExecuteMsg::Unbond {
        amount: Uint128::from(10311u128),
    };
    let res = execute(deps.as_mut(), env.clone(), info.clone(), msg)?;
    assert_eq!(
        res.messages
            .into_iter()
            .map(|it| it.msg)
            .collect::<Vec<CosmosMsg>>(),
        [
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: GENERATOR_PROXY.to_string(),
                msg: to_binary(&GeneratorExecuteMsg::Withdraw {
                    lp_token: LP_TOKEN.to_string(),
                    amount: Uint128::from(10311u128)
                })?,
                funds: vec![],
            }),
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: LP_TOKEN.to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Transfer {
                    recipient: USER_3.to_string(),
                    amount: Uint128::from(10311u128)
                })?,
                funds: vec![],
            }),
        ]
    );

    // increase generator balance by 10311
    deps.querier.set_balance(
        GENERATOR_PROXY.to_string(),
        LP_TOKEN.to_string(),
        Uint128::from(74689u128),
    );

    // query reward info for user_1, should be 74375 + 312 (from user_3 penalty)= 74687
    let msg = QueryMsg::RewardInfo {
        staker_addr: USER_1.to_string(),
    };
    let res: RewardInfoResponse = from_binary(&query(deps.as_ref(), env.clone(), msg)?)?;
    assert_eq!(
        res,
        RewardInfoResponse {
            staker_addr: USER_1.to_string(),
            reward_info: {
                RewardInfoResponseItem {
                    bond_share: Uint128::from(58333u128),
                    deposit_amount: Uint128::from(59999u128),
                    deposit_time: 33448,
                    staking_token: LP_TOKEN.to_string(),
                    bond_amount: Uint128::from(74689u128),
                    deposit_costs: vec![
                        Uint128::from(59999u128),
                        Uint128::from(59999u128),
                    ],
                }
            }
        }
    );

    Ok(())
}

fn compound(
    deps: &mut OwnedDeps<MockStorage, MockApi, WasmMockQuerier>,
) -> Result<(), ContractError> {
    let mut env = mock_env();

    // reset LP token balance of the contract
    deps.querier.set_balance(
        LP_TOKEN.to_string(),
        MOCK_CONTRACT_ADDR.to_string(),
        Uint128::from(1u128),
    );

    // set pending tokens
    deps.querier.set_balance(
        GENERATOR_PROXY.to_string(),
        ASTRO_TOKEN.to_string(),
        Uint128::from(10000u128),
    );
    deps.querier.set_balance(
        GENERATOR_PROXY.to_string(),
        REWARD_TOKEN.to_string(),
        Uint128::from(50000u128),
    );

    // set block height
    env.block.height = 700;

    // only controller can execute compound
    let info = mock_info(USER_1, &[]);
    let msg = ExecuteMsg::Compound {
        minimum_receive: Some(Uint128::from(29900u128)),
        slippage_tolerance: Some(Decimal::percent(3)),
    };
    let res = execute(deps.as_mut(), env.clone(), info.clone(), msg.clone());
    assert_error(res, "Unauthorized");

    let info = mock_info(CONTROLLER, &[]);
    let res = execute(deps.as_mut(), env.clone(), info.clone(), msg)?;
    assert_eq!(
        res.messages
            .into_iter()
            .map(|it| it.msg)
            .collect::<Vec<CosmosMsg>>(),
        [
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: GENERATOR_PROXY.to_string(),
                msg: to_binary(&GeneratorExecuteMsg::ClaimRewards {
                    lp_tokens: vec![LP_TOKEN.to_string()]
                })?,
                funds: vec![],
            }),
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: ASTRO_TOKEN.to_string(),
                msg: to_binary(&Cw20ExecuteMsg::IncreaseAllowance {
                    spender: COMPOUND_PROXY.to_string(),
                    amount: Uint128::from(9500u128),
                    expires: Some(Expiration::AtHeight(701))
                })?,
                funds: vec![],
            }),
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: ASTRO_TOKEN.to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Transfer {
                    recipient: FEE_COLLECTOR.to_string(),
                    amount: Uint128::from(500u128)
                })?,
                funds: vec![],
            }),
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: REWARD_TOKEN.to_string(),
                msg: to_binary(&Cw20ExecuteMsg::IncreaseAllowance {
                    spender: COMPOUND_PROXY.to_string(),
                    amount: Uint128::from(47500u128),
                    expires: Some(Expiration::AtHeight(701))
                })?,
                funds: vec![],
            }),
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: REWARD_TOKEN.to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Transfer {
                    recipient: FEE_COLLECTOR.to_string(),
                    amount: Uint128::from(2500u128)
                })?,
                funds: vec![],
            }),
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: COMPOUND_PROXY.to_string(),
                msg: to_binary(&CompoundProxyExecuteMsg::Compound {
                    rewards: vec![
                        Asset {
                            info: AssetInfo::Token {
                                contract_addr: Addr::unchecked(ASTRO_TOKEN),
                            },
                            amount: Uint128::from(9500u128),
                        },
                        Asset {
                            info: AssetInfo::Token {
                                contract_addr: Addr::unchecked(REWARD_TOKEN),
                            },
                            amount: Uint128::from(47500u128),
                        },
                    ],
                    to: None,
                    no_swap: None,
                    slippage_tolerance: Some(Decimal::percent(3)),
                })?,
                funds: vec![],
            }),
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: MOCK_CONTRACT_ADDR.to_string(),
                msg: to_binary(&ExecuteMsg::Callback(CallbackMsg::Stake {
                    prev_balance: Uint128::from(1u128),
                    minimum_receive: Some(Uint128::from(29900u128)),
                }))?,
                funds: vec![],
            }),
        ]
    );

    // receive 29899 LP token from compound proxy
    deps.querier.set_balance(
        LP_TOKEN.to_string(),
        MOCK_CONTRACT_ADDR.to_string(),
        Uint128::from(29900u128),
    );

    let msg = ExecuteMsg::Callback(CallbackMsg::Stake {
        prev_balance: Uint128::from(1u128),
        minimum_receive: Some(Uint128::from(29900u128)),
    });
    let info = mock_info(MOCK_CONTRACT_ADDR, &[]);

    // received less LP token than minimum_receive, received 29900 - 1 = 29899 LP
    let res = execute(deps.as_mut(), env.clone(), info.clone(), msg.clone());
    assert_error(
        res,
        "Assertion failed; minimum receive amount: 29900, actual amount: 29899",
    );

    deps.querier.set_balance(
        LP_TOKEN.to_string(),
        MOCK_CONTRACT_ADDR.to_string(),
        Uint128::from(29901u128),
    );
    let res = execute(deps.as_mut(), env.clone(), info, msg)?;
    assert_eq!(
        res.messages
            .into_iter()
            .map(|it| it.msg)
            .collect::<Vec<CosmosMsg>>(),
        [CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: LP_TOKEN.to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Send {
                contract: GENERATOR_PROXY.to_string(),
                amount: Uint128::from(29900u128),
                msg: to_binary(&GeneratorCw20HookMsg::Deposit {})?,
            })?,
            funds: vec![],
        }),]
    );

    Ok(())
}

fn callback(
    deps: &mut OwnedDeps<MockStorage, MockApi, WasmMockQuerier>,
) -> Result<(), ContractError> {
    let env = mock_env();

    let msg = ExecuteMsg::Callback(CallbackMsg::Stake {
        prev_balance: Uint128::zero(),
        minimum_receive: None,
    });

    let info = mock_info(USER_1, &[]);

    // only contract itself can execute callback
    let res = execute(deps.as_mut(), env.clone(), info, msg);
    assert_error(res, "Unauthorized");

    let msg = ExecuteMsg::Callback(CallbackMsg::BondTo {
        to: Addr::unchecked(USER_1),
        prev_balance: Uint128::zero(),
        minimum_receive: None,
    });
    let info = mock_info(USER_1, &[]);

    // only contract itself can execute callback
    let res = execute(deps.as_mut(), env.clone(), info, msg.clone());
    assert_error(res, "Unauthorized");

    Ok(())
}
