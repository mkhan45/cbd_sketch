use crate::{CodePtr, CodeEntry, Balloon, STEntry};
use crate::Run;

#[macro_export]
macro_rules! mif {
    ($self:ident: if ($cond:expr) then $t:expr, else $e:expr) => {
        let ceval = $cond;
        let left = ceval.maybe_true().then(|| $t);
        let right = ceval.maybe_false().then(|| $e);
        $self.merge(left);
        $self.merge(right);
    }
}

// TODO:
// 1. a CBD written in direct interpreter style
//      - with some mechanism for branching control flow within the CBD
// 2. an interpreter and validator written using that CBD
// 3. an abstract compiler written using the CBD, which can be used
//    on the interpreter to make a compiler
pub trait CBD_FR {
    type I32Val;
    type StackVal: Clone + Into<Self::LocalVal>;
    type LocalVal: Clone + Into<Self::StackVal>;
    type CondVal: Balloon;
    type MergeState;

    fn codeptr_mut(&mut self) -> &mut CodePtr;

    fn popi(&mut self) -> Self::I32Val;
    fn pushi_imm(&mut self, x: i32);
    fn pushi(&mut self, x: Self::I32Val);

    fn push(&mut self, x: Self::StackVal);
    fn pop(&mut self) -> Self::StackVal;

    fn set_local(&mut self, idx: i32, val: Self::LocalVal);
    fn get_local(&mut self, idx: i32) -> Self::LocalVal;

    fn start_block(&mut self, ty_index: usize);
    fn start_loop(&mut self, ty_index: usize);
    fn end(&mut self);

    fn i32_add(&mut self, x: Self::I32Val, y: Self::I32Val) -> Self::I32Val;
    fn i32_eqz(&mut self, x: Self::I32Val) -> Self::CondVal;

    // gotta make all control xfer return some mergeable state
    fn branch(&mut self, label_idx: usize) -> Self::MergeState;
    fn fallthru(&mut self) -> Self::MergeState;

    fn merge(&mut self, other: Option<Self::MergeState>);

    fn cbd_i32_const(&mut self) {
        let x = self.codeptr_mut().read_imm_i32();
        self.pushi_imm(x);
    }

    fn cbd_i32_add(&mut self) {
        let x = self.popi();
        let y = self.popi();
        let z = self.i32_add(x, y);
        self.pushi(z);
    }

    fn cbd_local_set(&mut self) {
        let idx = self.codeptr_mut().read_imm_i32();
        let val = self.pop();
        self.set_local(idx, val.into());
    }

    fn cbd_local_get(&mut self) {
        let idx = self.codeptr_mut().read_imm_i32();
        let local = self.get_local(idx);
        self.push(local.into());
    }

    fn cbd_local_tee(&mut self) {
        let idx = self.codeptr_mut().read_imm_i32();
        let val = self.pop(); // TODO: peek()?
        self.push(val.clone());
        self.set_local(idx, val.into());
    }

    fn cbd_block(&mut self) {
        let ty = self.codeptr_mut().read_block_type();
        self.start_block(ty);
    }

    fn cbd_loop(&mut self) {
        let ty = self.codeptr_mut().read_block_type();
        self.start_loop(ty);
    }

    fn cbd_br(&mut self) {
        let label_idx = self.codeptr_mut().read_imm_i32();
        self.branch(label_idx as usize);
    }

    fn cbd_br_if(&mut self) {
        let label_idx = self.codeptr_mut().read_imm_i32();
        let condv = self.popi();
        let condb = self.i32_eqz(condv); // make it a member fn just in case it could mutate,
                                          // like for compiler
        mif! {self:
            if (condb) then {
                self.fallthru()
            }, else {
                self.branch(label_idx as usize)
            }
        }
    }
    
    fn cbd_end(&mut self) {
        self.end();
    }
}

pub struct EvalFR {
    pub stack: Vec<i32>,
    pub locals: Vec<i32>,
    pub codeptr: CodePtr,
    pub sidetable: Vec<STEntry>,
    pub stp: usize,
}

impl CBD_FR for EvalFR {
    type I32Val = i32;
    type StackVal = i32;
    type LocalVal = i32;
    type CondVal = bool;
    type MergeState = ();

    fn codeptr_mut(&mut self) -> &mut CodePtr {
        &mut self.codeptr
    }

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

    fn i32_add(&mut self, x: i32, y: i32) -> i32 {
        x + y
    }

    fn i32_eqz(&mut self, x: i32) -> bool {
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

    fn merge(&mut self, _other: Option<()>) { }
}

use crate::cps::ContBlock;

pub struct AbstractCompiler<I: CBD_FR> {
    pub conts: Vec<Box<dyn Fn(*mut AbstractCompiler<I>)>>,
    pub interpretation: I,

    pub cont_blocks: *const Vec<ContBlock>,
    pub stp: usize,
}

pub struct AbstractMerge<T> {
    pub stp: usize,
    pub inner: T,
}

impl<I: CBD_FR> CBD_FR for AbstractCompiler<I> {
    type I32Val = I::I32Val;
    type StackVal = I::StackVal;
    type LocalVal = I::LocalVal;
    type CondVal = I::CondVal;
    type MergeState = AbstractMerge<I::MergeState>;

    fn codeptr_mut(&mut self) -> &mut CodePtr { self.interpretation.codeptr_mut() }

    fn popi(&mut self) -> Self::I32Val { self.interpretation.popi() }
    fn pushi_imm(&mut self, x: i32) { self.interpretation.pushi_imm(x) }
    fn pushi(&mut self, x: Self::I32Val) { self.interpretation.pushi(x) }

    fn push(&mut self, x: Self::StackVal) { self.interpretation.push(x) }
    fn pop(&mut self) -> Self::StackVal { self.interpretation.pop() }

    fn set_local(&mut self, idx: i32, val: Self::LocalVal) { self.interpretation.set_local(idx, val) }
    fn get_local(&mut self, idx: i32) -> Self::LocalVal { self.interpretation.get_local(idx) }

    fn start_block(&mut self, ty_index: usize) { self.interpretation.start_block(ty_index) }
    fn start_loop(&mut self, ty_index: usize) { self.interpretation.start_loop(ty_index) }
    fn end(&mut self) { self.interpretation.end() }

    fn i32_add(&mut self, x: Self::I32Val, y: Self::I32Val) -> Self::I32Val { self.interpretation.i32_add(x, y) }
    fn i32_eqz(&mut self, x: Self::I32Val) -> Self::CondVal { self.interpretation.i32_eqz(x) }

    fn branch(&mut self, label_idx: usize) -> Self::MergeState {
        let br_tgt_idx = unsafe { (*self.cont_blocks)[self.stp].br_tgt };
        let br_tgt_cont = unsafe { &(*self.conts)[br_tgt_idx] };

        // TODO: figure out actual execution graph of abstract compiler
        todo!();
        // self.interpretation.branch(label_idx);
        // AbstractMerge {
        //     stp: self.stp,
        //     inner: 
        // }
        // AbstractMerge {
        //     stp: br_tgt_idx,
        //     inner: self.interpretation.branch(label_idx),
        // }
    }

    fn fallthru(&mut self) -> Self::MergeState {
        let fallthru_idx = self.stp + 1;
        AbstractMerge {
            stp: fallthru_idx,
            inner: self.interpretation.fallthru(),
        }
    }

    fn merge(&mut self, other: Option<Self::MergeState>) {
        // merging other into self, run it for its state but keep our stp
        let
    }

    fn cbd_i32_const(&mut self) {
        let x = self.codeptr_mut().read_imm_i32();
        self.pushi_imm(x);
    }

    fn cbd_i32_add(&mut self) {
        let x = self.popi();
        let y = self.popi();
        let z = self.i32_add(x, y);
        self.pushi(z);
    }

    fn cbd_local_set(&mut self) {
        let idx = self.codeptr_mut().read_imm_i32();
        let val = self.pop();
        self.set_local(idx, val.into());
    }

    fn cbd_local_get(&mut self) {
        let idx = self.codeptr_mut().read_imm_i32();
        let local = self.get_local(idx);
        self.push(local.into());
    }

    fn cbd_local_tee(&mut self) {
        let idx = self.codeptr_mut().read_imm_i32();
        let val = self.pop(); // TODO: peek()?
        self.push(val.clone());
        self.set_local(idx, val.into());
    }

    fn cbd_block(&mut self) {
        let ty = self.codeptr_mut().read_block_type();
        self.start_block(ty);
    }

    fn cbd_loop(&mut self) {
        let ty = self.codeptr_mut().read_block_type();
        self.start_loop(ty);
    }

    fn cbd_br(&mut self) {
        let label_idx = self.codeptr_mut().read_imm_i32();
        self.branch(label_idx as usize);
    }

    fn cbd_br_if(&mut self) {
        let label_idx = self.codeptr_mut().read_imm_i32();
        let condv = self.popi();
        let condb = self.i32_eqz(condv); // make it a member fn just in case it could mutate,
                                          // like for compiler
        mif! {self:
            if (condb) then {
                self.fallthru()
            }, else {
                self.branch(label_idx as usize)
            }
        }
    }
    
    fn cbd_end(&mut self) {
        self.end();
    }
}

// pub struct CompiledFun<I: CBD_FR> {
//     pub conts: Vec<Box<dyn Fn(*const CompiledFun<I>, &mut I)>>,
// }

// impl WASMFun {
//     pub fn compile_fr<I: CBD_FR + Run>(&mut self, mut interpretation: I) -> CompiledFun<I> {
//         let mut res = CompiledFun { conts: vec![] };
//         for current_block in 0..self.cont_blocks.len() {
//             let start_ip = self.cont_blocks[current_block].ip;
//             let tgt_block = self.cont_blocks[current_block].br_tgt;

//             let fallthru_block = current_block + 1;

//             res.conts.push(Box::new(
//                 move |compiled: *const CompiledFun<I>, i: &mut I| unsafe {
//                     i.codeptr_mut().ip = start_ip;
//                     while let Some(op) = i.codeptr_mut().read_op() {
//                         match op {
//                             _ => i.step(op)
//                         }
//                     }
//                 }
//             ));
//         }

//         res
//     }
// }
