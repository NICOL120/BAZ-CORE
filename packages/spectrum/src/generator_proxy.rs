use cosmwasm_std::{Addr, CosmosMsg, Decimal, QuerierWrapper, StdResult, to_binary, Uint128, WasmMsg};
use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use astroport::asset::{Asset, AssetInfo};
use astroport::generator::PendingTokenResponse;
use astroport::restricted_vector::RestrictedVector;
use crate::helper::ScalingUint128;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    pub generator: String,
    pub astro_gov: String, // AstroGovUnchecked,
    pub owner: String,
    pub controller: String,
    pub astro_token: String,
    pub spastro_token: String,
    pub fee_distributor: String,
    pub income_distributor: String,
    pub max_quota: Uint128,
    pub spastro_rate: Decimal,
    pub fee_rate: Decimal,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    pub generator: Generator,
    pub astro_gov: Addr, // AstroGov,
    pub owner: Addr,
    pub controller: Addr,
    pub astro_token: Addr,
    pub spastro_token: Addr,
    pub fee_distributor: Addr,
    pub income_distributor: Addr,
    pub max_quota: Uint128,
    pub spastro_rate: Decimal,
    pub fee_rate: Decimal,
}

pub fn zero_address() -> Addr {
    Addr::unchecked("")
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema, Default)]
pub struct PoolInfo {
    pub total_bond_share: Uint128,
    pub reward_indexes: RestrictedVector<Addr, Decimal>,
    pub prev_reward_user_index: Decimal,
    pub prev_reward_debt_proxy: RestrictedVector<Addr, Uint128>,
}

impl PoolInfo {
    pub fn calc_bond_share(
        &self,
        total_bond_amount: Uint128,
        amount: Uint128,
        ceiling: bool,
    ) -> Uint128 {
        if self.total_bond_share.is_zero() || total_bond_amount.is_zero() {
            amount
        } else if ceiling {
            amount.multiply_ratio_and_ceil(self.total_bond_share, total_bond_amount)
        } else {
            amount.multiply_ratio(self.total_bond_share, total_bond_amount)
        }
    }

    pub fn calc_bond_amount(&self, total_bond_amount: Uint128, share: Uint128) -> Uint128 {
        if self.total_bond_share.is_zero() {
            Uint128::zero()
        } else {
            total_bond_amount.multiply_ratio(share, self.total_bond_share)
        }
    }

}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct UserInfo {
    pub bond_share: Uint128,
    pub reward_indexes: RestrictedVector<Addr, Decimal>,
    pub pending_rewards: RestrictedVector<Addr, Uint128>,
}

impl UserInfo {
    pub fn create(pool_info: &PoolInfo) -> UserInfo {
        UserInfo {
            bond_share: Uint128::zero(),
            reward_indexes: pool_info.reward_indexes.clone(),
            pending_rewards: RestrictedVector::default(),
        }
    }
}


#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct LockedIncome {
    pub start: u64,
    pub end: u64,
    pub amount: Uint128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema, Default)]
pub struct RewardInfo {
    pub reconciled_amount: Uint128,
    pub fee: Uint128,
    pub spastro_income: Uint128,
    pub locked_income: Option<LockedIncome>,
}

impl RewardInfo {
    pub fn realize_unlocked_amount(
        &mut self,
        now: u64,
    ) {
        if let Some(locked_income) = &self.locked_income {
            if now >= locked_income.end {
                self.spastro_income += locked_income.amount;
                self.locked_income = None;
            } else if now > locked_income.start {
                let unlocked_amount = locked_income.amount.multiply_ratio(
                    now - locked_income.start,
                    locked_income.end - locked_income.start,
                );
                self.spastro_income += unlocked_amount;
                self.locked_income = Some(LockedIncome {
                    start: now,
                    end: locked_income.end,
                    amount: locked_income.amount - unlocked_amount,
                });
            }
        }
    }

}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema, Default)]
pub struct PoolConfig {
    pub asset_rewards: Vec<AssetInfo>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct State {
    pub next_claim_period: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    Receive(Cw20ReceiveMsg),
    Callback(CallbackMsg),

    // owner
    UpdateConfig {
        proposed_owner: Option<String>,
        spastro_token: Option<String>,
        controller: Option<String>,
        fee_rate: Option<Decimal>,
    },
    ClaimOwner {},

    // controller's actions
    UpdateParameters {
        max_quota: Option<Uint128>,
        spastro_rate: Option<Decimal>,
    },
    UpdatePoolConfig {
        lp_token: String,
        asset_rewards: Option<Vec<AssetInfo>>,
    },
    ControllerVote {
        votes: Vec<(String, u16)>,
    },
    ExtendLockTime { time: u64 },
    SendIncome {},

    // anyone
    ReconcileGovIncome {},

    // from generator
    Withdraw {
        lp_token: String,
        amount: Uint128,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CallbackMsg {
    AfterClaimed {
        lp_token: Addr,
    },
    Deposit {
        lp_token: Addr,
        staker_addr: Addr,
        amount: Uint128,
    },
    Withdraw {
        lp_token: Addr,
        staker_addr: Addr,
        amount: Uint128,
    },
    AfterBondChanged {
        lp_token: Addr,
        prev_assets: Vec<Asset>,
    },
    ClaimRewards {
        lp_token: Addr,
        staker_addr: Addr,
    },
}

impl CallbackMsg {
    pub fn to_cosmos_msg(&self, contract_addr: &Addr) -> StdResult<CosmosMsg> {
        Ok(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: String::from(contract_addr),
            msg: to_binary(&ExecuteMsg::Callback(self.clone()))?,
            funds: vec![],
        }))
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Cw20HookMsg {
    // from generator
    Deposit {},

    // spASTRO
    Convert {},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Config {},
    PoolConfig {
        lp_token: String,
    },
    PoolInfo {
        lp_token: String,
    },
    UserInfo {
        lp_token: String,
        user: String,
    },
    RewardInfo {
        token: String,
    },
    State {},

    // from generator
    PendingToken { lp_token: String, user: String },
    Deposit { lp_token: String, user: String },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Generator(pub Addr);

impl Generator {
    pub fn query_pending_token(
        &self,
        querier: &QuerierWrapper,
        lp_token: &Addr,
        staker: &Addr,
    ) -> StdResult<PendingTokenResponse> {
        querier.query_wasm_smart(self.0.to_string(), &QueryMsg::PendingToken {
            lp_token: lp_token.to_string(),
            user: staker.to_string(),
        })
    }

    pub fn query_pool_balance(
        &self,
        querier: &QuerierWrapper,
        lp_token: &Addr,
        staker: &Addr,
    ) -> StdResult<Uint128> {
        querier.query_wasm_smart(self.0.to_string(),&QueryMsg::Deposit {
            lp_token: lp_token.to_string(),
            user: staker.to_string(),
        })
    }

    pub fn deposit_msg(&self, lp_token: String, amount: Uint128) -> StdResult<CosmosMsg> {
        Ok(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: lp_token,
            funds: vec![],
            msg: to_binary(&Cw20ExecuteMsg::Send {
                contract: self.0.to_string(),
                amount,
                msg: to_binary(&Cw20HookMsg::Deposit {})?,
            })?,
        }))
    }

    pub fn withdraw_msg(&self, lp_token: String, amount: Uint128) -> StdResult<CosmosMsg> {
        Ok(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: self.0.to_string(),
            funds: vec![],
            msg: to_binary(&ExecuteMsg::Withdraw {
                lp_token,
                amount,
            })?,
        }))
    }
}
