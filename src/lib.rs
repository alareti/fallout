use std::ptr;

#[derive(Debug, PartialEq)]
enum Error {
    Blocked,
    Decode,
}

#[derive(Copy, Clone, Debug)]
struct Encoded {
    t: u16,
}

unsafe impl Send for Sender {}
struct Sender {
    reg: *mut u16,
    level: bool,
}

unsafe impl Send for Receiver {}
struct Receiver {
    reg: *mut u16,
    level: bool,
    counter: usize,
}

impl Sender {
    fn try_send(&mut self, enc: Encoded) -> Result<(), Error> {
        let perceived;
        unsafe {
            perceived = ptr::read_volatile(self.reg);
        }

        // if self.counter > 1_000 {
        //     panic!("Sender reg: {perceived:0b}")
        // }

        // println!("{perceived:0b}");
        if (!self.level && perceived != 0) || (self.level && perceived != u16::MAX) {
            // self.counter += 1;
            return Err(Error::Blocked);
        }

        if (!self.level && perceived == u16::MAX) || (self.level && perceived == 0) {
            panic!("Sender out of sync with Receiver");
        }

        unsafe {
            ptr::write_volatile(self.reg, enc.t);
        }

        // loop {
        //     let perceived;
        //     unsafe {
        //         perceived = ptr::read_volatile(self.reg);
        //     }
        //     if perceived == enc.t {
        //         break;
        //     }
        // }

        // self.counter = 0;
        self.level = !self.level;
        Ok(())
    }

    fn encode(&self, t: u8) -> Encoded {
        let mut t_encoded: u16 = 0;
        for i in 0..8 {
            let bit = (t >> i) & 0b1;
            match bit {
                0b0 => t_encoded |= 0b10 << (2 * i),
                0b1 => t_encoded |= 0b01 << (2 * i),
                _ => unreachable!(),
            }
        }

        Encoded { t: t_encoded }
    }
}

impl Receiver {
    fn try_recv(&mut self) -> Result<u8, Error> {
        let perceived;
        unsafe {
            perceived = ptr::read_volatile(self.reg);
        }

        if (!self.level && perceived == u16::MAX) || (self.level && perceived == 0) {
            panic!("Receiver out of sync with Sender");
        }

        match self.decode(perceived) {
            Ok(t) => {
                let ack = match self.level {
                    false => u16::MAX,
                    true => 0,
                };

                unsafe {
                    ptr::write_volatile(self.reg, ack);
                }

                // loop {
                //     let perceived;
                //     unsafe {
                //         perceived = ptr::read_volatile(self.reg);
                //     }

                //     if perceived == ack {
                //         break;
                //     }
                // }

                self.level = !self.level;
                Ok(t)
            }
            Err(e) => Err(e),
        }
    }

    fn decode(&self, t_encoded: u16) -> Result<u8, Error> {
        let mut result: u8 = 0;

        for i in 0..8 {
            let symbol = (t_encoded >> (2 * i)) & 0b11;
            match symbol {
                0b10 => result |= 0b0 << i,
                0b01 => result |= 0b1 << i,
                0b00 | 0b11 => return Err(Error::Decode),
                _ => unreachable!(),
            }
        }

        Ok(result)
    }
}

fn dbg_encode(t: u8) -> Encoded {
    let mut t_encoded: u16 = 0;
    for i in 0..8 {
        let bit = (t >> i) & 0b1;
        match bit {
            0b0 => t_encoded |= 0b10 << (2 * i),
            0b1 => t_encoded |= 0b01 << (2 * i),
            _ => unreachable!(),
        }
    }

    Encoded { t: t_encoded }
}

// channel implies a memory leak of its internal
// boxed_reg. It's up to the user to deallocate it
// properly.
unsafe fn channel() -> (Sender, Receiver) {
    let boxed_reg = Box::new(0_u16);
    let reg_ptr = Box::into_raw(boxed_reg);

    (
        Sender {
            reg: reg_ptr,
            level: false,
        },
        Receiver {
            reg: reg_ptr,
            level: false,
            counter: 0,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    // #[test]
    // fn simple_transfer() {
    //     let (mut tx, mut rx) = unsafe { channel() };

    //     let msg = 0xA5;
    //     let msg_encoded = tx.encode(msg);
    //     assert_eq!(rx.try_recv(), Err(Error::Decode));
    //     assert_eq!(tx.try_send(msg_encoded), Ok(()));
    //     assert_eq!(tx.try_send(msg_encoded), Err(Error::Blocked));
    //     assert_eq!(rx.try_recv(), Ok(msg));
    //     assert_eq!(rx.try_recv(), Err(Error::Decode));

    //     let msg = 0xFF;
    //     let msg_encoded = tx.encode(msg);
    //     assert_eq!(rx.try_recv(), Err(Error::Decode));
    //     assert_eq!(tx.try_send(msg_encoded), Ok(()));
    //     assert_eq!(tx.try_send(msg_encoded), Err(Error::Blocked));
    //     assert_eq!(rx.try_recv(), Ok(msg));
    //     assert_eq!(rx.try_recv(), Err(Error::Decode));

    //     let msg = 0x00;
    //     let msg_encoded = tx.encode(msg);
    //     assert_eq!(rx.try_recv(), Err(Error::Decode));
    //     assert_eq!(tx.try_send(msg_encoded), Ok(()));
    //     assert_eq!(tx.try_send(msg_encoded), Err(Error::Blocked));
    //     assert_eq!(rx.try_recv(), Ok(msg));
    //     assert_eq!(rx.try_recv(), Err(Error::Decode));
    // }

    // #[test]
    // fn single_thread_loop() {
    //     let (mut tx, mut rx) = unsafe { channel() };
    //     let data = [0xA5, 0xF1, 0x23, 0x00];

    //     let range = 10_000_000;

    //     for _ in 0..range {
    //         for datum in data.iter() {
    //             loop {
    //                 match tx.try_send(tx.encode(*datum)) {
    //                     Ok(_) => break,
    //                     Err(_) => continue,
    //                 }
    //             }

    //             loop {
    //                 match rx.try_recv() {
    //                     Ok(d) => {
    //                         assert_eq!(d, *datum);
    //                         break;
    //                     }
    //                     Err(_) => continue,
    //                 }
    //             }
    //         }
    //     }
    // }

    #[test]
    fn thread_transfer() {
        use std::thread;

        let (mut tx, mut rx) = unsafe { channel() };

        let data = vec![0xA5, 0xF1, 0x23, 0x00];
        let c_data = data.clone();

        let range = 10_000_000;

        let range = 1_000_000;
        let mut counter = 0;

        let handle = thread::spawn(move || {
            for _ in 0..range {
                for datum in c_data.iter() {
                    loop {
                        if counter > range {
                            let mut perceived = vec![];
                            unsafe {
                                perceived.push(ptr::read_volatile(rx.reg));
                                perceived.push(ptr::read_volatile(rx.reg));
                                perceived.push(ptr::read_volatile(rx.reg));
                                perceived.push(ptr::read_volatile(rx.reg));
                            }
                            panic!(
                                "Encoded datum: {:016b}\nReceiver perceived[0]: {:016b}\nReceiver perceived[1]: {:016b}\nReceiver perceived[2]: {:016b}\nReceiver perceived[3]: {:016b}\n",
                                dbg_encode(*datum).t,
                                perceived[0],
                                perceived[1],
                                perceived[2],
                                perceived[3],
                            );
                        }

                        match rx.try_recv() {
                            Ok(d) => {
                                assert_eq!(d, *datum);
                                break;
                            }
                            Err(_) => {
                                counter += 1;
                                continue;
                            }
                        }
                    }
                }
            }
        });

        for _ in 0..range {
            for datum in data.iter() {
                loop {
                    match tx.try_send(tx.encode(*datum)) {
                        Ok(_) => break,
                        Err(_) => continue,
                    }
                }
            }
        }

        handle.join().unwrap();
    }
}
