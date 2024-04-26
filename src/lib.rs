use std::ptr;

#[derive(Debug, PartialEq)]
enum Error {
    Blocked,
}

unsafe impl Send for Sender {}
struct Sender {
    reg: *mut [usize; 2],
    level: bool,
}

unsafe impl Send for Receiver {}
struct Receiver {
    reg: *mut [usize; 2],
    level: bool,
}

// Sender will be odd parity
impl Sender {
    fn try_send(&mut self, t: usize) -> Result<(), Error> {
        let perceived;
        unsafe {
            perceived = ptr::read_volatile(self.reg);
        }

        if (!self.level && perceived != [0, 0])
            || (self.level && perceived != [usize::MAX, usize::MAX])
        {
            return Err(Error::Blocked);
        }

        if (!self.level && perceived == [usize::MAX, usize::MAX])
            || (self.level && perceived == [0, 0])
        {
            panic!("Sender out of sync with Receiver");
        }

        unsafe {
            ptr::write_volatile(self.reg, [t, !t]);
        }

        self.level = !self.level;
        Ok(())
    }
}

impl Receiver {
    fn try_recv(&mut self) -> Result<usize, Error> {
        let perceived;
        unsafe {
            perceived = ptr::read_volatile(self.reg);
        }

        if (!self.level && perceived == [usize::MAX, usize::MAX])
            || (self.level && perceived == [0, 0])
        {
            panic!("Receiver out of sync with Sender");
        }

        if perceived[0] ^ perceived[1] != usize::MAX {
            return Err(Error::Blocked);
        }

        let ack = match self.level {
            false => [usize::MAX, usize::MAX],
            true => [0, 0],
        };

        unsafe {
            ptr::write_volatile(self.reg, ack);
        }

        self.level = !self.level;
        Ok(perceived[0])
    }
}

// channel implies a memory leak of its internal
// boxed_reg. It's up to the user to deallocate it
// properly.
unsafe fn channel() -> (Sender, Receiver) {
    let boxed_reg = Box::new([0, 0]);
    let reg_ptr = Box::into_raw(boxed_reg);

    (
        Sender {
            reg: reg_ptr,
            level: false,
        },
        Receiver {
            reg: reg_ptr,
            level: false,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thread_transfer() {
        use std::thread;

        let (mut tx, mut rx) = unsafe { channel() };

        let data = vec![0xA5, 0xF1, 0x23, 0x00];
        let c_data = data.clone();

        let range = 1_000_000;
        let handle = thread::spawn(move || {
            for _ in 0..range {
                for datum in c_data.iter() {
                    loop {
                        match rx.try_recv() {
                            Ok(d) => {
                                assert_eq!(d, *datum);
                                break;
                            }
                            Err(_) => {
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
                    match tx.try_send(*datum) {
                        Ok(_) => break,
                        Err(_) => continue,
                    }
                }
            }
        }

        handle.join().unwrap();
    }

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
}
