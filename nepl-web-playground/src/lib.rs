use thiserror::Error;
use wasmi::{
    Config, Engine, Error as WasmiError, Linker, Module, Store, TrapCode, TypedFunc,
    TypedResumableCall,
};

#[derive(Debug, Error)]
pub enum FuelError {
    #[error("wasm instantiation failed: {0}")]
    Instantiate(#[from] WasmiError),
    #[error("fuel metering is disabled on this engine")]
    FuelDisabled,
    #[error("execution trapped: {0}")]
    Trap(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StepOutcome {
    Finished {
        remaining_fuel: u64,
    },
    OutOfFuel {
        required_fuel: u64,
        remaining_fuel: u64,
    },
}

#[derive(Debug)]
pub struct FuelStepper {
    store: Store<()>,
    _instance: wasmi::Instance,
    entry: TypedFunc<(), ()>,
}

impl FuelStepper {
    pub fn new(wasm: &[u8], entry_export: &str, initial_fuel: u64) -> Result<Self, FuelError> {
        let mut config = Config::default();
        config.consume_fuel(true);
        let engine = Engine::new(&config);
        let module = Module::new(&engine, wasm)?;
        let linker = Linker::new(&engine);
        let mut store = Store::new(&engine, ());
        store
            .set_fuel(initial_fuel)
            .map_err(|_| FuelError::FuelDisabled)?;
        let instance = linker.instantiate_and_start(&mut store, &module)?;
        let entry = instance.get_typed_func::<(), ()>(&store, entry_export)?;

        Ok(Self {
            store,
            _instance: instance,
            entry,
        })
    }

    pub fn add_fuel(&mut self, fuel: u64) -> Result<u64, FuelError> {
        let current = self.store.get_fuel().map_err(|_| FuelError::FuelDisabled)?;
        let updated = current.saturating_add(fuel);
        self.store
            .set_fuel(updated)
            .map_err(|_| FuelError::FuelDisabled)?;
        Ok(updated)
    }

    pub fn run_slice(&mut self) -> Result<StepOutcome, FuelError> {
        let result = self.entry.call_resumable(&mut self.store, ());

        match result {
            Ok(TypedResumableCall::Finished(())) => {
                let remaining = self.store.get_fuel().map_err(|_| FuelError::FuelDisabled)?;
                Ok(StepOutcome::Finished {
                    remaining_fuel: remaining,
                })
            }
            Ok(TypedResumableCall::OutOfFuel(invocation)) => {
                let required_fuel = invocation.required_fuel();
                let remaining = self.store.get_fuel().map_err(|_| FuelError::FuelDisabled)?;
                Ok(StepOutcome::OutOfFuel {
                    required_fuel,
                    remaining_fuel: remaining,
                })
            }
            Ok(TypedResumableCall::HostTrap(trap)) => {
                Err(FuelError::Trap(trap.host_error().to_string()))
            }
            Err(error) => {
                let trap = match error.as_trap_code() {
                    Some(TrapCode::OutOfFuel) => "out of fuel".to_string(),
                    Some(code) => format!("trap: {code:?}"),
                    None => error.to_string(),
                };
                Err(FuelError::Trap(trap))
            }
        }
    }

    pub fn clear_pending(&mut self) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use wasm_encoder::{
        BlockType, CodeSection, ExportKind, ExportSection, Function, FunctionSection, Instruction,
        Module, TypeSection, ValType,
    };

    #[test]
    fn pauses_infinite_loops_without_trapping() {
        let wasm = spin_loop_module();

        let mut stepper = FuelStepper::new(&wasm, "spin", 1).expect("stepper");
        let first = stepper.run_slice().expect("outcome");
        match first {
            StepOutcome::OutOfFuel { .. } => {}
            other => panic!("expected out of fuel, got {other:?}"),
        }

        let _ = stepper.add_fuel(5).expect("added fuel");
        let second = stepper.run_slice().expect("outcome");
        match second {
            StepOutcome::OutOfFuel { .. } => {}
            StepOutcome::Finished { .. } => panic!("infinite loop should not finish"),
        }

        stepper.clear_pending();
        let reset = stepper.run_slice();
        assert!(reset.is_ok(), "reset call should re-enter execution");
    }

    #[test]
    fn completes_after_replenishing_fuel() {
        let wasm = countdown_module();

        let mut stepper = FuelStepper::new(&wasm, "count", 1).expect("stepper");
        let first = stepper.run_slice().expect("outcome");
        match first {
            StepOutcome::OutOfFuel { required_fuel, .. } => {
                assert!(required_fuel > 0, "required fuel should be positive");
            }
            other => panic!("expected out of fuel, got {other:?}"),
        }

        let _ = stepper.add_fuel(500).expect("added fuel");
        let mut outcome = stepper.run_slice().expect("outcome");
        for _ in 0..3 {
            if let StepOutcome::Finished { .. } = outcome {
                break;
            }
            stepper.add_fuel(200).expect("added more fuel");
            outcome = stepper.run_slice().expect("outcome");
        }

        match outcome {
            StepOutcome::Finished { remaining_fuel } => {
                assert!(remaining_fuel > 0, "fuel should remain after completion");
            }
            other => panic!("expected completion, got {other:?}"),
        }
    }

    fn spin_loop_module() -> Vec<u8> {
        let mut types = TypeSection::new();
        types.ty().function([], []);

        let mut functions = FunctionSection::new();
        functions.function(0);

        let mut exports = ExportSection::new();
        exports.export("spin", ExportKind::Func, 0);

        let mut code = CodeSection::new();
        let mut func = Function::new([]);
        func.instruction(&Instruction::Loop(BlockType::Empty));
        func.instruction(&Instruction::Br(0));
        func.instruction(&Instruction::End);
        func.instruction(&Instruction::End);
        code.function(&func);

        let mut module = Module::new();
        module.section(&types);
        module.section(&functions);
        module.section(&exports);
        module.section(&code);
        module.finish()
    }

    fn countdown_module() -> Vec<u8> {
        let mut types = TypeSection::new();
        types.ty().function([], []);

        let mut functions = FunctionSection::new();
        functions.function(0);

        let mut exports = ExportSection::new();
        exports.export("count", ExportKind::Func, 0);

        let mut code = CodeSection::new();
        let mut func = Function::new([(1, ValType::I32)]);
        func.instruction(&Instruction::I32Const(3));
        func.instruction(&Instruction::LocalSet(0));
        func.instruction(&Instruction::Loop(BlockType::Empty));
        func.instruction(&Instruction::LocalGet(0));
        func.instruction(&Instruction::I32Eqz);
        func.instruction(&Instruction::BrIf(1));
        func.instruction(&Instruction::LocalGet(0));
        func.instruction(&Instruction::I32Const(1));
        func.instruction(&Instruction::I32Sub);
        func.instruction(&Instruction::LocalSet(0));
        func.instruction(&Instruction::Br(0));
        func.instruction(&Instruction::End);
        func.instruction(&Instruction::End);
        code.function(&func);

        let mut module = Module::new();
        module.section(&types);
        module.section(&functions);
        module.section(&exports);
        module.section(&code);
        module.finish()
    }
}
