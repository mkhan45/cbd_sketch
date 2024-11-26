use crate::{CodePtr, CodeEntry, Balloon, cbdif, STEntry, Type, SidetableMeta, Idk, CtlType, Opcode};

pub trait CBD {
    type I32Val;
    type StackVal: Clone + Into<Self::LocalVal>;
    type LocalVal: Clone + Into<Self::StackVal>;
    type CondVal: Balloon;

    fn codeptr_mut(&mut self) -> &mut CodePtr;

    fn popi(&mut self) -> Self::I32Val;
    fn pushi_imm(&mut self, x: i32);
    fn pushi(&mut self, x: Self::I32Val);

    fn push(&mut self, x: Self::StackVal);
    fn pop(&mut self) -> Self::StackVal;

    fn set_local(&mut self, idx: i32, val: Self::LocalVal);
    fn get_local(&mut self, idx: i32) -> Self::LocalVal;

    // fn push_state(&mut self);
    fn xfer_state(&mut self, stp: usize);

    fn start_block(&mut self, ty_index: usize);
    fn start_loop(&mut self, ty_index: usize);
    fn end(&mut self);

    fn i32_add(&mut self, x: Self::I32Val, y: Self::I32Val) -> Self::I32Val;
    fn i32_eqz(&mut self, x: Self::I32Val) -> Self::CondVal;

    fn branch(&mut self, label_idx: usize);
    fn fallthru(&mut self);

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
}

pub struct WASMFun {
    pub code: Vec<CodeEntry>,
    pub conts: Vec<Cont>,
    pub branches: Vec<Branch>,

    // redundant given conts and branches, but separating
    // conts and branches makes the mental model more explicit
    pub cont_blocks: Vec<ContBlock>,
}

pub struct Cont {
    pub ip: usize,
}

pub struct Branch {
    pub tgt_idx: usize,
}

pub struct ContBlock {
    pub ip: usize,
    // pub fallthru_cont: usize, // easily found at runtime, just the next ContBlock
    pub br_tgt: usize, // only for blocks ended by a Br
    // pub data: T
}

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
                    branches.push(Branch { tgt_idx: ctls[ctl_idx].cont_idx });
                }
                _ => todo!(),
            }
        }

        let code = codeptr.code;

        let mut cont_blocks = vec![];
        let mut br_idx = 0;
        for i in 0..(conts.len() - 1) {
            let current_cont = &conts[i];
            let next_cont = &conts[i + 1];

            let ip = current_cont.ip;
            let br_tgt = {
                match code[next_cont.ip] {
                    CodeEntry::Op(Opcode::Br | Opcode::BrIf) => {
                        let cont_idx = branches[br_idx].tgt_idx;
                        br_idx += 1;
                        cont_idx
                    }
                    CodeEntry::Op(_) => 0,
                    _ => panic!(),
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
}
