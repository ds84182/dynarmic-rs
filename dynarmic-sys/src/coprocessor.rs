use super::Jit;
use std::ffi::c_void;
use std::marker::PhantomData;
use std::cell::Cell;

pub type RawCallbackFn = extern fn(&mut Jit, user_arg: *mut c_void, arg0: u32, arg1: u32) -> u64;

#[repr(C)]
pub struct RawCallback<'a> {
    func: RawCallbackFn,
    user_arg: *mut c_void,
    _phantom: PhantomData<&'a ()>,
}

extern fn callback_handler_delegator<'jit, J: for<'a> From<&'a mut Jit>, C: 'jit, H: CallbackHandler<J, C>>(jit: &mut Jit, user_arg: *mut c_void, arg0: u32, arg1: u32) -> u64 {
    let context = unsafe { &*(user_arg as *const C) };
    H::handle(jit.into(), context, arg0, arg1)
}

pub trait CallbackHandler<J, C> {
    fn handle(jit: J, context: &C, arg0: u32, arg1: u32) -> u64;
}

impl RawCallback<'_> {
    pub fn from<'jit, J: for<'a> From<&'a mut Jit>, C: 'jit, H: CallbackHandler<J, C>>(context: &'jit C) -> RawCallback<'jit> {
        RawCallback {
            func: callback_handler_delegator::<J, C, H>,
            user_arg: context as *const _ as *mut c_void,
            _phantom: PhantomData,
        }
    }
}

#[repr(C)]
pub enum CallbackOrAccess<'jit, T: 'jit> {
    None,
    Callback(RawCallback<'jit>),
    Access(T)
}

#[repr(C)]
pub enum Callback<'jit> {
    None,
    Some(RawCallback<'jit>)
}

#[repr(C)]
enum FFIOption<T> {
    None,
    Some(T)
}

impl<T> From<Option<T>> for FFIOption<T> {
    fn from(value: Option<T>) -> Self {
        match value {
            Some(v) => FFIOption::Some(v),
            None => FFIOption::None,
        }
    }
}

impl<T> Into<Option<T>> for FFIOption<T> {
    fn into(self) -> Option<T> {
        match self {
            FFIOption::Some(v) => Some(v),
            FFIOption::None => None
        }
    }
}

pub type CallbackOrAccessOneWord<'jit> = CallbackOrAccess<'jit, &'jit Cell<u32>>;
pub type CallbackOrAccessOneWordMut<'jit> = CallbackOrAccess<'jit, &'jit Cell<u32>>;
pub type CallbackOrAccessTwoWords<'jit> = CallbackOrAccess<'jit, [&'jit Cell<u32>; 2]>;
pub type CallbackOrAccessTwoWordsMut<'jit> = CallbackOrAccess<'jit, [&'jit Cell<u32>; 2]>;

#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum CoprocReg {
    C0, C1, C2, C3, C4, C5, C6, C7, C8, C9, C10, C11, C12, C13, C14, C15
}

#[allow(unused_variables)]
pub trait Coprocessor<'jit> {
    fn compile_internal_operation(&'jit self, two: bool, opc1: u32, cr_d: CoprocReg, cr_n: CoprocReg, cr_m: CoprocReg, opc2: u32) -> Callback<'jit> {
        Callback::None
    }

    fn compile_send_one_word(&'jit self, two: bool, opc1: u32, cr_n: CoprocReg, cr_m: CoprocReg, opc2: u32) -> CallbackOrAccessOneWordMut<'jit> {
        CallbackOrAccess::None
    }

    fn compile_send_two_words(&'jit self, two: bool, opc: u32, cr_m: CoprocReg) -> CallbackOrAccessTwoWordsMut<'jit> {
        CallbackOrAccess::None
    }

    fn compile_get_one_word(&'jit self, two: bool, opc1: u32, cr_n: CoprocReg, cr_m: CoprocReg, opc2: u32) -> CallbackOrAccessOneWord<'jit> {
        CallbackOrAccess::None
    }

    fn compile_get_two_words(&'jit self, two: bool, opc: u32, cr_m: CoprocReg) -> CallbackOrAccessTwoWords<'jit> {
        CallbackOrAccess::None
    }

    fn compile_load_words(&'jit self, two: bool, long_transfer: bool, cr_d: CoprocReg, option: Option<u8>) -> Callback<'jit> {
        Callback::None
    }

    fn compile_store_words(&'jit self, two: bool, long_transfer: bool, cr_d: CoprocReg, option: Option<u8>) -> Callback<'jit> {
        Callback::None
    }
}

#[repr(C)]
pub struct CoprocessorCallbacks<'jit> {
    this: *mut c_void,
    compile_internal_operation: extern fn(this: *const c_void, two: bool, opc1: u32, cr_d: CoprocReg, cr_n: CoprocReg, cr_m: CoprocReg, opc2: u32) -> Callback<'jit>,
    compile_send_one_word: extern fn(this: *const c_void, two: bool, opc1: u32, cr_n: CoprocReg, cr_m: CoprocReg, opc2: u32) -> CallbackOrAccessOneWordMut<'jit>,
    compile_send_two_words: extern fn(this: *const c_void, two: bool, opc: u32, cr_m: CoprocReg) -> CallbackOrAccessTwoWordsMut<'jit>,
    compile_get_one_word: extern fn(this: *const c_void, two: bool, opc1: u32, cr_n: CoprocReg, cr_m: CoprocReg, opc2: u32) -> CallbackOrAccessOneWord<'jit>,
    compile_get_two_words: extern fn(this: *const c_void, two: bool, opc: u32, cr_m: CoprocReg) -> CallbackOrAccessTwoWords<'jit>,
    compile_load_words: extern fn(this: *const c_void, two: bool, long_transfer: bool, cr_d: CoprocReg, option: FFIOption<u8>) -> Callback<'jit>,
    compile_store_words: extern fn(this: *const c_void, two: bool, long_transfer: bool, cr_d: CoprocReg, option: FFIOption<u8>) -> Callback<'jit>,
    destroy: extern fn(this: *mut c_void),
}

impl<'jit> CoprocessorCallbacks<'jit> {
    pub fn callbacks_from<T: Coprocessor<'jit> + 'jit>(coproc: Box<T>) -> Self {
        extern fn compile_internal_operation<'jit, T: Coprocessor<'jit> + 'jit>(this: *const c_void, two: bool, opc1: u32, cr_d: CoprocReg, cr_n: CoprocReg, cr_m: CoprocReg, opc2: u32) -> Callback<'jit> {
            unsafe { &*(this as *const T) }.compile_internal_operation(two, opc1, cr_d, cr_n, cr_m, opc2)
        }

        extern fn compile_send_one_word<'jit, T: Coprocessor<'jit> + 'jit>(this: *const c_void, two: bool, opc1: u32, cr_n: CoprocReg, cr_m: CoprocReg, opc2: u32) -> CallbackOrAccessOneWordMut<'jit> {
            unsafe { &*(this as *const T) }.compile_send_one_word(two, opc1, cr_n, cr_m, opc2)
        }

        extern fn compile_send_two_words<'jit, T: Coprocessor<'jit> + 'jit>(this: *const c_void, two: bool, opc: u32, cr_m: CoprocReg) -> CallbackOrAccessTwoWordsMut<'jit> {
            unsafe { &*(this as *const T) }.compile_send_two_words(two, opc, cr_m)
        }

        extern fn compile_get_one_word<'jit, T: Coprocessor<'jit> + 'jit>(this: *const c_void, two: bool, opc1: u32, cr_n: CoprocReg, cr_m: CoprocReg, opc2: u32) -> CallbackOrAccessOneWord<'jit> {
            unsafe { &*(this as *const T) }.compile_get_one_word(two, opc1, cr_n, cr_m, opc2)
        }

        extern fn compile_get_two_words<'jit, T: Coprocessor<'jit> + 'jit>(this: *const c_void, two: bool, opc: u32, cr_m: CoprocReg) -> CallbackOrAccessTwoWords<'jit> {
            unsafe { &*(this as *const T) }.compile_get_two_words(two, opc, cr_m)
        }

        extern fn compile_load_words<'jit, T: Coprocessor<'jit> + 'jit>(this: *const c_void, two: bool, long_transfer: bool, cr_d: CoprocReg, option: FFIOption<u8>) -> Callback<'jit> {
            unsafe { &*(this as *const T) }.compile_load_words(two, long_transfer, cr_d, option.into())
        }

        extern fn compile_store_words<'jit, T: Coprocessor<'jit> + 'jit>(this: *const c_void, two: bool, long_transfer: bool, cr_d: CoprocReg, option: FFIOption<u8>) -> Callback<'jit> {
            unsafe { &*(this as *const T) }.compile_store_words(two, long_transfer, cr_d, option.into())
        }

        extern fn destroy<'jit, T: Coprocessor<'jit>>(this: *mut c_void) {
            unsafe { Box::from_raw(this as *mut T); }
        }

        CoprocessorCallbacks {
            this: Box::into_raw(coproc) as *mut c_void,
            compile_internal_operation: compile_internal_operation::<T>,
            compile_send_one_word: compile_send_one_word::<T>,
            compile_send_two_words: compile_send_two_words::<T>,
            compile_get_one_word: compile_get_one_word::<T>,
            compile_get_two_words: compile_get_two_words::<T>,
            compile_load_words: compile_load_words::<T>,
            compile_store_words: compile_store_words::<T>,
            destroy: destroy::<T>,
        }
    }
}
