use crate::{CodePtr, CodeEntry, Balloon, STEntry, Idk};
use std::marker::PhantomData;
use crate::Run;
use std::collections::VecDeque;

#[macro_export]
macro_rules! mif {
    ($self:ident: if ($cond:expr) then $t:expr, else $e:expr) => {
        let ceval = $cond;
        if ceval.maybe_true() {
            let t = $t;
            $self.merge(t);
        }
        if ceval.maybe_false() {
            let e = $e;
            $self.merge(e);
        }
    }
}

pub trait Merge {
    fn merge(&mut self, other: &Self);
}

impl Merge for () {
    fn merge(&mut self, _: &()) {}
}

// should have a wrapper type, or expect CBD_FR to impl
impl Merge for usize {
    fn merge(&mut self, _: &usize) {}
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
    type MergeState: Merge;

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

    fn merge(&mut self, other: Self::MergeState);

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
        self.stp += 1;
    }

    fn merge(&mut self, _other: ()) {}
}

use crate::cps::ContBlock;

pub struct AbstractRuntime<I: CBD_FR> {
    pub block_states: Vec<I::MergeState>,
    pub interpreter: I,
}

pub struct AbstractCompiler {
    pub block_bodies: Vec<Vec<String>>,
    pub var_idx: usize,

    pub cont_blocks: *const Vec<ContBlock>,
    pub codeptr: CodePtr,
    pub stp: usize,
}

impl AbstractCompiler {
    pub fn fv(&mut self) -> usize {
        self.var_idx += 1;
        self.var_idx
    }
}

impl CBD_FR for AbstractCompiler {
    // vals are compiler indices
    type I32Val = usize;
    type StackVal = usize;
    type LocalVal = usize;
    type CondVal = usize;
    type MergeState = usize; // stp

    fn codeptr_mut(&mut self) -> &mut CodePtr { &mut self.codeptr }

    fn popi(&mut self) -> Self::I32Val {
        let i = self.fv();
        self.block_bodies[self.stp].push(format!("let x{i} = i.popi()"));
        i
    }

    fn pushi_imm(&mut self, x: i32) {
        self.block_bodies[self.stp].push(format!("i.pushi_imm({x})"));
    }

    fn pushi(&mut self, x: Self::I32Val) {
        self.block_bodies[self.stp].push(format!("i.pushi(x{x})"));
    }

    fn push(&mut self, x: Self::StackVal) {
        self.block_bodies[self.stp].push(format!("i.pushi(x{x})"));
    }

    fn pop(&mut self) -> Self::StackVal {
        let i = self.fv();
        self.block_bodies[self.stp].push(format!("let x{i} = i.pop()"));
        i
    }

    fn set_local(&mut self, idx: i32, val: Self::LocalVal) {
        self.block_bodies[self.stp].push(format!("i.set_local({idx}, x{val})"));
    }

    fn get_local(&mut self, idx: i32) -> Self::LocalVal {
        let i = self.fv();
        self.block_bodies[self.stp].push(format!("let x{i} = i.get_local({idx})"));
        i
    }

    fn start_block(&mut self, ty_index: usize) { 
        self.block_bodies[self.stp].push(format!("i.start_block({ty_index})"));
    }
    fn start_loop(&mut self, ty_index: usize) {
        let f = self.stp + 1;
        self.block_bodies[self.stp].push(format!("wl.push_back({f})"));

        self.stp += 1;
        self.block_bodies[self.stp].push(format!("i.start_loop({ty_index})"));
    }
    fn end(&mut self) {
        self.block_bodies[self.stp].push(format!("i.end()"));

        let f = self.stp + 1;
        self.block_bodies[self.stp].push(format!("wl.push_back({f})"));

        self.stp += 1;
    }

    fn i32_add(&mut self, x: Self::I32Val, y: Self::I32Val) -> Self::I32Val {
        let i = self.fv();
        self.block_bodies[self.stp].push(format!("let x{i} = x{x} + x{y}"));
        i
    }
    fn i32_eqz(&mut self, x: Self::I32Val) -> Self::CondVal {
        let i = self.fv();
        self.block_bodies[self.stp].push(format!("let x{i} = i.i32_eqz(x{x})"));
        i
    }

    fn cbd_br_if(&mut self) {
        let _label_idx = self.codeptr_mut().read_imm_i32();
        let condv = self.popi();
        let condb = self.i32_eqz(condv);

        let fallthru = self.stp + 1;
        let branch = unsafe { (*self.cont_blocks)[self.stp].br_tgt };

        self.block_bodies[self.stp].push(format!("
        let _ = if (x{condb}.maybe_true()) {{ i.merge(state_{fallthru}); wl.push_back({fallthru}) }} else {{}};
        let _ = if (x{condb}.maybe_false()) {{ i.merge(state_{branch}); wl.push_back({branch}) }} else {{}}"));
        self.stp += 1;
    }

    fn branch(&mut self, _label_idx: usize) -> Self::MergeState {
        let tgt = unsafe { (*self.cont_blocks)[self.stp].br_tgt };
        self.block_bodies[self.stp].push(format!("wl.push_back({tgt})"));
        self.stp += 1;
        tgt
    }

    fn fallthru(&mut self) -> Self::MergeState {
        let f = self.stp + 1;
        self.block_bodies[self.stp].push(format!("wl.push_back({f})"));
        self.stp += 1;
        self.stp
    }

    fn merge(&mut self, other: Self::MergeState) {
        // merging self into other:
        let other_stp = other;
        let self_stp = self.stp;
        self.block_bodies[self.stp].push(format!("state_{self_stp}.merge(state_{other_stp})"));
    }
}

impl AbstractCompiler {
    pub fn emit(&self) -> String {
        let mut buf = String::new();

        for state_idx in 0..self.block_bodies.len() {
            buf.push_str(&format!("const state_{state_idx}: () = ();\n"));
        }

        for (i, block_lines) in self.block_bodies.iter().enumerate() {
            buf.push_str(&format!("\nfn block_{i}(i: &mut AI, wl: &mut VecDeque<usize>) {{\n\t"));
            buf.push_str(&format!("i.merge(state_{i});\n\t"));
            buf.push_str(&block_lines.join(";\n\t"));
            if !block_lines.is_empty() { buf.push_str(";") }

            // let fallthru = stp + 1;
            buf.push_str(&format!("\n}} /* block_{i} */\n"));
        }

        buf
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

#[test]
fn test_gen() {
    type AI = EvalFR;

    let mut wl = std::collections::VecDeque::<usize>::new();

const state_0: () = ();
const state_1: () = ();
const state_2: () = ();
const state_3: () = ();
const state_4: () = ();
const state_5: () = ();

const BLOCKS: [&'static dyn Fn(&mut AI, &mut VecDeque<usize>); 6] = [
    &block_0,
    &block_1,
    &block_2,
    &block_3,
    &block_4,
    &block_5,
];

fn block_0(i: &mut AI, wl: &mut VecDeque<usize>) {
	i.pushi_imm(5);
	i.start_block(0);
	i.pushi_imm(-15);
	i.pushi_imm(20);
	let x1 = i.popi();
	let x2 = i.popi();
	let x3 = x1 + x2;
	i.pushi(x3);
	let x4 = i.popi();
	let x5 = i.popi();
	let x6 = x4 + x5;
	i.pushi(x6);
	wl.push_back(2);
} /* block_0 */

fn block_1(i: &mut AI, wl: &mut VecDeque<usize>) {
	i.pushi_imm(-999);
	i.end();
	wl.push_back(2);
} /* block_1 */

fn block_2(i: &mut AI, wl: &mut VecDeque<usize>) {
	let x7 = i.pop();
	i.set_local(0, x7);
	i.pushi_imm(0);
	let x8 = i.pop();
	i.set_local(1, x8);
	wl.push_back(3);
} /* block_2 */

fn block_3(i: &mut AI, wl: &mut VecDeque<usize>) {
	i.start_loop(0);
	let x9 = i.get_local(0);
	i.pushi(x9);
	let x10 = i.get_local(1);
	i.pushi(x10);
	let x11 = i.popi();
	let x12 = i.popi();
	let x13 = x11 + x12;
	i.pushi(x13);
	let x14 = i.pop();
	i.set_local(1, x14);
	let x15 = i.get_local(0);
	i.pushi(x15);
	i.pushi_imm(-1);
	let x16 = i.popi();
	let x17 = i.popi();
	let x18 = x16 + x17;
	i.pushi(x18);
	let x19 = i.pop();
	i.set_local(0, x19);
	let x20 = i.get_local(0);
	i.pushi(x20);
	let x21 = i.popi();
	let x22 = i.i32_eqz(x21);
	
        let _ = if (x22.maybe_true()) { i.merge(state_4); wl.push_back(4) } else {};
        let _ = if (x22.maybe_false()) { i.merge(state_3); wl.push_back(3) } else {};
} /* block_3 */

fn block_4(i: &mut AI, wl: &mut VecDeque<usize>) {
	i.end();
	wl.push_back(5);
} /* block_4 */

fn block_5(i: &mut AI, wl: &mut VecDeque<usize>) {
	let x23 = i.get_local(1);
	i.pushi(x23);
} /* block_5 */


    // handwritten
    let nlocals = 2;
    let mut interpreter = EvalFR {
            stack: vec![],
            locals: vec![0; nlocals],
            codeptr: CodePtr { code: vec![], ip: 0 },
            sidetable: vec![],
            stp: 0,
    };

    wl.push_back(0);
    while let Some(b) = wl.pop_front() {
        BLOCKS[b](&mut interpreter, &mut wl);
    }
    dbg!(interpreter.stack);
}
