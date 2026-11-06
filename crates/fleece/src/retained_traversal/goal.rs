// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! What drives a retained traversal: selection among pending candidates and
//! assessment of each retained text. Eidetic's lexical `GoalAssessor` (S1) is
//! the ordinary driver; `ModelGoalAssessor` (S3, S1 plus an embedder) is the
//! same four calls with a model beside S1, so one bounded traversal serves
//! both and the model's record lands in the receipt.

use eidetic::models::recall::{RecallSourceId, RetainedSourceText};
use eidetic_search::goal::{CandidateLabel, GoalAssessor, GoalRecord, GoalVerdict};
use eidetic_search::goal_model::{GoalEmbedder, ModelGoalAssessor, ModelGoalRecord};
use serde::{Deserialize, Serialize};

/// A goal driver for [`run_retained_traversal_with`](super::run_retained_traversal_with).
pub trait RetainedGoal {
    /// Longest retained text (body plus title, in characters) it will assess.
    fn max_text_chars(&self) -> usize;
    /// Index of the pending candidate to look up next; `None` stops the walk.
    fn select(&mut self, candidates: &[CandidateLabel]) -> Option<usize>;
    /// Judge one observed text.
    ///
    /// # Errors
    /// The refusal reason (oversized text).
    fn assess(&mut self, retained: &RetainedSourceText) -> Result<GoalVerdict, String>;
    /// The lexical (S1) record. A model driver keeps S1's record unchanged.
    fn record(&self) -> GoalRecord;
    /// The model record, for a driver that has a model beside S1.
    fn model_record(&self) -> Option<ModelGoalRecord> {
        None
    }
}

impl RetainedGoal for GoalAssessor {
    fn max_text_chars(&self) -> usize {
        self.settings().max_text_chars
    }
    fn select(&mut self, candidates: &[CandidateLabel]) -> Option<usize> {
        GoalAssessor::select(self, candidates)
    }
    fn assess(&mut self, retained: &RetainedSourceText) -> Result<GoalVerdict, String> {
        GoalAssessor::assess(self, retained)
    }
    fn record(&self) -> GoalRecord {
        GoalAssessor::record(self)
    }
}

impl<E: GoalEmbedder> RetainedGoal for ModelGoalAssessor<E> {
    fn max_text_chars(&self) -> usize {
        self.s1().settings().max_text_chars
    }
    fn select(&mut self, candidates: &[CandidateLabel]) -> Option<usize> {
        ModelGoalAssessor::select(self, candidates)
    }
    fn assess(&mut self, retained: &RetainedSourceText) -> Result<GoalVerdict, String> {
        ModelGoalAssessor::assess(self, retained)
    }
    fn record(&self) -> GoalRecord {
        ModelGoalAssessor::record(self).s1
    }
    fn model_record(&self) -> Option<ModelGoalRecord> {
        Some(ModelGoalAssessor::record(self))
    }
}

/// One selection as the traversal saw it: the pending candidates offered and
/// the one taken. A model driver may take a candidate other than S1's pick,
/// and a goal record names only the pick, so this keeps the offer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelectionTrace {
    pub candidates: Vec<RecallSourceId>,
    pub chosen: usize,
}
