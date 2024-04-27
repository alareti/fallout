use std::sync::atomic::{compiler_fence, Ordering};

struct Lock {
    contenders: Vec<Contender>,
}

impl Lock {
    fn new() -> Self {
        let boxed_reg = Box::new([usize::MAX, usize::MAX]);
        let reg_ptr = Box::into_raw(boxed_reg);

        let boxed_t = Box::new(0);
        let t_ptr = Box::into_raw(boxed_t);

        let mut contenders = Vec::new();
        let max = 8;
        for i in 0..max {
            contenders.push(Contender {
                reg: reg_ptr,
                id: !(usize::MAX << (max - i)),
            });
        }

        Lock { contenders }
    }
}

impl Iterator for Lock {
    type Item = Contender;

    fn next(&mut self) -> Option<Self::Item> {
        self.contenders.pop()
    }
}

struct Contender {
    reg: *mut [usize; 2],
    id: usize,
}

impl Contender {
    fn contest(&self) -> Result<(), ()> {
        compiler_fence(Ordering::AcqRel);

        let r;
        unsafe {
            r = self.reg.read();
        }

        // Ensure that lock is not already established
        if r[0] != usize::MAX || r[1] != usize::MAX {
            return Err(());
        }

        // Pull down r0 if insensitive, pull down r1 if sensitive
        let enc = [r[0] & self.id, r[1] & !self.id];
        unsafe {
            self.reg.write(enc);
        }

        // I really hope the compiler doesn't
        // optimize this out. If so, I'll try
        // putting this in a different function
        let r;
        unsafe {
            r = self.reg.read();
        }

        // Ensure one's sensitive region is not
        // contaminated
        let test = self.id & (r[0] ^ r[1]) == self.id;

        // Ensure that the other regions have been
        // properly pulled down
        let test = test && r[0] & !self.id == 0;
        let test = test && r[1] & self.id == 0;

        if !test {
            return Err(());
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contender_ids() {
        let arena = Lock::new();

        for (i, contender) in arena.enumerate() {
            let expected = match i {
                0 => 0b00000001,
                1 => 0b00000011,
                2 => 0b00000111,
                3 => 0b00001111,
                4 => 0b00011111,
                5 => 0b00111111,
                6 => 0b01111111,
                7 => 0b11111111,
                _ => unreachable!(),
            };

            let id = contender.id;
            assert_eq!(expected, id, "\nExpected: {expected:016b}\nGot: {id:016b}");
        }
    }

    #[test]
    fn two_contenders() {
        use std::rc::Rc;
        use std::thread;

        let mut lock = Lock::new();
        let c0 = lock.next().unwrap();
        let c1 = lock.next().unwrap();

        let data: Rc<usize> = Rc::new(0);
        let c_data = Rc::clone(&data);

        let range = 10;
    }
}
