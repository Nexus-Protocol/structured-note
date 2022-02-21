use cosmwasm_bignumber::Decimal256;
use cosmwasm_std::{Addr, Decimal, Order, StdResult, Storage, Uint128};
use cw_storage_plus::{Item, Map};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

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

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
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
        .map(|cdp| {
            let (_, v) = cdp?;
            Ok(CDP {
                idx: v.idx,
                masset_token: v.masset_token,
                farmers: v.farmers,
            })
        })
        .collect()
}

pub fn save_cdp(storage: &mut dyn Storage, cdp: &CDP) -> StdResult<()> {
    KEY_CDPS.save(storage, &cdp.masset_token, cdp)
}

pub fn remove_cdp(storage: &mut dyn Storage, masset_token: &Addr) {
    KEY_CDPS.remove(storage, masset_token)
}

pub fn update_cdp(storage: &mut dyn Storage, depositing_state: &DepositingState) -> StdResult<CDP> {
    let action = |cdp: Option<CDP>| -> StdResult<CDP> {
        match cdp {
            None => Ok(
                CDP {
                    idx: depositing_state.cdp_idx,
                    masset_token: depositing_state.masset_token.clone(),
                    farmers: vec![depositing_state.farmer_addr.clone()],
                }
            ),
            Some(mut cdp) => {
                if !cdp.farmers.contains(&depositing_state.farmer_addr) {
                    cdp.farmers.push(depositing_state.farmer_addr.clone());
                }
                Ok(cdp)
            }
        }
    };
    KEY_CDPS.update(storage, &depositing_state.masset_token, action)
}

pub fn load_depositing_state(storage: &dyn Storage) -> StdResult<DepositingState> {
    KEY_DEPOSITING.load(storage)
}

pub fn store_depositing_state(storage: &mut dyn Storage, data: &DepositingState) -> StdResult<()> {
    KEY_DEPOSITING.save(storage, data)
}

pub fn increment_iteration_index(storage: &mut dyn Storage) -> StdResult<DepositingState> {
    KEY_DEPOSITING.update(storage, |mut ds: DepositingState| -> StdResult<DepositingState> {
        ds.cur_iteration_index += 1;
        Ok(ds)
    })
}

pub fn load_position(storage: &dyn Storage, farmer_addr: &Addr, masset_token: &Addr) -> StdResult<Position> {
    KEY_POSITIONS.load(storage, (farmer_addr, masset_token))
}

pub fn upsert_position(storage: &mut dyn Storage,
                       depositing_state: &DepositingState,
                       loan_diff: Uint128,
                       collateral_diff: Uint128,
) -> StdResult<Position> {
    let action = |p: Option<Position>| -> StdResult<Position> {
        match p {
            None => Ok(
                Position {
                    farmer_addr: depositing_state.farmer_addr.clone(),
                    masset_token: depositing_state.masset_token.clone(),
                    cdp_idx: depositing_state.cdp_idx,
                    leverage_iter_amount: depositing_state.max_iteration_index,
                    total_loan_amount: loan_diff,
                    total_collateral_amount: collateral_diff,
                }
            ),
            Some(mut position) => {
                position.total_collateral_amount += collateral_diff;
                position.total_loan_amount += loan_diff;
                Ok(position)
            }
        }
    };
    KEY_POSITIONS.update(storage, (&depositing_state.farmer_addr, &depositing_state.masset_token), action)
}

pub fn load_all_positions(storage: &dyn Storage) -> StdResult<Vec<Position>> {
    KEY_POSITIONS
        .range(storage, None, None, Order::Ascending)
        .map(|position| {
            let (_, v) = position?;
            Ok(Position {
                farmer_addr: v.farmer_addr,
                masset_token: v.masset_token,
                cdp_idx: v.cdp_idx,
                leverage_iter_amount: v.leverage_iter_amount,
                total_loan_amount: v.total_loan_amount,
                total_collateral_amount: v.total_collateral_amount,
            })
        })
        .collect()
}

pub fn load_positions_by_farmer_addr(storage: &dyn Storage, farmer_addr: &Addr) -> StdResult<Vec<Position>> {
    KEY_POSITIONS
        .prefix(farmer_addr)
        .range(storage, None, None, Order::Ascending)
        .map(|position| {
            let (_, v) = position?;
            Ok(Position {
                farmer_addr: v.farmer_addr,
                masset_token: v.masset_token,
                cdp_idx: v.cdp_idx,
                leverage_iter_amount: v.leverage_iter_amount,
                total_loan_amount: v.total_loan_amount,
                total_collateral_amount: v.total_collateral_amount,
            })
        })
        .collect()
}

pub fn save_position(storage: &mut dyn Storage, position: &Position) -> StdResult<()> {
    KEY_POSITIONS.save(storage, (&position.farmer_addr, &position.masset_token), position)
}

pub fn remove_position(storage: &mut dyn Storage, farmer_addr: &Addr, masset_token: &Addr) {
    KEY_POSITIONS.remove(storage, (farmer_addr, masset_token))
}

impl DepositingState {
    pub fn template(farmer_addr: Addr, masset_token: Addr, aim_collateral_ratio: Option<Decimal>, leverage_iter_amount: Option<u8>, mirror_ts_factory_addr: Addr) -> DepositingState {
        DepositingState {
            cdp_idx: Default::default(),
            farmer_addr,
            masset_token,
            aim_collateral_ratio: aim_collateral_ratio.unwrap_or_default(),
            max_iteration_index: leverage_iter_amount.unwrap_or_default(),
            cur_iteration_index: 0,
            initial_cdp_collateral_amount: Uint128::zero(),
            initial_cdp_loan_amount: Uint128::zero(),
            asset_price_in_collateral_asset: Decimal::zero(),
            mirror_ts_factory_addr,
        }
    }
}