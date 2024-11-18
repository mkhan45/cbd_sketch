#![allow(dead_code)]

trait Balloon {
    fn maybe_true(&self) -> bool;
    fn maybe_false(&self) -> bool;
}

impl Balloon for bool {
    fn maybe_true(&self) -> bool { *self }
    fn maybe_false(&self) -> bool { !self }
}

struct Idk;
impl Balloon for Idk {
    fn maybe_true(&self) -> bool { true }
    fn maybe_false(&self) -> bool { true }
}

macro_rules! cbdif {
    (if ($cond:expr) then $t:expr, else $e:expr) => {
        let ceval = $cond;
        if ceval.maybe_true() {
            $t
        } 
        if ceval.maybe_false() {
            $e
        }
    }
}

macro_rules! cbd {
    () => {
        fn cbd_i32_const(&mut self) {
            let x = self.codeptr.read_imm_i32();
            self.pushi_imm(x);
        }

        fn cbd_i32_add(&mut self) {
            let x = self.popi();
            let y = self.popi();
            let z = Self::addi32(x, y);
            self.pushi(z);
        }

        fn cbd_block(&mut self) {
            let ty = self.codeptr.read_block_type();
            self.start_block(ty);
        }

        fn cbd_br(&mut self) {
            let label_idx = self.codeptr.read_imm_i32();
            self.branch(label_idx as usize);
        }

        fn cbd_br_if(&mut self) {
            let label_idx = self.codeptr.read_imm_i32();
            let condv = self.popi();
            let condb = Self::i32_eqz(condv); // make it a member fn just in case it could mutate,
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

        fn dispatch(&mut self) {
            use Opcode::*;

            while let Some(op) = self.codeptr.read_op() {
                // could probably be macro'd
                match op {
                    I32Const => self.cbd_i32_const(),
                    I32Add => self.cbd_i32_add(),
                    Block => self.cbd_block(),
                    End => self.cbd_end(),
                    Br => self.cbd_br(),
                }
            }
        }
    }
}

#[derive(Copy, Clone)]
enum Opcode {
    I32Const,
    I32Add,
    Block,
    End,
    Br,
}

#[derive(Copy, Clone)]
enum CodeEntry {
    Op(Opcode),
    I32Imm(i32),
    BlockType(usize),
}
struct CodePtr {
    pub code: Vec<CodeEntry>,
    pub ip: usize,
}
impl CodePtr {
    pub fn next(&mut self) -> Option<&CodeEntry> {
        let ret = self.code.get(self.ip);
        self.ip += 1;
        ret
    }

    pub fn read_op(&mut self) -> Option<Opcode> {
        match self.next() {
            Some(CodeEntry::Op(o)) => Some(*o),
            Some(_) => panic!("not an opcode"),
            None => None,
        }
    }

    pub fn read_block_type(&mut self) -> usize {
        match self.next() {
            Some(CodeEntry::BlockType(i)) => *i,
            _ => panic!("not a block type"),
        }
    }
    pub fn read_imm_i32(&mut self) -> i32 {
        match self.next() {
            Some(CodeEntry::I32Imm(i)) => *i,
            _ => panic!("not an i32 imm"),
        }
    }
}

#[derive(Debug, Copy, Clone)]
struct STEntry {
    pub ip_delta: isize,
    pub target_stp: usize, // not sure why wizard uses deltas?
}

struct Eval {
    pub stack: Vec<i32>,
    pub codeptr: CodePtr,
    pub sidetable: Vec<STEntry>,
    pub stp: usize,
}

impl Eval {
    fn popi(&mut self) -> i32 {
        self.stack.pop().unwrap()
    }

    fn pushi_imm(&mut self, x: i32) {
        self.pushi(x)
    }
    fn pushi(&mut self, x: i32) {
        self.stack.push(x)
    }

    fn start_block(&mut self, _ty_index: usize) { }
    fn end(&mut self) { }

    fn addi32(x: i32, y: i32) -> i32 {
        x + y
    }

    fn i32_eqz(x: i32) -> bool {
        x == 0
    }

    fn branch(&mut self, _label_idx: usize) {
        let ste = self.sidetable[self.stp];
        self.codeptr.ip = ((self.codeptr.ip as isize) + ste.ip_delta) as usize; // lame
        self.stp = ste.target_stp;
    }

    fn fallthru(&mut self) {
        self.codeptr.ip += 1;
    }

    cbd!();
}

#[derive(Eq, PartialEq)]
enum Type {
    I32,
}

#[derive(PartialEq, Eq)]
enum CtlType {
    Func,
    Block,
    Loop,
}

struct CtlEntry {
    tipe: CtlType,
    cont_ip: usize,
    stp: usize, // essentially idx of first branch
}

struct SidetableMeta {
    br_ip: usize,
    target_ctl_idx: usize,
}

struct Validate {
    pub stack: Vec<Type>,
    pub ctl_stack: Vec<CtlEntry>,
    pub ctl_sp: usize,
    pub codeptr: CodePtr,
    pub sidetable_meta: Vec<SidetableMeta>, // idx = br_index
}

impl Validate {
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

    fn start_block(&mut self, _ty_index: usize) {
        self.ctl_stack.push(CtlEntry {
            tipe: CtlType::Block,
            cont_ip: 0, // filled in later
            stp: 0, // filled in later
        });
        self.ctl_sp += 1;
    }

    fn addi32(_: Type, _: Type) -> Type {
        Type::I32
    }

    fn i32_eqz(t: Type) -> Idk {
        assert!(t == Type::I32);
        Idk
    }

    fn branch(&mut self, label_idx: usize) {
        let st_idx = self.ctl_sp - 1 - label_idx;
        if let Some(prev_br) = self.sidetable_meta.last_mut() {
            prev_br.target_ctl_idx = self.ctl_sp - 1;
        }
        self.sidetable_meta.push(SidetableMeta {
            br_ip: self.codeptr.ip,
            target_ctl_idx: 0, // filled later by line above
        });
        let _target_ctl = &self.ctl_stack[st_idx];
        // validate
    }

    fn fallthru(&mut self) {
        self.codeptr.ip += 1;
    }

    fn end(&mut self) {
        let ctl = &mut self.ctl_stack[self.ctl_sp - 1];
        if ctl.tipe == CtlType::Block {
            ctl.cont_ip = self.codeptr.ip;
        }
    }

    fn build_sidetable(&self) -> Vec<STEntry> {
        self.sidetable_meta.iter().map(|br_meta| {
            let target_ctl = &self.ctl_stack[br_meta.target_ctl_idx];
            STEntry {
                ip_delta: (target_ctl.cont_ip as isize) - (br_meta.br_ip as isize),
                target_stp: target_ctl.stp,
            }
        }).collect()
    }

    cbd!();
}

fn main() {
    use CodeEntry::*;
    use Opcode::*;
    let code = vec![
        Op(I32Const), I32Imm(5),
        Op(Block), BlockType(0),
            Op(I32Const), I32Imm(15),
            Op(I32Const), I32Imm(-20),
            Op(Br), I32Imm(0),
            Op(I32Add),
        Op(End),
    ];

    let mut validate = Validate {
        stack: vec![],
        ctl_stack: vec![CtlEntry { tipe: CtlType::Func, cont_ip: code.len(), stp: 0 }],
        ctl_sp: 0,
        codeptr: CodePtr { code: code.clone(), ip: 0 },
        sidetable_meta: vec![],
    };
    validate.dispatch();
    let sidetable = validate.build_sidetable();
    dbg!(&sidetable);

    let mut eval = Eval {
        stack: vec![],
        codeptr: CodePtr { code, ip: 0 },
        sidetable,
        stp: 0,
    };
    eval.dispatch();
    dbg!(eval.stack);
}
