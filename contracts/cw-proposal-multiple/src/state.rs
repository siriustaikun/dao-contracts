use std::{convert::TryInto, ops::Mul};

use crate::{proposal::MultipleChoiceProposal, voting_strategy::VotingStrategy};
use cosmwasm_std::{Addr, BlockInfo, CosmosMsg, Empty, StdError, StdResult, Storage, Uint128};
use cw_storage_plus::{Item, Map};
use cw_utils::{Duration, Expiration};
use indexable_hooks::Hooks;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use voting::{
    deposit::CheckedDepositInfo,
    voting::{compare_vote_count, MultipleChoiceVote, MultipleChoiceVotes, VoteCmp},
};

/// The governance module's configuration.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    /// The threshold a proposal must reach to complete.
    pub voting_strategy: VotingStrategy,
    /// The default maximum amount of time a proposal may be voted on
    /// before expiring.
    pub max_voting_period: Duration,
    /// If set to true only members may execute passed
    /// proposals. Otherwise, any address may execute a passed
    /// proposal.
    pub only_members_execute: bool,
    /// The address of the DAO that this governance module is
    /// associated with.
    pub dao: Addr,
    /// Information about the depost required to create a
    /// proposal. None if no deposit is required, Some otherwise.
    pub deposit_info: Option<CheckedDepositInfo>,
}

/// Information about a vote that was cast.
#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
pub struct VoteInfo {
    /// The address that voted.
    pub voter: Addr,
    /// Position on the vote.
    pub vote: MultipleChoiceVote,
    /// The voting power behind the vote.
    pub power: Uint128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub enum MultipleChoiceOptionType {
    /// Choice that represents selecting none of the options; still counts toward quorum
    /// and allows proposals with all bad options to be voted against.
    None,
    Standard,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct MultipleChoiceOption {
    pub option_type: MultipleChoiceOptionType,
    pub description: String,
    pub msgs: Vec<CosmosMsg<Empty>>,
    pub vote_count: Uint128,
}

pub fn parse_id(data: &[u8]) -> StdResult<u64> {
    match data[0..8].try_into() {
        Ok(bytes) => Ok(u64::from_be_bytes(bytes)),
        Err(_) => Err(StdError::generic_err(
            "Corrupted data found. 8 byte expected.",
        )),
    }
}

// we cast a ballot with our chosen vote and a given weight
// stored under the key that voted
#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
pub struct Ballot {
    /// The amount of voting power behind the vote.
    pub power: Uint128,
    /// The position.
    pub vote: MultipleChoiceVote,
}

pub fn next_id(store: &mut dyn Storage) -> StdResult<u64> {
    let id: u64 = PROPOSAL_COUNT.may_load(store)?.unwrap_or_default() + 1;
    PROPOSAL_COUNT.save(store, &id)?;
    Ok(id)
}

pub const CONFIG: Item<Config> = Item::new("config");
pub const PROPOSAL_COUNT: Item<u64> = Item::new("proposal_count");
pub const PROPOSALS: Map<u64, MultipleChoiceProposal> = Map::new("proposals");
pub const BALLOTS: Map<(u64, Addr), Ballot> = Map::new("ballots");
pub const PROPOSAL_HOOKS: Hooks = Hooks::new("proposal_hooks");
pub const VOTE_HOOKS: Hooks = Hooks::new("vote_hooks");
