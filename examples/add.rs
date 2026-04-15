// Compute 1 + 2 = 3
//
// Bytecode:
//   0: LOADI R[1], 1
//   1: LOADI R[2], 2
//   2: ADD   R[1]       R[1] = R[1] + R[2] = 3
//   3: RETURN R[1]

use future_vm::{Executor, Instruction, Iseq, OpCode, VM};

fn main() {
    let iseq = Iseq {
        name: "main".into(),
        argc: 0,
        max_regs: 2,
        symbols: vec![],
        instructions: vec![
            Instruction::new(OpCode::LoadI, 1, 1, 0),
            Instruction::new(OpCode::LoadI, 2, 2, 0),
            Instruction::new(OpCode::Add, 1, 0, 0),
            Instruction::new(OpCode::Return, 1, 0, 0),
        ],
    };

    let vm = VM::new(vec![iseq.clone()]);
    let executor = Executor::new();
    let result = executor.run(vm.execute(&iseq, vec![]));
    println!("1 + 2 = {:?}", result);
}
