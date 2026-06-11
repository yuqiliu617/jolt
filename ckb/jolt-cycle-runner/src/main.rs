//! Runs a CKB script ELF under ckb-vm (interpreter64, `estimate_cycles` cost
//! model — the same model CKB nodes charge) and prints debug-syscall output
//! plus the total cycle count.
//!
//! Implements two syscalls the bench binary uses:
//! - 2177 (debug): prints the message to stdout
//! - 2042 (current_cycles): returns cycles consumed so far

use ckb_vm::cost_model::estimate_cycles;
use ckb_vm::registers::{A0, A7};
use ckb_vm::{
    Bytes, DefaultMachineRunner, Memory, Register, SupportMachine, Syscalls,
};

struct DebugSyscall;

impl<Mac: SupportMachine> Syscalls<Mac> for DebugSyscall {
    fn initialize(&mut self, _machine: &mut Mac) -> Result<(), ckb_vm::error::Error> {
        Ok(())
    }

    fn ecall(&mut self, machine: &mut Mac) -> Result<bool, ckb_vm::error::Error> {
        if machine.registers()[A7].to_i32() != 2177 {
            return Ok(false);
        }

        let mut addr = machine.registers()[A0].to_u64();
        let mut buffer = Vec::new();
        loop {
            let byte = machine
                .memory_mut()
                .load8(&Mac::REG::from_u64(addr))?
                .to_u8();
            if byte == 0 {
                break;
            }
            buffer.push(byte);
            addr += 1;
        }

        println!("{}", String::from_utf8_lossy(&buffer));
        Ok(true)
    }
}

struct CurrentCycles;

impl<Mac: SupportMachine> Syscalls<Mac> for CurrentCycles {
    fn initialize(&mut self, _machine: &mut Mac) -> Result<(), ckb_vm::error::Error> {
        Ok(())
    }

    fn ecall(&mut self, machine: &mut Mac) -> Result<bool, ckb_vm::error::Error> {
        if machine.registers()[A7].to_u64() != 2042 {
            return Ok(false);
        }
        // Mirror the node: the syscall itself costs 500 cycles, charged before
        // the value is read.
        machine.add_cycles(500)?;
        let cycles = machine.cycles();
        machine.set_register(A0, Mac::REG::from_u64(cycles));
        Ok(true)
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args()
        .nth(1)
        .ok_or("usage: jolt-cycle-runner <riscv64-elf>")?;
    let code: Bytes = std::fs::read(&path)?.into();

    let core_machine = ckb_vm::DefaultCoreMachine::<u64, ckb_vm::SparseMemory<u64>>::new(
        ckb_vm::ISA_IMC | ckb_vm::ISA_B | ckb_vm::ISA_MOP,
        ckb_vm::machine::VERSION2,
        u64::MAX,
    );
    let mut machine = ckb_vm::RustDefaultMachineBuilder::new(core_machine)
        .instruction_cycle_func(Box::new(estimate_cycles))
        .syscall(Box::new(DebugSyscall))
        .syscall(Box::new(CurrentCycles))
        .build();
    machine.load_program(&code, [Ok(Bytes::from("bench"))].into_iter())?;

    let exit = machine.run();
    println!("exit={exit:?} total_cycles={}", machine.cycles());
    std::process::exit(exit? as i32);
}
