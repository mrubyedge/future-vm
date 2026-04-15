// Step through recursive sum(5) = 15, showing each poll result
//
// sum(n):
//   if n <= 0, return 0
//   else return n + sum(n - 1)

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
            Instruction::new(OpCode::Return, 3, 0, 0), //  4: return 0
            Instruction::new(OpCode::Move, 2, 1, 0),   //  5: R[2] = n
            Instruction::new(OpCode::SubI, 2, 1, 0),   //  6: R[2] = n - 1
            Instruction::new(OpCode::SSend, 2, 0, 0),  //  7: R[2] = sum(n - 1)
            Instruction::new(OpCode::Add, 1, 0, 0),    //  8: R[1] = n + sum(n - 1)
            Instruction::new(OpCode::Return, 1, 0, 0), //  9: return result
        ],
    };

    let vm = VM::new(vec![sum_iseq.clone()]);
    let executor = Executor::new();
    let mut future = std::pin::pin!(vm.execute(&sum_iseq, vec![Value::Integer(5)]));

    let mut step = 0;
    loop {
        step += 1;
        match executor.step(future.as_mut()) {
            None => {
                println!("step {:>3}: Pending", step);
            }
            Some(result) => {
                println!("step {:>3}: Ready -> {:?}", step, result);
                break;
            }
        }
    }
}
