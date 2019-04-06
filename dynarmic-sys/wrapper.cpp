#include <array>
#include <cstdint>

#include <dynarmic/A32/a32.h>
#include <dynarmic/A32/config.h>

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

struct JitWrapper {
  void *user_data;
  Dynarmic::A32::Jit jit;

  JitWrapper(void *ud, Dynarmic::A32::UserConfig config) : user_data(ud), jit(config) {}
};

extern "C" JitWrapper *dynarmic_new(void *user_data, RustCallbacks::CallbackData *callbacks, std::array<u8*, Dynarmic::A32::UserConfig::NUM_PAGE_TABLE_ENTRIES> *page_table) {
  auto dynarmicCallbacks = new RustCallbacks();
  dynarmicCallbacks->callbacks = *callbacks;

  auto config = Dynarmic::A32::UserConfig();

  config.callbacks = dynarmicCallbacks;
  config.page_table = page_table;

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

