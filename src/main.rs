#![allow(dead_code)]

mod tf;
use tf::{TypedEval, TypedValidate, TypedCompiler, CBD};

mod cps;
mod frfr;

use frfr::{CBD_FR, EvalFR, AbstractCompiler};

#[cfg(test)]
mod test;

pub trait Balloon {
    fn maybe_true(&self) -> bool;
    fn maybe_false(&self) -> bool;
}

impl Balloon for bool {
    fn maybe_true(&self) -> bool { *self }
    fn maybe_false(&self) -> bool { !self }
}

pub struct Idk;
impl Balloon for Idk {
    fn maybe_true(&self) -> bool { true }
    fn maybe_false(&self) -> bool { true }
}

// for abstract compiler where usize is used as var idx
impl Balloon for usize {
    fn maybe_true(&self) -> bool { true }
    fn maybe_false(&self) -> bool { true }
}

#[macro_export]
macro_rules! cbdif {
    (if ($cond:expr) then $t:expr, else $e:expr) => {
        let ceval = $cond;
        if ceval.maybe_true() {
            $t
        } 
        if ceval.maybe_false() {
            $e
        }
    }
}

macro_rules! cbd {
    () => {
        fn cbd_i32_const(&mut self) {
            let x = self.codeptr.read_imm_i32();
            self.pushi_imm(x);
        }

        fn cbd_i32_add(&mut self) {
            let x = self.popi();
            let y = self.popi();
            let z = Self::addi32(x, y);
            self.pushi(z);
        }

        fn cbd_local_set(&mut self) {
            let idx = self.codeptr.read_imm_i32();
            let val = self.pop();
            self.set_local(idx, val);
        }

        fn cbd_local_get(&mut self) {
            let idx = self.codeptr.read_imm_i32();
            let local = self.get_local(idx);
            self.push(local);
        }

        fn cbd_local_tee(&mut self) {
            let idx = self.codeptr.read_imm_i32();
            let val = self.pop(); // TODO: peek()?
            self.push(val);
            self.set_local(idx, val);
        }

        fn cbd_block(&mut self) {
            let ty = self.codeptr.read_block_type();
            self.start_block(ty);
        }

        fn cbd_loop(&mut self) {
            let ty = self.codeptr.read_block_type();
            self.start_loop(ty);
        }

        fn cbd_br(&mut self) {
            let label_idx = self.codeptr.read_imm_i32();
            self.branch(label_idx as usize);
        }

        fn cbd_br_if(&mut self) {
            let label_idx = self.codeptr.read_imm_i32();
            let condv = self.popi();
            let condb = Self::i32_eqz(condv); // make it a member fn just in case it could mutate,
                                              // like for compiler
            cbdif! {
                if (condb) then {
                    self.fallthru();
                }, else {
                    self.branch(label_idx as usize);
                }
            }
        }
        
        fn cbd_end(&mut self) {
            self.end();
        }

        fn dispatch(&mut self) {
            while let Some(op) = self.codeptr.read_op() {
                op_dispatch!(op, self)
            }
        }
    }
}

#[macro_export]
macro_rules! mk_opcodes {
    ($(($op:ident, $f:ident)),*) => {
        #[derive(Debug, Copy, Clone)]
        pub enum Opcode {
            $(
                $op,
            )*
        }

        #[macro_export]
        macro_rules! op_dispatch {
            ($dispatch_op:expr, $dispatcher:expr) => {{
                use Opcode::*;
                match $dispatch_op {
                    $($op => $dispatcher.$f()),*
                }
            }}
        }
    }
}

mk_opcodes! {
    (I32Const, cbd_i32_const),
    (I32Add, cbd_i32_add),
    (LocalSet, cbd_local_set),
    (LocalGet, cbd_local_get),
    (Block, cbd_block),
    (Loop, cbd_loop),
    (End, cbd_end),
    (Br, cbd_br),
    (BrIf, cbd_br_if)
}

#[derive(Copy, Clone, Debug)]
pub enum CodeEntry {
    Op(Opcode),
    I32Imm(i32),
    BlockType(usize),
}
pub struct CodePtr {
    pub code: Vec<CodeEntry>,
    pub ip: usize,
}
impl CodePtr {
    pub fn next(&mut self) -> Option<&CodeEntry> {
        let ret = self.code.get(self.ip);
        self.ip += 1;
        ret
    }

    pub fn read_op(&mut self) -> Option<Opcode> {
        match self.next() {
            Some(CodeEntry::Op(o)) => Some(*o),
            Some(_) => panic!("not an opcode"),
            None => None,
        }
    }

    pub fn read_block_type(&mut self) -> usize {
        match self.next() {
            Some(CodeEntry::BlockType(i)) => *i,
            _ => panic!("not a block type"),
        }
    }
    pub fn read_imm_i32(&mut self) -> i32 {
        match self.next() {
            Some(CodeEntry::I32Imm(i)) => *i,
            _ => panic!("not an i32 imm"),
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub struct STEntry {
    pub ip_delta: isize,
    pub stp_delta: isize,
}

pub struct Eval {
    pub stack: Vec<i32>,
    pub locals: Vec<i32>,
    pub codeptr: CodePtr,
    pub sidetable: Vec<STEntry>,
    pub stp: usize,
}

impl Eval {
    fn popi(&mut self) -> i32 {
        self.stack.pop().unwrap()
    }

    fn pushi_imm(&mut self, x: i32) {
        self.pushi(x)
    }
    fn pushi(&mut self, x: i32) {
        self.stack.push(x)
    }

    fn push(&mut self, x: i32) {
        self.stack.push(x)
    }
    fn pop(&mut self) -> i32 {
        self.stack.pop().unwrap()
    }

    fn set_local(&mut self, idx: i32, val: i32) {
        self.locals[idx as usize] = val;
    }

    fn get_local(&mut self, idx: i32) -> i32 {
        self.locals[idx as usize]
    }

    fn start_block(&mut self, _ty_index: usize) { }
    fn start_loop(&mut self, _ty_index: usize) { }
    fn end(&mut self) { }

    fn addi32(x: i32, y: i32) -> i32 {
        x + y
    }

    fn i32_eqz(x: i32) -> bool {
        x == 0
    }

    fn branch(&mut self, _label_idx: usize) {
        self.stp += 1;
        let ste = self.sidetable[self.stp];
        // stupid casts
        self.codeptr.ip = ((self.codeptr.ip as isize) + ste.ip_delta) as usize;
        self.stp = ((self.stp as isize) + ste.stp_delta) as usize;
    }

    fn fallthru(&mut self) {
        self.stp += 1; // we know that only one of branch or fallthru will run in this case,
                       // but seems iffy
    }

    cbd!();
}

#[derive(Copy, Clone, Eq, PartialEq)]
pub enum Type {
    I32,
}

#[derive(Debug, PartialEq, Eq)]
enum CtlType {
    Func,
    Block,
    Loop,
}

#[derive(Debug, PartialEq, Eq)]
pub struct CtlEntry {
    tipe: CtlType,
    cont_ip: usize,
    cont_stp: usize, // essentially idx of first branch
}

pub struct SidetableMeta {
    br_ip: usize,
    target_ctl_idx: usize,
}

struct Validate {
    pub stack: Vec<Type>,
    pub locals: Vec<Type>,
    pub ctl_entries: Vec<CtlEntry>,
    pub ctl_stack: Vec<usize>,
    pub codeptr: CodePtr,
    pub sidetable_meta: Vec<SidetableMeta>, // idx = br_index
}

impl Validate {
    fn popi(&mut self) -> Type {
        assert!(self.stack.pop().is_some_and(|t| t == Type::I32));
        Type::I32
    }

    fn pushi_imm(&mut self, _: i32) {
        self.stack.push(Type::I32)
    }

    fn pushi(&mut self, t: Type) {
        assert!(t == Type::I32);
        self.stack.push(Type::I32)
    }

    fn push(&mut self, t: Type) {
        self.stack.push(t)
    }

    fn pop(&mut self) -> Type {
        self.stack.pop().unwrap()
    }

    fn set_local(&mut self, idx: i32, val: Type) {
        self.locals[idx as usize] = val;
    }

    fn get_local(&mut self, idx: i32) -> Type {
        self.locals[idx as usize]
    }

    fn is_loop(&self) -> bool {
        self.ctl_entries.last().unwrap().tipe == CtlType::Loop
    }

    fn start_block(&mut self, _ty_index: usize) {
        self.ctl_stack.push(self.ctl_entries.len());
        self.ctl_entries.push(CtlEntry {
            tipe: CtlType::Block,
            cont_ip: 0, // filled in later
            cont_stp: self.sidetable_meta.len() - 1,
        });
    }

    fn start_loop(&mut self, _ty_index: usize) { 
        self.ctl_stack.push(self.ctl_entries.len());
        self.ctl_entries.push(CtlEntry {
            tipe: CtlType::Loop,
            cont_ip: self.codeptr.ip,
            cont_stp: self.sidetable_meta.len() - 1,
        });
    }

    fn addi32(_: Type, _: Type) -> Type {
        Type::I32
    }

    fn i32_eqz(t: Type) -> Idk {
        assert!(t == Type::I32);
        Idk
    }

    fn branch(&mut self, label_idx: usize) {
        let ctl_idx = self.ctl_stack[self.ctl_stack.len() - 1 - label_idx];
        self.sidetable_meta.push(SidetableMeta {
            br_ip: self.codeptr.ip,
            target_ctl_idx: ctl_idx,
        });
        // validate
    }

    fn fallthru(&mut self) {
        // validate
    }

    fn end(&mut self) {
        let ctl_idx = self.ctl_stack.pop().unwrap();
        let ctl = &mut self.ctl_entries[ctl_idx];
        if ctl.tipe == CtlType::Block {
            ctl.cont_ip = self.codeptr.ip;
            ctl.cont_stp = self.sidetable_meta.len() - 1;
        }
    }

    fn build_sidetable(&self) -> Vec<STEntry> {
        self.sidetable_meta.iter().enumerate().map(|(stp, br_meta)| {
            let target_ctl_idx = br_meta.target_ctl_idx;
            let target_ctl = &self.ctl_entries[target_ctl_idx];
            STEntry {
                ip_delta: (target_ctl.cont_ip as isize) - (br_meta.br_ip as isize),
                stp_delta: (target_ctl.cont_stp as isize) - (stp as isize),
            }
        }).collect()
    }

    cbd!();
}

impl TypedEval {
    fn dispatch(&mut self) {
        while let Some(op) = self.codeptr_mut().read_op() {
            op_dispatch!(op, self)
        }
    }
}

impl TypedValidate {
    fn dispatch(&mut self) {
        while let Some(op) = self.codeptr_mut().read_op() {
            op_dispatch!(op, self)
        }
    }
}

impl TypedCompiler {
    fn dispatch(&mut self) {
        while let Some(op) = self.codeptr_mut().read_op() {
            op_dispatch!(op, self)
        }
    }
}

pub trait Run {
    fn run(&mut self);
    fn step(&mut self, op: Opcode);
}

impl<T: CBD_FR> Run for T {
    fn run(&mut self) {
        while let Some(op) = self.codeptr_mut().read_op() {
            op_dispatch!(op, self)
        }
    }

    fn step(&mut self, op: Opcode) {
        op_dispatch!(op, self)
    }
}

fn sum_code() -> Vec<CodeEntry> {
    use CodeEntry::*;
    use Opcode::*;
    vec![
        Op(I32Const), I32Imm(5),
        Op(Block), BlockType(0),
            Op(I32Const), I32Imm(-15),
            Op(I32Const), I32Imm(20),
            Op(I32Add),
            Op(I32Add),
            Op(Br), I32Imm(0),
            Op(I32Const), I32Imm(-999),
        Op(End),

        Op(LocalSet), I32Imm(0), // index

        Op(I32Const), I32Imm(0), // accumulator
        Op(LocalSet), I32Imm(1),

        Op(Loop), BlockType(0),
            Op(LocalGet),I32Imm(0), // add
            Op(LocalGet),I32Imm(1),
            Op(I32Add),
            Op(LocalSet), I32Imm(1),

            Op(LocalGet),I32Imm(0), // decr/test
            Op(I32Const), I32Imm(-1),
            Op(I32Add),
            Op(LocalSet), I32Imm(0),
            Op(LocalGet),I32Imm(0),
            Op(BrIf), I32Imm(0),
        Op(End),
        Op(LocalGet),I32Imm(1),
    ]
}

fn main() {
    let code = sum_code();

    let nlocals = 2;

    let mut validate = Validate {
        stack: vec![],
        locals: vec![Type::I32; nlocals],
        ctl_entries: vec![CtlEntry { tipe: CtlType::Func, cont_ip: code.len(), cont_stp: 0 }],
        ctl_stack: vec![0],
        codeptr: CodePtr { code: code.clone(), ip: 0 },
        sidetable_meta: vec![SidetableMeta { br_ip: 0, target_ctl_idx: 0 } ],
    };
    validate.dispatch();
    // dbg!(&validate.ctl_stack);
    // dbg!(&validate.ctl_entries);
    let sidetable = validate.build_sidetable();

    let mut eval = Eval {
        stack: vec![],
        locals: vec![0; nlocals],
        codeptr: CodePtr { code: code.clone(), ip: 0 },
        sidetable: sidetable,
        stp: 0,
    };
    eval.dispatch();
    dbg!(eval.stack);

    let mut tvalidate = TypedValidate {
        stack: vec![],
        locals: vec![Type::I32; nlocals],
        ctl_entries: vec![CtlEntry { tipe: CtlType::Func, cont_ip: code.len(), cont_stp: 0 }],
        ctl_stack: vec![0],
        codeptr: CodePtr { code: code.clone(), ip: 0 },
        sidetable_meta: vec![SidetableMeta { br_ip: 0, target_ctl_idx: 0 } ],
    };
    tvalidate.dispatch();
    // dbg!(&validate.ctl_stack);
    // dbg!(&validate.ctl_entries);
    let sidetable = tvalidate.build_sidetable();

    let mut teval = TypedEval {
        stack: vec![],
        locals: vec![0; nlocals],
        codeptr: CodePtr { code: code.clone(), ip: 0 },
        sidetable: sidetable.clone(),
        stp: 0,
    };
    teval.dispatch();
    dbg!(teval.stack);

    // let mut tcompiler = TypedCompiler {
    //     gen: String::new(),
    //     codeptr: CodePtr { code: code.clone(), ip: 0 },
    //     ic: 0,
    // };
    // tcompiler.dispatch();
    // println!("{}", tcompiler.gen);

    // let mut wasm_fun = cps::WASMFun::new(code.clone());
    // dbg!(&wasm_fun.cont_blocks);

    // let mut interpreter = cps::CPSEval { stack: vec![], locals: vec![0; nlocals] };
    // let interpreter = wasm_fun.run(interpreter);
    // dbg!(interpreter.stack);

    // unsafe {
    //     let mut interpreter = cps::CPSEval { stack: vec![], locals: vec![0; nlocals] };
    //     let mut codeptr = CodePtr { code: code.clone(), ip: 0 };

    //     let compiled = wasm_fun.compile::<cps::CPSEval>();
    //     let compiled_ptr: *const _ = &compiled;

    //     let first_cont = &(*compiled_ptr).conts[0];
    //     let interpreter = first_cont(compiled_ptr, interpreter, &mut codeptr);

    //     dbg!(interpreter.stack);
    // }

    let mut fr_eval = EvalFR {
        stack: vec![],
        locals: vec![0; nlocals],
        codeptr: CodePtr { code: code.clone(), ip: 0 },
        sidetable: sidetable.clone(),
        stp: 0,
    };
    fr_eval.run();
    dbg!(fr_eval.stack);

    let wasm_fun = crate::cps::WASMFun::new(code.clone());
    dbg!(&wasm_fun.cont_blocks);

    let mut ac = AbstractCompiler {
        block_bodies: vec![vec![]; wasm_fun.cont_blocks.len()],
        var_idx: 0,
        codeptr: CodePtr { code: code.clone(), ip: 0 },
        cont_blocks: &wasm_fun.cont_blocks,
        stp: 0,
    };
    ac.run();
    dbg!(&ac.block_bodies);
    let code = ac.emit();
    println!("{}", code);
}
