use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll, Waker};

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

    /// Create a VmFuture that executes the instruction sequence.
    /// Each poll executes one instruction and returns Pending.
    /// Returns Ready only when a Return instruction is reached.
    pub fn execute<'a>(&'a self, iseq: &'a Iseq, args: Vec<Value>) -> VmFuture<'a> {
        VmFuture::new(self, iseq, args)
    }
}

/// Future that executes VM instructions one per poll.
/// Returns Pending after each instruction, Ready on Return.
pub struct VmFuture<'a> {
    vm: &'a VM,
    iseq: &'a Iseq,
    regs: Vec<Value>,
    pc: usize,
    call: Option<Pin<Box<VmFuture<'a>>>>,
}

impl<'a> VmFuture<'a> {
    fn new(vm: &'a VM, iseq: &'a Iseq, args: Vec<Value>) -> Self {
        let mut regs = vec![Value::Nil; iseq.max_regs + 1]; // 1-based indexing
        for (i, arg) in args.into_iter().enumerate() {
            regs[i + 1] = arg; // arguments start at R[1]
        }
        Self {
            vm,
            iseq,
            regs,
            pc: 0,
            call: None,
        }
    }
}

impl<'a> Future for VmFuture<'a> {
    type Output = Value;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Value> {
        let this = self.get_mut();

        // If a sub-call (SSEND) is in progress, poll it first
        if let Some(ref mut call) = this.call {
            match call.as_mut().poll(cx) {
                Poll::Ready(result) => {
                    let a = this.iseq.instructions[this.pc].a as usize;
                    this.regs[a] = result;
                    this.call = None;
                    this.pc += 1;
                    return Poll::Pending;
                }
                Poll::Pending => return Poll::Pending,
            }
        }

        let inst = this.iseq.instructions[this.pc];
        let a = inst.a as usize;

        match inst.op {
            OpCode::Move => {
                this.regs[a] = this.regs[inst.b as usize].clone();
                this.pc += 1;
                Poll::Pending
            }
            OpCode::LoadI => {
                this.regs[a] = Value::Integer(inst.b as i64);
                this.pc += 1;
                Poll::Pending
            }
            OpCode::Add => {
                let v = this.regs[a].as_integer() + this.regs[a + 1].as_integer();
                this.regs[a] = Value::Integer(v);
                this.pc += 1;
                Poll::Pending
            }
            OpCode::AddI => {
                let v = this.regs[a].as_integer() + inst.b as i64;
                this.regs[a] = Value::Integer(v);
                this.pc += 1;
                Poll::Pending
            }
            OpCode::Sub => {
                let v = this.regs[a].as_integer() - this.regs[a + 1].as_integer();
                this.regs[a] = Value::Integer(v);
                this.pc += 1;
                Poll::Pending
            }
            OpCode::SubI => {
                let v = this.regs[a].as_integer() - inst.b as i64;
                this.regs[a] = Value::Integer(v);
                this.pc += 1;
                Poll::Pending
            }
            OpCode::Le => {
                let v = this.regs[a].as_integer() <= this.regs[a + 1].as_integer();
                this.regs[a] = Value::Bool(v);
                this.pc += 1;
                Poll::Pending
            }
            OpCode::Jmp => {
                this.pc = a;
                Poll::Pending
            }
            OpCode::JmpNot => {
                if this.regs[a].is_falsy() {
                    this.pc = inst.b as usize;
                } else {
                    this.pc += 1;
                }
                Poll::Pending
            }
            OpCode::SSend => {
                let vm = this.vm;
                let sym = &this.iseq.symbols[inst.b as usize];
                let target = vm
                    .iseqs
                    .iter()
                    .find(|s| s.name == *sym)
                    .unwrap_or_else(|| panic!("function not found: {}", sym));
                let c = inst.c as usize;
                let call_args: Vec<Value> =
                    (a..=a + c).map(|i| this.regs[i].clone()).collect();
                this.call = Some(Box::pin(VmFuture::new(vm, target, call_args)));
                Poll::Pending
            }
            OpCode::Return => Poll::Ready(this.regs[a].clone()),
        }
    }
}

/// Executor that runs async tasks by polling futures directly,
/// without relying on an external runtime (tokio, async-std, etc.).
pub struct Executor;

impl Executor {
    pub fn new() -> Self {
        Self
    }

    /// Poll a future exactly once.
    /// Returns Some(output) if the future completed, None if still pending.
    pub fn step<F: Future>(&self, future: Pin<&mut F>) -> Option<F::Output> {
        let waker = Waker::noop();
        let mut cx = Context::from_waker(waker);
        match future.poll(&mut cx) {
            Poll::Ready(output) => Some(output),
            Poll::Pending => None,
        }
    }

    /// Run a future to completion and return the result.
    pub fn run<F: Future>(&self, future: F) -> F::Output {
        let mut future = std::pin::pin!(future);
        let waker = Waker::noop();
        let mut cx = Context::from_waker(waker);
        loop {
            match future.as_mut().poll(&mut cx) {
                Poll::Ready(output) => return output,
                Poll::Pending => {
                    continue;
                },
            }
        }
    }
}

impl Default for Executor {
    fn default() -> Self {
        Self::new()
    }
}

// --- wasm exports ---
// setup(): initialize VM with fib(10), returns 0
// entrypoint(): poll once, returns 0 if pending, (result + 1) if done
#[cfg(target_arch = "wasm32")]
mod wasm {
    use super::*;
    use std::cell::UnsafeCell;

    // SAFETY: wasm32 is single-threaded, no data races possible
    struct WasmCell<T>(UnsafeCell<T>);
    unsafe impl<T> Sync for WasmCell<T> {}
    impl<T> WasmCell<T> {
        const fn new(val: T) -> Self {
            Self(UnsafeCell::new(val))
        }
        fn get(&self) -> *mut T {
            self.0.get()
        }
    }

    struct VmData {
        vm: VM,
        iseq: Iseq,
    }

    static DATA: WasmCell<Option<Box<VmData>>> = WasmCell::new(None);
    static FUTURE: WasmCell<Option<Pin<Box<VmFuture<'static>>>>> = WasmCell::new(None);

    fn build_fib_iseq() -> Iseq {
        Iseq {
            name: "fib".into(),
            argc: 1,
            max_regs: 3,
            symbols: vec!["fib".into()],
            instructions: vec![
                Instruction::new(OpCode::Move, 2, 1, 0),
                Instruction::new(OpCode::LoadI, 3, 1, 0),
                Instruction::new(OpCode::Le, 2, 0, 0),
                Instruction::new(OpCode::JmpNot, 2, 5, 0),
                Instruction::new(OpCode::Return, 1, 0, 0),
                Instruction::new(OpCode::Move, 2, 1, 0),
                Instruction::new(OpCode::SubI, 2, 1, 0),
                Instruction::new(OpCode::SSend, 2, 0, 0),
                Instruction::new(OpCode::Move, 3, 1, 0),
                Instruction::new(OpCode::SubI, 3, 2, 0),
                Instruction::new(OpCode::SSend, 3, 0, 0),
                Instruction::new(OpCode::Add, 2, 0, 0),
                Instruction::new(OpCode::Return, 2, 0, 0),
            ],
        }
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn setup(n: i32) -> i32 {
        unsafe {
            // Drop old future before data to avoid dangling refs
            *FUTURE.get() = None;
            *DATA.get() = None;

            let iseq = build_fib_iseq();
            let vm = VM::new(vec![iseq.clone()]);
            *DATA.get() = Some(Box::new(VmData { vm, iseq }));

            // SAFETY: VmData is heap-allocated in a static global and
            // will not be moved or dropped while the future is alive.
            let data = (*DATA.get()).as_ref().unwrap().as_ref();
            let vm_ref: &'static VM = std::mem::transmute(&data.vm);
            let iseq_ref: &'static Iseq = std::mem::transmute(&data.iseq);

            let future = vm_ref.execute(iseq_ref, vec![Value::Integer(n as i64)]);
            *FUTURE.get() = Some(Box::pin(future));
        }
        0
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn entrypoint() -> i32 {
        unsafe {
            let future = (*FUTURE.get())
                .as_mut()
                .expect("call setup() first");
            let executor = Executor::new();
            match executor.step(future.as_mut()) {
                None => 0,
                Some(Value::Integer(n)) => n as i32 + 1,
                Some(_) => 1,
            }
        }
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
        // LOADI R[1], 42
        // RETURN R[1]
        let iseq = Iseq {
            name: "main".into(),
            argc: 0,
            max_regs: 1,
            symbols: vec![],
            instructions: vec![
                inst(OpCode::LoadI, 1, 42, 0),
                inst(OpCode::Return, 1, 0, 0),
            ],
        };

        let vm = VM::new(vec![iseq.clone()]);
        let executor = Executor::new();
        let result = executor.run(vm.execute(&iseq, vec![]));
        assert_eq!(result, Value::Integer(42));
    }

    #[test]
    fn test_arithmetic() {
        // LOADI R[1], 10
        // LOADI R[2], 3
        // ADD   R[1]         -> R[1] = 10 + 3 = 13
        // ADDI  R[1], 7      -> R[1] = 13 + 7 = 20
        // LOADI R[2], 5
        // SUB   R[1]         -> R[1] = 20 - 5 = 15
        // SUBI  R[1], 3      -> R[1] = 15 - 3 = 12
        // RETURN R[1]
        let iseq = Iseq {
            name: "main".into(),
            argc: 0,
            max_regs: 2,
            symbols: vec![],
            instructions: vec![
                inst(OpCode::LoadI, 1, 10, 0),
                inst(OpCode::LoadI, 2, 3, 0),
                inst(OpCode::Add, 1, 0, 0),
                inst(OpCode::AddI, 1, 7, 0),
                inst(OpCode::LoadI, 2, 5, 0),
                inst(OpCode::Sub, 1, 0, 0),
                inst(OpCode::SubI, 1, 3, 0),
                inst(OpCode::Return, 1, 0, 0),
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
        // R[1] = n (argument)
        // R[2] = accumulator
        // R[3] = counter (counts down from n to 1)
        // R[4], R[5] = temporaries for LE comparison
        //
        // 0: LOADI R[2], 0        result = 0
        // 1: MOVE  R[3], R[1]     counter = n
        // --- loop (pc=2) ---
        // 2: MOVE  R[4], R[3]     R[4] = counter
        // 3: LOADI R[5], 0        R[5] = 0
        // 4: LE    R[4]           R[4] = (counter <= 0)
        // 5: JMPNOT R[4], 7       if counter > 0, goto 7
        // 6: RETURN R[2]          return result
        // --- body ---
        // 7: ADD   R[2]           result += counter (R[2] + R[3])
        // 8: SUBI  R[3], 1        counter -= 1
        // 9: JMP   2              goto loop start
        let iseq = Iseq {
            name: "sum".into(),
            argc: 1,
            max_regs: 5,
            symbols: vec![],
            instructions: vec![
                inst(OpCode::LoadI, 2, 0, 0),     // 0
                inst(OpCode::Move, 3, 1, 0),      // 1
                inst(OpCode::Move, 4, 3, 0),      // 2
                inst(OpCode::LoadI, 5, 0, 0),     // 3
                inst(OpCode::Le, 4, 0, 0),        // 4
                inst(OpCode::JmpNot, 4, 7, 0),    // 5
                inst(OpCode::Return, 2, 0, 0),    // 6
                inst(OpCode::Add, 2, 0, 0),       // 7
                inst(OpCode::SubI, 3, 1, 0),      // 8
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
                inst(OpCode::Add, 1, 0, 0),    // R[1] = R[1] + R[2]
                inst(OpCode::Return, 1, 0, 0),
            ],
        };

        // main(): calls add(21, 21)
        //
        // 0: LOADI R[1], 21
        // 1: LOADI R[2], 21
        // 2: SSEND a=1, b=0, c=1   call symbols[0]="add" with R[1],R[2]
        // 3: RETURN R[1]
        let main_iseq = Iseq {
            name: "main".into(),
            argc: 0,
            max_regs: 2,
            symbols: vec!["add".into()],
            instructions: vec![
                inst(OpCode::LoadI, 1, 21, 0),
                inst(OpCode::LoadI, 2, 21, 0),
                inst(OpCode::SSend, 1, 0, 1),
                inst(OpCode::Return, 1, 0, 0),
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
        // R[1] = n (argument, not modified)
        // R[2] = temporary / result of fib(n-1)
        // R[3] = temporary / result of fib(n-2)
        //
        //  0: MOVE  R[2], R[1]       R[2] = n
        //  1: LOADI R[3], 1          R[3] = 1
        //  2: LE    R[2]             R[2] = (n <= 1)
        //  3: JMPNOT R[2], 5         if n > 1, goto 5
        //  4: RETURN R[1]            base case: return n
        //  5: MOVE  R[2], R[1]       R[2] = n
        //  6: SUBI  R[2], 1          R[2] = n - 1
        //  7: SSEND a=2, b=0, c=0   R[2] = fib(n-1)
        //  8: MOVE  R[3], R[1]       R[3] = n
        //  9: SUBI  R[3], 2          R[3] = n - 2
        // 10: SSEND a=3, b=0, c=0   R[3] = fib(n-2)
        // 11: ADD   R[2]             R[2] = fib(n-1) + fib(n-2)
        // 12: RETURN R[2]
        let fib_iseq = Iseq {
            name: "fib".into(),
            argc: 1,
            max_regs: 3,
            symbols: vec!["fib".into()],
            instructions: vec![
                inst(OpCode::Move, 2, 1, 0),      //  0
                inst(OpCode::LoadI, 3, 1, 0),     //  1
                inst(OpCode::Le, 2, 0, 0),        //  2
                inst(OpCode::JmpNot, 2, 5, 0),    //  3
                inst(OpCode::Return, 1, 0, 0),    //  4
                inst(OpCode::Move, 2, 1, 0),      //  5
                inst(OpCode::SubI, 2, 1, 0),      //  6
                inst(OpCode::SSend, 2, 0, 0),     //  7
                inst(OpCode::Move, 3, 1, 0),      //  8
                inst(OpCode::SubI, 3, 2, 0),      //  9
                inst(OpCode::SSend, 3, 0, 0),     // 10
                inst(OpCode::Add, 2, 0, 0),       // 11
                inst(OpCode::Return, 2, 0, 0),    // 12
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

    #[test]
    fn test_step() {
        // LOADI R[1], 10
        // ADDI  R[1], 5
        // RETURN R[1]
        let iseq = Iseq {
            name: "main".into(),
            argc: 0,
            max_regs: 1,
            symbols: vec![],
            instructions: vec![
                inst(OpCode::LoadI, 1, 10, 0),
                inst(OpCode::AddI, 1, 5, 0),
                inst(OpCode::Return, 1, 0, 0),
            ],
        };

        let vm = VM::new(vec![iseq.clone()]);
        let executor = Executor::new();
        let mut future = std::pin::pin!(vm.execute(&iseq, vec![]));

        // LOADI -> Pending
        assert_eq!(executor.step(future.as_mut()), None);
        // ADDI -> Pending
        assert_eq!(executor.step(future.as_mut()), None);
        // RETURN -> Ready
        assert_eq!(executor.step(future.as_mut()), Some(Value::Integer(15)));
    }
}
