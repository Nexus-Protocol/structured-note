use cosmwasm_bignumber::{Decimal256, Uint256};
use cosmwasm_std::{Addr, Decimal, Order, StdError, StdResult, Storage, Uint128, Uint64};
use cw_storage_plus::{Bound, Item, Map};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use terraswap::asset::Asset;

static KEY_CONFIG: Item<Config> = Item::new("config");
static KEY_DEPOSITING: Item<DepositingState> = Item::new("depositing");
// Map<cdp.masset_token, CDP>
static KEY_CDPS: Map<&Addr, CDP> = Map::new("cdps");
// Map<(position.farmer_addr, position.masset_token), Position>
static KEY_POSITIONS: Map<(&Addr, &Addr), Position> = Map::new("positions");

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    pub stable_denom: String,
    pub governance_contract: Addr,
    pub mirror_mint_contract: Addr,
    pub anchor_market_contract: Addr,
    pub aterra_addr: Addr,
    pub nexus_treasury: Addr,
    pub protocol_fee: Decimal256,
    pub min_over_collateralization: Decimal,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct CDP {
    pub idx: Uint128,
    pub masset_token: Addr,
    pub farmers: Vec<Addr>,
}

pub struct DepositingState {
    pub cdp_idx: Uint128,
    pub farmer_addr: Addr,
    pub masset_token: Addr,
    pub aim_collateral_ratio: Decimal,
    pub max_iteration_index: u8,
    pub cur_iteration_index: u8,
    pub initial_cdp_collateral_amount: Uint128,
    pub initial_cdp_loan_amount: Uint128,
    pub asset_price_in_collateral_asset: Decimal,
    pub mirror_ts_factory_addr: Addr,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Position {
    pub farmer_addr: Addr,
    pub masset_token: Addr,
    pub cdp_idx: Uint128,
    pub leverage_iter_amount: u8,
    pub total_loan_amount: Uint128,
    pub total_collateral_amount: Uint128,
    pub aterra_in_contract: Uint128,
}

pub fn load_config(storage: &dyn Storage) -> StdResult<Config> {
    KEY_CONFIG.load(storage)
}

pub fn store_config(storage: &mut dyn Storage, config: &Config) -> StdResult<()> {
    KEY_CONFIG.save(storage, config)
}

pub fn load_cdp(storage: &dyn Storage, masset_token: &Addr) -> StdResult<CDP> {
    KEY_CDPS.load(storage, masset_token)
}

pub fn load_all_cdps(storage: &dyn Storage) -> StdResult<Vec<CDP>> {
    KEY_CDPS
        .range(storage, None, None, Order::Ascending)
        .collect()
}

pub fn save_cdp(storage: &mut dyn Storage, cdp: &CDP) -> StdResult<()> {
    KEY_CDPS.save(storage, &cdp.masset_token, cdp)
}

pub fn remove_cdp(storage: &mut dyn Storage, masset_token: &Addr) {
    KEY_CDPS.remove(storage: &mut dyn Storage, masset_token)
}

pub fn add_farmer_to_cdp(storage: &mut dyn Storage, masset_token: &Addr, farmer_addr: &Addr) -> StdResult<CDP> {
    KEY_CDPS.update(
        storage,
        masset_token,
        |mut cdp: CDP| -> StdResult<_> {
            cdp.farmers.push(farmer_addr.clone());
            Ok(cdp)
        })
}

pub fn load_depositing_state(storage: &dyn Storage) -> StdResult<DepositingState> {
    KEY_DEPOSITING.load(storage)
}

pub fn store_depositing_state(storage: &mut dyn Storage, data: &DepositingState) -> StdResult<()> {
    KEY_DEPOSITING.save(storage, data)
}

pub fn load_position(storage: &dyn Storage, farmer_addr: &Addr, masset_token: &Addr) -> StdResult<Position> {
    KEY_POSITIONS.load(storage, (farmer_addr, masset_token))
}

pub fn update_position_on_deposit(
    storage: &mut dyn Storage,
    masset_token: &Addr,
    loan_diff: Uint128,
    collateral_diff: Uint128,
    aterra_amount_in_contract: Uint128,
) -> StdResult<Position> {
    KEY_POSITIONS.update(
        storage,
        (&depositing_state.farmer_addr, masset_token),
        |mut position: Position| -> StdResult<_> {
            position.total_collateral_amount += collateral_diff;
            position.total_loan_amount += loan_diff;
            position.aterra_in_contract = aterra_amount_in_contract;
            Ok(position)
        })
}

pub fn load_all_positions(storage: &dyn Storage) -> StdResult<Vec<Position>> {
    KEY_POSITIONS
        .range(&storage, None, None, Order::Ascending)
        .collect()
}

pub fn load_positions_by_farmer_addr(storage: &dyn Storage, farmer_addr: &Addr) -> StdResult<Vec<Position>> {
    KEY_POSITIONS
        .prefix(farmer_addr)
        .range(&storage, None, None, Order::Ascending)
        .collect()
}

pub fn save_position(storage: &mut dyn Storage, position: &Position) -> StdResult<()> {
    KEY_POSITIONS.save(storage, (&position.farmer_addr, &position.masset_token), position)
}

pub fn remove_position(storage: &mut dyn Storage, farmer_addr: &Addr, masset_token: &Addr) {
    KEY_POSITIONS.remove(storage: &mut dyn Storage, (farmer_addr, masset_token))
}

impl DepositingState {
    pub fn template(farmer_addr: Addr, masset_token: Addr, aim_collateral_ratio: Option<Decimal>, leverage_iter_amount: Option<u8>) -> DepositingState {
        DepositingState {
            cdp_idx: Default::default(),
            farmer_addr,
            masset_token,
            aim_collateral_ratio: aim_collateral_ratio.unwrap_or_else(Decimal::zero()),
            max_iteration_index: leverage_iter_amount.unwrap_or_else(0),
            cur_iteration_index: 0,
            initial_cdp_collateral_amount: Uint128::zero(),
            initial_cdp_loan_amount: Uint128::zero(),
            asset_price_in_collateral_asset: Decimal::zero(),
            mirror_ts_factory_addr: Default::default(),
        }
    }
}




