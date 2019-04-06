pub mod memory;

use dynarmic_sys::*;
use std::cell::{RefCell, Ref, RefMut};

use memory::Memory;

pub trait Handlers: Sized {
    type Memory: Memory;

    fn memory(&self) -> &Self::Memory;
    
    fn handle_svc(&mut self, _context: JitContext, _swi: u32) {}
}

pub struct Context<H: Handlers> {
    handlers: H,
    ticks: u64,
}

pub struct JitContext<'a> {
    jit: RefCell<&'a mut Jit>,
}

impl<'a> JitContext<'a> {
    pub fn regs(&self) -> Ref<[u32; 16]> {
        Ref::map(self.jit.borrow(), |jit| unsafe { dynarmic_regs(jit) })
    }

    pub fn regs_mut(&self) -> RefMut<[u32; 16]> {
        RefMut::map(self.jit.borrow_mut(), |jit| unsafe { dynarmic_regs_mut(jit) })
    }

    pub fn extregs(&self) -> Ref<[u32; 64]> {
        Ref::map(self.jit.borrow(), |jit| unsafe { dynarmic_extregs(jit) })
    }

    pub fn extregs_mut(&self) -> RefMut<[u32; 64]> {
        RefMut::map(self.jit.borrow_mut(), |jit| unsafe { dynarmic_extregs_mut(jit) })
    }

    pub fn cpsr(&self) -> u32 {
        unsafe { dynarmic_cpsr(*self.jit.borrow()) }
    }

    pub fn set_cpsr(&self, cpsr: u32) {
        unsafe { dynarmic_set_cpsr(*self.jit.borrow(), cpsr) }
    }

    pub fn fpscr(&self) -> u32 {
        unsafe { dynarmic_fpscr(*self.jit.borrow()) }
    }

    pub fn set_fpscr(&self, fpscr: u32) {
        unsafe { dynarmic_set_fpscr(*self.jit.borrow(), fpscr) }
    }

    pub fn halt(&self) {
        unsafe { dynarmic_halt(*self.jit.borrow()) }
    }
}

impl<H: Handlers> Context<H> {
    fn from_jit<'a, 'b: 'a>(jit: &'a mut Jit) -> &'b mut Self {
        let ud = unsafe {
            dynarmic_get_userdata(jit)
        };
        unsafe { std::mem::transmute(ud) }
    }

    extern fn read<T: memory::Primitive>(jit: &mut Jit, addr: u32) -> T {
        Self::from_jit(jit).handlers.memory().read(addr)
    }

    extern fn write<T: memory::Primitive>(jit: &mut Jit, addr: u32, value: T) {
        Self::from_jit(jit).handlers.memory().write(addr, value)
    }

    extern fn is_read_only_memory(jit: &mut Jit, addr: u32) -> bool {
        Self::from_jit(jit).handlers.memory().is_read_only(addr)
    }

    extern fn call_svc(jit: &mut Jit, svc: u32) {
        let context = Self::from_jit(jit);
        let jit_context = JitContext {
            jit: RefCell::new(jit),
        };
        context.handlers.handle_svc(jit_context, svc);
    }

    extern fn exception_raised(jit: &mut Jit, addr: u32, ex: Exception) {
        unimplemented!()
    }

    extern fn add_ticks(jit: &mut Jit, ticks: u64) {
        let ctx = Self::from_jit(jit);
        ctx.ticks = ctx.ticks.saturating_sub(ticks);
    }

    extern fn get_ticks_remaining(jit: &mut Jit) -> u64 {
        let ctx = Self::from_jit(jit);
        ctx.ticks
    }

    fn callbacks() -> Callbacks {
        Callbacks {
            read8: Self::read,
            read16: Self::read,
            read32: Self::read,
            read64: Self::read,
            write8: Self::write,
            write16: Self::write,
            write32: Self::write,
            write64: Self::write,
            is_read_only_memory: Self::is_read_only_memory,
            call_svc: Self::call_svc,
            exception_raised: Self::exception_raised,
            add_ticks: Self::add_ticks,
            get_ticks_remaining: Self::get_ticks_remaining,
        }
    }
}

pub struct Executor<H: Handlers> {
    jit: &'static mut Jit,
    _context: Box<Context<H>>,
}

impl<H: Handlers> Executor<H> {
    pub fn new(handlers: H) -> Self {
        let mut context = Box::new(Context {
            handlers,
            ticks: std::u64::MAX,
        });

        let callbacks = Context::<H>::callbacks();

        let jit = unsafe {
            dynarmic_new(
                context.as_mut() as *mut Context<H> as *mut _,
                &callbacks,
                None
            )
        };

        Executor {
            jit,
            _context: context
        }
    }

    pub fn run(&mut self) {
        unsafe { dynarmic_run(self.jit) }
    }

    pub fn context(&mut self) -> JitContext {
        JitContext {
            jit: RefCell::new(self.jit),
        }
    }
}

impl<H: Handlers> Drop for Executor<H> {
    fn drop(&mut self) {
        unsafe { dynarmic_delete(self.jit) }
    }
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;
    use super::*;
    #[test]
    fn it_works() {
        struct TestHandlers {
            memory: Rc<memory::MemoryImpl>,
        }

        impl Handlers for TestHandlers {
            type Memory = memory::MemoryImpl;

            fn memory(&self) -> &Self::Memory {
                &self.memory
            }
        }

        let mut mem = memory::MemoryImpl::new();

        mem.map_memory(0x00000000, 1, true);
        mem.write(0, 0x0088u16);
        mem.write(2, 0xE7FEu16);

        let handlers = TestHandlers {
            memory: Rc::new(mem)
        };

        let mut executor = Executor::new(handlers);

        {
            let context = executor.context();

            context.set_cpsr(0x30); // Thumb mode

            let mut regs = context.regs_mut();
            regs[0] = 1;
            regs[1] = 2;
            regs[15] = 0; // PC = 0
        }

        executor.run();

        {
            let context = executor.context();
            let regs = context.regs();
            eprintln!("{:X?}", regs);
            assert_eq!(regs[0], 8);
        }
    }
}
