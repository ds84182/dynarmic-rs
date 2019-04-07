use std::ffi::c_void;

#[repr(C)]
pub struct Jit(c_void);

trait MemoryType {}

#[repr(C)]
pub enum Exception {
    UndefinedInstruction,
    UnpredictableInstruction,
    Breakpoint
}

pub type MemoryReadCallback<T> = extern fn(&mut Jit, u32) -> T;
pub type MemoryWriteCallback<T> = extern fn(&mut Jit, u32, T) -> ();
pub type IsReadOnlyMemoryCallback = extern fn(&mut Jit, u32) -> bool;
pub type CallSVCCallback = extern fn(&mut Jit, u32) -> ();
pub type ExceptionRaisedCallback = extern fn(&mut Jit, u32, Exception);
pub type AddTicksCallback = extern fn(&mut Jit, u64);
pub type GetTicksRemainingCallback = extern fn(&mut Jit) -> u64;

#[repr(C)]
pub struct Callbacks {
    pub read8: MemoryReadCallback<u8>,
    pub read16: MemoryReadCallback<u16>,
    pub read32: MemoryReadCallback<u32>,
    pub read64: MemoryReadCallback<u64>,

    pub write8: MemoryWriteCallback<u8>,
    pub write16: MemoryWriteCallback<u16>,
    pub write32: MemoryWriteCallback<u32>,
    pub write64: MemoryWriteCallback<u64>,

    pub is_read_only_memory: IsReadOnlyMemoryCallback,
    pub call_svc: CallSVCCallback,
    pub exception_raised: ExceptionRaisedCallback,
    pub add_ticks: AddTicksCallback,
    pub get_ticks_remaining: GetTicksRemainingCallback,
}

const PAGE_BITS: usize = 12;
const NUM_PAGE_TABLE_ENTRIES: usize = 1 << (32 - PAGE_BITS);

extern {
    pub fn dynarmic_new<'a>(ud: *mut c_void, callbacks: &Callbacks, page_table: *const [*mut u8; NUM_PAGE_TABLE_ENTRIES]) -> &'a mut Jit;
    pub fn dynarmic_delete(jit: &mut Jit);
    pub fn dynarmic_get_userdata(jit: &Jit) -> *mut c_void;
    pub fn dynarmic_run(jit: &mut Jit);

    #[link_name="dynarmic_regs"]
    pub fn dynarmic_regs_mut(jit: &mut Jit) -> &mut [u32; 16];
    pub fn dynarmic_regs(jit: &Jit) -> &[u32; 16];
    
    #[link_name="dynarmic_extregs"]
    pub fn dynarmic_extregs_mut(jit: &mut Jit) -> &mut [u32; 64];
    pub fn dynarmic_extregs(jit: &Jit) -> &[u32; 64];

    pub fn dynarmic_cpsr(jit: &Jit) -> u32;
    pub fn dynarmic_set_cpsr(jit: &Jit, cpsr: u32);
    
    pub fn dynarmic_fpscr(jit: &Jit) -> u32;
    pub fn dynarmic_set_fpscr(jit: &Jit, fpscr: u32);
    
    pub fn dynarmic_halt(jit: &Jit);
}


#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn it_works() {
        let mut memory = Box::new([0u8; 4096]);
        memory[0] = 0x88; // lsls r0, r1, #2
        memory[1] = 0x00;
        memory[2] = 0xFE; // b +#0 (infinite loop)
        memory[3] = 0xE7;

        struct Context {
            ticks_left: u64,
            memory: Box<[u8; 4096]>,
        }

        let mut context = Box::new(Context {
            ticks_left: 1,
            memory
        });

        fn get_context(jit: &mut Jit) -> &mut Context {
            let ud = unsafe {
                dynarmic_get_userdata(jit)
            };
            unsafe { std::mem::transmute(ud) }
        }

        extern fn read8(jit: &mut Jit, addr: u32) -> u8 {
            let addr = addr as usize;
            get_context(jit).memory[addr]
        }
        extern fn read16(jit: &mut Jit, addr: u32) -> u16 {
            let addr = addr as usize;
            let ctx = get_context(jit);
            (ctx.memory[addr] as u16) | ((ctx.memory[addr + 1] as u16) << 8)
        }
        extern fn read32(jit: &mut Jit, addr: u32) -> u32 {
            let addr = addr as usize;
            let ctx = get_context(jit);
            (ctx.memory[addr] as u32) | ((ctx.memory[addr + 1] as u32) << 8) |
            ((ctx.memory[addr + 2] as u32) << 16) | ((ctx.memory[addr + 3] as u32) << 24)
        }
        extern fn read64(jit: &mut Jit, addr: u32) -> u64 {
            let addr = addr as usize;
            let ctx = get_context(jit);
            (ctx.memory[addr] as u64) | ((ctx.memory[addr + 1] as u64) << 8) |
            ((ctx.memory[addr + 2] as u64) << 16) | ((ctx.memory[addr + 3] as u64) << 24) |
            ((ctx.memory[addr + 4] as u64) << 32) | ((ctx.memory[addr + 5] as u64) << 40) |
            ((ctx.memory[addr + 6] as u64) << 48) | ((ctx.memory[addr + 7] as u64) << 56)
        }

        extern fn write8(jit: &mut Jit, addr: u32, value: u8) {
            panic!("Unhandled write 8 0x{:X}: 0x{:X}", addr, value)
        }
        extern fn write16(jit: &mut Jit, addr: u32, value: u16) {
            panic!("Unhandled write 16 0x{:X}: 0x{:X}", addr, value)
        }
        extern fn write32(jit: &mut Jit, addr: u32, value: u32) {
            panic!("Unhandled write 32 0x{:X}: 0x{:X}", addr, value)
        }
        extern fn write64(jit: &mut Jit, addr: u32, value: u64) {
            panic!("Unhandled write 64 0x{:X}: 0x{:X}", addr, value)
        }

        extern fn is_read_only_memory(jit: &mut Jit, addr: u32) -> bool { true }
        extern fn call_svc(jit: &mut Jit, svc: u32) { unimplemented!() }
        extern fn exception_raised(jit: &mut Jit, addr: u32, ex: Exception) { unimplemented!() }

        extern fn add_ticks(jit: &mut Jit, ticks: u64) {
            let ctx = get_context(jit);
            ctx.ticks_left = ctx.ticks_left.saturating_sub(ticks);
        }

        extern fn get_ticks_remaining(jit: &mut Jit) -> u64 {
            let ctx = get_context(jit);
            ctx.ticks_left
        }

        let callbacks = Callbacks {
            read8,
            read16,
            read32,
            read64,
            write8,
            write16,
            write32,
            write64,
            is_read_only_memory,
            call_svc,
            exception_raised,
            add_ticks,
            get_ticks_remaining,
        };

        let jit = unsafe {
            dynarmic_new(
                context.as_mut() as *mut Context as *mut _,
                &callbacks,
                std::ptr::null(),
            )
        };

        {
            let regs = unsafe { dynarmic_regs_mut(jit) };
            regs[0] = 1;
            regs[1] = 2;
            regs[15] = 0; // PC = 0
        }

        unsafe { dynarmic_set_cpsr(jit, 0x00000030) }; // Thumb mode

        unsafe { dynarmic_run(jit) };

        {
            let regs = unsafe { dynarmic_regs_mut(jit) };
            eprintln!("{:X?}", regs);
            assert_eq!(regs[0], 8);
        }

        unsafe { dynarmic_delete(jit) };
    }
}
