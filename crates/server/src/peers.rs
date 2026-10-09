/*!
 * Connections held by each peer address, so that one machine cannot take
 * every connection slot of the server.
 */

use std::{
    collections::HashMap,
    net::IpAddr,
    num::NonZeroUsize,
    sync::{Arc, Mutex, MutexGuard, PoisonError},
};

/// Open connections per IP address, against an optional limit.
#[derive(Debug)]
pub(crate) struct PeerSlots {
    limit: Option<NonZeroUsize>,
    open: Mutex<HashMap<IpAddr, usize>>,
}

/// One connection slot of an address, given back when dropped.
#[derive(Debug)]
pub(crate) struct PeerSlot {
    slots: Arc<PeerSlots>,
    address: IpAddr,
}

impl PeerSlots {
    pub(crate) fn new(limit: Option<NonZeroUsize>) -> Self {
        Self {
            limit,
            open: Mutex::new(HashMap::new()),
        }
    }

    /// Takes a slot for `address`, unless it already holds as many as allowed.
    pub(crate) fn claim(self: &Arc<Self>, address: IpAddr) -> Option<PeerSlot> {
        let address = address.to_canonical();
        let mut open = self.lock();
        let count = open.entry(address).or_default();
        if self.limit.is_some_and(|limit| *count >= limit.get()) {
            return None;
        }
        *count += 1;
        Some(PeerSlot {
            slots: Arc::clone(self),
            address,
        })
    }

    fn release(&self, address: IpAddr) {
        let mut open = self.lock();
        if let Some(count) = open.get_mut(&address) {
            *count = count.saturating_sub(1);
            if *count == 0 {
                open.remove(&address);
            }
        }
    }

    /**
     * The counts stay consistent even if a thread panicked while holding
     * the lock: every update is a single step.
     */
    fn lock(&self) -> MutexGuard<'_, HashMap<IpAddr, usize>> {
        self.open.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl Drop for PeerSlot {
    fn drop(&mut self) {
        self.slots.release(self.address);
    }
}

#[cfg(test)]
mod tests {
    use std::net::{Ipv4Addr, Ipv6Addr};

    use super::*;

    const ALICE: IpAddr = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 10));
    const BOB: IpAddr = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 11));

    fn limited(limit: usize) -> Arc<PeerSlots> {
        Arc::new(PeerSlots::new(NonZeroUsize::new(limit)))
    }

    #[test]
    fn an_address_holds_at_most_its_limit_without_affecting_others() {
        let slots = limited(2);
        let held: Vec<PeerSlot> = (0..2).filter_map(|_| slots.claim(ALICE)).collect();
        assert_eq!(held.len(), 2);
        assert!(slots.claim(ALICE).is_none());
        assert!(slots.claim(BOB).is_some());
    }

    #[test]
    fn closing_a_connection_frees_its_slot() {
        let slots = limited(1);
        let first = slots.claim(ALICE);
        assert!(slots.claim(ALICE).is_none());
        drop(first);
        let second = slots.claim(ALICE);
        assert!(second.is_some());
        drop(second);
        assert!(
            slots.lock().is_empty(),
            "forgotten addresses take no memory"
        );
    }

    #[test]
    fn ipv4_peers_count_alike_over_ipv6() {
        let slots = limited(1);
        let _held = slots.claim(ALICE);
        let mapped = IpAddr::V6(Ipv6Addr::new(0, 0, 0, 0, 0, 0xffff, 0xc0a8, 0x010a));
        assert!(slots.claim(mapped).is_none());
    }

    #[test]
    fn without_a_limit_everyone_is_admitted() {
        let slots = Arc::new(PeerSlots::new(None));
        let held: Vec<PeerSlot> = (0..1_000).filter_map(|_| slots.claim(ALICE)).collect();
        assert_eq!(held.len(), 1_000);
    }
}
