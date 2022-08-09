use crate::error::ContractError;
use crate::querier::query_pair_info;
use crate::state::{Config, CONFIG, PAIR_PROXY};
use std::convert::TryInto;

use astroport::factory::PairType;
use cosmwasm_std::{
    entry_point, to_binary, Addr, Binary, Coin, CosmosMsg, Decimal, Decimal256, Deps, DepsMut, Env,
    Isqrt, MessageInfo, QuerierWrapper, Response, StdError, StdResult, Uint128, Uint256, WasmMsg,
};
use cw20::{Cw20ExecuteMsg, Expiration};
use spectrum::compound_proxy::{
    CallbackMsg, ConfigResponse, ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg,
};

use astroport::asset::{addr_validate_to_lower, Asset, AssetInfo};
use astroport::pair::{
    Cw20HookMsg as AstroportPairCw20HookMsg, ExecuteMsg as AstroportPairExecuteMsg,
};
use cw2::set_contract_version;
use spectrum::farm_helper::deposit_asset;

/// Contract name that is used for migration.
const CONTRACT_NAME: &str = "spectrum-compound-proxy";
/// Contract version that is used for migration.
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");
/// Scaling denominator for commission
const COMMISSION_DENOM: u64 = 10000u64;

fn validate_commission(commission_bps: u64) -> StdResult<u64> {
    if commission_bps >= 10000u64 {
        Err(StdError::generic_err("commission rate must be 0 to 9999"))
    } else {
        Ok(commission_bps)
    }
}

/// (we require 0-1)
fn validate_percentage(value: Decimal, field: &str) -> StdResult<Decimal> {
    if value > Decimal::one() {
        Err(StdError::generic_err(field.to_string() + " must be 0 to 1"))
    } else {
        Ok(value)
    }
}

/// ## Description
/// Creates a new contract with the specified parameters in the [`InstantiateMsg`].
/// Returns the [`Response`] with the specified attributes if the operation was successful, or a [`ContractError`] if the contract was not created
/// ## Params
/// * **deps** is the object of type [`DepsMut`].
///
/// * **env** is the object of type [`Env`].
///
/// * **_info** is the object of type [`MessageInfo`].
/// * **msg** is a message of type [`InstantiateMsg`] which contains the basic settings for creating a contract
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    let commission_bps = validate_commission(msg.commission_bps)?;
    let slippage_tolerance = validate_percentage(msg.slippage_tolerance, "slippage_tolerance")?;
    let pair_contract = addr_validate_to_lower(deps.api, msg.pair_contract.as_str())?;
    let pair_info = query_pair_info(deps.as_ref(), &pair_contract)?;

    let config = Config {
        pair_info,
        commission_bps,
        slippage_tolerance,
    };
    CONFIG.save(deps.storage, &config)?;

    for (asset_info, pair_proxy) in msg.pair_proxies {
        asset_info.check(deps.api)?;
        let pair_proxy_addr = addr_validate_to_lower(deps.api, &pair_proxy)?;
        PAIR_PROXY.save(deps.storage, asset_info.to_string(), &pair_proxy_addr)?;
    }

    Ok(Response::new())
}

/// ## Description
/// Available the execute messages of the contract.
/// ## Params
/// * **deps** is the object of type [`Deps`].
///
/// * **env** is the object of type [`Env`].
///
/// * **info** is the object of type [`MessageInfo`].
///
/// * **msg** is the object of type [`ExecuteMsg`].
///
/// ## Queries
/// * **ExecuteMsg::UpdateConfig { params: Binary }** Not supported.
///
/// * **ExecuteMsg::Receive(msg)** Receives a message of type [`Cw20ReceiveMsg`] and processes
/// it depending on the received template.
///
/// * **ExecuteMsg::ProvideLiquidity {
///             assets,
///             slippage_tolerance,
///             auto_stake,
///             receiver,
///         }** Provides liquidity with the specified input parameters.
///
/// * **ExecuteMsg::Swap {
///             offer_asset,
///             belief_price,
///             max_spread,
///             to,
///         }** Performs an swap operation with the specified parameters.
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Compound { rewards, to } => {
            let to_addr = if let Some(to_addr) = to {
                Some(addr_validate_to_lower(deps.api, &to_addr)?)
            } else {
                None
            };
            compound(deps, env, info.clone(), info.sender, rewards, to_addr)
        }
        ExecuteMsg::Callback(msg) => handle_callback(deps, env, info, msg),
    }
}

/// ## Description
/// Performs an swap operation with the specified parameters. CONTRACT - a user must do token approval.
/// Returns an [`ContractError`] on failure, otherwise returns the [`Response`] with the specified attributes if the operation was successful.
/// ## Params
/// * **deps** is the object of type [`DepsMut`].
///
/// * **env** is the object of type [`Env`].
///
/// * **info** is the object of type [`MessageInfo`].
///
/// * **sender** is the object of type [`Addr`]. Sets the default recipient of the swap operation.
///
/// * **offer_asset** is the object of type [`Asset`]. Proposed asset for swapping.
///
/// * **belief_price** is the object of type [`Option<Decimal>`]. Used to calculate the maximum spread.
///
/// * **max_spread** is the object of type [`Option<Decimal>`]. Sets the maximum spread of the swap operation.
///
/// * **to** is the object of type [`Option<Addr>`]. Sets the recipient of the swap operation.
#[allow(clippy::too_many_arguments)]
pub fn compound(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    sender: Addr,
    rewards: Vec<Asset>,
    to: Option<Addr>,
) -> Result<Response, ContractError> {
    let receiver = to.unwrap_or(sender);

    let mut messages: Vec<CosmosMsg> = vec![];

    // Swap reward to asset in the pair
    for reward in rewards {
        deposit_asset(&env, &info, &mut messages, &reward)?;
        let pair_proxy = PAIR_PROXY.may_load(deps.storage, reward.info.to_string())?;
        if let Some(pair_proxy) = pair_proxy {
            let swap_reward = swap_msg(
                pair_proxy.to_string(),
                &reward,
                None,
                Some(Decimal::percent(50u64)),
                None,
            )?;
            messages.push(swap_reward);
        }
    }

    messages.push(CallbackMsg::OptimalSwap {}.into_cosmos_msg(&env.contract.address)?);
    messages.push(
        CallbackMsg::ProvideLiquidity {
            receiver: receiver.to_string(),
        }
        .into_cosmos_msg(&env.contract.address)?,
    );

    Ok(Response::new()
        .add_messages(messages)
        .add_attribute("action", "compound"))
}

/// # Description
/// Handle the callbacks describes in the [`CallbackMsg`]. Returns an [`ContractError`] on failure, otherwise returns the [`Response`]
/// object with the specified submessages if the operation was successful.
/// # Params
/// * **deps** is the object of type [`DepsMut`].
///
/// * **env** is the object of type [`Env`].
///
/// * **info** is the object of type [`MessageInfo`].
///
/// * **msg** is the object of type [`CallbackMsg`]. Sets the callback action.
///
/// ## Executor
/// Callback functions can only be called this contract itself
pub fn handle_callback(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: CallbackMsg,
) -> Result<Response, ContractError> {
    // Callback functions can only be called this contract itself
    if info.sender != env.contract.address {
        return Err(ContractError::Unauthorized {});
    }
    match msg {
        CallbackMsg::OptimalSwap {} => optimal_swap(deps, env, info),
        CallbackMsg::ProvideLiquidity { receiver } => provide_liquidity(deps, env, info, receiver),
    }
}

pub fn optimal_swap(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
) -> Result<Response, ContractError> {
    let config: Config = CONFIG.load(deps.storage)?;

    let mut messages: Vec<CosmosMsg> = vec![];

    match config.pair_info.pair_type {
        PairType::Stable {} => {
            //Do nothing for stable pair
        }
        _ => {
            let asset_a_info = config.pair_info.asset_infos[0].clone();
            let asset_b_info = config.pair_info.asset_infos[1].clone();

            let asset_a_amount =
                asset_a_info.query_pool(&deps.querier, env.contract.address.clone())?;
            let asset_b_amount = asset_b_info.query_pool(&deps.querier, env.contract.address)?;

            let asset_a = Asset {
                info: asset_a_info,
                amount: asset_a_amount,
            };

            let asset_b = Asset {
                info: asset_b_info,
                amount: asset_b_amount,
            };
            if !asset_a.amount.is_zero() || !asset_b_amount.is_zero() {
                calculate_optimal_swap(
                    &deps.querier,
                    &config,
                    asset_a,
                    asset_b,
                    None,
                    None,
                    &mut messages,
                )?;
            }
        }
    }

    Ok(Response::new()
        .add_messages(messages)
        .add_attribute("action", "optimal_swap"))
}

fn calculate_optimal_swap(
    querier: &QuerierWrapper,
    config: &Config,
    asset_a: Asset,
    asset_b: Asset,
    belief_price: Option<Decimal>,
    max_spread: Option<Decimal>,
    messages: &mut Vec<CosmosMsg>,
) -> StdResult<()> {
    let [pool_a, pool_b] = config
        .pair_info
        .query_pools(querier, config.pair_info.contract_addr.clone())?;
    let provide_a_amount: Uint256 = asset_a.amount.into();
    let provide_b_amount: Uint256 = asset_b.amount.into();
    let pool_a_amount: Uint256 = pool_a.amount.into();
    let pool_b_amount: Uint256 = pool_b.amount.into();
    let provide_a_area = provide_a_amount * pool_b_amount;
    let provide_b_area = provide_b_amount * pool_a_amount;

    #[allow(clippy::comparison_chain)]
    if provide_a_area > provide_b_area {
        let swap_amount = get_swap_amount(
            provide_a_amount,
            provide_b_amount,
            pool_a_amount,
            pool_b_amount,
            config.commission_bps,
        )?;
        if !swap_amount.is_zero() {
            let swap_asset = Asset {
                info: asset_a.info,
                amount: swap_amount,
            };
            let return_amount = simulate(
                pool_a_amount,
                pool_b_amount,
                swap_asset.amount.into(),
                Decimal256::from_ratio(config.commission_bps, COMMISSION_DENOM),
            )?;
            if !return_amount.is_zero() {
                messages.push(swap_msg(
                    config.pair_info.contract_addr.to_string(),
                    &swap_asset,
                    belief_price,
                    max_spread,
                    None,
                )?);
            }
        }
    } else if provide_a_area < provide_b_area {
        let swap_amount = get_swap_amount(
            provide_b_amount,
            provide_a_amount,
            pool_b_amount,
            pool_a_amount,
            config.commission_bps,
        )?;
        if !swap_amount.is_zero() {
            let swap_asset = Asset {
                info: asset_b.info,
                amount: swap_amount,
            };
            let return_amount = simulate(
                pool_b_amount,
                pool_a_amount,
                swap_asset.amount.into(),
                Decimal256::from_ratio(config.commission_bps, COMMISSION_DENOM),
            )?;
            if !return_amount.is_zero() {
                messages.push(swap_msg(
                    config.pair_info.contract_addr.to_string(),
                    &swap_asset,
                    belief_price,
                    max_spread,
                    None,
                )?);
            }
        }
    };

    Ok(())
}

pub fn provide_liquidity(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    receiver: String,
) -> Result<Response, ContractError> {
    let config: Config = CONFIG.load(deps.storage)?;

    let pair_contract = config.pair_info.contract_addr;
    let asset_infos = config.pair_info.asset_infos;

    let assets: [Asset; 2] = [
        Asset {
            info: asset_infos[0].clone(),
            amount: asset_infos[0]
                .query_pool(&deps.querier, env.contract.address.clone())?,
        },
        Asset {
            info: asset_infos[1].clone(),
            amount: asset_infos[1]
                .query_pool(&deps.querier, env.contract.address)?,
        },
    ];

    let mut messages: Vec<CosmosMsg> = vec![];
    let mut funds: Vec<Coin> = vec![];
    for asset in assets.iter() {
        if asset.is_native_token() {
            funds.push(Coin {
                denom: asset.info.to_string(),
                amount: asset.amount,
            });
        } else {
            messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: asset.info.to_string(),
                msg: to_binary(&Cw20ExecuteMsg::IncreaseAllowance {
                    spender: pair_contract.to_string(),
                    amount: asset.amount,
                    expires: Some(Expiration::AtHeight(env.block.height + 1)),
                })?,
                funds: vec![],
            }));
        }
    }

    let provide_liquidity = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: pair_contract.to_string(),
        msg: to_binary(&AstroportPairExecuteMsg::ProvideLiquidity {
            assets,
            slippage_tolerance: Some(config.slippage_tolerance),
            receiver: Some(receiver.to_string()),
            auto_stake: None,
        })?,
        funds,
    });
    messages.push(provide_liquidity);

    Ok(Response::new()
        .add_messages(messages)
        .add_attribute("action", "provide_liquidity")
        .add_attribute("receiver", receiver))
}

/// ## Description
/// Available the query messages of the contract.
/// ## Params
/// * **deps** is the object of type [`Deps`].
///
/// * **_env** is the object of type [`Env`].
///
/// * **msg** is the object of type [`QueryMsg`].
///
/// ## Queries
/// * **QueryMsg::Config {}** Returns information about the controls settings in a
/// [`ConfigResponse`] object.
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
    }
}

/// ## Description
/// Returns information about the controls settings in a [`ConfigResponse`] object.
/// ## Params
/// * **deps** is the object of type [`Deps`].
pub fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    let config: Config = CONFIG.load(deps.storage)?;
    Ok(ConfigResponse {
        pair_info: config.pair_info,
    })
}

fn get_swap_amount(
    amount_a: Uint256,
    amount_b: Uint256,
    pool_a: Uint256,
    pool_b: Uint256,
    commission_bps: u64,
) -> StdResult<Uint128> {
    let pool_ax = amount_a + pool_a;
    let pool_bx = amount_b + pool_b;
    let area_ax = pool_ax * pool_b;
    let area_bx = pool_bx * pool_a;

    let a = Uint256::from(commission_bps * commission_bps) * area_ax
        + Uint256::from(4u64 * (COMMISSION_DENOM - commission_bps) * COMMISSION_DENOM) * area_bx;
    let b = Uint256::from(commission_bps) * area_ax + area_ax.isqrt() * a.isqrt();
    let result = b / Uint256::from(2u64 * COMMISSION_DENOM) / pool_bx - pool_a;

    result
        .try_into()
        .map_err(|_| StdError::generic_err("overflow"))
}

fn simulate(
    offer_pool: Uint256,
    ask_pool: Uint256,
    offer_amount: Uint256,
    commission_rate: Decimal256,
) -> StdResult<Uint128> {
    // offer => ask
    // ask_amount = (ask_pool - cp / (offer_pool + offer_amount)) * (1 - commission_rate)
    let cp: Uint256 = offer_pool * ask_pool;
    let return_amount: Uint256 = (Decimal256::from_ratio(ask_pool, 1u64)
        - Decimal256::from_ratio(cp, offer_pool + offer_amount))
        * Uint256::from(1u64);

    // calculate commission
    let commission_amount: Uint256 = return_amount * commission_rate;

    // commission will be absorbed to pool
    let return_amount: Uint256 = return_amount - commission_amount;

    return_amount
        .try_into()
        .map_err(|_| StdError::generic_err("overflow"))
}

/// Generate msg for swapping specified asset
fn swap_msg(
    pair_contract: String,
    asset: &Asset,
    belief_price: Option<Decimal>,
    max_spread: Option<Decimal>,
    to: Option<String>,
) -> StdResult<CosmosMsg> {
    let wasm_msg = match &asset.info {
        AssetInfo::Token { contract_addr } => WasmMsg::Execute {
            contract_addr: contract_addr.to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Send {
                contract: pair_contract,
                amount: asset.amount,
                msg: to_binary(&AstroportPairCw20HookMsg::Swap {
                    belief_price,
                    max_spread,
                    to,
                })?,
            })?,
            funds: vec![],
        },

        AssetInfo::NativeToken { denom } => WasmMsg::Execute {
            contract_addr: pair_contract,
            msg: to_binary(&AstroportPairExecuteMsg::Swap {
                offer_asset: asset.clone(),
                belief_price,
                max_spread,
                to: None,
            })?,
            funds: vec![Coin {
                denom: denom.clone(),
                amount: asset.amount,
            }],
        },
    };

    Ok(CosmosMsg::Wasm(wasm_msg))
}

/// ## Description
/// Used for migration of contract. Returns the default object of type [`Response`].
/// ## Params
/// * **_deps** is the object of type [`DepsMut`].
///
/// * **_env** is the object of type [`Env`].
///
/// * **_msg** is the object of type [`MigrateMsg`].
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> StdResult<Response> {
    Ok(Response::default())
}
