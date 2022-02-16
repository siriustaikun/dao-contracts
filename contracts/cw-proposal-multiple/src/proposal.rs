use std::{convert::TryInto, ops::Sub};

use cosmwasm_std::{Addr, BlockInfo, Uint128};
use cw_utils::Expiration;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use voting::{
    deposit::CheckedDepositInfo,
    proposal::{Proposal, Status},
    threshold::{PercentageThreshold, Threshold},
    voting::{
        compare_vote_count, does_vote_count_fail, does_vote_count_pass, MultipleChoiceVote,
        MultipleChoiceVotes, VoteCmp,
    },
};

use crate::{
    query::ProposalResponse,
    state::{MultipleChoiceOption, MultipleChoiceOptionType},
    voting_strategy::VotingStrategy,
};

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
pub struct MultipleChoiceProposal {
    pub title: String,
    pub description: String,
    pub proposer: Addr,
    pub start_height: u64,
    pub expiration: Expiration,
    pub choices: Vec<MultipleChoiceOption>,
    pub status: Status,

    pub voting_strategy: VotingStrategy,
    /// The total power when the proposal started (used to calculate percentages)
    pub total_power: Uint128,

    pub votes: MultipleChoiceVotes,
    /// Information about the deposit that was sent as part of this
    /// proposal. None if no deposit.
    pub deposit_info: Option<CheckedDepositInfo>,
}

impl Proposal for MultipleChoiceProposal {
    fn proposer(&self) -> Addr {
        return self.proposer.clone();
    }
    fn deposit_info(&self) -> Option<CheckedDepositInfo> {
        return self.deposit_info.clone();
    }
    fn status(&self) -> Status {
        return self.status;
    }
}

impl MultipleChoiceProposal {
    /// Consumes the proposal and returns a version which may be used
    /// in a query response. The difference being that proposal
    /// statuses are only updated on vote, execute, and close
    /// events. It is possible though that since a vote has occured
    /// the proposal expiring has changed its status. This method
    /// recomputes the status so that queries get accurate
    /// information.
    pub fn into_response(mut self, block: &BlockInfo, id: u64) -> ProposalResponse {
        self.update_status(block);
        ProposalResponse { id, proposal: self }
    }

    /// Gets the current status of the proposal.
    pub fn current_status(&self, block: &BlockInfo) -> Status {
        if self.status == Status::Open && self.is_passed() {
            Status::Passed
        } else if self.status == Status::Open
            && (self.expiration.is_expired(block) || self.is_rejected(block))
        {
            Status::Rejected
        } else {
            self.status
        }
    }

    /// Sets a proposals status to its current status.
    pub fn update_status(&mut self, block: &BlockInfo) {
        self.status = self.current_status(block);
    }

    /// Returns true iff this proposal is sure to pass (even before
    /// expiration if no future sequence of possible votes can cause
    /// it to fail). Passing in the case of multiple choice proposals
    /// means that one of the options that is not "None of the above" has won the most votes.
    pub fn is_passed(&self) -> bool {
        // Proposal can only pass if quorum has been met.
        if does_vote_count_pass(
            self.votes.total(),
            self.total_power,
            self.voting_strategy.get_quorum(),
        ) {
            let (choice_idx, winning_choice) = self.calculate_winning_choice();
            // Check that the winning choice is not None.
            if winning_choice.option_type != MultipleChoiceOptionType::None {
                // If the leading choice cannot possibly be outwon by the "None" option, the proposal has passed.
                // This means that the difference between them must be greater than the remaining vote power.
                let none_count = u128::from(self.get_none_vote_count());
                let winning_choice_count = self.votes.vote_weights[choice_idx].u128();
                let remaining_power = self.total_power.sub(self.votes.total()).u128();
                if winning_choice_count - none_count > remaining_power {
                    return true;
                }
            }
        }

        return false;
    }

    pub fn is_rejected(&self, block: &BlockInfo) -> bool {
        match (
            does_vote_count_pass(
                self.votes.total(),
                self.total_power,
                self.voting_strategy.get_quorum(),
            ),
            self.expiration.is_expired(block),
        ) {
            // Quorum is met and proposal is expired.
            (true, true) => {
                // Proposal is rejected if "None" is the winning option.
                let (_, winning_choice) = self.calculate_winning_choice();
                if winning_choice.option_type == MultipleChoiceOptionType::None {
                    return true;
                }

                // Proposal is rejected if all of the options are tied.
                if let Some(first) = self.votes.vote_weights.first() {
                    return self.votes.vote_weights.iter().all(|x| *x == *first);
                }
                return false;
            }
            // Quorum is met and proposal has not expired OR Quorum is not met and proposal is not expired.
            (true, false) | (false, false) => {
                // Proposal is rejected if "None" has the majority of the total power because there is no way for
                // another option to outnumber it.
                let none_vote_count = Uint128::from(self.get_none_vote_count());
                return does_vote_count_fail(
                    none_vote_count,
                    self.total_power,
                    PercentageThreshold::Majority {},
                );
            }
            // Quorum is not met and proposal is expired.
            (false, true) => return true,
        }
    }

    pub fn calculate_winning_choice(&self) -> (usize, &MultipleChoiceOption) {
        match self.voting_strategy {
            VotingStrategy::SingleChoice { quorum: _ } => {
                let choice_idx = self
                    .votes
                    .vote_weights
                    .iter()
                    .enumerate()
                    .max_by(|a, b| a.cmp(b))
                    .map(|(idx, _)| idx)
                    .unwrap();

                return (choice_idx, &self.choices[choice_idx]);
            }
            VotingStrategy::RankedChoice { quorum: _ } => todo!(),
        }
    }

    pub fn get_none_vote_count(&self) -> u64 {
        return self
            .votes
            .vote_weights
            .iter()
            .enumerate()
            .filter(|(idx, x)| self.choices[*idx].option_type == MultipleChoiceOptionType::None)
            .count() as u64;
    }
}
