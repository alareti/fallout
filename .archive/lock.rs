// We only need compiler_fence for to hint at the
// compiler not to reorder our stuff.
use std::sync::atomic::{compiler_fence, Ordering};

struct Lock {
    contenders: Vec<Contender>,
}

impl Lock {
    fn new() -> Self {
        let boxed_reg = Box::new([usize::MAX, usize::MAX]);
        let reg_ptr = Box::into_raw(boxed_reg);

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

unsafe impl Send for Contender {}
struct Contender {
    reg: *mut [usize; 2],
    id: usize,
}

impl Contender {
    fn contest(&self) -> Result<(), ()> {
        let indent = match self.id {
            0b1 => "",
            0b11 => "\t\t",
            0b111 => "\t\t\t\t",
            _ => "",
        };
        println!("{indent}Contender {:0b}: Initiating contest", self.id);
        println!(
            "{indent}Contender {:0b}: Pointer address: {}",
            self.id, self.reg as usize
        );

        compiler_fence(Ordering::SeqCst);
        let r;
        unsafe {
            r = self.reg.read();
        }

        println!(
            "{indent}Contender {:0b}: Checking high impedance\n\t{indent}r0: {:08b}\n\t{indent}r1: {:08b}",
            self.id, r[0], r[1]
        );

        // Ensure that lock is not already established
        if r[0] != usize::MAX || r[1] != usize::MAX {
            println!(
                "{indent}Contender {:0b}: Lock already established. Returning",
                self.id
            );
            return Err(());
        }

        println!(
            "{indent}Contender {:0b}: Success, initiating pull down",
            self.id
        );

        // Pull down r0 if insensitive, pull down r1 if sensitive
        let enc = [r[0] & self.id, r[1] & !self.id];
        unsafe {
            self.reg.write(enc);
        }

        println!("{indent}Contender {:0b}: Sleeping", self.id);
        use std::{thread, time};
        thread::sleep(time::Duration::from_nanos(200));

        let r;
        unsafe {
            r = self.reg.read();
        }

        println!(
            "{indent}Contender {:0b}: Register after pull down\n\t{indent}r0: {:08b}\n\t{indent}r1: {:08b}",
            self.id, r[0], r[1]
        );

        // Ensure one's sensitive region is not
        // contaminated
        let test = self.id & (r[0] ^ r[1]) == self.id;

        // println!("First test: {test}");

        // Ensure that the other regions have been
        // properly pulled down
        let test = test && r[0] & !self.id == 0;
        // println!("Second test: {test}");

        let test = test && r[1] & self.id == 0;
        // println!("Third test: {test}");

        if !test {
            return Err(());
        }

        Ok(())
    }

    fn reset(&self) {
        unsafe {
            self.reg.write([usize::MAX, usize::MAX]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eight_contenders() {
        use crate::push_pull;
        use std::{thread, time};

        let mut lock = Lock::new();
        let mut sockets = Vec::new();

        for _ in 0..2 {
            let contender = lock.next().unwrap();

            let ((c_to_main, main_from_c), (main_to_c, c_from_main)) =
                unsafe { (push_pull::channel(), push_pull::channel()) };

            sockets.push((main_to_c, main_from_c));

            thread::spawn(move || {
                let mut rx = c_from_main;
                let mut tx = c_to_main;
                for _ in 0..1 {
                    loop {
                        if rx.try_recv().is_ok() {
                            break;
                        }
                    }

                    loop {
                        if contender.contest().is_ok() {
                            break;
                        }
                    }

                    loop {
                        if tx.try_send(0x0).is_ok() {
                            contender.reset();
                            break;
                        }
                    }
                }
            });
        }

        for socket in sockets.iter_mut() {
            let tx = &mut socket.0;
            tx.try_send(0x000).unwrap();
        }

        for _ in 0..1 {
            thread::sleep(time::Duration::from_millis(1));

            let mut resolved = Ok(false);
            for socket in sockets.iter_mut() {
                let rx = &mut socket.1;

                if rx.try_recv().is_ok() {
                    if resolved.is_err() {
                        panic!("Lock mechanism failed");
                    }

                    if !resolved.unwrap() {
                        resolved = Ok(true);
                    } else {
                        resolved = Err(());
                    }
                }
            }
        }
    }

    // #[test]
    // fn contender_ids() {
    //     let arena = Lock::new();

    //     for (i, contender) in arena.enumerate() {
    //         let expected = match i {
    //             0 => 0b00000001,
    //             1 => 0b00000011,
    //             2 => 0b00000111,
    //             3 => 0b00001111,
    //             4 => 0b00011111,
    //             5 => 0b00111111,
    //             6 => 0b01111111,
    //             7 => 0b11111111,
    //             _ => unreachable!(),
    //         };

    //         let id = contender.id;
    //         assert_eq!(expected, id, "\nExpected: {expected:016b}\nGot: {id:016b}");
    //     }
    // }

    // #[test]
    // fn one_contender() {
    //     use crate::push_pull;
    //     use std::thread;

    //     let mut lock = Lock::new();
    //     let c0 = lock.next().unwrap();

    //     let ((c0_to_main, main_from_c0), (main_to_c0, c0_from_main)) =
    //         unsafe { (push_pull::channel(), push_pull::channel()) };

    //     let range = 10;

    //     let handle = thread::spawn(move || {
    //         let mut rx = c0_from_main;
    //         let mut tx = c0_to_main;

    //         for _ in 0..range {
    //             loop {
    //                 if rx.try_recv().is_ok() {
    //                     break;
    //                 }
    //             }

    //             loop {
    //                 if c0.contest().is_ok() {
    //                     break;
    //                 }
    //             }

    //             loop {
    //                 if tx.try_send(0).is_ok() {
    //                     c0.reset();
    //                     break;
    //                 }
    //             }
    //         }
    //     });

    //     let mut rx = main_from_c0;
    //     let mut tx = main_to_c0;
    //     for _ in 0..range {
    //         loop {
    //             if tx.try_send(0).is_ok() {
    //                 break;
    //             }
    //         }
    //         loop {
    //             if rx.try_recv().is_ok() {
    //                 break;
    //             }
    //         }
    //     }

    //     handle.join().unwrap()
    // }
}
