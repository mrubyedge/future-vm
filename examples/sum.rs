// Compute 1 + 2 + ... + 30 = 465 using recursive SSEND
//
// sum(n):
//   if n <= 0, return 0
//   else return n + sum(n - 1)
//
// Bytecode for sum(n):
//   R[1] = n (argument, not modified)
//   R[2] = temporary / comparison result
//   R[3] = 0 (for LE comparison / base case return)
//
//    0: MOVE  R[2], R[1]       R[2] = n
//    1: LOADI R[3], 0          R[3] = 0
//    2: LE    R[2]             R[2] = (n <= 0)
//    3: JMPNOT R[2], 5         if n > 0, goto 5
//    4: RETURN R[3]            return 0 (base case)
//    5: MOVE  R[2], R[1]       R[2] = n
//    6: SUBI  R[2], 1          R[2] = n - 1
//    7: SSEND a=2, b=0, c=0   R[2] = sum(n - 1)
//    8: ADD   R[1]             R[1] = n + sum(n - 1)  (R[1] + R[2])
//    9: RETURN R[1]

use future_vm::{Executor, Instruction, Iseq, OpCode, VM, Value};

fn main() {
    let sum_iseq = Iseq {
        name: "sum".into(),
        argc: 1,
        max_regs: 3,
        symbols: vec!["sum".into()],
        instructions: vec![
            Instruction::new(OpCode::Move, 2, 1, 0),   //  0: R[2] = n
            Instruction::new(OpCode::LoadI, 3, 0, 0),  //  1: R[3] = 0
            Instruction::new(OpCode::Le, 2, 0, 0),     //  2: R[2] = (n <= 0)
            Instruction::new(OpCode::JmpNot, 2, 5, 0), //  3: if n > 0, goto 5
            Instruction::new(OpCode::Return, 3, 0, 0),  //  4: return 0
            Instruction::new(OpCode::Move, 2, 1, 0),   //  5: R[2] = n
            Instruction::new(OpCode::SubI, 2, 1, 0),   //  6: R[2] = n - 1
            Instruction::new(OpCode::SSend, 2, 0, 0),  //  7: R[2] = sum(n - 1)
            Instruction::new(OpCode::Add, 1, 0, 0),    //  8: R[1] = n + sum(n - 1)
            Instruction::new(OpCode::Return, 1, 0, 0),  //  9: return result
        ],
    };

    let vm = VM::new(vec![sum_iseq.clone()]);
    let executor = Executor::new();
    let result = executor.run(vm.execute(&sum_iseq, vec![Value::Integer(30)]));
    println!("1 + 2 + ... + 30 = {:?}", result);
}
