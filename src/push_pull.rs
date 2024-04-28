// We only need compiler_fence for to hint at the
// compiler not to reorder our stuff.
// use std::sync::atomic::{compiler_fence, Ordering};

#[derive(Debug, PartialEq)]
pub enum Error {
    Blocked,
}

#[derive(Debug, PartialEq)]
pub enum SendErr {
    NoAck(usize),
    MustRecv(usize),
}

#[derive(Debug, PartialEq)]
pub enum RecvErr {
    Blocked,
    MustSend,
}

pub struct MainSocket {
    channel: *mut [usize; 2],
    has_received: bool,
}

impl MainSocket {
    pub fn try_send(&mut self, t: usize) -> Result<(), SendErr> {
        // We must receive before we can send
        if !self.has_received {
            return Err(SendErr::MustRecv(t));
        }

        // Write data with odd parity
        unsafe {
            self.channel.write([t, !t]);
        }

        // We must now receive before we
        // can write again
        self.has_received = false;
        Ok(())
    }

    pub fn try_recv(&mut self) -> Result<usize, RecvErr> {
        // We must send before we can receive
        if self.has_received {
            return Err(RecvErr::MustSend);
        }

        let perceived;
        unsafe {
            perceived = self.channel.read();
        }

        // Ensure even parity.
        // Otherwise sender's transmission has not
        // yet propagated to us.
        if (perceived[0] ^ perceived[1]) != 0 {
            return Err(RecvErr::Blocked);
        }

        self.has_received = true;
        Ok(perceived[0])
    }
}

pub struct SubSocket {
    channel: *mut [usize; 2],
    has_received: bool,
}

impl SubSocket {
    pub fn try_send(&mut self, t: usize) -> Result<(), SendErr> {
        // We must receive before we can send
        if !self.has_received {
            return Err(SendErr::MustRecv(t));
        }

        // Write data with even parity
        unsafe {
            self.channel.write([t, t]);
        }

        // We must now receive before we
        // can write again
        self.has_received = false;
        Ok(())
    }

    pub fn try_recv(&mut self) -> Result<usize, RecvErr> {
        // We must send before we can receive
        if self.has_received {
            return Err(RecvErr::MustSend);
        }

        let perceived;
        unsafe {
            perceived = self.channel.read();
        }

        // Ensure odd parity.
        // Otherwise sender's transmission has not
        // yet propagated to us yet.
        if (perceived[0] ^ perceived[1]) != usize::MAX {
            return Err(RecvErr::Blocked);
        }

        self.has_received = true;
        Ok(perceived[0])
    }
}

pub fn push_pull() -> (MainSocket, SubSocket) {
    let boxed_reg = Box::new([0, 0]);
    let reg_ptr = Box::into_raw(boxed_reg);

    let main = MainSocket {
        channel: reg_ptr,
        has_received: true,
    };

    let sub = SubSocket {
        channel: reg_ptr,
        has_received: false,
    };

    (main, sub)
}

unsafe impl Send for Sender {}
pub struct Sender {
    reg: *mut [usize; 2],
}

unsafe impl Send for Receiver {}
pub struct Receiver {
    reg: *mut [usize; 2],
}

impl Sender {
    pub fn try_send(&mut self, t: usize) -> Result<(), Error> {
        // Inform the compiler not to reorder things
        // This does not emit any machine code.
        // compiler_fence(Ordering::AcqRel);

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
    pub fn try_recv(&mut self) -> Result<usize, Error> {
        // Inform the compiler not to reorder things
        // This does not emit any machine code.
        // compiler_fence(Ordering::AcqRel);

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
pub unsafe fn channel() -> (Sender, Receiver) {
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

        // Keep this low for now
        let range = 1_000;
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
