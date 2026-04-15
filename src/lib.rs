use std::future::Future;
use std::task::{Context, Poll, Waker};

use futures::future::BoxFuture;
use futures::FutureExt;

/// Opcodes for the VM
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpCode {
    Move,
    LoadI,
    Add,
    AddI,
    Sub,
    SubI,
    Le,
    Jmp,
    JmpNot,
    SSend,
    Return,
}

/// A single instruction with up to 3 operands (a, b, c)
#[derive(Debug, Clone, Copy)]
pub struct Instruction {
    pub op: OpCode,
    pub a: i32,
    pub b: i32,
    pub c: i32,
}

impl Instruction {
    pub fn new(op: OpCode, a: i32, b: i32, c: i32) -> Self {
        Self { op, a, b, c }
    }
}

/// Instruction sequence (bytecode and metadata for a function)
#[derive(Debug, Clone)]
pub struct Iseq {
    /// Function name
    pub name: String,
    /// Number of arguments
    pub argc: usize,
    /// Maximum number of registers used
    pub max_regs: usize,
    /// Symbol table used internally
    pub symbols: Vec<String>,
    /// Instruction list
    pub instructions: Vec<Instruction>,
}

/// Runtime value
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Integer(i64),
    Bool(bool),
    Nil,
}

impl Value {
    fn as_integer(&self) -> i64 {
        match self {
            Value::Integer(n) => *n,
            _ => panic!("expected integer, got {:?}", self),
        }
    }

    fn is_falsy(&self) -> bool {
        matches!(self, Value::Bool(false) | Value::Nil)
    }
}

/// Register-based virtual machine
pub struct VM {
    iseqs: Vec<Iseq>,
}

impl VM {
    pub fn new(iseqs: Vec<Iseq>) -> Self {
        Self { iseqs }
    }

    /// Execute an instruction sequence asynchronously.
    /// Returns a BoxFuture to support recursive calls via SSEND.
    pub fn execute<'a>(&'a self, iseq: &'a Iseq, args: Vec<Value>) -> BoxFuture<'a, Value> {
        async move {
            let mut regs = vec![Value::Nil; iseq.max_regs];
            for (i, arg) in args.into_iter().enumerate() {
                regs[i] = arg;
            }

            let mut pc: usize = 0;
            loop {
                let inst = iseq.instructions[pc];
                let a = inst.a as usize;

                match inst.op {
                    OpCode::Move => {
                        regs[a] = regs[inst.b as usize].clone();
                    }
                    OpCode::LoadI => {
                        regs[a] = Value::Integer(inst.b as i64);
                    }
                    OpCode::Add => {
                        let v = regs[a].as_integer() + regs[a + 1].as_integer();
                        regs[a] = Value::Integer(v);
                    }
                    OpCode::AddI => {
                        let v = regs[a].as_integer() + inst.b as i64;
                        regs[a] = Value::Integer(v);
                    }
                    OpCode::Sub => {
                        let v = regs[a].as_integer() - regs[a + 1].as_integer();
                        regs[a] = Value::Integer(v);
                    }
                    OpCode::SubI => {
                        let v = regs[a].as_integer() - inst.b as i64;
                        regs[a] = Value::Integer(v);
                    }
                    OpCode::Le => {
                        let v = regs[a].as_integer() <= regs[a + 1].as_integer();
                        regs[a] = Value::Bool(v);
                    }
                    OpCode::Jmp => {
                        pc = a;
                        continue;
                    }
                    OpCode::JmpNot => {
                        if regs[a].is_falsy() {
                            pc = inst.b as usize;
                            continue;
                        }
                    }
                    OpCode::SSend => {
                        let sym = &iseq.symbols[inst.b as usize];
                        let target = self
                            .iseqs
                            .iter()
                            .find(|s| s.name == *sym)
                            .unwrap_or_else(|| panic!("function not found: {}", sym));
                        let c = inst.c as usize;
                        let call_args: Vec<Value> =
                            (a..=a + c).map(|i| regs[i].clone()).collect();
                        let result = self.execute(target, call_args).await;
                        regs[a] = result;
                    }
                    OpCode::Return => {
                        return regs[a].clone();
                    }
                }
                pc += 1;
            }
        }
        .boxed()
    }
}

/// Executor that runs async tasks by polling futures directly,
/// without relying on an external runtime (tokio, async-std, etc.).
pub struct Executor;

impl Executor {
    pub fn new() -> Self {
        Self
    }

    /// Run a future to completion and return the result.
    pub fn run<F: Future>(&self, future: F) -> F::Output {
        let mut future = std::pin::pin!(future);
        let waker = Waker::noop();
        let mut cx = Context::from_waker(waker);
        loop {
            match future.as_mut().poll(&mut cx) {
                Poll::Ready(output) => return output,
                Poll::Pending => continue,
            }
        }
    }
}

impl Default for Executor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: create an instruction concisely
    fn inst(op: OpCode, a: i32, b: i32, c: i32) -> Instruction {
        Instruction::new(op, a, b, c)
    }

    #[test]
    fn test_loadi_and_return() {
        // LOADI R[0], 42
        // RETURN R[0]
        let iseq = Iseq {
            name: "main".into(),
            argc: 0,
            max_regs: 1,
            symbols: vec![],
            instructions: vec![
                inst(OpCode::LoadI, 0, 42, 0),
                inst(OpCode::Return, 0, 0, 0),
            ],
        };

        let vm = VM::new(vec![iseq.clone()]);
        let executor = Executor::new();
        let result = executor.run(vm.execute(&iseq, vec![]));
        assert_eq!(result, Value::Integer(42));
    }

    #[test]
    fn test_arithmetic() {
        // LOADI R[0], 10
        // LOADI R[1], 3
        // ADD   R[0]         -> R[0] = 10 + 3 = 13
        // ADDI  R[0], 7      -> R[0] = 13 + 7 = 20
        // LOADI R[1], 5
        // SUB   R[0]         -> R[0] = 20 - 5 = 15
        // SUBI  R[0], 3      -> R[0] = 15 - 3 = 12
        // RETURN R[0]
        let iseq = Iseq {
            name: "main".into(),
            argc: 0,
            max_regs: 2,
            symbols: vec![],
            instructions: vec![
                inst(OpCode::LoadI, 0, 10, 0),
                inst(OpCode::LoadI, 1, 3, 0),
                inst(OpCode::Add, 0, 0, 0),
                inst(OpCode::AddI, 0, 7, 0),
                inst(OpCode::LoadI, 1, 5, 0),
                inst(OpCode::Sub, 0, 0, 0),
                inst(OpCode::SubI, 0, 3, 0),
                inst(OpCode::Return, 0, 0, 0),
            ],
        };

        let vm = VM::new(vec![iseq.clone()]);
        let executor = Executor::new();
        let result = executor.run(vm.execute(&iseq, vec![]));
        assert_eq!(result, Value::Integer(12));
    }

    #[test]
    fn test_loop_sum() {
        // sum(n): compute 1 + 2 + ... + n using a loop
        //
        // R[0] = n (argument)
        // R[1] = accumulator
        // R[2] = counter (counts down from n to 1)
        // R[3], R[4] = temporaries for LE comparison
        //
        // 0: LOADI R[1], 0        result = 0
        // 1: MOVE  R[2], R[0]     counter = n
        // --- loop (pc=2) ---
        // 2: MOVE  R[3], R[2]     R[3] = counter
        // 3: LOADI R[4], 0        R[4] = 0
        // 4: LE    R[3]           R[3] = (counter <= 0)
        // 5: JMPNOT R[3], 7       if counter > 0, goto 7
        // 6: RETURN R[1]          return result
        // --- body ---
        // 7: ADD   R[1]           result += counter (R[1] + R[2])
        // 8: SUBI  R[2], 1        counter -= 1
        // 9: JMP   2              goto loop start
        let iseq = Iseq {
            name: "sum".into(),
            argc: 1,
            max_regs: 5,
            symbols: vec![],
            instructions: vec![
                inst(OpCode::LoadI, 1, 0, 0),     // 0
                inst(OpCode::Move, 2, 0, 0),      // 1
                inst(OpCode::Move, 3, 2, 0),      // 2
                inst(OpCode::LoadI, 4, 0, 0),     // 3
                inst(OpCode::Le, 3, 0, 0),        // 4
                inst(OpCode::JmpNot, 3, 7, 0),    // 5
                inst(OpCode::Return, 1, 0, 0),    // 6
                inst(OpCode::Add, 1, 0, 0),       // 7
                inst(OpCode::SubI, 2, 1, 0),      // 8
                inst(OpCode::Jmp, 2, 0, 0),       // 9
            ],
        };

        let vm = VM::new(vec![iseq.clone()]);
        let executor = Executor::new();
        let result = executor.run(vm.execute(&iseq, vec![Value::Integer(10)]));
        assert_eq!(result, Value::Integer(55));
    }

    #[test]
    fn test_ssend() {
        // add(a, b): returns a + b
        let add_iseq = Iseq {
            name: "add".into(),
            argc: 2,
            max_regs: 2,
            symbols: vec![],
            instructions: vec![
                inst(OpCode::Add, 0, 0, 0),    // R[0] = R[0] + R[1]
                inst(OpCode::Return, 0, 0, 0),
            ],
        };

        // main(): calls add(21, 21)
        //
        // 0: LOADI R[0], 21
        // 1: LOADI R[1], 21
        // 2: SSEND a=0, b=0, c=1   call symbols[0]="add" with R[0],R[1]
        // 3: RETURN R[0]
        let main_iseq = Iseq {
            name: "main".into(),
            argc: 0,
            max_regs: 2,
            symbols: vec!["add".into()],
            instructions: vec![
                inst(OpCode::LoadI, 0, 21, 0),
                inst(OpCode::LoadI, 1, 21, 0),
                inst(OpCode::SSend, 0, 0, 1),
                inst(OpCode::Return, 0, 0, 0),
            ],
        };

        let vm = VM::new(vec![add_iseq, main_iseq.clone()]);
        let executor = Executor::new();
        let result = executor.run(vm.execute(&main_iseq, vec![]));
        assert_eq!(result, Value::Integer(42));
    }

    #[test]
    fn test_recursive_fib() {
        // fib(n): compute Fibonacci number recursively
        //
        // R[0] = n (argument, not modified)
        // R[1] = temporary / result of fib(n-1)
        // R[2] = temporary / result of fib(n-2)
        //
        //  0: MOVE  R[1], R[0]       R[1] = n
        //  1: LOADI R[2], 1          R[2] = 1
        //  2: LE    R[1]             R[1] = (n <= 1)
        //  3: JMPNOT R[1], 5         if n > 1, goto 5
        //  4: RETURN R[0]            base case: return n
        //  5: MOVE  R[1], R[0]       R[1] = n
        //  6: SUBI  R[1], 1          R[1] = n - 1
        //  7: SSEND a=1, b=0, c=0   R[1] = fib(n-1)
        //  8: MOVE  R[2], R[0]       R[2] = n
        //  9: SUBI  R[2], 2          R[2] = n - 2
        // 10: SSEND a=2, b=0, c=0   R[2] = fib(n-2)
        // 11: ADD   R[1]             R[1] = fib(n-1) + fib(n-2)
        // 12: RETURN R[1]
        let fib_iseq = Iseq {
            name: "fib".into(),
            argc: 1,
            max_regs: 3,
            symbols: vec!["fib".into()],
            instructions: vec![
                inst(OpCode::Move, 1, 0, 0),      //  0
                inst(OpCode::LoadI, 2, 1, 0),     //  1
                inst(OpCode::Le, 1, 0, 0),        //  2
                inst(OpCode::JmpNot, 1, 5, 0),    //  3
                inst(OpCode::Return, 0, 0, 0),    //  4
                inst(OpCode::Move, 1, 0, 0),      //  5
                inst(OpCode::SubI, 1, 1, 0),      //  6
                inst(OpCode::SSend, 1, 0, 0),     //  7
                inst(OpCode::Move, 2, 0, 0),      //  8
                inst(OpCode::SubI, 2, 2, 0),      //  9
                inst(OpCode::SSend, 2, 0, 0),     // 10
                inst(OpCode::Add, 1, 0, 0),       // 11
                inst(OpCode::Return, 1, 0, 0),    // 12
            ],
        };

        let vm = VM::new(vec![fib_iseq.clone()]);
        let executor = Executor::new();

        // fib(10) = 55
        let result = executor.run(vm.execute(&fib_iseq, vec![Value::Integer(10)]));
        assert_eq!(result, Value::Integer(55));

        // fib(0) = 0, fib(1) = 1 (base cases)
        let r0 = executor.run(vm.execute(&fib_iseq, vec![Value::Integer(0)]));
        assert_eq!(r0, Value::Integer(0));
        let r1 = executor.run(vm.execute(&fib_iseq, vec![Value::Integer(1)]));
        assert_eq!(r1, Value::Integer(1));
    }
}
