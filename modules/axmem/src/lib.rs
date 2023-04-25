#![no_std]

extern crate alloc;
use core::ptr::copy_nonoverlapping;

use alloc::vec::Vec;

#[macro_use]
extern crate log;

use axalloc::{global_allocator, GlobalPage};
use axhal::{
    mem::{memory_regions, phys_to_virt, virt_to_phys, PhysAddr, VirtAddr, PAGE_SIZE_4K},
    paging::{MappingFlags, PageSize, PageTable},
};

/// PageTable + MemoryArea for a process (task)
pub struct MemorySet {
    page_table: PageTable,
    owned_mem: Vec<MapArea>,
}

pub struct MapArea {
    pub pages: GlobalPage,
    pub vaddr: VirtAddr,
    pub flags: MappingFlags,
}

impl MemorySet {
    pub fn new_with_kernel_mapped() -> Self {
        let mut page_table = PageTable::try_new().expect("Error allocating page table.");

        for r in memory_regions() {
            page_table
                .map_region(phys_to_virt(r.paddr), r.paddr, r.size, r.flags.into(), true)
                .expect("Error mapping kernel memory");
        }

        Self {
            page_table,
            owned_mem: Vec::new(),
        }
    }

    pub fn map_elf(&mut self, elf: &xmas_elf::ElfFile) {
        let elf_header = elf.header;
        let magic = elf_header.pt1.magic;
        assert_eq!(magic, [0x7f, 0x45, 0x4c, 0x46], "invalid elf!");

        elf.program_iter().for_each(|ph| {
            let ph_type = ph
                .get_type()
                .expect("Error loading Program Header Type in app ELF file.");

            if ph_type == xmas_elf::program::Type::Load {
                let start_va = ph.virtual_addr() as usize;
                let end_va = (ph.virtual_addr() + ph.mem_size()) as usize;

                let mut flags = MappingFlags::USER;
                if ph.flags().is_read() {
                    flags |= MappingFlags::READ;
                }
                if ph.flags().is_write() {
                    flags |= MappingFlags::WRITE;
                }
                if ph.flags().is_execute() {
                    flags |= MappingFlags::EXECUTE;
                }

                self.alloc_region(
                    (ph.virtual_addr() as usize).into(),
                    ph.mem_size() as usize,
                    flags,
                    Some(&elf.input[ph.offset() as usize..(ph.offset() + ph.file_size()) as usize]),
                );
            }
        });
    }

    pub fn page_table_root_ppn(&self) -> PhysAddr {
        self.page_table.root_paddr()
    }

    pub fn max_va(&self) -> VirtAddr {
        self.owned_mem
            .iter()
            .map(|MapArea { pages, vaddr, .. }| *vaddr + pages.size())
            .max()
            .unwrap_or_default()
    }

    pub fn map_region(
        &mut self,
        vaddr: VirtAddr,
        paddr: PhysAddr,
        size: usize,
        flags: MappingFlags,
        allow_huge: bool,
    ) {
        self.page_table
            .map_region(vaddr, paddr, size, flags, allow_huge)
            .expect("Error mapping allocated memory");
    }

    pub fn alloc_region(
        &mut self,
        vaddr: VirtAddr,
        size: usize,
        flags: MappingFlags,
        data: Option<&[u8]>,
    ) {
        let num_pages = (size + PAGE_SIZE_4K - 1) / PAGE_SIZE_4K;
        let mut pages = GlobalPage::alloc_contiguous(num_pages, PAGE_SIZE_4K)
            .expect("Error allocating memory when trying to map");

        self.map_region(
            vaddr,
            pages.start_paddr(virt_to_phys),
            pages.size(),
            flags,
            false,
        );

        // clear the allocated region and copy data
        pages.zero();
        if let Some(data) = data {
            pages.as_slice_mut()[..data.len()].copy_from_slice(data);
        }

        self.owned_mem.push(MapArea {
            pages,
            vaddr,
            flags,
        });
    }

    // for fork
    pub fn clone_mapped(&self) -> Self {
        let mut new = Self::new_with_kernel_mapped();

        self.owned_mem.iter().for_each(
            |MapArea {
                 pages,
                 vaddr,
                 flags,
             }| {
                let data = pages.as_slice();
                new.alloc_region(*vaddr, pages.size(), *flags, Some(data));
            },
        );

        new
    }
}
