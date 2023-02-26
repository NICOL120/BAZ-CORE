use astroport::asset::{Asset, AssetInfo};
use cosmwasm_std::{Uint128};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// This structure stores general parameters for the contract.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    /// Address that's allowed to update config
    pub owner: String,
    /// Address that's allowed to update bridge assets
    pub operator: String,
    /// The factory contract address
    pub factory_contract: String,
    /// The stablecoin asset info
    pub stablecoin: AssetInfo,
    /// The beneficiary addresses to received fees in stablecoin
    pub target_list: Vec<(String, u64)>,
}

/// This structure describes the functions that can be executed in this contract.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    /// Collects and swaps fee tokens to stablecoin
    Collect {
        /// The assets to swap to stablecoin
        assets: Vec<AssetWithLimit>,
        /// The minimum expected amount of stablecoine
        minimum_receive: Option<Uint128>,
    },
    /// Updates contract config
    UpdateConfig {
        /// The operator address
        operator: Option<String>,
        /// The factory contract address
        factory_contract: Option<String>,
        /// The list of target address to receive fees in stablecoin
        target_list: Option<Vec<(String, u64)>>,
    },
    /// Add bridge tokens used to swap bazific fee tokens to stablecoin (effectively declaring a swap route)
    UpdateBridges {
        /// List of bridge assets to be added
        add: Option<Vec<(AssetInfo, AssetInfo)>>,
        /// List of asset to be removed
        remove: Option<Vec<AssetInfo>>,
    },
    /// Swap fee tokens via bridge assets
    SwapBridgeAssets { assets: Vec<AssetInfo>, depth: u64 },
    /// Distribute stablecoin to beneficiary
    DistributeFees {
        /// The minimum expected amount of stablecoine
        minimum_receive: Option<Uint128>,
    },
    /// Creates a request to change the contract's ownership
    ProposeNewOwner {
        /// The newly proposed owner
        owner: String,
        /// The validity period of the proposal to change the owner
        expires_in: u64,
    },
    /// Removes a request to change contract ownership
    DropOwnershipProposal {},
    /// Claims contract ownership
    ClaimOwnership {},
}

/// This structure describes the query functions available in the contract.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    /// Returns information about the maker configs that contains in the [`ConfigResponse`]
    Config {},
    /// Returns the balance for each asset in the bazified input parameters
    Balances {
        assets: Vec<AssetInfo>,
    },
    /// Returns list of bridge assets
    Bridges {},
    /// Simulate collects and swaps fee tokens to stablecoin
    CollectSimulation {
        /// The assets to swap to stablecoin
        assets: Vec<AssetWithLimit>,
    }
}

/// A custom struct used to return multiple asset balances.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct BalancesResponse {
    /// List of asset and balance in the contract
    pub balances: Vec<Asset>,
}

/// This structure holds the parameters that are returned from a collect simulation response
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct CollectSimulationResponse {
    /// The amount of stablecoin returned from swap
    pub return_amount: Uint128,
}

/// This structure describes a migration message.
/// We currently take no arguments for migrations.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct MigrateMsg {}

/// This struct holds parameters to help with swapping a bazific amount of a fee token to ASTRO.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct AssetWithLimit {
    /// Information about the fee token to swap
    pub info: AssetInfo,
    /// The amount of tokens to swap
    pub limit: Option<Uint128>,
}
