use crate::{CodePtr, CodeEntry, Balloon};

// TODO:
// 1. a CBD written in direct interpreter style
//      - with some mechanism for branching control flow within the CBD
// 2. an interpreter and validator written using that CBD
// 3. an abstract compiler written using the CBD, which can be used
//    on the interpreter to make a compiler
pub trait CBD_FR {
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

    fn cbd_br_if(&mut self, target_stp: usize, fallthru_stp: usize) -> usize {
        let condv = self.popi();
        let condb = self.i32_eqz(condv);
        self.cond_xfer_state(condb, fallthru_stp, target_stp)
    }
}
