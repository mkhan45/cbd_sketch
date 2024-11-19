use crate::{CodePtr, Balloon, cbdif, STEntry, Type, SidetableMeta, CtlEntry, Idk, CtlType};

// I don't think Virgil really has the type system
// to check this, but it might make sense to build
// it the DSL such that typechecking the resulting virgil
// makes sure these are consistent
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

pub struct TypedEval {
    pub stack: Vec<i32>,
    pub locals: Vec<i32>,
    pub codeptr: CodePtr,
    pub sidetable: Vec<STEntry>,
    pub stp: usize,
}

impl CBD for TypedEval {
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
}

pub struct TypedValidate {
    pub stack: Vec<Type>,
    pub locals: Vec<Type>,
    pub ctl_entries: Vec<CtlEntry>,
    pub ctl_stack: Vec<usize>,
    pub codeptr: CodePtr,
    pub sidetable_meta: Vec<SidetableMeta>, // idx = br_index
}

impl CBD for TypedValidate {
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

impl TypedValidate {
    fn is_loop(&self) -> bool {
        self.ctl_entries.last().unwrap().tipe == CtlType::Loop
    }

    pub fn build_sidetable(&self) -> Vec<STEntry> {
        self.sidetable_meta.iter().enumerate().map(|(stp, br_meta)| {
            let target_ctl_idx = br_meta.target_ctl_idx;
            let target_ctl = &self.ctl_entries[target_ctl_idx];
            STEntry {
                ip_delta: (target_ctl.cont_ip as isize) - (br_meta.br_ip as isize),
                stp_delta: (target_ctl.cont_stp as isize) - (stp as isize),
            }
        }).collect()
    }
}
