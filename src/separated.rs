use crate::{CodePtr, CodeEntry, Balloon, STEntry, Idk, Type, CtlEntry, SidetableMeta, CtlType};

// base CBD which doesn't have any control flow
pub trait CBDBase {
    type I32Val;
    type StackVal: Clone + Into<Self::LocalVal>;
    type LocalVal: Clone + Into<Self::StackVal>;

    fn popi(&mut self) -> Self::I32Val;
    fn pushi_imm(&mut self, x: i32);
    fn pushi(&mut self, x: Self::I32Val);

    fn push(&mut self, x: Self::StackVal);
    fn pop(&mut self) -> Self::StackVal;

    fn set_local(&mut self, idx: i32, val: Self::LocalVal);
    fn get_local(&mut self, idx: i32) -> Self::LocalVal;

    fn i32_add(&mut self, x: Self::I32Val, y: Self::I32Val) -> Self::I32Val;
    fn i32_eqz(&mut self, x: Self::I32Val) -> bool;
}

// with this CBD, implementations must implement their own control flow primitives
// we have to think of intra vs inter opcode branching
pub trait CBDCtl: CBDBase {
    type CondVal: Balloon;

    fn codeptr_mut(&mut self) -> &mut CodePtr;

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
}

impl CBDCtl for EvalSeparated {
    type CondVal = bool;

    fn codeptr_mut(&mut self) -> &mut CodePtr {
        &mut self.codeptr
    }

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
