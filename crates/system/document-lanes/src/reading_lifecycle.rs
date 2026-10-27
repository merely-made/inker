/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

//! Portable lifetime and asynchronous-result correlation for retained reading
//! accesses. This module owns identity decisions; hosts still perform all
//! rendering, storage, browser, and media effects.

use std::collections::BTreeMap;
use uuid::Uuid;

/// The product owner of a set of retained reading accesses.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReadingOwner {
    pub session: String,
    pub persona: String,
}

/// Lightweight facts projected by the host from its current graph/workbench.
/// `membership` must change when an access is removed and recreated, including
/// recreation under the same member UUID. `source` is the current address.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccessProjection<L> {
    pub access: Uuid,
    pub membership: L,
    pub source: String,
    pub visible: bool,
}

/// Host-independent differences produced by reconciling one owner snapshot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AccessChange {
    OwnerChanged,
    Added(Uuid),
    Retired(Uuid),
    SourceChanged(Uuid),
    VisibilityChanged { access: Uuid, visible: bool },
}

/// The finite asynchronous operations whose results affect a reading access.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum OperationKind {
    ClipRead,
    ClipExport,
    MediaPlayback,
}

/// Opaque correlation token for one asynchronous operation on one access.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OperationTicket {
    owner_epoch: u64,
    access: Uuid,
    access_epoch: u64,
    operation: OperationKind,
    operation_epoch: u64,
}

#[derive(Clone, Debug)]
struct AccessState<L> {
    membership: L,
    source: String,
    visible: bool,
    epoch: u64,
    operations: BTreeMap<OperationKind, u64>,
}

/// Runtime access identity and operation correlation for a retained reader.
///
/// This deliberately owns no document bytes, graph/session authority, clip
/// access, or host resources. It can be used by native and browser hosts alike.
pub struct ReadingLifecycle<L> {
    owner: Option<ReadingOwner>,
    owner_epoch: u64,
    accesses: BTreeMap<Uuid, AccessState<L>>,
}

impl<L> Default for ReadingLifecycle<L> {
    fn default() -> Self {
        Self {
            owner: None,
            owner_epoch: 0,
            accesses: BTreeMap::new(),
        }
    }
}

impl<L: Clone + PartialEq> ReadingLifecycle<L> {
    /// Reconcile an owner and current retained/visible access projection.
    /// Repeating an unchanged projection is idempotent. Membership or source
    /// changes retire the old operation identity and issue a new access epoch.
    pub fn reconcile(
        &mut self,
        owner: ReadingOwner,
        projection: impl IntoIterator<Item = AccessProjection<L>>,
    ) -> Vec<AccessChange> {
        let mut changes = Vec::new();
        if self.owner.as_ref() != Some(&owner) {
            self.owner = Some(owner);
            self.owner_epoch = fresh_epoch();
            self.accesses.clear();
            changes.push(AccessChange::OwnerChanged);
        }

        let incoming: BTreeMap<_, _> = projection
            .into_iter()
            .map(|access| (access.access, access))
            .collect();
        let retired: Vec<_> = self
            .accesses
            .keys()
            .filter(|access| !incoming.contains_key(access))
            .copied()
            .collect();
        for access in retired {
            self.accesses.remove(&access);
            changes.push(AccessChange::Retired(access));
        }

        for (id, projected) in incoming {
            match self.accesses.get_mut(&id) {
                Some(state)
                    if state.membership == projected.membership
                        && state.source == projected.source =>
                {
                    if state.visible != projected.visible {
                        state.visible = projected.visible;
                        changes.push(AccessChange::VisibilityChanged {
                            access: id,
                            visible: projected.visible,
                        });
                    }
                },
                Some(state) => {
                    let source_changed = state.source != projected.source;
                    state.membership = projected.membership;
                    state.source = projected.source;
                    state.visible = projected.visible;
                    state.epoch = fresh_epoch();
                    state.operations.clear();
                    changes.push(if source_changed {
                        AccessChange::SourceChanged(id)
                    } else {
                        AccessChange::Retired(id)
                    });
                },
                None => {
                    let epoch = fresh_epoch();
                    self.accesses.insert(
                        id,
                        AccessState {
                            membership: projected.membership,
                            source: projected.source,
                            visible: projected.visible,
                            epoch,
                            operations: BTreeMap::new(),
                        },
                    );
                    changes.push(AccessChange::Added(id));
                },
            }
        }
        changes
    }

    /// Invalidate an access whose held content changed while graph membership
    /// and address remained stable. Call only after the host accepts a new
    /// held source and is about to rebind its content session.
    pub fn rebind(&mut self, access: Uuid) -> bool {
        let Some(state) = self.accesses.get_mut(&access) else {
            return false;
        };
        state.epoch = fresh_epoch();
        state.operations.clear();
        true
    }

    /// Start or supersede one named operation for the current access.
    pub fn begin_operation(
        &mut self,
        access: Uuid,
        operation: OperationKind,
    ) -> Option<OperationTicket> {
        let state = self.accesses.get_mut(&access)?;
        let operation_epoch = fresh_epoch();
        state.operations.insert(operation.clone(), operation_epoch);
        Some(OperationTicket {
            owner_epoch: self.owner_epoch,
            access,
            access_epoch: state.epoch,
            operation,
            operation_epoch,
        })
    }

    /// Whether a completion still belongs to the current owner, access source,
    /// and latest operation of its kind.
    pub fn accepts(&self, ticket: &OperationTicket) -> bool {
        self.accesses.get(&ticket.access).is_some_and(|state| {
            self.owner_epoch == ticket.owner_epoch
                && state.epoch == ticket.access_epoch
                && state.operations.get(&ticket.operation) == Some(&ticket.operation_epoch)
        })
    }

    /// Consume a current completion so it cannot apply twice. Use `accepts`
    /// for a precheck when more domain-specific scope checks must follow, then
    /// call this immediately before committing the result's host effect.
    pub fn complete(&mut self, ticket: &OperationTicket) -> bool {
        if !self.accepts(ticket) {
            return false;
        }
        self.accesses
            .get_mut(&ticket.access)
            .and_then(|state| state.operations.remove(&ticket.operation))
            == Some(ticket.operation_epoch)
    }

    pub fn contains(&self, access: Uuid) -> bool {
        self.accesses.contains_key(&access)
    }

    pub fn is_visible(&self, access: Uuid) -> bool {
        self.accesses
            .get(&access)
            .is_some_and(|state| state.visible)
    }
}

fn fresh_epoch() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(1);
    let mut current = NEXT.load(Ordering::Relaxed);
    loop {
        let next = current
            .checked_add(1)
            .expect("reading lifecycle epoch space exhausted");
        match NEXT.compare_exchange_weak(current, next, Ordering::Relaxed, Ordering::Relaxed) {
            Ok(_) => return current,
            Err(observed) => current = observed,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn owner(session: &str, persona: &str) -> ReadingOwner {
        ReadingOwner {
            session: session.into(),
            persona: persona.into(),
        }
    }
    fn access(id: u128, membership: u64, source: &str, visible: bool) -> AccessProjection<u64> {
        AccessProjection {
            access: Uuid::from_u128(id),
            membership,
            source: source.into(),
            visible,
        }
    }

    #[test]
    fn owner_access_and_source_changes_revoke_prior_tickets() {
        let id = Uuid::from_u128(1);
        let mut lifecycle = ReadingLifecycle::default();
        lifecycle.reconcile(owner("g1", "alice"), [access(1, 4, "a", true)]);
        let original = lifecycle
            .begin_operation(id, OperationKind::ClipRead)
            .unwrap();
        assert!(lifecycle.accepts(&original));

        lifecycle.reconcile(owner("g2", "alice"), [access(1, 4, "a", true)]);
        assert!(!lifecycle.accepts(&original));
        let next = lifecycle
            .begin_operation(id, OperationKind::ClipRead)
            .unwrap();
        lifecycle.reconcile(owner("g2", "bob"), [access(1, 4, "a", true)]);
        assert!(!lifecycle.accepts(&next));

        lifecycle.reconcile(owner("g2", "bob"), [access(1, 4, "b", true)]);
        let rebound = lifecycle
            .begin_operation(id, OperationKind::ClipRead)
            .unwrap();
        lifecycle.reconcile(owner("g2", "bob"), [access(1, 5, "b", true)]);
        assert!(!lifecycle.accepts(&rebound));
    }

    #[test]
    fn removal_visibility_and_reconciliation_are_explicit_and_idempotent() {
        let id = Uuid::from_u128(2);
        let mut lifecycle = ReadingLifecycle::default();
        assert_eq!(
            lifecycle.reconcile(owner("g", "a"), [access(2, 1, "a", true)]),
            [AccessChange::OwnerChanged, AccessChange::Added(id)]
        );
        assert!(
            lifecycle
                .reconcile(owner("g", "a"), [access(2, 1, "a", true)])
                .is_empty()
        );
        let ticket = lifecycle
            .begin_operation(id, OperationKind::MediaPlayback)
            .unwrap();
        assert_eq!(
            lifecycle.reconcile(owner("g", "a"), [access(2, 1, "a", false)]),
            [AccessChange::VisibilityChanged {
                access: id,
                visible: false
            }]
        );
        assert!(!lifecycle.is_visible(id));
        assert!(
            lifecycle.accepts(&ticket),
            "hiding does not itself rebind an access"
        );
        assert_eq!(
            lifecycle.reconcile(owner("g", "a"), []),
            [AccessChange::Retired(id)]
        );
        assert!(!lifecycle.accepts(&ticket));
    }

    #[test]
    fn operation_supersession_is_scoped_by_kind() {
        let id = Uuid::from_u128(3);
        let mut lifecycle = ReadingLifecycle::default();
        lifecycle.reconcile(owner("g", "a"), [access(3, 1, "a", true)]);
        let first = lifecycle
            .begin_operation(id, OperationKind::MediaPlayback)
            .unwrap();
        let other = lifecycle
            .begin_operation(id, OperationKind::ClipExport)
            .unwrap();
        let latest = lifecycle
            .begin_operation(id, OperationKind::MediaPlayback)
            .unwrap();
        assert!(!lifecycle.accepts(&first));
        assert!(lifecycle.accepts(&other));
        assert!(lifecycle.accepts(&latest));
    }

    #[test]
    fn same_address_held_content_rebind_revokes_pending_work() {
        let id = Uuid::from_u128(4);
        let mut lifecycle = ReadingLifecycle::default();
        lifecycle.reconcile(owner("g", "a"), [access(4, 1, "a", true)]);
        let old = lifecycle
            .begin_operation(id, OperationKind::MediaPlayback)
            .unwrap();
        assert!(lifecycle.rebind(id));
        assert!(!lifecycle.accepts(&old));
        let current = lifecycle
            .begin_operation(id, OperationKind::MediaPlayback)
            .unwrap();
        assert!(lifecycle.accepts(&current));
        assert!(lifecycle.rebind(id));
    }

    #[test]
    fn tickets_are_instance_scoped_and_completion_is_one_shot() {
        let id = Uuid::from_u128(5);
        let projection = [access(5, 1, "a", true)];
        let mut first = ReadingLifecycle::default();
        let mut second = ReadingLifecycle::default();
        first.reconcile(owner("g", "a"), projection.clone());
        second.reconcile(owner("g", "a"), projection);
        let ticket = first
            .begin_operation(id, OperationKind::ClipExport)
            .unwrap();
        assert!(!second.accepts(&ticket));
        assert!(first.complete(&ticket));
        assert!(!first.accepts(&ticket));
        assert!(!first.complete(&ticket));
    }
}
