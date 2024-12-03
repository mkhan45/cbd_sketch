use crate::{CodePtr, CodeEntry, Balloon, cbdif, STEntry, Type, SidetableMeta, Idk, CtlType, Opcode};

pub trait CPSCBD {
    type I32Val: Clone + From<i32>;
    type StackVal: Clone + Into<Self::LocalVal>;
    type LocalVal: Clone + Into<Self::StackVal>;
    type CondVal: Balloon;

    fn popi(&mut self) -> Self::I32Val;
    fn pushi_imm(&mut self, x: i32);
    fn pushi(&mut self, x: Self::I32Val);

    fn push(&mut self, x: Self::StackVal);
    fn pop(&mut self) -> Self::StackVal;

    fn set_local(&mut self, idx: i32, val: Self::LocalVal);
    fn get_local(&mut self, idx: i32) -> Self::LocalVal;

    fn xfer_state(&mut self, stp: usize) -> usize;
    fn cond_xfer_state(&mut self, cond: Self::CondVal, left_stp: usize, right_stp: usize) -> usize;

    fn i32_add(&mut self, x: Self::I32Val, y: Self::I32Val) -> Self::I32Val;
    fn i32_eqz(&mut self, x: Self::I32Val) -> Self::CondVal;

    fn cbd_i32_const(&mut self, x: i32) {
        self.pushi(x.into());
    }

    fn cbd_i32_add(&mut self) {
        let x = self.popi();
        let y = self.popi();
        let z = self.i32_add(x, y);
        self.pushi(z);
    }

    fn cbd_local_set(&mut self, idx: i32) {
        let val = self.pop();
        self.set_local(idx, val.into());
    }

    fn cbd_local_get(&mut self, idx: i32) {
        let local = self.get_local(idx);
        self.push(local.into());
    }

    fn cbd_local_tee(&mut self, idx: i32) {
        let val = self.pop(); // TODO: peek()?
        self.push(val.clone());
        self.set_local(idx, val.into());
    }

    fn cbd_block(&mut self, typ_idx: usize) {
    }

    fn cbd_loop(&mut self, typ_idx: usize) {
    }

    fn cbd_br(&mut self, target_stp: usize) -> usize {
        self.xfer_state(target_stp)
    }

    fn cbd_br_if(&mut self, target_stp: usize, fallthru_stp: usize) -> usize {
        let condv = self.popi();
        let condb = self.i32_eqz(condv);
        self.cond_xfer_state(condb, fallthru_stp, target_stp)
    }
    
    fn cbd_end(&mut self) {
    }
}

pub trait CPSCBDDebug: CPSCBD + std::fmt::Debug {
    fn stack(&self) -> &[Self::StackVal];
}

pub struct WASMFun {
    pub code: Vec<CodeEntry>,
    pub conts: Vec<Cont>,
    pub branches: Vec<Branch>,

    // redundant given conts and branches, but separating
    // conts and branches makes the mental model more explicit
    pub cont_blocks: Vec<ContBlock>,
}

#[derive(Debug)]
pub struct Cont {
    pub ip: usize,
}

#[derive(Debug)]
pub struct Branch {
    pub tgt_idx: usize,
}

#[derive(Debug, Copy, Clone)]
pub struct ContBlock {
    pub ip: usize,
    // pub fallthru_cont: usize, // easily found at runtime, just the next ContBlock
    pub br_tgt: usize, // only for blocks ended by a Br
    // pub data: T
}

#[derive(Debug)]
pub struct CtlEntry {
    pub ty: CtlType,
    pub entry_ip: usize,
    pub cont_idx: usize,
    pub fallthru_ip: usize,
}

impl WASMFun {
    pub fn new(code: Vec<CodeEntry>) -> Self {
        let mut ctls = vec![
            CtlEntry {
                ty: CtlType::Func,
                entry_ip: 0,
                cont_idx: 0, // set to conts.len() - 1
                fallthru_ip: code.len(),
            }
        ];
        let mut ctl_stack = vec![0];
        let mut codeptr = CodePtr { code, ip: 0 };

        let mut conts = vec![Cont { ip: 0 }];
        let mut branches = vec![];

        while let Some(op) = codeptr.next() {
            use {Opcode::*, CodeEntry::*};
            match op {
                Op(I32Const | LocalSet | LocalGet) => {
                    codeptr.read_imm_i32();
                }
                Op(Block) => {
                    let _bt = codeptr.read_block_type();
                    ctl_stack.push(ctls.len());
                    ctls.push(CtlEntry {
                        ty: CtlType::Block,
                        entry_ip: codeptr.ip,
                        cont_idx: 0,
                        fallthru_ip: 0,
                    });
                }
                Op(Loop) => {
                    let _bt = codeptr.read_block_type();
                    ctl_stack.push(ctls.len());
                    ctls.push(CtlEntry {
                        ty: CtlType::Loop,
                        entry_ip: codeptr.ip,
                        cont_idx: conts.len(),
                        fallthru_ip: 0,
                    });
                    conts.push(Cont { ip: codeptr.ip });
                }
                Op(End) => {
                    let ctl_idx = ctl_stack.pop().unwrap();
                    let ctl = &mut ctls[ctl_idx];
                    ctl.fallthru_ip = codeptr.ip;
                    if ctl.ty == CtlType::Block {
                        ctl.cont_idx = conts.len();
                    }
                    conts.push(Cont { ip: codeptr.ip });
                }
                Op(BrIf | Br) => {
                    let depth = codeptr.read_imm_i32() as usize;

                    conts.push(Cont { ip: codeptr.ip });
                    let ctl_idx = ctl_stack[ctl_stack.len() - 1 - depth];
                    branches.push(Branch { tgt_idx: ctl_idx });
                }
                Op(_) => {},
                I32Imm(_) | BlockType(_) => panic!(),
            }
        }

        dbg!(&conts);
        dbg!(&branches);

        let code = codeptr.code;

        let mut cont_blocks = vec![];
        let mut br_idx = 0;
        for i in 0..(conts.len() - 1) {
            let current_cont = &conts[i];
            let next_cont = &conts[i + 1];

            let ip = current_cont.ip;
            let br_tgt = {
                match code[next_cont.ip - 2] {
                    CodeEntry::Op(Opcode::Br | Opcode::BrIf) => {
                        let ctl_idx = branches[br_idx].tgt_idx;
                        let cont_idx = ctls[ctl_idx].cont_idx;
                        br_idx += 1;
                        cont_idx
                    }
                    _ => 0,
                }
            };
            cont_blocks.push(ContBlock { ip, br_tgt });
        }

        let last_cont = &conts[conts.len() - 1];
        cont_blocks.push(ContBlock {
            ip: last_cont.ip,
            br_tgt: 0,
        });

        Self {
            code,
            conts,
            branches,
            cont_blocks,
        }
    }

    // this is bad because it can't handle multiple simultaneous out-branches
    pub fn run<I: CPSCBD>(&mut self, mut interpreter: I) -> I {
        let mut codeptr = CodePtr { code: std::mem::take(&mut self.code), ip: 0 };
        let mut current_block = 0;

        while let Some(op) = codeptr.next() {
            use {Opcode::*, CodeEntry::*};
            match op {
                Op(I32Const) => {
                    let imm = codeptr.read_imm_i32();
                    interpreter.cbd_i32_const(imm);
                }
                Op(I32Add) => interpreter.cbd_i32_add(),
                Op(LocalSet) => {
                    let local_idx = codeptr.read_imm_i32();
                    interpreter.cbd_local_set(local_idx);
                }
                Op(LocalGet) => {
                    let local_idx = codeptr.read_imm_i32();
                    interpreter.cbd_local_get(local_idx);
                }
                Op(Block) => {
                    let typ_idx = codeptr.read_block_type(); 
                    interpreter.cbd_block(typ_idx);
                }
                Op(Loop) => {
                    let typ_idx = codeptr.read_block_type(); 
                    interpreter.cbd_loop(typ_idx);
                    current_block += 1;
                }
                Op(Br) => {
                    let _depth = codeptr.read_imm_i32();
                    let cur_block = self.cont_blocks[current_block];
                    let tgt_block = cur_block.br_tgt;
                    let end_block = interpreter.cbd_br(tgt_block);

                    current_block = end_block;
                    codeptr.ip = self.cont_blocks[current_block].ip;
                }
                Op(BrIf) => {
                    let _depth = codeptr.read_imm_i32();
                    let cur_block = self.cont_blocks[current_block];
                    let tgt_block = cur_block.br_tgt;
                    let end_block = interpreter.cbd_br_if(tgt_block, current_block + 1);

                    current_block = end_block;
                    codeptr.ip = self.cont_blocks[current_block].ip;
                }
                Op(End) => {
                    interpreter.cbd_end();
                    current_block += 1;
                }
                _ => panic!(),
            }
        }

        self.code = codeptr.code;
        interpreter
    }

    // TODO:
    // make a run() that iterates current_block in this style and uses xfer_state and fetch_state()
    // or similar.
    pub fn compile<I: CPSCBD>(self) -> CompiledFun<I> {
        let mut res = CompiledFun { conts: vec![] };

        for current_block in 0..self.cont_blocks.len() {
            let start_ip = self.cont_blocks[current_block].ip;
            let tgt_block = self.cont_blocks[current_block].br_tgt;

            let fallthru_block = current_block + 1;

            res.conts.push(Box::new(move |compiled: *const CompiledFun<I>, mut interpreter: I, codeptr: &mut CodePtr| unsafe {
                codeptr.ip = start_ip;
                // TODO: xfer state into this cont
                while let Some(op) = codeptr.next() {
                    use {Opcode::*, CodeEntry::*};
                    match op {
                        Op(I32Const) => {
                            let imm = codeptr.read_imm_i32();
                            interpreter.cbd_i32_const(imm);
                        }
                        Op(I32Add) => interpreter.cbd_i32_add(),
                        Op(LocalSet) => {
                            let local_idx = codeptr.read_imm_i32();
                            interpreter.cbd_local_set(local_idx);
                        }
                        Op(LocalGet) => {
                            let local_idx = codeptr.read_imm_i32();
                            interpreter.cbd_local_get(local_idx);
                        }
                        Op(Block) => {
                            let typ_idx = codeptr.read_block_type(); 
                            interpreter.cbd_block(typ_idx);
                        }
                        Op(Loop) => {
                            let typ_idx = codeptr.read_block_type(); 
                            interpreter.cbd_loop(typ_idx);

                            let cont = &(*compiled).conts[fallthru_block];
                            return cont(compiled, interpreter, codeptr);
                        }
                        Op(Br) => {
                            let _depth = codeptr.read_imm_i32();

                            let cont = &(*compiled).conts[tgt_block];
                            return cont(compiled, interpreter, codeptr);
                        }
                        Op(BrIf) => {
                            let _depth = codeptr.read_imm_i32();
                            let end_block = interpreter.cbd_br_if(tgt_block, fallthru_block);

                            let cont = &(*compiled).conts[end_block];
                            return cont(compiled, interpreter, codeptr);
                        }
                        Op(End) => {
                            interpreter.cbd_end();

                            let cont = &(*compiled).conts[fallthru_block];
                            return cont(compiled, interpreter, codeptr);
                        }
                        _ => {
                            dbg!(op);
                            panic!();
                        }
                    }
                }
                return interpreter
            }));
        }
        res.conts.push(Box::new(|_, i, _| i));

        res
    }
}

pub struct CompiledFun<I: CPSCBD> {
    pub conts: Vec<Box<dyn Fn(*const CompiledFun<I>, I, &mut CodePtr) -> I>>,
}

#[derive(Debug)]
pub struct CPSEval {
    pub stack: Vec<i32>,
    pub locals: Vec<i32>,
}

impl CPSCBD for CPSEval {
    type I32Val = i32;
    type StackVal = i32;
    type LocalVal = i32;
    type CondVal = bool;

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

    fn i32_add(&mut self, x: i32, y: i32) -> i32 {
        x + y
    }

    fn i32_eqz(&mut self, x: i32) -> bool {
        x == 0
    }

    fn xfer_state(&mut self, stp: usize) -> usize { stp }
    fn cond_xfer_state(&mut self, cond: bool, left_stp: usize, right_stp: usize) -> usize { 
        if cond { left_stp } else { right_stp }
    }
}

impl CPSCBDDebug for CPSEval {
    fn stack(&self) -> &[i32] {
        &self.stack
    }
}
