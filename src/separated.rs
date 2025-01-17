use crate::{Opcode, CodePtr, CodeEntry, Balloon, STEntry, Idk, Type, CtlEntry, SidetableMeta, CtlType};

// base CBD which doesn't have any control flow
pub trait CBDBase {
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

    fn i32_add(&mut self, x: Self::I32Val, y: Self::I32Val) -> Self::I32Val;

    // TODO: which trait does this go in?
    fn i32_eqz(&mut self, x: Self::I32Val) -> Self::CondVal;

    fn start_block(&mut self, ty_index: usize);
    fn start_loop(&mut self, ty_index: usize);

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
        let val = self.pop();
        self.push(val.clone());
        self.set_local(idx, val.into());
    }
}

// with this CBD, implementations must implement their own control flow primitives
// we have to think of intra vs inter opcode branching
pub trait CBDCtl: CBDBase {
    fn branch(&mut self, label_idx: usize);
    fn fallthru(&mut self);
    fn end(&mut self);

    fn cbd_br(&mut self) {
        let label_idx = self.codeptr_mut().read_imm_i32();
        self.branch(label_idx as usize);
    }

    fn cbd_br_if(&mut self) {
        let label_idx = self.codeptr_mut().read_imm_i32();
        let condv = self.popi();
        let condb = self.i32_eqz(condv); 

        if condb.maybe_true() {
            self.fallthru()
        }
        if condb.maybe_false() {
            self.branch(label_idx as usize)
        }
    }

    fn cbd_end(&mut self) {
        self.end();
    }

    fn cbd_block(&mut self) {
        let ty = self.codeptr_mut().read_block_type();
        self.start_block(ty);
    }

    fn cbd_loop(&mut self) {
        let ty = self.codeptr_mut().read_block_type();
        self.start_loop(ty);
    }
}

pub struct EvalSeparated {
    pub stack: Vec<i32>,
    pub locals: Vec<i32>,
    pub codeptr: CodePtr,
    pub sidetable: Vec<STEntry>,
    pub stp: usize,
}

impl CBDBase for EvalSeparated {
    type I32Val = i32;
    type StackVal = i32;
    type LocalVal = i32;
    type CondVal = bool;

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

    fn i32_add(&mut self, x: i32, y: i32) -> i32 {
        x + y
    }

    fn i32_eqz(&mut self, x: i32) -> bool {
        x == 0
    }

    fn start_block(&mut self, _ty_index: usize) {}
    fn start_loop(&mut self, _ty_index: usize) {}
}

impl CBDCtl for EvalSeparated {
    fn branch(&mut self, _label_idx: usize) {
        self.stp += 1;
        let ste = self.sidetable[self.stp];
        self.codeptr.ip = ((self.codeptr.ip as isize) + ste.ip_delta) as usize;
        self.stp = ((self.stp as isize) + ste.stp_delta) as usize;
    }

    fn fallthru(&mut self) {
        self.stp += 1;
    }

    fn end(&mut self) {}
}

// TODO:
// 1. impl CBDCtl validator which builds sidetable
//      - Why isn't validator an instance of the abstract interpretation?
//          - it keeps keep a more limited sidetable, and doesn't ever need
//            to run stuff more than once
//          - maybe it should be
// 2. impl CBDCtl abstract interpreter which requires another CBDAbstract trait?
//      - the abstract *interpreter* will do all its sidetable stuff at runtime,
//        but we will use it to make an abstract compiler that specializes to it
//      - we should be able to get a concrete interpreter by implementing the CBDAbstract
//        trait with no-ops
//      - and a validator by implementing abstract stuff in sane ways, but not necessarily
//        similarly to how the base validator works

pub struct SeparatedValidate {
    pub stack: Vec<Type>,
    pub locals: Vec<Type>,
    pub ctl_entries: Vec<CtlEntry>,
    pub ctl_stack: Vec<usize>,
    pub codeptr: CodePtr,
    pub sidetable_meta: Vec<SidetableMeta>, // idx = br_index
}

impl CBDBase for SeparatedValidate {
    type I32Val = Type;
    type StackVal = Type;
    type LocalVal = Type;
    type CondVal = Idk;

    fn codeptr_mut(&mut self) -> &mut CodePtr {
        &mut self.codeptr
    }


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

    fn i32_add(&mut self, _: Type, _: Type) -> Type {
        Type::I32
    }

    fn i32_eqz(&mut self, t: Type) -> Idk {
        assert!(t == Type::I32);
        Idk
    }
}

impl CBDCtl for SeparatedValidate {
    fn branch(&mut self, label_idx: usize) {
        let ctl_idx = self.ctl_stack.last().unwrap() - label_idx;
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
}

pub trait CBDAbstract: CBDBase {
    type MergeState: Default;
    fn merge(&mut self, other: &Self::MergeState);
    fn merge_into(&self, other: &mut Self::MergeState);
    fn merge_state(&self) -> Self::MergeState;
}

// could have this for normal runner
// as well
pub struct CBDAI<T: CBDAbstract> {
    pub interpreter: T,

    pub states: Vec<T::MergeState>,
    pub ctl_stack: Vec<usize>,
}

pub enum AICtl {
    Func { ret_cfg_idx: usize },
    Block { end_cfg_idx: usize },
    Loop { start_cfg_idx: usize, end_cfg_idx: usize },
}

impl<T: CBDAbstract> CBDAI<T> {
    pub fn run(&mut self, code: Vec<CodeEntry>) {
        let interpreter = &mut self.interpreter;
        let mut codeptr = CodePtr { code, ip: 0 };
        let mut ctls: Vec<AICtl> = vec![ AICtl::Func { ret_cfg_idx: 0 }, ];
        let mut ctl_stack: Vec<usize> = vec![0];
        let mut cfg_states: Vec<Option<T::MergeState>> = vec![None];

        while let Some(op) = codeptr.read_op() {
            match op {
                Opcode::Block => {
                    // blocks go on the CTL stack, and have a CFG node
                    // for the end label
                    ctl_stack.push(ctls.len());
                    ctls.push(AICtl::Block { end_cfg_idx: cfg_states.len() });
                    cfg_states.push(None);
                }
                Opcode::Loop => {
                    // loops go on the CTL stack, and create a CFG
                    // node for both the start and end labels
                    ctl_stack.push(ctls.len());

                    // we already have the intial state for the start label
                    let start_cfg_idx = cfg_states.len();
                    cfg_states.push(Some(interpreter.merge_state()));

                    let end_cfg_idx = cfg_states.len();
                    cfg_states.push(None);
                    ctls.push(AICtl::Loop { start_cfg_idx, end_cfg_idx });
                }
                Opcode::End => {
                    // Ends pop from CTL stack and start a CFG node
                    let ctl_idx = ctl_stack.pop().unwrap();
                    let ctl = &ctls[ctl_idx];
                    let target_cfg_idx = match ctl {
                        AICtl::Func { ret_cfg_idx } => ret_cfg_idx,
                        AICtl::Block { end_cfg_idx } | AICtl::Loop { end_cfg_idx, .. } => end_cfg_idx,
                    };

                    if let Some(s) = cfg_states[*target_cfg_idx].as_mut() {
                        interpreter.merge_into(s);
                    } else {
                        cfg_states[*target_cfg_idx] = Some(interpreter.merge_state());
                    }
                }

                // merge into target
                Opcode::Br | Opcode::BrIf => {
                    let label_offset = codeptr.read_imm_i32() as usize;
                    let target_ctl = &ctls[ctls.len() - 1 - label_offset];
                    let target_cfg_idx = match target_ctl {
                        AICtl::Func { ret_cfg_idx } => ret_cfg_idx,
                        AICtl::Block { end_cfg_idx } => end_cfg_idx,

                        // breaking out of a loop goes to start, not end
                        AICtl::Loop { start_cfg_idx, .. } => start_cfg_idx,
                    };

                    if let Some(s) = cfg_states[*target_cfg_idx].as_mut() {
                        interpreter.merge_into(s);
                    } else {
                        cfg_states[*target_cfg_idx] = Some(interpreter.merge_state());
                    }
                }
                
                // non-ctl stuff, should be op_dispatch!
                Opcode::I32Const => interpreter.cbd_i32_const(),
                Opcode::I32Add => interpreter.cbd_i32_add(),
                Opcode::LocalSet => interpreter.cbd_local_set(),
                Opcode::LocalGet => interpreter.cbd_local_get(),
            }
        }
    }
}
