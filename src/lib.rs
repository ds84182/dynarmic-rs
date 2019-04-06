mod memory;

use dynarmic_sys::*;
use std::cell::{RefCell, Ref, RefMut};

pub struct Context {
    memory: RefCell<memory::Memory>,
    svc_handler: Option<Box<SvcHandler>>,
    ticks: u64,
}

pub trait SvcHandler {
    fn handle_svc(&mut self, context: JitContext, swi: u32);
}

pub struct JitContext<'a> {
    jit: RefCell<&'a mut Jit>,
    context: &'a Context,
}

impl<'a> JitContext<'a> {
    pub fn regs(&self) -> Ref<[u32; 16]> {
        Ref::map(self.jit.borrow(), |jit| unsafe { dynarmic_regs(jit) })
    }

    pub fn regs_mut(&'a self) -> RefMut<[u32; 16]> {
        RefMut::map(self.jit.borrow_mut(), |jit| unsafe { dynarmic_regs_mut(jit) })
    }

    pub fn extregs(&'a self) -> Ref<[u32; 64]> {
        Ref::map(self.jit.borrow(), |jit| unsafe { dynarmic_extregs(jit) })
    }

    pub fn extregs_mut(&'a self) -> RefMut<[u32; 64]> {
        RefMut::map(self.jit.borrow_mut(), |jit| unsafe { dynarmic_extregs_mut(jit) })
    }

    pub fn cpsr(&'a self) -> u32 {
        unsafe { dynarmic_cpsr(*self.jit.borrow()) }
    }

    pub fn set_cpsr(&'a self, cpsr: u32) {
        unsafe { dynarmic_set_cpsr(*self.jit.borrow(), cpsr) }
    }

    pub fn fpscr(&'a self) -> u32 {
        unsafe { dynarmic_fpscr(*self.jit.borrow()) }
    }

    pub fn set_fpscr(&'a self, fpscr: u32) {
        unsafe { dynarmic_set_fpscr(*self.jit.borrow(), fpscr) }
    }

    pub fn halt(&'a self) {
        unsafe { dynarmic_halt(*self.jit.borrow()) }
    }

    pub fn read<T: memory::Primitive>(&'a self, addr: u32) -> T {
        self.context.memory.borrow().read(addr)
    }

    pub fn write<T: memory::Primitive>(&'a self, addr: u32, value: T) {
        self.context.memory.borrow().write(addr, value)
    }

    pub fn map_memory(&self, addr: u32, pages: u32, read_only: bool) {
        self.context.memory.borrow_mut().map_memory(addr, pages, read_only);
    }
}

impl Context {
    fn from_jit<'a, 'b: 'a>(jit: &'a mut Jit) -> &'b mut Context {
        let ud = unsafe {
            dynarmic_get_userdata(jit)
        };
        unsafe { std::mem::transmute(ud) }
    }

    extern fn read<T: memory::Primitive>(jit: &mut Jit, addr: u32) -> T {
        Context::from_jit(jit).memory.borrow().read(addr)
    }

    extern fn write<T: memory::Primitive>(jit: &mut Jit, addr: u32, value: T) {
        Context::from_jit(jit).memory.borrow().write(addr, value)
    }

    extern fn is_read_only_memory(jit: &mut Jit, addr: u32) -> bool {
        Context::from_jit(jit).memory.borrow().is_read_only(addr)
    }

    extern fn call_svc(jit: &mut Jit, svc: u32) {
        let context = Context::from_jit(jit);
        let mut handler = context.svc_handler.take().expect("No svc handler installed");
        let jit_context = JitContext {
            context, jit: RefCell::new(jit),
        };
        handler.handle_svc(jit_context, svc);
    }

    extern fn exception_raised(jit: &mut Jit, addr: u32, ex: Exception) {
        unimplemented!()
    }

    extern fn add_ticks(jit: &mut Jit, ticks: u64) {
        let ctx = Context::from_jit(jit);
        ctx.ticks = ctx.ticks.saturating_sub(ticks);
    }

    extern fn get_ticks_remaining(jit: &mut Jit) -> u64 {
        let ctx = Context::from_jit(jit);
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

pub struct Executor {
    jit: &'static mut Jit,
    context: Box<Context>,
}

impl Executor {
    pub fn new(svc_handler: Option<Box<SvcHandler>>) -> Executor {
        let mut context = Box::new(Context {
            memory: RefCell::new(memory::Memory::new()),
            svc_handler,
            ticks: std::u64::MAX,
        });

        let callbacks = Context::callbacks();

        let jit = unsafe {
            dynarmic_new(
                context.as_mut() as *mut Context as *mut _,
                &callbacks,
                None
            )
        };

        Executor {
            jit,
            context
        }
    }

    pub fn run(&mut self) {
        unsafe { dynarmic_run(self.jit) }
    }

    pub fn context(&mut self) -> JitContext {
        JitContext {
            jit: RefCell::new(self.jit),
            context: &self.context,
        }
    }
}

impl Drop for Executor {
    fn drop(&mut self) {
        unsafe { dynarmic_delete(self.jit) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn it_works() {
        let mut executor = Executor::new(None);

        {
            let context = executor.context();
            context.map_memory(0x00000000, 1, true);
            context.write(0, 0x0088u16);
            context.write(2, 0xE7FEu16);

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
