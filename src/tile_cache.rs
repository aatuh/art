//! Deterministic fixed-slot residency cache for streamed Earth texture tiles.

use std::collections::{BTreeMap, BTreeSet};

use crate::planet_tiles::TileAssetRequest;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SlotReservation {
    Resident { slot: u16 },
    Vacant { slot: u16 },
    Evicted {
        slot: u16,
        previous: TileAssetRequest,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Entry {
    slot: u16,
    last_used_tick: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResidentTileCache {
    capacity: u16,
    tick: u64,
    entries: BTreeMap<TileAssetRequest, Entry>,
    slots: BTreeMap<u16, TileAssetRequest>,
    free_slots: BTreeSet<u16>,
}

impl ResidentTileCache {
    pub fn new(capacity: u16) -> Self {
        Self {
            capacity,
            tick: 0,
            entries: BTreeMap::new(),
            slots: BTreeMap::new(),
            free_slots: (0..capacity).collect(),
        }
    }

    pub fn capacity(&self) -> usize {
        usize::from(self.capacity)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn slot_for(&self, request: TileAssetRequest) -> Option<u16> {
        self.entries.get(&request).map(|entry| entry.slot)
    }

    /// Reserves a texture-array/atlas slot for an asset and marks it recently used.
    ///
    /// When full, the least-recently-used asset is evicted. Ties are deterministic:
    /// the lower slot number wins, which keeps tests and cache behavior reproducible.
    pub fn reserve(&mut self, request: TileAssetRequest) -> Option<SlotReservation> {
        self.tick = self.tick.saturating_add(1);
        if let Some(entry) = self.entries.get_mut(&request) {
            entry.last_used_tick = self.tick;
            return Some(SlotReservation::Resident { slot: entry.slot });
        }
        if self.capacity == 0 {
            return None;
        }

        if let Some(slot) = self.free_slots.pop_first() {
            self.insert(request, slot);
            return Some(SlotReservation::Vacant { slot });
        }

        let (previous, entry) = self
            .entries
            .iter()
            .min_by_key(|(_, entry)| (entry.last_used_tick, entry.slot))
            .map(|(request, entry)| (*request, *entry))?;
        self.entries.remove(&previous);
        self.slots.remove(&entry.slot);
        self.insert(request, entry.slot);
        Some(SlotReservation::Evicted {
            slot: entry.slot,
            previous,
        })
    }

    pub fn touch(&mut self, request: TileAssetRequest) -> bool {
        let Some(entry) = self.entries.get_mut(&request) else {
            return false;
        };
        self.tick = self.tick.saturating_add(1);
        entry.last_used_tick = self.tick;
        true
    }

    pub fn remove(&mut self, request: TileAssetRequest) -> Option<u16> {
        let entry = self.entries.remove(&request)?;
        self.slots.remove(&entry.slot);
        self.free_slots.insert(entry.slot);
        Some(entry.slot)
    }

    pub fn resident_requests(&self) -> impl Iterator<Item = TileAssetRequest> + '_ {
        self.entries.keys().copied()
    }

    fn insert(&mut self, request: TileAssetRequest, slot: u16) {
        self.entries.insert(
            request,
            Entry {
                slot,
                last_used_tick: self.tick,
            },
        );
        self.slots.insert(slot, request);
    }
}

#[cfg(test)]
mod tests {
    use super::{ResidentTileCache, SlotReservation};
    use crate::planet_tiles::{CubeFace, TileAssetRequest, TileId, TileLayer};

    fn request(x: u32) -> TileAssetRequest {
        TileAssetRequest {
            layer: TileLayer::Surface,
            tile: TileId::new(CubeFace::PositiveZ, 3, x, 0).expect("valid tile"),
        }
    }

    #[test]
    fn zero_capacity_refuses_reservations() {
        let mut cache = ResidentTileCache::new(0);
        assert_eq!(cache.reserve(request(0)), None);
        assert!(cache.is_empty());
    }

    #[test]
    fn vacant_slots_are_allocated_in_stable_order() {
        let mut cache = ResidentTileCache::new(3);
        assert_eq!(
            cache.reserve(request(0)),
            Some(SlotReservation::Vacant { slot: 0 })
        );
        assert_eq!(
            cache.reserve(request(1)),
            Some(SlotReservation::Vacant { slot: 1 })
        );
        assert_eq!(cache.slot_for(request(1)), Some(1));
    }

    #[test]
    fn reserving_resident_asset_refreshes_without_moving_it() {
        let mut cache = ResidentTileCache::new(2);
        cache.reserve(request(0));
        cache.reserve(request(1));
        assert_eq!(
            cache.reserve(request(0)),
            Some(SlotReservation::Resident { slot: 0 })
        );
        assert_eq!(
            cache.reserve(request(2)),
            Some(SlotReservation::Evicted {
                slot: 1,
                previous: request(1),
            })
        );
    }

    #[test]
    fn least_recently_used_asset_is_evicted() {
        let mut cache = ResidentTileCache::new(2);
        cache.reserve(request(0));
        cache.reserve(request(1));
        cache.touch(request(0));
        assert_eq!(
            cache.reserve(request(2)),
            Some(SlotReservation::Evicted {
                slot: 1,
                previous: request(1),
            })
        );
        assert_eq!(cache.slot_for(request(0)), Some(0));
        assert_eq!(cache.slot_for(request(2)), Some(1));
    }

    #[test]
    fn removal_reuses_the_freed_slot() {
        let mut cache = ResidentTileCache::new(2);
        cache.reserve(request(0));
        cache.reserve(request(1));
        assert_eq!(cache.remove(request(0)), Some(0));
        assert_eq!(
            cache.reserve(request(2)),
            Some(SlotReservation::Vacant { slot: 0 })
        );
    }

    #[test]
    fn resident_iteration_is_unique() {
        let mut cache = ResidentTileCache::new(4);
        cache.reserve(request(2));
        cache.reserve(request(0));
        cache.reserve(request(1));
        let resident = cache.resident_requests().collect::<Vec<_>>();
        assert_eq!(resident.len(), 3);
        assert!(resident.windows(2).all(|pair| pair[0] < pair[1]));
    }
}
