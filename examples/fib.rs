// Compute fib(10) = 55 using recursive SSEND
//
// fib(n):
//   if n <= 1, return n
//   else return fib(n - 1) + fib(n - 2)
//
// Bytecode for fib(n):
//   R[1] = n (argument, not modified)
//   R[2] = temporary / result of fib(n-1)
//   R[3] = temporary / result of fib(n-2)
//
//    0: MOVE  R[2], R[1]       R[2] = n
//    1: LOADI R[3], 1          R[3] = 1
//    2: LE    R[2]             R[2] = (n <= 1)
//    3: JMPNOT R[2], 5         if n > 1, goto 5
//    4: RETURN R[1]            base case: return n
//    5: MOVE  R[2], R[1]       R[2] = n
//    6: SUBI  R[2], 1          R[2] = n - 1
//    7: SSEND a=2, b=0, c=0   R[2] = fib(n - 1)
//    8: MOVE  R[3], R[1]       R[3] = n
//    9: SUBI  R[3], 2          R[3] = n - 2
//   10: SSEND a=3, b=0, c=0   R[3] = fib(n - 2)
//   11: ADD   R[2]             R[2] = fib(n-1) + fib(n-2)
//   12: RETURN R[2]

use future_vm::{Executor, Instruction, Iseq, OpCode, VM, Value};

fn main() {
    let fib_iseq = Iseq {
        name: "fib".into(),
        argc: 1,
        max_regs: 3,
        symbols: vec!["fib".into()],
        instructions: vec![
            Instruction::new(OpCode::Move, 2, 1, 0),      //  0
            Instruction::new(OpCode::LoadI, 3, 1, 0),     //  1
            Instruction::new(OpCode::Le, 2, 0, 0),        //  2
            Instruction::new(OpCode::JmpNot, 2, 5, 0),    //  3
            Instruction::new(OpCode::Return, 1, 0, 0),    //  4
            Instruction::new(OpCode::Move, 2, 1, 0),      //  5
            Instruction::new(OpCode::SubI, 2, 1, 0),      //  6
            Instruction::new(OpCode::SSend, 2, 0, 0),     //  7
            Instruction::new(OpCode::Move, 3, 1, 0),      //  8
            Instruction::new(OpCode::SubI, 3, 2, 0),      //  9
            Instruction::new(OpCode::SSend, 3, 0, 0),     // 10
            Instruction::new(OpCode::Add, 2, 0, 0),       // 11
            Instruction::new(OpCode::Return, 2, 0, 0),    // 12
        ],
    };

    for i in [10, 20, 30] {
        let vm = VM::new(vec![fib_iseq.clone()]);
        let executor = Executor::new();
        let result = executor.run(vm.execute(&fib_iseq, vec![Value::Integer(i)]));
        println!("fib({}) = {:?}", i, result);
    }
}
