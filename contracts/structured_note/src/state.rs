use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_bignumber::{Decimal256, Uint256};
use cosmwasm_std::{Addr, Order, StdResult, Storage, Uint128, Uint64};
use cw_storage_plus::{Item, Map};
use terraswap::asset::Asset;

static KEY_CONFIG: Item<Config> = Item::new("config");
// Map<cdp.idx, CDP>
static KEY_CDPS: Map<&str, CDP> = Map::new("cdps");
// Map<(position.user_addr, position.cdp_idx), Position>
static KEY_POSITIONS: Map<(&Addr, &str), Postion> = Map::new("positions");

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    pub stable_denom: String,
    pub governance_contract: Addr,
    pub mirror_mint_contract: Addr,
    pub anchor_market_contract: Addr,
    pub anchor_token: Addr,
    pub nexus_treasury: Addr,
    pub protocol_fee: Decimal256,
}

struct CDP {
    idx: Uint256,
    ltv_min: Decimal256,
    ltv_max: Decimal256,
    ltv_aim: Decimal256,
}

struct Position {
    user_addr: Addr,
    cdp_idx: Uint256,
    leverage_iter_amount: Uint64,
    total_debt_amount: Uint256,
    total_collateral_amount: Uint256,
    final_aust_amount: Uint256,
    liquidation_ltv: Decimal256,
}

pub fn load_config(storage: &dyn Storage) -> StdResult<Config> {
    KEY_CONFIG.load(storage)
}

pub fn store_config(storage: &mut dyn Storage, config: &Config) -> StdResult<()> {
    KEY_CONFIG.save(storage, config)
}

pub fn load_all_cdps(storage: &dyn Storage) -> StdResult<Vec<CDP>> {
    KEY_CDPS
        .range(&storage, Bound::None, Bound::None, Order::Ascending)
        .collect()
}

pub fn save_cdp(storage: &mut dyn Storage, cdp: &CDP) -> StdResult<()> {
    KEY_CDPS.save(storage, &cdp.idx[..], cdp)
}

pub fn remove_cdp(storage: &mut dyn Storage, cdp_idx: &str) {
    KEY_CDPS.remove(storage: &mut dyn Storage, cdp_idx)
}

pub fn load_all_positions(storage: &dyn Storage) -> StdResult<Vec<Position>> {
    KEY_POSITIONS
        .range(&storage, Bound::None, Bound::None, Order::Ascending)
        .collect()
}

pub fn load_positions_by_user_addr(storage: &dyn Storage, user_addr: &Addr) -> StdResult<Vec<Position>> {
    KEY_POSITIONS
        .prefix(user_addr)
        .range(&storage, Bound::None, Bound::None, Order::Ascending)
        .collect()
}

pub fn save_position(storage: &mut dyn Storage, position: &Position) -> StdResult<()> {
    KEY_POSITIONS.save(storage, (&position.user_addr, position.cdp_idx[..]), position)
}

pub fn remove_position(storage: &mut dyn Storage, user_addr: &Addr, cdp_idx: &str) {
    KEY_POSITIONS.remove(storage: &mut dyn Storage, (user_addr, cdp_idx))
}





