#[derive(Debug, PartialEq)]
enum Error {
    Blocked,
}

unsafe impl Send for Sender {}
struct Sender {
    reg: *mut [usize; 2],
}

unsafe impl Send for Receiver {}
struct Receiver {
    reg: *mut [usize; 2],
}

impl Sender {
    fn try_send(&mut self, t: usize) -> Result<(), Error> {
        let perceived;
        unsafe {
            perceived = self.reg.read();
        }

        if (perceived[0] ^ perceived[1]) != 0 {
            return Err(Error::Blocked);
        }

        unsafe {
            self.reg.write([t, !t]);
        }

        Ok(())
    }
}

impl Receiver {
    fn try_recv(&mut self) -> Result<usize, Error> {
        let perceived;
        unsafe {
            perceived = self.reg.read();
        }

        if (perceived[0] ^ perceived[1]) != usize::MAX {
            return Err(Error::Blocked);
        }

        unsafe {
            self.reg.write([perceived[0], perceived[0]]);
        }

        Ok(perceived[0])
    }
}

// channel implies a memory leak of its internal
// boxed_reg. It's up to the user to deallocate it
// properly.
unsafe fn channel() -> (Sender, Receiver) {
    let boxed_reg = Box::new([0, 0]);
    let reg_ptr = Box::into_raw(boxed_reg);

    (Sender { reg: reg_ptr }, Receiver { reg: reg_ptr })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thread_transfer() {
        use std::thread;

        let (mut tx, mut rx) = unsafe { channel() };

        let data = vec![
            0x0000_0000_0000_0000,
            0x0000_0000_0000_0001,
            0x0000_0000_0000_0001,
            0x0000_0000_0000_0001,
            0x0000_0000_0000_0001,
            0x0000_0000_0000_0001,
            0x0000_0000_0000_0000,
            0x0000_0000_0000_0000,
            0x0000_0000_0000_0000,
            0x0000_0000_0000_0000,
            0x0000_0000_0000_0000,
            0x0000_0000_0000_0001,
            0x0000_0000_0000_0000,
            0x0000_0000_0000_0001,
            0x0000_0000_0000_0000,
            0x0000_0000_0000_0001,
            0x0000_0000_0000_0000,
            0xDEAD_BEEF_0BAD_B001,
            0x0000_0000_0000_00A5,
            0x0000_0000_0000_00F1,
            0x0000_0000_0000_0023,
            0x0000_0000_0000_0000,
            0x5555_5555_5555_5555,
            0xAAAA_AAAA_AAAA_AAAA,
            0xAAAA_AAAA_AAAA_AAAA,
            0x5555_5555_5555_5555,
            0x5555_5555_5555_5555,
            0x0000_0000_0000_00A5,
            0x0000_0000_0000_00F1,
            0x0000_0000_0000_0023,
            0x0000_0000_0000_0000,
            0xFFFF_FFFF_FFFF_FFFF,
            0xFFFF_FFFF_FFFF_FFFF,
            0xFFFF_FFFF_FFFF_FFFF,
            0xFFFF_FFFF_FFFF_FFFF,
            0x0000_0000_0000_0000,
            0xFFFF_FFFF_FFFF_FFFF,
            0x0000_0000_0000_0000,
        ];
        let c_data = data.clone();

        let range = 1_000_000;
        let handle = thread::spawn(move || {
            for _ in 0..range {
                for (i, datum) in c_data.iter().enumerate() {
                    loop {
                        if let Ok(d) = rx.try_recv() {
                            assert_eq!(
                                d,
                                *datum,
                                "At data[{}]:\nExpected: {datum:016x}\nGot: {d:016x}",
                                i % c_data.len(),
                            );
                            break;
                        }
                    }
                }
            }
        });

        for _ in 0..range {
            for datum in data.iter() {
                loop {
                    if tx.try_send(*datum).is_ok() {
                        break;
                    }
                }
            }
        }

        handle.join().unwrap();
    }

    #[test]
    fn simple_transfer() {
        let (mut tx, mut rx) = unsafe { channel() };

        let msg = 0xA5;
        assert_eq!(rx.try_recv(), Err(Error::Blocked));
        assert_eq!(tx.try_send(msg), Ok(()));
        assert_eq!(tx.try_send(msg), Err(Error::Blocked));
        assert_eq!(rx.try_recv(), Ok(msg));
        assert_eq!(rx.try_recv(), Err(Error::Blocked));

        let msg = 0xFF;
        assert_eq!(rx.try_recv(), Err(Error::Blocked));
        assert_eq!(tx.try_send(msg), Ok(()));
        assert_eq!(tx.try_send(msg), Err(Error::Blocked));
        assert_eq!(rx.try_recv(), Ok(msg));
        assert_eq!(rx.try_recv(), Err(Error::Blocked));

        let msg = 0x00;
        assert_eq!(rx.try_recv(), Err(Error::Blocked));
        assert_eq!(tx.try_send(msg), Ok(()));
        assert_eq!(tx.try_send(msg), Err(Error::Blocked));
        assert_eq!(rx.try_recv(), Ok(msg));
        assert_eq!(rx.try_recv(), Err(Error::Blocked));
    }

    #[test]
    fn single_thread_loop() {
        let (mut tx, mut rx) = unsafe { channel() };
        let data = [0xA5, 0xF1, 0x23, 0x00];

        let range = 1_000_000;

        for _ in 0..range {
            for datum in data.iter() {
                loop {
                    match tx.try_send(*datum) {
                        Ok(_) => break,
                        Err(_) => continue,
                    }
                }

                loop {
                    match rx.try_recv() {
                        Ok(d) => {
                            assert_eq!(d, *datum);
                            break;
                        }
                        Err(_) => continue,
                    }
                }
            }
        }
    }
}
