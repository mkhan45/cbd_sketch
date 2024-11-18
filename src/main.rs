trait Balloon {
    fn maybe_true(&self) -> bool;
    fn maybe_false(&self) -> bool;
}

impl Balloon for bool {
    fn maybe_true(&self) -> bool { *self }
    fn maybe_false(&self) -> bool { !self }
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
        fn i32_add(&mut self) {
            let x = self.popi();
            let y = self.popi();
            let z = Self::addi32(x, y);
            self.pushi(z);
        }
    }
}

struct Eval {
    pub stack: Vec<i32>,
}

impl Eval {
    fn popi(&mut self) -> i32 {
        self.stack.pop().unwrap()
    }

    fn pushi(&mut self, x: i32) {
        self.stack.push(x)
    }

    fn addi32(x: i32, y: i32) -> i32 {
        x + y
    }

    cbd!();
}

#[derive(Eq, PartialEq)]
enum Type {
    I32,
}

struct Validate {
    pub stack: Vec<Type>,
}

impl Validate {
    fn popi(&mut self) -> Type {
        assert!(self.stack.pop().is_some_and(|t| t == Type::I32));
        Type::I32
    }

    fn pushi(&mut self, t: Type) {
        assert!(t == Type::I32);
        self.stack.push(Type::I32)
    }

    fn addi32(_: Type, _: Type) -> Type {
        Type::I32
    }

    cbd!();
}

fn main() {
    cbdif!{
        if (true) then {
            println!("{}", true);
        }, else {
            println!("{}", false);
        }
    }
}
