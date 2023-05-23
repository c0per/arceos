#![no_std]

extern crate alloc;
use alloc::{collections::BTreeMap, vec::Vec};
use core::{mem::size_of, ptr::copy_nonoverlapping};

#[macro_use]
extern crate log;

use axalloc::{global_allocator, GlobalPage};
use axhal::{
    mem::{memory_regions, phys_to_virt, virt_to_phys, PhysAddr, VirtAddr, PAGE_SIZE_4K},
    paging::{MappingFlags, PageSize, PageTable},
};
use xmas_elf::symbol_table::Entry;

pub(crate) const REL_GOT: u32 = 6;
pub(crate) const REL_PLT: u32 = 7;
pub(crate) const REL_RELATIVE: u32 = 8;
pub(crate) const R_RISCV_64: u32 = 2;
pub(crate) const R_RISCV_RELATIVE: u32 = 3;

pub(crate) const AT_PHDR: u8 = 3;
pub(crate) const AT_PHENT: u8 = 4;
pub(crate) const AT_PHNUM: u8 = 5;
pub(crate) const AT_PAGESZ: u8 = 6;
pub(crate) const AT_BASE: u8 = 7;
pub(crate) const AT_ENTRY: u8 = 9;
pub(crate) const AT_RANDOM: u8 = 25;

/// PageTable + MemoryArea for a process (task)
pub struct MemorySet {
    page_table: PageTable,
    owned_mem: Vec<MapArea>,
    pub entry: usize,
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
            debug!(
                "mapping kernel region [0x{:x}, 0x{:x})",
                usize::from(phys_to_virt(r.paddr)),
                usize::from(phys_to_virt(r.paddr)) + r.size,
            );
            page_table
                .map_region(phys_to_virt(r.paddr), r.paddr, r.size, r.flags.into(), true)
                .expect("Error mapping kernel memory");
        }

        Self {
            page_table,
            owned_mem: Vec::new(),
            entry: 0,
        }
    }

    pub fn map_elf(&mut self, elf: &xmas_elf::ElfFile) -> BTreeMap<u8, usize> {
        let elf_header = elf.header;
        let magic = elf_header.pt1.magic;
        assert_eq!(magic, [0x7f, 0x45, 0x4c, 0x46], "invalid elf!");

        // Some elf will load ELF Header (offset == 0) to vaddr 0. In that case, base_addr will be added to all the LOAD.
        let (base_addr, elf_header_vaddr): (usize, usize) = if let Some(header) = elf
            .program_iter()
            .find(|ph| ph.get_type() == Ok(xmas_elf::program::Type::Load) && ph.offset() == 0)
        {
            // Loading ELF Header into memory.
            let vaddr = header.virtual_addr() as usize;

            if vaddr == 0 {
                (0x400_0000, 0x400_0000)
            } else {
                (0, vaddr)
            }
        } else {
            (0, 0)
        };
        info!("Base addr for the elf: 0x{:x}", base_addr);

        // Load Elf "LOAD" segments at base_addr.
        elf.program_iter()
            .filter(|ph| ph.get_type() == Ok(xmas_elf::program::Type::Load))
            .for_each(|ph| {
                let mut start_va = ph.virtual_addr() as usize + base_addr;
                let end_va = (ph.virtual_addr() + ph.mem_size()) as usize + base_addr;
                let mut start_offset = ph.offset() as usize;
                let end_offset = (ph.offset() + ph.file_size()) as usize;

                // Virtual address from elf may not be aligned.
                assert_eq!(start_va % PAGE_SIZE_4K, start_offset % PAGE_SIZE_4K);
                let front_pad = start_va % PAGE_SIZE_4K;
                start_va -= front_pad;
                start_offset -= front_pad;
                assert!(start_offset >= 0);

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

                debug!("elf section [0x{:x}, 0x{:x})", start_va, end_va);

                self.alloc_region(
                    VirtAddr::from(start_va),
                    end_va - start_va,
                    flags,
                    Some(&elf.input[start_offset..end_offset]),
                );
            });

        // Relocate .rela.dyn sections
        if let Some(rela_dyn) = elf.find_section_by_name(".rela.dyn") {
            let data = match rela_dyn.get_data(&elf) {
                Ok(xmas_elf::sections::SectionData::Rela64(data)) => data,
                _ => panic!("Invalid data in .rela.dyn section"),
            };

            if let Some(dyn_sym_table) = elf.find_section_by_name(".dynsym") {
                let dyn_sym_table = match dyn_sym_table.get_data(&elf) {
                    Ok(xmas_elf::sections::SectionData::DynSymbolTable64(dyn_sym_table)) => {
                        dyn_sym_table
                    }
                    _ => panic!("Invalid data in .dynsym section"),
                };

                info!("Relocating .rela.dyn");
                for entry in data {
                    match entry.get_type() {
                        REL_GOT | REL_PLT | R_RISCV_64 => {
                            let dyn_sym = &dyn_sym_table[entry.get_symbol_table_index() as usize];
                            let sym_val = if dyn_sym.shndx() == 0 {
                                let name = dyn_sym.get_name(&elf).unwrap();
                                panic!(r#"Symbol "{}" not found"#, name);
                            } else {
                                base_addr + dyn_sym.value() as usize
                            };

                            let value = sym_val + entry.get_addend() as usize;
                            let addr = base_addr + entry.get_offset() as usize;

                            info!("relocating: addr 0x{:x}", addr);

                            unsafe {
                                copy_nonoverlapping(
                                    value.to_ne_bytes().as_ptr(),
                                    addr as *mut u8,
                                    size_of::<usize>() / size_of::<u8>(),
                                );
                            }
                        }
                        REL_RELATIVE | R_RISCV_RELATIVE => {
                            let value = base_addr + entry.get_addend() as usize;
                            let addr = base_addr + entry.get_offset() as usize;

                            info!("relocating: addr 0x{:x}", addr);

                            unsafe {
                                copy_nonoverlapping(
                                    value.to_ne_bytes().as_ptr(),
                                    addr as *mut u8,
                                    size_of::<usize>() / size_of::<u8>(),
                                );
                            }
                        }
                        other => panic!("Unknown relocation type: {}", other),
                    }
                }
            }
        }

        // Relocate .rela.plt sections
        if let Some(rela_plt) = elf.find_section_by_name(".rela.plt") {
            let data = match rela_plt.get_data(&elf) {
                Ok(xmas_elf::sections::SectionData::Rela64(data)) => data,
                _ => panic!("Invalid data in .rela.plt section"),
            };
            let dyn_sym_table = match elf
                .find_section_by_name(".dynsym")
                .expect("Dynamic Symbol Table not found for .rela.plt section")
                .get_data(&elf)
            {
                Ok(xmas_elf::sections::SectionData::DynSymbolTable64(dyn_sym_table)) => {
                    dyn_sym_table
                }
                _ => panic!("Invalid data in .dynsym section"),
            };

            info!("Relocating .rela.plt");
            for entry in data {
                match entry.get_type() {
                    5 => {
                        let dyn_sym = &dyn_sym_table[entry.get_symbol_table_index() as usize];
                        let sym_val = if dyn_sym.shndx() == 0 {
                            let name = dyn_sym.get_name(&elf).unwrap();
                            panic!(r#"Symbol "{}" not found"#, name);
                        } else {
                            dyn_sym.value() as usize
                        };

                        let value = base_addr + sym_val;
                        let addr = base_addr + entry.get_offset() as usize;

                        info!("relocating: addr 0x{:x}", addr);

                        unsafe {
                            copy_nonoverlapping(
                                value.to_ne_bytes().as_ptr(),
                                addr as *mut u8,
                                size_of::<usize>() / size_of::<u8>(),
                            );
                        }
                    }
                    other => panic!("Unknown relocation type: {}", other),
                }
            }
        }

        self.entry = elf.header.pt2.entry_point() as usize + base_addr;

        let mut map = BTreeMap::new();
        map.insert(
            AT_PHDR,
            elf_header_vaddr + elf.header.pt2.ph_offset() as usize,
        );
        map.insert(AT_PHENT, elf.header.pt2.ph_entry_size() as usize);
        map.insert(AT_PHNUM, elf.header.pt2.ph_count() as usize);
        map.insert(AT_RANDOM, 0);
        map.insert(AT_PAGESZ, PAGE_SIZE_4K);
        map
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

        debug!(
            "allocating [0x{:x}, 0x{:x}) to [0x{:x}, 0x{:x})",
            usize::from(vaddr),
            usize::from(vaddr) + size,
            usize::from(pages.start_vaddr()),
            usize::from(pages.start_vaddr()) + pages.size(),
        );

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
