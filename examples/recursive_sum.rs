// Compute 1 + 2 + ... + 30 = 465 using recursive SSEND
//
// sum(n):
//   if n <= 0, return 0
//   else return n + sum(n - 1)
//
// Bytecode for sum(n):
//   R[0] = n (argument, not modified)
//   R[1] = temporary / comparison result
//   R[2] = 0 (for LE comparison)
//
//    0: MOVE  R[1], R[0]       R[1] = n
//    1: LOADI R[2], 0          R[2] = 0
//    2: LE    R[1]             R[1] = (n <= 0)
//    3: JMPNOT R[1], 5         if n > 0, goto 5
//    4: RETURN R[2]            return 0 (base case)
//    5: MOVE  R[1], R[0]       R[1] = n
//    6: SUBI  R[1], 1          R[1] = n - 1
//    7: SSEND a=1, b=0, c=0   R[1] = sum(n - 1)
//    8: ADD   R[0]             R[0] = n + sum(n - 1)  (R[0] + R[1])
//
// Wait, ADD R[0] does R[0] = R[0] + R[1]. But R[0] = n (unchanged)
// and R[1] = sum(n-1) after SSEND. So R[0] = n + sum(n-1). Correct!
//
//    9: RETURN R[0]

use future_vm::{Executor, Instruction, Iseq, OpCode, VM, Value};

fn main() {
    let sum_iseq = Iseq {
        name: "sum".into(),
        argc: 1,
        max_regs: 3,
        symbols: vec!["sum".into()],
        instructions: vec![
            Instruction::new(OpCode::Move, 1, 0, 0),   //  0: R[1] = n
            Instruction::new(OpCode::LoadI, 2, 0, 0),  //  1: R[2] = 0
            Instruction::new(OpCode::Le, 1, 0, 0),     //  2: R[1] = (n <= 0)
            Instruction::new(OpCode::JmpNot, 1, 5, 0), //  3: if n > 0, goto 5
            Instruction::new(OpCode::Return, 2, 0, 0),  //  4: return 0
            Instruction::new(OpCode::Move, 1, 0, 0),   //  5: R[1] = n
            Instruction::new(OpCode::SubI, 1, 1, 0),   //  6: R[1] = n - 1
            Instruction::new(OpCode::SSend, 1, 0, 0),  //  7: R[1] = sum(n - 1)
            Instruction::new(OpCode::Add, 0, 0, 0),    //  8: R[0] = n + sum(n - 1)
            Instruction::new(OpCode::Return, 0, 0, 0),  //  9: return result
        ],
    };

    let vm = VM::new(vec![sum_iseq.clone()]);
    let executor = Executor::new();
    let result = executor.run(vm.execute(&sum_iseq, vec![Value::Integer(30)]));
    println!("1 + 2 + ... + 30 = {:?}", result);
}
