use std::collections::BTreeMap;
use byteorder::{LE, ByteOrder};
use std::cell::Cell;

const PAGE_BITS: u32 = 12;
const NUM_PAGE_TABLE_ENTRIES: u32 = 1 << (32 - PAGE_BITS);
const PAGE_LOWER_MASK: u32 = (1 << PAGE_BITS) - 1;
const PAGE_UPPER_MASK: u32 = !PAGE_LOWER_MASK;
const PAGE_SIZE: usize = 1 << PAGE_BITS;

pub trait Primitive: Sized {
    const ALIGN: usize = Self::SIZE - 1;
    const SIZE: usize = std::mem::size_of::<Self>();
    fn read(b: &[u8]) -> Self;
    fn write(self, b: &mut [u8]);
}

impl Primitive for u8 {
    fn read(b: &[u8]) -> Self {
        b[0]
    }
    fn write(self, b: &mut [u8]) {
        b[0] = self;
    }
}

impl Primitive for u16 {
    fn read(b: &[u8]) -> Self {
        LE::read_u16(b)
    }
    fn write(self, b: &mut [u8]) {
        LE::write_u16(b, self)
    }
}

impl Primitive for u32 {
    fn read(b: &[u8]) -> Self {
        LE::read_u32(b)
    }
    fn write(self, b: &mut [u8]) {
        LE::write_u32(b, self)
    }
}

impl Primitive for u64 {
    fn read(b: &[u8]) -> Self {
        LE::read_u64(b)
    }
    fn write(self, b: &mut [u8]) {
        LE::write_u64(b, self)
    }
}

impl<T: Primitive + Copy + Default> Primitive for [T; 2] {
    fn read(b: &[u8]) -> Self {
        let mut out = Self::default();
        for (i, out) in out.iter_mut().enumerate() {
            *out = T::read(&b[i*T::SIZE..(i+1)*T::SIZE]);
        }
        out
    }
    fn write(self, b: &mut [u8]) {
        for (i, item) in self.iter().enumerate() {
            T::write(*item, &mut b[i*T::SIZE..(i+1)*T::SIZE])
        }
    }
}

pub trait Memory {
    fn read<T: Primitive>(&self, addr: u32) -> T;
    fn write<T: Primitive>(&self, addr: u32, value: T);
    fn is_read_only(&self, addr: u32) -> bool;
}

pub enum PageSpanKind {
    Normal {
        backing: Cell<Box<[u8]>>,
    },
    MMIO {
        handler: Cell<Option<Box<IOPage>>>,
    }
}

pub trait IOPage {
    fn read(&mut self, o: usize, b: &mut [u8]);
    fn write(&mut self, o: usize, b: &[u8]);
}

impl PageSpanKind {
    fn read<T: Primitive>(&self, offset: usize) -> T {
        let offset = offset & !T::ALIGN;
        match self {
            PageSpanKind::Normal { backing } => {
                let bytes = backing.replace(Box::new([]));
                let src = &bytes[offset..(offset + T::SIZE)];
                let value = T::read(src);
                backing.set(bytes);
                value
            },
            PageSpanKind::MMIO { handler } => {
                let mut h = handler.take().expect("Attempt to reentrantly read IO page");
                let mut src = [0u8; 8];
                h.read(offset, &mut src[..T::SIZE]);
                handler.set(Some(h));
                T::read(&src[..])
            }
        }
    }

    fn write<T: Primitive>(&self, offset: usize, value: T) {
        let offset = offset & !T::ALIGN;
        match self {
            PageSpanKind::Normal { backing } => {
                let mut bytes = backing.replace(Box::new([]));
                let dest = &mut bytes[offset..(offset + T::SIZE)];
                T::write(value, dest);
                backing.set(bytes);
            },
            PageSpanKind::MMIO { handler } => {
                let mut h = handler.take().expect("Attempt to reentrantly write IO page");
                let mut dest = [0u8; 8];
                T::write(value, &mut dest[..T::SIZE]);
                h.write(offset, &mut dest[..T::SIZE]);
                handler.set(Some(h));
            }
        }
    }
}

pub struct PageSpan {
    size: u32, // In pages
    kind: PageSpanKind,
    read_only: bool,
}

pub struct MemoryImpl {
    pages: BTreeMap<u32, PageSpan>, // Page -> PageSpan mapping
}

struct MemoryLookup<T> {
    item: T,
    offset: u32, // Offset from start of item
}

impl MemoryImpl {
    pub fn new() -> MemoryImpl {
        MemoryImpl {
            pages: Default::default()
        }
    }

    fn lookup(&self, page: u32) -> Option<MemoryLookup<&PageSpan>> {
        use std::ops::Bound::Included;
        let (found_page, found_item) = self.pages.range((Included(&0), Included(&page))).rev().next()?;
        if (found_page + found_item.size) > page {
            Some(MemoryLookup {
                item: found_item,
                offset: page - found_page
            })
        } else {
            None
        }
    }

    fn lookup_mut(&mut self, page: u32) -> Option<MemoryLookup<&mut PageSpan>> {
        use std::ops::Bound::Included;
        let (found_page, found_item) = self.pages.range_mut((Included(&0), Included(&page))).rev().next()?;
        if (found_page + found_item.size) > page {
            Some(MemoryLookup {
                item: found_item,
                offset: page - found_page
            })
        } else {
            None
        }
    }

    fn is_mapped(&self, addr: u32) -> bool {
        self.lookup((addr & PAGE_UPPER_MASK) >> PAGE_BITS).is_some()
    }

    pub fn map_memory(&mut self, addr: u32, pages: u32, read_only: bool) {
        let page_span = PageSpan {
            size: pages,
            kind: PageSpanKind::Normal {
                backing: Cell::new(vec![0u8; (pages << PAGE_BITS) as usize].into_boxed_slice()),
            },
            read_only,
        };

        self.pages.insert(addr >> PAGE_BITS, page_span);
    }
}

impl Memory for MemoryImpl {
    fn read<T: Primitive>(&self, addr: u32) -> T {
        let page = (addr & !PAGE_LOWER_MASK) >> PAGE_BITS;
        let MemoryLookup { item, offset } = self.lookup(page).expect("Unmapped memory access");
        item.kind.read((offset as usize) + (addr & PAGE_LOWER_MASK) as usize)
    }

    fn write<T: Primitive>(&self, addr: u32, value: T) {
        let page = (addr & !PAGE_LOWER_MASK) >> PAGE_BITS;
        let MemoryLookup { item, offset } = self.lookup(page).expect("Unmapped memory access");
        item.kind.write((offset as usize) + (addr & PAGE_LOWER_MASK) as usize, value)
    }

    fn is_read_only(&self, addr: u32) -> bool {
        self.lookup((addr & PAGE_UPPER_MASK) >> PAGE_BITS).unwrap().item.read_only
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_mem_lookup_fails() {
        let mem = MemoryImpl::new();
        assert!(mem.lookup(0).is_none());
        assert!(mem.lookup(1).is_none());
    }

    #[test]
    fn single_page_lookup_works() {
        let mut mem = MemoryImpl::new();
        mem.map_memory(0, 1, false);
        assert!(mem.lookup(0).is_some());
        assert!(mem.lookup(1).is_none());
    }

    #[test]
    fn multi_page_lookup_works() {
        let mut mem = MemoryImpl::new();
        mem.map_memory(0, 2, false);
        assert!(mem.lookup(0).is_some());
        assert!(mem.lookup(1).is_some());
    }
}
