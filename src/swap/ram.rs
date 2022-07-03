use embedded_storage::Storage;

use crate::{
    log,
    state::{ExchangeProgress, State, Update},
    swap::{MemoryError, Swap},
    Address,
};

pub struct Ram;

impl Swap for Ram {
    fn exchange<InternalMemory: Storage, HardwareState: State, const INTERNAL_PAGE_SIZE: usize>(
        &mut self,
        internal_memory: &mut InternalMemory,
        state: &mut HardwareState,
        exchange: ExchangeProgress,
    ) -> Result<(), MemoryError> {
        let ExchangeProgress {
            a,
            b,
            page_index,
            step,
            ..
        } = exchange;

        // TODO: Sanity Check start_index
        if a.size != b.size {
            return Err(MemoryError::BankSizeNotEqual);
        }

        if a.size == 0 || b.size == 0 {
            return Err(MemoryError::BankSizeZero);
        }

        let size = a.size; // Both are equal

        let full_pages = size / INTERNAL_PAGE_SIZE as Address;
        let remaining_page_length = size as usize % INTERNAL_PAGE_SIZE;

        let mut page_a_buf = [0_u8; INTERNAL_PAGE_SIZE];
        let mut page_b_buf = [0_u8; INTERNAL_PAGE_SIZE];
        // can we reduce this to 1 buf and fancy operations?
        // probably not with the read/write API.
        // classic memory exchange problem :)

        let mut last_state = state.read();

        // Set this in the exchanging part to know whether we are in a recovery process from a
        // failed update or on the initial update
        let recovering = matches!(last_state.update, Update::Revert(_));

        // TODO: Fix
        let a_location = a.location;
        let b_location = b.location;

        for page_index in page_index..full_pages {
            let offset = page_index * INTERNAL_PAGE_SIZE as u32;
            log::trace!(
                "Exchange: Page {}, from a ({}) to b ({})",
                page_index,
                a_location + offset,
                b_location + offset
            );

            internal_memory
                .read(a_location + offset, &mut page_a_buf)
                .map_err(|_| MemoryError::ReadFailure)?;
            internal_memory
                .read(b_location + offset, &mut page_b_buf)
                .map_err(|_| MemoryError::ReadFailure)?;
            internal_memory
                .write(a_location + offset, &page_b_buf)
                .map_err(|_| MemoryError::WriteFailure)?;
            internal_memory
                .write(b_location + offset, &page_a_buf)
                .map_err(|_| MemoryError::WriteFailure)?;

            // Store the exchange progress

            last_state.update = Update::Exchanging(ExchangeProgress {
                a,
                b,
                recovering,
                page_index,
                step,
            });

            state
                .write(&last_state)
                .map_err(|_| MemoryError::WriteFailure)?;
        }
        // TODO: Fit this into the while loop
        if remaining_page_length > 0 {
            let offset = full_pages * INTERNAL_PAGE_SIZE as u32;

            internal_memory
                .read(
                    a.location + offset,
                    &mut page_a_buf[0..remaining_page_length],
                )
                .map_err(|_| MemoryError::ReadFailure)?;
            internal_memory
                .read(
                    b.location + offset,
                    &mut page_b_buf[0..remaining_page_length],
                )
                .map_err(|_| MemoryError::ReadFailure)?;
            internal_memory
                .write(a.location + offset, &page_a_buf[0..remaining_page_length])
                .map_err(|_| MemoryError::WriteFailure)?;
            internal_memory
                .write(b.location + offset, &page_b_buf[0..remaining_page_length])
                .map_err(|_| MemoryError::WriteFailure)?;
        }

        Ok(())
    }
}
