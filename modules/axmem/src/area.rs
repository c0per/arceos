use alloc::vec::Vec;
use axalloc::PhysPage;
use axhal::{
    mem::{virt_to_phys, VirtAddr, PAGE_SIZE_4K},
    paging::{MappingFlags, PageTable},
};
use axio::SeekFrom;

use crate::MemBackend;

pub struct MapArea {
    pub pages: Vec<Option<PhysPage>>,
    pub vaddr: VirtAddr,
    pub flags: MappingFlags,
    pub backend: Option<MemBackend>,
}

impl MapArea {
    /// Create a lazy-load area and map it in page table (page fault PTE).
    pub fn new_lazy(
        start: VirtAddr,
        num_pages: usize,
        flags: MappingFlags,
        backend: Option<MemBackend>,
        page_table: &mut PageTable,
    ) -> Self {
        let mut pages = Vec::with_capacity(num_pages);
        for _ in 0..num_pages {
            pages.push(None);
        }

        let _ = page_table
            .map_fault_region(start, num_pages * PAGE_SIZE_4K)
            .unwrap();

        Self {
            pages,
            vaddr: start,
            flags,
            backend,
        }
    }

    /// Allocated an area and map it in page table.
    pub fn new_alloc(
        start: VirtAddr,
        num_pages: usize,
        flags: MappingFlags,
        backend: Option<MemBackend>,
        page_table: &mut PageTable,
    ) -> Self {
        let pages = PhysPage::alloc_contiguous(num_pages, PAGE_SIZE_4K)
            .expect("Error allocating memory when trying to map");

        let _ = page_table
            .map_region(
                start,
                virt_to_phys(pages[0].as_ref().unwrap().start_vaddr),
                num_pages * PAGE_SIZE_4K,
                flags,
                false,
            )
            .unwrap();

        Self {
            pages,
            vaddr: start,
            flags,
            backend,
        }
    }

    pub fn handle_page_fault(
        &mut self,
        addr: VirtAddr,
        flags: MappingFlags,
        page_table: &mut PageTable,
    ) {
        info!(
            "handling {:?} page fault in area [{:?}, {:?})",
            addr,
            self.vaddr,
            self.end_va()
        );
        assert!(
            self.vaddr <= addr && addr < self.end_va(),
            "Try to handle page fault address out of bound"
        );
        if !self.flags.contains(flags) {
            panic!(
                "Try to access {:?} memory with {:?} flag",
                self.flags, flags
            );
        }

        let page_index = (usize::from(addr) - usize::from(self.vaddr)) / PAGE_SIZE_4K;
        if page_index >= self.pages.len() {
            unreachable!("Phys page index out of bound");
        }
        if self.pages[page_index].is_some() {
            panic!("Page fault in page already loaded");
        }

        info!("page index {}", page_index);

        // Allocate new page
        let mut page = PhysPage::alloc().expect("Error allocating new phys page for page fault");

        info!(
            "new phys page virtual (offset) address {:?}",
            page.start_vaddr
        );

        // Read data from backend to fill with 0.
        match &mut self.backend {
            Some(backend) => {
                if backend
                    .read_from_seek(
                        SeekFrom::Current((page_index * PAGE_SIZE_4K) as i64),
                        page.as_slice_mut(),
                    )
                    .is_err()
                {
                    warn!("Failed to read from backend to memory");
                    page.fill(0);
                }
            }
            None => page.fill(0),
        };

        // Map newly allocated page in the page_table
        page_table
            .map_overwrite(
                addr.align_down_4k(),
                virt_to_phys(page.start_vaddr),
                axhal::paging::PageSize::Size4K,
                self.flags,
            )
            .expect("Map in page fault handler failed");

        self.pages[page_index] = Some(page);
    }

    pub fn sync_page_with_backend(&self, page_index: usize) {
        if page_index >= self.pages.len() {
            panic!("Sync page index out of bound");
        }

        if let Some(page) = &self.pages[page_index] {
            if let Some(backend) = &self.backend {
                let _ = backend
                    .write_to_seek(
                        SeekFrom::Current((page_index * PAGE_SIZE_4K) as i64),
                        page.as_slice(),
                    )
                    .unwrap();
            }
        } else {
            debug!("Tried to sync an unallocated page");
        }
    }
}

impl MapArea {
    pub fn size(&self) -> usize {
        self.pages.len() * PAGE_SIZE_4K
    }

    pub fn end_va(&self) -> VirtAddr {
        self.vaddr + self.size()
    }

    /// Convert to a raw pointer.
    pub fn as_ptr(&self) -> *const u8 {
        self.vaddr.as_ptr()
    }

    /// Convert to a mutable raw pointer.
    pub fn as_mut_ptr(&mut self) -> *mut u8 {
        self.vaddr.as_mut_ptr()
    }

    /// Forms a slice that can read data.
    pub fn as_slice(&self) -> &[u8] {
        unsafe { core::slice::from_raw_parts(self.as_ptr(), self.size()) }
    }

    /// Forms a mutable slice that can write data.
    pub fn as_slice_mut(&mut self) -> &mut [u8] {
        unsafe { core::slice::from_raw_parts_mut(self.as_mut_ptr(), self.size()) }
    }

    /// Fill `self` with `byte`.
    pub fn fill(&mut self, byte: u8) {
        unsafe { core::ptr::write_bytes(self.as_mut_ptr(), byte, self.size()) }
    }

    /// If [start, end) overlaps with self.
    pub fn overlap_with(&self, start: VirtAddr, end: VirtAddr) -> bool {
        let area_end = self.vaddr + self.size();

        self.vaddr <= start && start < area_end || start <= self.vaddr && self.vaddr < end
    }

    pub fn contained_in(&self, start: VirtAddr, end: VirtAddr) -> bool {
        start <= self.vaddr && self.vaddr + self.size() <= end
    }

    pub fn contains(&self, start: VirtAddr, end: VirtAddr) -> bool {
        self.vaddr <= start && end <= self.vaddr + self.size()
    }
}

impl Drop for MapArea {
    fn drop(&mut self) {
        self.pages
            .iter()
            .enumerate()
            .filter(|(_, page)| page.is_some())
            .for_each(|(page_index, _)| self.sync_page_with_backend(page_index))
    }
}
