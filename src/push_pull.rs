// We only need compiler_fence for to hint at the
// compiler not to reorder our stuff.
// It works (for now) without this compiler hint
// but it's here in case I want to include it later
// use std::sync::atomic::{compiler_fence, Ordering};

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

unsafe impl Send for MainSocket {}
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

unsafe impl Send for SubSocket {}
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

// unsafe because MainSocket and SubSocket do not
// deallocate memory - they leak.
pub unsafe fn push_pull() -> (MainSocket, SubSocket) {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_transfer() {
        let (mut main, mut sub) = unsafe { push_pull() };

        let msg = 0xA5;
        assert_eq!(sub.try_recv(), Err(RecvErr::Blocked));
        assert_eq!(main.try_send(msg), Ok(()));
        assert_eq!(main.try_send(msg), Err(SendErr::MustRecv(msg)));
        assert_eq!(main.try_recv(), Err(RecvErr::Blocked));
        assert_eq!(sub.try_recv(), Ok(msg));
        assert_eq!(sub.try_recv(), Err(RecvErr::MustSend));
        assert_eq!(sub.try_send(msg), Ok(()));
        assert_eq!(sub.try_send(msg), Err(SendErr::MustRecv(msg)));
        assert_eq!(sub.try_recv(), Err(RecvErr::Blocked));
        assert_eq!(main.try_send(msg), Err(SendErr::MustRecv(msg)));
        assert_eq!(main.try_recv(), Ok(msg));

        let msg = 0xFF;
        assert_eq!(main.try_send(msg), Ok(()));
        assert_eq!(main.try_send(msg), Err(SendErr::MustRecv(msg)));
        assert_eq!(main.try_recv(), Err(RecvErr::Blocked));
        assert_eq!(sub.try_recv(), Ok(msg));
        assert_eq!(sub.try_recv(), Err(RecvErr::MustSend));
        assert_eq!(sub.try_send(msg), Ok(()));
        assert_eq!(sub.try_send(msg), Err(SendErr::MustRecv(msg)));
        assert_eq!(sub.try_recv(), Err(RecvErr::Blocked));
        assert_eq!(main.try_send(msg), Err(SendErr::MustRecv(msg)));
        assert_eq!(main.try_recv(), Ok(msg));
    }

    #[test]
    fn single_thread_loop() {
        let (mut main, mut sub) = unsafe { push_pull() };
        let data = [0xA5, 0xF1, 0x23, 0x00];

        let range = 1_000_000;
        for _ in 0..range {
            for datum in data.iter() {
                loop {
                    match main.try_send(*datum) {
                        Ok(_) => break,
                        Err(_) => continue,
                    }
                }

                loop {
                    match sub.try_recv() {
                        Ok(d) => {
                            assert_eq!(d, *datum);
                            break;
                        }
                        Err(_) => continue,
                    }
                }

                loop {
                    match sub.try_send(*datum) {
                        Ok(_) => break,
                        Err(_) => continue,
                    }
                }

                loop {
                    match main.try_recv() {
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

    #[test]
    fn thread_transfer() {
        use std::thread;

        let (mut main, mut sub) = unsafe { push_pull() };

        let data = vec![
            0x0000_0000_DEAD_BEEF,
            0xDEAD_BEEF_0000_0000,
            0x0000_0000_0000_0000,
            0xDEAD_BEEF_DEAD_BEEF,
        ];
        let c_data = data.clone();

        // Keep this low for now
        let range = 1_000_000;
        let handle = thread::spawn(move || {
            for _ in 0..range {
                for (i, datum) in c_data.iter().enumerate() {
                    loop {
                        if let Ok(d) = sub.try_recv() {
                            assert_eq!(
                                d,
                                *datum,
                                "Sub: At data[{}]:\nExpected: {datum:016x}\nGot: {d:016x}",
                                i % c_data.len(),
                            );
                            break;
                        }
                    }
                    if sub.try_send(*datum).is_err() {
                        panic!("Sub couldn't send datum");
                    }
                }
            }
        });

        for i in 0..range {
            for datum in data.iter() {
                if main.try_send(*datum).is_err() {
                    panic!("Main couldn't send datum");
                }
                loop {
                    if let Ok(d) = main.try_recv() {
                        assert_eq!(
                            d,
                            *datum,
                            "Main: At data[{}]:\nExpected: {datum:016x}\nGot: {d:016x}",
                            i % data.len(),
                        );
                        break;
                    }
                }
            }
        }

        handle.join().unwrap();
    }
}
