use std::convert::TryFrom;

use cosmwasm_std::StdError;

pub mod state;
mod anchor;
mod mirror;
mod terraswap;
mod contract;
mod commands;
mod utils;

pub enum SubmsgIds {
    //Deposit
    OpenCDP,
    DepositToCDP,
    MintAsset,
    SellAsset,
    DepositOnReply,
    ExitOnDeposit,
    //Withdraw
    RedeemStable,
    BuyAsset,
    BurnAsset,
    CloseOnReply,
    ExitOnClosure,
}

impl TryFrom<u64> for SubmsgIds {
    type Error = StdError;

    fn try_from(v: u64) -> Result<Self, Self::Error> {
        match v {
            x if x == SubmsgIds::OpenCDP.id() => Ok(SubmsgIds::OpenCDP),
            x if x == SubmsgIds::DepositToCDP.id() => Ok(SubmsgIds::DepositToCDP),
            x if x == SubmsgIds::MintAsset.id() => Ok(SubmsgIds::MintAsset),
            x if x == SubmsgIds::SellAsset.id() => Ok(SubmsgIds::SellAsset),
            x if x == SubmsgIds::DepositOnReply.id() => Ok(SubmsgIds::DepositOnReply),
            x if x == SubmsgIds::ExitOnDeposit.id() => Ok(SubmsgIds::ExitOnDeposit),
            x if x == SubmsgIds::RedeemStable.id() => Ok(SubmsgIds::RedeemStable),
            x if x == SubmsgIds::BuyAsset.id() => Ok(SubmsgIds::BuyAsset),
            x if x == SubmsgIds::BurnAsset.id() => Ok(SubmsgIds::BurnAsset),
            x if x == SubmsgIds::CloseOnReply.id() => Ok(SubmsgIds::CloseOnReply),
            x if x == SubmsgIds::ExitOnClosure.id() => Ok(SubmsgIds::ExitOnClosure),
            unknown => Err(StdError::generic_err(format!(
                "unknown reply message id: {}",
                unknown
            ))),
        }
    }
}

impl SubmsgIds {
    pub const fn id(&self) -> u64 {
        match self {
            SubmsgIds::OpenCDP => 0,
            SubmsgIds::DepositToCDP => 1,
            SubmsgIds::MintAsset => 2,
            SubmsgIds::SellAsset => 3,
            SubmsgIds::DepositOnReply => 4,
            SubmsgIds::ExitOnDeposit => 5,
            SubmsgIds::RedeemStable => 6,
            SubmsgIds::BuyAsset => 7,
            SubmsgIds::BurnAsset => 8,
            SubmsgIds::CloseOnReply => 9,
            SubmsgIds::ExitOnClosure => 10,
        }
    }
}

#[inline]
fn concat(namespace: &[u8], key: &[u8]) -> Vec<u8> {
    let mut k = namespace.to_vec();
    k.extend_from_slice(key);
    k
}