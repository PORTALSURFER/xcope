//! Shared processor/controller instance pairing for one plugin lifecycle.

use std::sync::{Arc, Mutex, OnceLock, Weak};

use crate::XcopeShared;

#[derive(Copy, Clone)]
pub(super) enum SharedRole {
    Processor,
    Controller,
}

struct SharedRegistryEntry {
    shared: Weak<XcopeShared>,
    processor_claimed: bool,
    controller_claimed: bool,
}

fn shared_registry() -> &'static Mutex<Vec<SharedRegistryEntry>> {
    static REGISTRY: OnceLock<Mutex<Vec<SharedRegistryEntry>>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(Vec::new()))
}

/// Acquire or create one shared runtime for the requested class role.
pub(super) fn acquire_shared_for_role(role: SharedRole) -> Arc<XcopeShared> {
    let mut registry = match shared_registry().lock() {
        Ok(registry) => registry,
        Err(_) => return Arc::new(XcopeShared::new()),
    };
    registry.retain(|entry| entry.shared.upgrade().is_some());

    for entry in registry.iter_mut() {
        let Some(shared) = entry.shared.upgrade() else {
            continue;
        };
        match role {
            SharedRole::Processor if !entry.processor_claimed => {
                entry.processor_claimed = true;
                return shared;
            }
            SharedRole::Controller if !entry.controller_claimed => {
                entry.controller_claimed = true;
                return shared;
            }
            _ => {}
        }
    }

    let shared = Arc::new(XcopeShared::new());
    registry.push(SharedRegistryEntry {
        shared: Arc::downgrade(&shared),
        processor_claimed: matches!(role, SharedRole::Processor),
        controller_claimed: matches!(role, SharedRole::Controller),
    });
    shared
}

/// Release one role-claim when a class instance drops.
pub(super) fn release_shared_for_role(shared: &Arc<XcopeShared>, role: SharedRole) {
    let mut registry = match shared_registry().lock() {
        Ok(registry) => registry,
        Err(_) => return,
    };

    registry.retain(|entry| entry.shared.upgrade().is_some());
    for entry in registry.iter_mut() {
        let Some(candidate) = entry.shared.upgrade() else {
            continue;
        };
        if !Arc::ptr_eq(&candidate, shared) {
            continue;
        }
        match role {
            SharedRole::Processor => entry.processor_claimed = false,
            SharedRole::Controller => entry.controller_claimed = false,
        }
    }
}
