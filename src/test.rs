use crate::tf::{TypedEval, TypedValidate, CBD};
use crate::{CodePtr, SidetableMeta, CtlType, CtlEntry, Type, sum_code};

#[cfg(test)]
impl TypedEval {
    fn run(&mut self) {
        self.pushi(5);
        self.pushi(-15);
        self.pushi(20);
        let x_1 = self.stack.pop().unwrap();
        let x_2 = self.stack.pop().unwrap();
        let x_3 = x_1 + x_2;
        self.stack.push(x_3);
        let x_4 = self.stack.pop().unwrap();
        let x_5 = self.stack.pop().unwrap();
        let x_6 = x_4 + x_5;
        self.stack.push(x_6);

        self.stp += 1;
        let ste = self.sidetable[self.stp];
        // stupid casts
        self.codeptr.ip = ((self.codeptr.ip as isize) + ste.ip_delta) as usize;
        self.stp = ((self.stp as isize) + ste.stp_delta) as usize;

        self.pushi(-999);
        let x_7 = self.stack.pop().unwrap();
        self.locals[0 as usize] = x_7;
        self.pushi(0);
        let x_8 = self.stack.pop().unwrap();
        self.locals[1 as usize] = x_8;
        let x_9 = self.locals[0 as usize];
        self.stack.push(x_9);
        let x_10 = self.locals[1 as usize];
        self.stack.push(x_10);
        let x_11 = self.stack.pop().unwrap();
        let x_12 = self.stack.pop().unwrap();
        let x_13 = x_11 + x_12;
        self.stack.push(x_13);
        let x_14 = self.stack.pop().unwrap();
        self.locals[1 as usize] = x_14;
        let x_15 = self.locals[0 as usize];
        self.stack.push(x_15);
        self.pushi(-1);
        let x_16 = self.stack.pop().unwrap();
        let x_17 = self.stack.pop().unwrap();
        let x_18 = x_16 + x_17;
        self.stack.push(x_18);
        let x_19 = self.stack.pop().unwrap();
        self.locals[0 as usize] = x_19;
        let x_20 = self.locals[0 as usize];
        self.stack.push(x_20);
        let x_21 = self.stack.pop().unwrap();
        let x_22 = x_21 == 0;
        self.stp += 1;

        self.stp += 1;
        let ste = self.sidetable[self.stp];
        // stupid casts
        self.codeptr.ip = ((self.codeptr.ip as isize) + ste.ip_delta) as usize;
        self.stp = ((self.stp as isize) + ste.stp_delta) as usize;

        let x_23 = self.locals[1 as usize];
        self.stack.push(x_23);

    }
}

#[test]
fn test_compile() {
    let nlocals = 2;
    let code = sum_code();
    let mut validate = TypedValidate {
        stack: vec![],
        locals: vec![Type::I32; nlocals],
        ctl_entries: vec![CtlEntry { tipe: CtlType::Func, cont_ip: code.len(), cont_stp: 0 }],
        ctl_stack: vec![0],
        codeptr: CodePtr { code: code.clone(), ip: 0 },
        sidetable_meta: vec![SidetableMeta { br_ip: 0, target_ctl_idx: 0 } ],
    };
    validate.dispatch();
    // dbg!(&validate.ctl_stack);
    // dbg!(&validate.ctl_entries);
    let sidetable = validate.build_sidetable();
    let mut teval = TypedEval {
        stack: vec![],
        locals: vec![0; nlocals],
        codeptr: CodePtr { code: vec![], ip: 0 },
        sidetable,
        stp: 0,
    };
    teval.run();
    dbg!(teval.stack);
}
