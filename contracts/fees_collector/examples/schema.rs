use std::env::current_dir;
use std::fs::create_dir_all;

use cosmwasm_schema::{remove_schemas, schema_for, export_schema};

use baz:fees_collector::{ExecuteMsg, InstantiateMsg, QueryMsg, BalancesResponse, AssetWithLimit, CollectSimulationResponse};
use spectrum_fees_collector::state::Config;

fn main() {
    let mut out_dir = current_dir().unwrap();
    out_dir.push("schema");
    create_dir_all(&out_dir).unwrap();
    remove_schemas(&out_dir).unwrap();

    export_schema(&schema_for!(InstantiateMsg), &out_dir);
    export_schema(&schema_for!(ExecuteMsg), &out_dir);
    export_schema(&schema_for!(QueryMsg), &out_dir);
    export_schema(&schema_for!(BalancesResponse), &out_dir);
    export_schema(&schema_for!(AssetWithLimit), &out_dir);
    export_schema(&schema_for!(Config), &out_dir);
    export_schema(&schema_for!(CollectSimulationResponse), &out_dir);
}
