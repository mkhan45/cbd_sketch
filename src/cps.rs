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

    fn xfer_state(&mut self, stp: usize);
    fn cond_xfer_state(&mut self, cond: Self::CondVal, left_stp: usize, right_stp: usize);

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

    fn cbd_block(&mut self, typ_idx: i32) {
    }

    fn cbd_loop(&mut self, typ_idx: i32) {
    }

    fn cbd_br(&mut self, target_stp: usize) {
        self.xfer_state(target_stp);
    }

    fn cbd_br_if(&mut self, target_stp: usize, fallthru_stp: usize) {
        let condv = self.popi();
        let condb = self.i32_eqz(condv);
        self.cond_xfer_state(condb, fallthru_stp, target_stp);
    }
    
    fn cbd_end(&mut self) {
    }
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

        Self {
            code,
            conts,
            branches,
            cont_blocks,
        }
    }

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
                    let typ_idx = codeptr.read_imm_i32(); 
                    interpreter.cbd_block(typ_idx);
                }
                Op(Loop) => {
                    let typ_idx = codeptr.read_imm_i32(); 
                    interpreter.cbd_loop(typ_idx);
                    current_block += 1;
                }
                Op(Br) => {
                    let _depth = codeptr.read_imm_i32();
                    let cur_block = self.cont_blocks[current_block];
                    let tgt_block = cur_block.br_tgt;
                    interpreter.cbd_br(tgt_block);
                    current_block += 1;
                }
                Op(BrIf) => {
                    let _depth = codeptr.read_imm_i32();
                    let cur_block = self.cont_blocks[current_block];
                    let tgt_block = cur_block.br_tgt;
                    interpreter.cbd_br_if(tgt_block, current_block + 1);
                    current_block += 1;
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
}
