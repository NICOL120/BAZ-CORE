use std::collections::HashMap;
use std::ops::Deref;
use cosmwasm_std::{Addr, BalanceResponse, BankQuery, Binary, Coin, ContractResult, Empty, from_binary, from_slice, OwnedDeps, Querier, QuerierResult, QueryRequest, StdError, StdResult, SystemError, SystemResult, to_binary, Uint128, WasmQuery};
use cosmwasm_std::testing::{MockApi, MockStorage};
use cw_storage_plus::Map;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use astroport::asset::token_asset;
use astroport::generator::{PendingTokenResponse, UserInfoV2};

pub fn mock_dependencies() -> OwnedDeps<MockStorage, MockApi, WasmMockQuerier> {
    let custom_querier: WasmMockQuerier = WasmMockQuerier::new();

    OwnedDeps {
        storage: MockStorage::default(),
        api: MockApi::default(),
        querier: custom_querier,
        custom_query_type: Default::default()
    }
}

const ASTRO_TOKEN: &str = "astro";
const REWARD_TOKEN: &str = "reward";
const GENERATOR: &str = "generator";
const USER_INFO: Map<(&Addr, &Addr), UserInfoV2> = Map::new("user_info");

pub struct WasmMockQuerier {
    balances: HashMap<(String, String), Uint128>,
    raw: HashMap<(String, Binary), Binary>,
}

impl WasmMockQuerier {
    pub fn new() -> Self {
        WasmMockQuerier {
            balances: HashMap::new(),
            raw: HashMap::new(),
        }
    }

    pub fn set_balance(&mut self, token: String, addr: String, amount: Uint128) {
        self.balances.insert((token, addr), amount);
    }

    pub fn set_user_info(&mut self, lp_token: &Addr, user: &Addr, user_info: &UserInfoV2) -> StdResult<()> {
        let key = Binary::from(USER_INFO.key((lp_token, user)).deref());
        self.raw.insert((GENERATOR.to_string(), key), to_binary(user_info)?);

        Ok(())
    }

    fn get_balance(&self, token: String, addr: String) -> Uint128 {
        *self.balances.get(&(token, addr)).unwrap_or(&Uint128::zero())
    }

    fn execute_query(&self, request: &QueryRequest<Empty>) -> QuerierResult {
        let result = match request {
            QueryRequest::Bank(BankQuery::Balance {
                                   address,
                                   denom,
                               }) => {
                let amount = self.get_balance(denom.clone(), address.clone());
                to_binary(&BalanceResponse {
                    amount: Coin {
                        denom: denom.clone(),
                        amount,
                    },
                })
            },
            QueryRequest::Wasm(WasmQuery::Smart {
                                   contract_addr,
                                   msg,
                               }) => self.execute_wasm_query(contract_addr, msg),
            QueryRequest::Wasm(WasmQuery::Raw {
                                   contract_addr,
                                   key,
                               }) => {
                let value = self.raw.get(&(contract_addr.clone(), key.clone()));
                if let Some(binary) = value {
                    Ok(binary.clone())
                } else {
                    Err(StdError::generic_err("Raw data not found"))
                }
            },
            _ => return QuerierResult::Err(SystemError::Unknown {}),
        };
        QuerierResult::Ok(ContractResult::from(result))
    }

    fn execute_wasm_query(&self, contract_addr: &String, msg: &Binary) -> StdResult<Binary> {
        match from_binary(msg)? {
            MockQueryMsg::Balance {
                address,
            } => {
                let balance = self.get_balance(contract_addr.clone(), address);
                to_binary(&cw20::BalanceResponse {
                    balance,
                })
            },
            MockQueryMsg::Deposit {
                lp_token,
                ..
            } => {
                let balance = self.get_balance(contract_addr.clone(), lp_token);
                to_binary(&balance)
            },
            MockQueryMsg::PendingToken { .. } => {
                let pending = self.get_balance(contract_addr.clone(), ASTRO_TOKEN.to_string());
                let reward = self.get_balance(contract_addr.clone(), REWARD_TOKEN.to_string());
                to_binary(&PendingTokenResponse {
                    pending,
                    pending_on_proxy: Some(vec![
                        token_asset(Addr::unchecked(REWARD_TOKEN), reward),
                    ]),
                })
            }
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
enum MockQueryMsg {
    Balance {
        address: String,
    },
    Deposit {
        lp_token: String,
        user: String,
    },
    PendingToken {
        lp_token: String,
        user: String
    },
}

impl Querier for WasmMockQuerier {
    fn raw_query(&self, bin_request: &[u8]) -> QuerierResult {
        // MockQuerier doesn't support Custom, so we ignore it completely here
        let request: QueryRequest<Empty> = match from_slice(bin_request) {
            Ok(v) => v,
            Err(e) => {
                return SystemResult::Err(SystemError::InvalidRequest {
                    error: format!("Parsing query request: {}", e),
                    request: bin_request.into(),
                })
            }
        };
        self.execute_query(&request)
    }
}
