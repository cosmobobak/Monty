use std::sync::atomic::{AtomicI32, Ordering};

use crate::{chess::Board, mcts::MctsParams};

pub const CORRECTION_HISTORY_SIZE: usize = 16_384;
pub const CORRECTION_HISTORY_GRAIN: i32 = 256;
pub const CORRECTION_HISTORY_WEIGHT_SCALE: i32 = 256;
pub const CORRECTION_HISTORY_MAX: i32 = CORRECTION_HISTORY_GRAIN * 32;

#[repr(transparent)]
pub struct CorrectionHistoryTable {
    table: [[AtomicI32; 2]; CORRECTION_HISTORY_SIZE],
}

impl CorrectionHistoryTable {
    pub fn boxed() -> Box<Self> {
        #![allow(clippy::cast_ptr_alignment)]
        // SAFETY: we're allocating a zeroed block of memory, and then casting it to a Box<Self>
        // this is fine! because [[HistoryTable; BOARD_N_SQUARES]; 12] is just a bunch of i16s
        // at base, which are fine to zero-out.
        unsafe {
            let layout = std::alloc::Layout::new::<Self>();
            let ptr = std::alloc::alloc_zeroed(layout);
            if ptr.is_null() {
                std::alloc::handle_alloc_error(layout);
            }
            Box::from_raw(ptr.cast())
        }
    }

    pub fn clear(&mut self) {
        self.table.iter_mut().for_each(|t| {
            t.iter()
                .for_each(|a| a.store(0, std::sync::atomic::Ordering::Relaxed))
        });
    }

    #[allow(clippy::cast_possible_truncation)]
    pub fn get(&self, side: usize, key: u64) -> &AtomicI32 {
        &self.table[(key % CORRECTION_HISTORY_SIZE as u64) as usize][side]
    }

    /// Update the correction history for a pawn pattern.
    pub fn update_correction_history(&self, pos: &Board, depth: i32, diff: i32) {
        fn update(entry: &AtomicI32, new_weight: i32, scaled_diff: i32) {
            let update = entry.load(Ordering::Relaxed)
                * (CORRECTION_HISTORY_WEIGHT_SCALE - new_weight)
                + scaled_diff * new_weight;
            entry.store(
                i32::clamp(
                    update / CORRECTION_HISTORY_WEIGHT_SCALE,
                    -CORRECTION_HISTORY_MAX,
                    CORRECTION_HISTORY_MAX,
                ),
                Ordering::Relaxed,
            );
        }
        let scaled_diff = diff * CORRECTION_HISTORY_GRAIN;
        let new_weight = 16.min(1 + depth);
        debug_assert!(new_weight <= CORRECTION_HISTORY_WEIGHT_SCALE);
        let us = pos.stm();

        update(self.get(us, pos.pawn_hash()), new_weight, scaled_diff);
    }

    #[allow(clippy::cast_possible_truncation)]
    pub fn correction(&self, conf: &MctsParams, pos: &Board) -> i32 {
        let pawn = self.get(pos.stm(), pos.pawn_hash()).load(Ordering::Relaxed);
        let adjustment = i64::from(pawn) * i64::from(conf.pawn_corrhist_weight());
        (adjustment / 1024) as i32 / CORRECTION_HISTORY_GRAIN
    }
}
