/// A slot in the Decoded Picture Buffer.
pub struct DpbSlot<T> {
    pub resource: T,
    pub poc: i32,
    pub in_use: bool,
}

/// Manages Decoded Picture Buffer slots for HW video decoders.
/// Used by D3D12 Video and Vulkan Video backends.
pub struct DpbManager<T> {
    slots: Vec<DpbSlot<T>>,
}

impl<T> DpbManager<T> {
    pub fn new(max_slots: usize, init: impl Fn(usize) -> T) -> Self {
        let slots = (0..max_slots)
            .map(|i| DpbSlot {
                resource: init(i),
                poc: -1,
                in_use: false,
            })
            .collect();
        Self { slots }
    }

    /// Allocate a slot for the given POC. Returns the slot index, or None if full.
    pub fn allocate(&mut self, poc: i32) -> Option<usize> {
        // First try to find a free slot
        if let Some(idx) = self.slots.iter().position(|s| !s.in_use) {
            self.slots[idx].poc = poc;
            self.slots[idx].in_use = true;
            return Some(idx);
        }
        // If full, evict the slot with lowest POC
        if let Some(idx) = self
            .slots
            .iter()
            .enumerate()
            .min_by_key(|(_, s)| s.poc)
            .map(|(i, _)| i)
        {
            self.slots[idx].poc = poc;
            self.slots[idx].in_use = true;
            return Some(idx);
        }
        None
    }

    /// Release the slot with the given POC.
    pub fn release(&mut self, poc: i32) {
        if let Some(slot) = self.slots.iter_mut().find(|s| s.poc == poc) {
            slot.in_use = false;
        }
    }

    /// Get references for the given POC list.
    pub fn get_reference_indices(&self, ref_pocs: &[i32]) -> Vec<usize> {
        ref_pocs
            .iter()
            .filter_map(|&poc| self.slots.iter().position(|s| s.poc == poc && s.in_use))
            .collect()
    }

    /// Get a reference to a slot by index.
    pub fn slot(&self, index: usize) -> Option<&DpbSlot<T>> {
        self.slots.get(index)
    }

    /// Reset all slots (e.g., after seek).
    pub fn reset(&mut self) {
        for slot in &mut self.slots {
            slot.in_use = false;
            slot.poc = -1;
        }
    }

    /// Number of slots.
    pub fn len(&self) -> usize {
        self.slots.len()
    }

    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allocate_and_release_cycle() {
        let mut dpb: DpbManager<String> = DpbManager::new(2, |i| format!("tex_{i}"));
        assert_eq!(dpb.len(), 2);

        let idx0 = dpb.allocate(0).unwrap();
        assert!(dpb.slot(idx0).unwrap().in_use);
        assert_eq!(dpb.slot(idx0).unwrap().poc, 0);

        let idx1 = dpb.allocate(1).unwrap();
        assert_ne!(idx0, idx1);

        // Release slot 0 and reallocate
        dpb.release(0);
        assert!(!dpb.slot(idx0).unwrap().in_use);

        let idx2 = dpb.allocate(2).unwrap();
        assert_eq!(idx2, idx0); // reuses freed slot
        assert_eq!(dpb.slot(idx2).unwrap().poc, 2);
    }

    #[test]
    fn eviction_when_full() {
        let mut dpb: DpbManager<u32> = DpbManager::new(2, |i| i as u32);

        dpb.allocate(10).unwrap();
        dpb.allocate(20).unwrap();

        // Both slots in use, allocate should evict lowest POC (10)
        let idx = dpb.allocate(30).unwrap();
        assert_eq!(dpb.slot(idx).unwrap().poc, 30);
        // The evicted slot should have been the one with poc=10
        // Verify poc=20 is still present
        let refs = dpb.get_reference_indices(&[20]);
        assert_eq!(refs.len(), 1);
        let refs_10 = dpb.get_reference_indices(&[10]);
        assert!(refs_10.is_empty());
    }

    #[test]
    fn get_reference_indices_valid_and_invalid() {
        let mut dpb: DpbManager<u32> = DpbManager::new(4, |i| i as u32);
        dpb.allocate(0).unwrap();
        dpb.allocate(1).unwrap();
        dpb.allocate(2).unwrap();

        let refs = dpb.get_reference_indices(&[0, 2, 99]);
        assert_eq!(refs.len(), 2); // 99 not found
    }

    #[test]
    fn reset_clears_all_slots() {
        let mut dpb: DpbManager<u32> = DpbManager::new(3, |i| i as u32);
        dpb.allocate(0).unwrap();
        dpb.allocate(1).unwrap();
        dpb.allocate(2).unwrap();

        dpb.reset();
        for i in 0..dpb.len() {
            let slot = dpb.slot(i).unwrap();
            assert!(!slot.in_use);
            assert_eq!(slot.poc, -1);
        }
    }

    #[test]
    fn empty_dpb() {
        let dpb: DpbManager<u32> = DpbManager::new(0, |i| i as u32);
        assert!(dpb.is_empty());
        assert_eq!(dpb.len(), 0);
    }

    #[test]
    fn non_empty_dpb() {
        let dpb: DpbManager<u32> = DpbManager::new(3, |i| i as u32);
        assert!(!dpb.is_empty());
    }

    #[test]
    fn release_nonexistent_poc_is_noop() {
        let mut dpb: DpbManager<u32> = DpbManager::new(2, |i| i as u32);
        dpb.allocate(0).unwrap();
        dpb.release(999); // should not panic or change anything
        assert!(dpb.slot(0).unwrap().in_use);
    }
}
