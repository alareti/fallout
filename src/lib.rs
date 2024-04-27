use std::ptr;

// const ODD_PARITY_MASK: usize = {
//     #[cfg(target_pointer_width = "16")]
//     {
//         0xAAAA
//     }
//
//     #[cfg(target_pointer_width = "32")]
//     {
//         0xAAAA_AAAA
//     }
//
//     #[cfg(target_pointer_width = "64")]
//     {
//         0xAAAA_AAAA_AAAA_AAAA
//     }
//
//     #[cfg(target_pointer_width = "128")]
//     {
//         0xAAAA_AAAA_AAAA_AAAA_AAAA_AAAA_AAAA_AAAAA
//     }
// };
//
// const EVEN_PARITY_MASK: usize = {
//     #[cfg(target_pointer_width = "16")]
//     {
//         0x5555
//     }
//
//     #[cfg(target_pointer_width = "32")]
//     {
//         0x5555_5555
//     }
//
//     #[cfg(target_pointer_width = "64")]
//     {
//         0x5555_5555_5555_5555
//     }
//
//     #[cfg(target_pointer_width = "128")]
//     {
//         0x5555_5555_5555_5555_5555_5555_5555_5555
//     }
// };

#[derive(Debug, PartialEq)]
enum Error {
    Blocked,
}

unsafe impl Send for Sender {}
struct Sender {
    reg: *mut [usize; 2],
    level: bool,
    // history: vec![0, 0, 0, 0, 0],
}

unsafe impl Send for Receiver {}
struct Receiver {
    reg: *mut [usize; 2],
    level: bool,
}

impl Sender {
    fn try_send(&mut self, t: usize) -> Result<(), Error> {
        let perceived;
        unsafe {
            perceived = ptr::read(self.reg);
        }

        if (perceived[0] ^ perceived[1]) != 0 {
            return Err(Error::Blocked);
        }

        unsafe {
            ptr::write(self.reg, [t, !t]);
        }

        // println!(
        //     "Sender perceived: {:016x} {:016x}",
        //     perceived[0], perceived[1]
        // );
        // println!("Sender writing: {:016x} {:016x}", t, !t);

        Ok(())
    }
}

impl Receiver {
    fn try_recv(&mut self) -> Result<usize, Error> {
        let perceived;
        unsafe {
            perceived = ptr::read(self.reg);
        }

        if (perceived[0] ^ perceived[1]) != usize::MAX {
            return Err(Error::Blocked);
        }

        // println!(
        //     "Receiver perceived: {:016x} {:016x}",
        //     perceived[0], perceived[1]
        // );
        // println!(
        //     "Receiver writing: {:016x} {:016x}",
        //     perceived[0], perceived[0]
        // );

        unsafe {
            ptr::write(self.reg, [perceived[0], perceived[0]]);
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

        let range = 1_000_000_000;
        // let mut rx_stuck = 0;
        // let mut tx_stuck = 0;

        let handle = thread::spawn(move || {
            for _ in 0..range {
                for (i, datum) in c_data.iter().enumerate() {
                    // println!(
                    //     "Receiver now handling data[{}]: {datum:016x}",
                    //     i % c_data.len()
                    // );
                    loop {
                        // if rx_stuck > 1_000_000 {
                        //     let r0;
                        //     let r1;
                        //     unsafe {
                        //         r0 = ptr::read_volatile(rx.reg);
                        //         r1 = ptr::read_volatile(rx.reg);
                        //     }
                        //     panic!("rx stuck on iter {i} at level {}, expected {datum:016x}\n\tR0: {:016x}  {:016x}", rx.level, r0[0], r0[1]);
                        // }

                        match rx.try_recv() {
                            Ok(d) => {
                                assert_eq!(
                                    d,
                                    *datum,
                                    "At data[{}]:\nExpected: {datum:016x}\nGot: {d:016x}",
                                    i % c_data.len(),
                                );
                                // rx_stuck = 0;
                                break;
                            }
                            Err(_) => {
                                // rx_stuck += 1;
                            }
                        }
                    }
                }
            }
        });

        for _ in 0..range {
            for (i, datum) in data.iter().enumerate() {
                // println!("Sender now handling data[{}]: {datum:016x}", i % data.len());
                loop {
                    // if tx_stuck > 1_000 {
                    //     panic!("tx stuck on iter {i}");
                    // }

                    match tx.try_send(*datum) {
                        Ok(_) => {
                            // tx_stuck = 0;
                            break;
                        }
                        Err(_) => {
                            // tx_stuck += 1;
                        }
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
