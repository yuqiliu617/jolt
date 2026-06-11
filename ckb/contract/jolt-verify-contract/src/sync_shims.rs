//! `__sync_*` libcall shims for `+forced-atomics`.
//!
//! With `-C target-feature=-a,+forced-atomics`, LLVM lowers atomic RMW ops to
//! `__sync_*` libcalls (while plain atomic load/store become `__atomic_*`,
//! which ckb-std's `dummy-atomic` provides). ckb-vm is single-threaded, so
//! plain load-modify-store is sufficient.

#![expect(
    clippy::missing_safety_doc,
    reason = "ABI shims; callers are compiler-generated"
)]

#[unsafe(no_mangle)]
pub unsafe extern "C" fn __sync_fetch_and_add_8(ptr: *mut u64, val: u64) -> u64 {
    let old = *ptr;
    *ptr = old.wrapping_add(val);
    old
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn __sync_fetch_and_sub_8(ptr: *mut u64, val: u64) -> u64 {
    let old = *ptr;
    *ptr = old.wrapping_sub(val);
    old
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn __sync_val_compare_and_swap_4(
    ptr: *mut u32,
    expected: u32,
    desired: u32,
) -> u32 {
    let old = *ptr;
    if old == expected {
        *ptr = desired;
    }
    old
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn __sync_val_compare_and_swap_8(
    ptr: *mut u64,
    expected: u64,
    desired: u64,
) -> u64 {
    let old = *ptr;
    if old == expected {
        *ptr = desired;
    }
    old
}
