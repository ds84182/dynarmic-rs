#include <array>
#include <cstdint>
#include <optional>

#include <dynarmic/A32/a32.h>
#include <dynarmic/A32/config.h>
#include <dynarmic/A32/coprocessor.h>

using u8 = std::uint8_t;
using u16 = std::uint16_t;
using u32 = std::uint32_t;
using u64 = std::uint64_t;

struct JitWrapper;

class RustCallbacks : public Dynarmic::A32::UserCallbacks {
public:
  using Jit = JitWrapper;

  template <typename T>
  using MemoryReadCB = T(*)(Jit*, u32);
  template <typename T>
  using MemoryWriteCB = void(*)(Jit*, u32, T);
  using IsReadOnlyMemoryCB = bool(*)(Jit*, u32);
  using CallSVCCB = void(*)(Jit*, u32);
  using ExceptionRaisedCB = void(*)(Jit*, u32, Dynarmic::A32::Exception);
  using AddTicksCB = void(*)(Jit*, u64);
  using GetTicksRemainingCB = u64(*)(Jit*);

  struct CallbackData {
    MemoryReadCB<u8> Read8;
    MemoryReadCB<u16> Read16;
    MemoryReadCB<u32> Read32;
    MemoryReadCB<u64> Read64;

    MemoryWriteCB<u8> Write8;
    MemoryWriteCB<u16> Write16;
    MemoryWriteCB<u32> Write32;
    MemoryWriteCB<u64> Write64;

    IsReadOnlyMemoryCB IsReadOnlyMemory;
    CallSVCCB CallSVC;
    ExceptionRaisedCB ExceptionRaised;
    AddTicksCB AddTicks;
    GetTicksRemainingCB GetTicksRemaining;
  };

  u8 MemoryRead8(u32 vaddr) override {
    return callbacks.Read8(jit, vaddr);
  }

  u16 MemoryRead16(u32 vaddr) override {
    return callbacks.Read16(jit, vaddr);
  }

  u32 MemoryRead32(u32 vaddr) override {
    return callbacks.Read32(jit, vaddr);
  }

  u64 MemoryRead64(u32 vaddr) override {
    return callbacks.Read64(jit, vaddr);
  }

  void MemoryWrite8(u32 vaddr, u8 value) override {
    return callbacks.Write8(jit, vaddr, value);
  }

  void MemoryWrite16(u32 vaddr, u16 value) override {
    return callbacks.Write16(jit, vaddr, value);
  }

  void MemoryWrite32(u32 vaddr, u32 value) override {
    return callbacks.Write32(jit, vaddr, value);
  }

  void MemoryWrite64(u32 vaddr, u64 value) override {
    return callbacks.Write64(jit, vaddr, value);
  }

  bool IsReadOnlyMemory(u32 vaddr) override {
    return callbacks.IsReadOnlyMemory ? callbacks.IsReadOnlyMemory(jit, vaddr) : false;
  }

  void InterpreterFallback(u32, std::size_t) override {
    abort();
  }

  void CallSVC(u32 swi) override {
    callbacks.CallSVC(jit, swi);
  }

  void ExceptionRaised(u32 pc, Dynarmic::A32::Exception exception) override {
    callbacks.ExceptionRaised(jit, pc, exception);
  }

  void AddTicks(u64 ticks) override {
    callbacks.AddTicks(jit, ticks);
  }

  u64 GetTicksRemaining() override {
    return callbacks.GetTicksRemaining(jit);
  }

  CallbackData callbacks;
  Jit* jit = nullptr;
};

class RustCoprocessor : public Dynarmic::A32::Coprocessor {
public:
  using Jit = Dynarmic::A32::Jit; // Interchangable with JitContext

  using RawCallbackFn = u64(*)(Jit*, void*, u32, u32);

  struct RawCallback {
    RawCallbackFn func;
    void *user_data;
  };

  enum class CallbackOrAccessTag {
    None, Callback, Access
  };

  template <typename T>
  struct CallbackOrAccess {
    CallbackOrAccessTag tag;
    union {
      RawCallback callback;
      T access;
    };
  };

  using CoprocReg = Dynarmic::A32::CoprocReg;

  template <typename T>
  struct Option {
    bool some;
    T value;

    static Option<T> Some(T value) {
      Option v;
      v.some = true;
      v.value = value;
      return v;
    }

    static Option<T> None() {
      Option v;
      v.some = false;
      return v;
    }
  };

  using Callback = Option<RawCallback>; // except nullable

  struct CallbackData {
    void *user_data;
    Callback (*compile_internal_operation)(const void *self, bool two, u32 opc1, CoprocReg cr_d, CoprocReg cr_n, CoprocReg cr_m, u32 opc2);
    CallbackOrAccess<u32*> (*compile_send_one_word)(const void *self, bool two, u32 opc1, CoprocReg cr_n, CoprocReg cr_m, u32 opc2);
    CallbackOrAccess<std::array<u32*, 2>> (*compile_send_two_words)(const void *self, bool two, u32 opc, CoprocReg cr_m);
    CallbackOrAccess<const u32*> (*compile_get_one_word)(const void *self, bool two, u32 opc1, CoprocReg cr_n, CoprocReg cr_m, u32 opc2);
    CallbackOrAccess<std::array<const u32*, 2>> (*compile_get_two_words)(const void *self, bool two, u32 opc, CoprocReg cr_m);
    Callback (*compile_load_words)(const void *self, bool two, bool long_transfer, CoprocReg cr_d, Option<u8> option);
    Callback (*compile_store_words)(const void *self, bool two, bool long_transfer, CoprocReg cr_d, Option<u8> option);
    void (*destroy)(void *self);
  };

  CallbackData callback_data;

  using Cp = Dynarmic::A32::Coprocessor;

  RustCoprocessor(CallbackData *cd) : callback_data(*cd) {}

  virtual ~RustCoprocessor() {
    callback_data.destroy(callback_data.user_data);
  }

  virtual std::optional<Cp::Callback> CompileInternalOperation(bool two, unsigned opc1, CoprocReg CRd, CoprocReg CRn, CoprocReg CRm, unsigned opc2) override {
    auto cb = callback_data.compile_internal_operation(callback_data.user_data, two, opc1, CRd, CRn, CRm, opc2);

    if (cb.some) {
      return std::optional(Cp::Callback {
        cb.value.func,
        cb.value.user_data,
      });
    } else {
      return std::nullopt;
    }
  }

  virtual Cp::CallbackOrAccessOneWord CompileSendOneWord(bool two, unsigned opc1, CoprocReg CRn, CoprocReg CRm, unsigned opc2) override {
    auto cb = callback_data.compile_send_one_word(callback_data.user_data, two, opc1, CRn, CRm, opc2);

    switch (cb.tag) {
      case CallbackOrAccessTag::None:
        return boost::blank {};
      case CallbackOrAccessTag::Callback:
        return Cp::Callback {
          cb.callback.func,
          cb.callback.user_data
        };
      case CallbackOrAccessTag::Access:
        return cb.access;
    }
  }

  virtual Cp::CallbackOrAccessTwoWords CompileSendTwoWords(bool two, unsigned opc, CoprocReg CRm) override {
    auto cb = callback_data.compile_send_two_words(callback_data.user_data, two, opc, CRm);

    switch (cb.tag) {
      case CallbackOrAccessTag::None:
        return boost::blank {};
      case CallbackOrAccessTag::Callback:
        return Cp::Callback {
          cb.callback.func,
          cb.callback.user_data
        };
      case CallbackOrAccessTag::Access:
        return cb.access;
    }
  }

  virtual Cp::CallbackOrAccessOneWord CompileGetOneWord(bool two, unsigned opc1, CoprocReg CRn, CoprocReg CRm, unsigned opc2) override {
    auto cb = callback_data.compile_get_one_word(callback_data.user_data, two, opc1, CRn, CRm, opc2);

    switch (cb.tag) {
      case CallbackOrAccessTag::None:
        return boost::blank {};
      case CallbackOrAccessTag::Callback:
        return Cp::Callback {
          cb.callback.func,
          cb.callback.user_data
        };
      case CallbackOrAccessTag::Access:
        return const_cast<u32*>(cb.access);
    }
  }

  virtual Cp::CallbackOrAccessTwoWords CompileGetTwoWords(bool two, unsigned opc, CoprocReg CRm) override {
    auto cb = callback_data.compile_get_two_words(callback_data.user_data, two, opc, CRm);

    switch (cb.tag) {
      case CallbackOrAccessTag::None:
        return boost::blank {};
      case CallbackOrAccessTag::Callback:
        return Cp::Callback {
          cb.callback.func,
          cb.callback.user_data
        };
      case CallbackOrAccessTag::Access:
        return *reinterpret_cast<std::array<u32*, 2>*>(&cb.access);
    }
  }

  virtual std::optional<Cp::Callback> CompileLoadWords(bool two, bool long_transfer, CoprocReg CRd, std::optional<std::uint8_t> option) override {
    auto cb = callback_data.compile_load_words(callback_data.user_data, two, long_transfer, CRd, option.has_value() ? Option<u8>::Some(option.value()) : Option<u8>::None());

    if (cb.some) {
      return std::optional(Cp::Callback {
        cb.value.func,
        cb.value.user_data,
      });
    } else {
      return std::nullopt;
    }
  }

  virtual std::optional<Cp::Callback> CompileStoreWords(bool two, bool long_transfer, CoprocReg CRd, std::optional<std::uint8_t> option) override {
    auto cb = callback_data.compile_store_words(callback_data.user_data, two, long_transfer, CRd, option.has_value() ? Option<u8>::Some(option.value()) : Option<u8>::None());

    if (cb.some) {
      return std::optional(Cp::Callback {
        cb.value.func,
        cb.value.user_data,
      });
    } else {
      return std::nullopt;
    }
  }
};

struct JitWrapper {
  // JIT structure needs to be first so we can convert Jit* into JitWrapper*
  Dynarmic::A32::Jit jit;
  void *user_data;

  JitWrapper(void *ud, Dynarmic::A32::UserConfig config) : user_data(ud), jit(config) {}
};

extern "C" JitWrapper *dynarmic_new(void *user_data, RustCallbacks::CallbackData *callbacks, std::array<u8*, Dynarmic::A32::UserConfig::NUM_PAGE_TABLE_ENTRIES> *page_table, std::array<RustCoprocessor::CallbackData*, 16> *coprocessors) {
  auto dynarmicCallbacks = new RustCallbacks();
  dynarmicCallbacks->callbacks = *callbacks;

  auto config = Dynarmic::A32::UserConfig();

  config.callbacks = dynarmicCallbacks;
  config.page_table = page_table;

  if (coprocessors) {
    for (int i=0; i<16; i++) {
      auto cp = coprocessors->at(i);
      if (cp) {
        config.coprocessors[i] = std::make_shared<RustCoprocessor>(cp);
      }
    }
  }

  auto jit = new JitWrapper(
    user_data,
    config
  );

  dynarmicCallbacks->jit = jit;

  return jit;
}

extern "C" void dynarmic_delete(JitWrapper *w) {
  delete w;
}

extern "C" void *dynarmic_get_userdata(JitWrapper *w) {
  return w->user_data;
}

extern "C" void dynarmic_run(JitWrapper *w) {
  w->jit.Run();
}

extern "C" u32 *dynarmic_regs(JitWrapper *w) {
  return w->jit.Regs().data();
}

extern "C" u32 *dynarmic_extregs(JitWrapper *w) {
  return w->jit.ExtRegs().data();
}

extern "C" u32 dynarmic_cpsr(JitWrapper *w) {
  return w->jit.Cpsr();
}

extern "C" void dynarmic_set_cpsr(JitWrapper *w, u32 cpsr) {
  w->jit.SetCpsr(cpsr);
}

extern "C" u32 dynarmic_fpscr(JitWrapper *w) {
  return w->jit.Fpscr();
}

extern "C" void dynarmic_set_fpscr(JitWrapper *w, u32 fpscr) {
  w->jit.SetFpscr(fpscr);
}

extern "C" void dynarmic_halt(JitWrapper *w) {
  w->jit.HaltExecution();
}

