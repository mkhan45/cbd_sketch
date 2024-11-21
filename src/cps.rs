use crate::{CodeEntry, CodePtr};

pub struct WASMFun {
    pub code: Vec<CodeEntry>,
    pub conts: Vec<Cont>,
}

pub struct Cont {
    pub ip: usize,
}

impl WASMFun {
    pub fn new(code: Vec<CodeEntry>) -> Self {
        let mut conts = vec![];
        
        let mut ctl_stack = vec![0];
        let mut codeptr = CodePtr { code, ip: 0 };

        while let Some(op) = codeptr.next() {
            match op {
                _ => todo!(),
            }
        }

        Self {
            code: codeptr.code,
            conts,
        }
    }
}
