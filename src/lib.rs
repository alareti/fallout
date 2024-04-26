use std::ptr;

struct RawSender {
    reg: *mut u16,
    blocked: bool,
    level: bool,
}

impl RawSender {
    fn try_send(&mut self, t_encoded: u16) -> Result<(), ()> {
        if self.blocked {
            return Err(());
        }

        let perceived;
        unsafe {
            perceived = ptr::read_volatile(self.reg);
        }

        // println!("{perceived:0b}");
        if (!self.level && perceived != 0) || (self.level && perceived != u16::MAX) {
            return Err(());
        }

        if (!self.level && perceived == u16::MAX) || (self.level && perceived == 0) {
            panic!("RawSender out of sync with RawReceiver");
        }

        unsafe {
            ptr::write_volatile(self.reg, t_encoded);
        }

        loop {
            let perceived;
            unsafe {
                perceived = ptr::read_volatile(self.reg);
            }
            if perceived == t_encoded {
                break;
            }
        }

        self.blocked = true;
        Ok(())
    }

    fn try_unblock(&mut self) -> Result<(), ()> {
        // println!("Called");
        if !self.blocked {
            return Err(());
        }

        let perceived;
        unsafe {
            perceived = ptr::read_volatile(self.reg);
        }
        // println!("{perceived:0b}");

        if (!self.level && perceived != u16::MAX) || (self.level && perceived != 0) {
            return Err(());
        }
        if (!self.level && perceived == 0) || (self.level && perceived == u16::MAX) {
            panic!("RawSender not in sync with RawReader");
        }

        self.level = !self.level;
        self.blocked = false;
        Ok(())
    }

    fn encode(&self, t: u8) -> u16 {
        let mut t_encoded: u16 = 0;
        for i in 0..8 {
            let bit = (t >> i) & 0b1;
            match bit {
                0b0 => t_encoded |= 0b10 << (2 * i),
                0b1 => t_encoded |= 0b01 << (2 * i),
                _ => unreachable!(),
            }
        }

        t_encoded
    }
}

struct RawReceiver {
    reg: *mut u16,
    level: bool,
}

impl RawReceiver {
    fn try_recv(&mut self) -> Result<u8, ()> {
        let perceived;
        unsafe {
            perceived = ptr::read_volatile(self.reg);
        }

        if (!self.level && perceived == u16::MAX) || (self.level && perceived == 0) {
            panic!("RawReceiver out of sync with RawSender");
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

                loop {
                    let perceived;
                    unsafe {
                        perceived = ptr::read_volatile(self.reg);
                    }
                    if perceived == ack {
                        break;
                    }
                }

                self.level = !self.level;
                Ok(t)
            }
            Err(()) => Err(()),
        }
    }

    fn decode(&self, t_encoded: u16) -> Result<u8, ()> {
        let mut result: u8 = 0;

        for i in 0..8 {
            let symbol = (t_encoded >> (2 * i)) & 0b11;
            match symbol {
                0b10 => result |= 0b0 << i,
                0b01 => result |= 0b1 << i,
                0b00 | 0b11 => return Err(()),
                _ => unreachable!(),
            }
        }

        Ok(result)
    }
}

fn raw_channel(reg: &mut u16) -> (RawSender, RawReceiver) {
    (
        RawSender {
            reg,
            blocked: false,
            level: false,
        },
        RawReceiver { reg, level: false },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_simple_transfer() {
        let reg = &mut 0;
        let (mut tx, mut rx) = raw_channel(reg);

        let msg = 0xA5;
        let msg_encoded = tx.encode(msg);
        assert_eq!(rx.try_recv(), Err(()));
        assert_eq!(tx.try_unblock(), Err(()));
        assert_eq!(tx.try_send(msg_encoded), Ok(()));
        assert_eq!(tx.try_unblock(), Err(()));
        assert_eq!(tx.try_send(msg_encoded), Err(()));
        assert_eq!(tx.try_unblock(), Err(()));
        assert_eq!(rx.try_recv(), Ok(msg));
        assert_eq!(rx.try_recv(), Err(()));

        let msg = 0xFF;
        let msg_encoded = tx.encode(msg);
        assert_eq!(rx.try_recv(), Err(()));
        assert_eq!(tx.try_unblock(), Ok(()));
        assert_eq!(tx.try_send(msg_encoded), Ok(()));
        assert_eq!(tx.try_unblock(), Err(()));
        assert_eq!(tx.try_unblock(), Err(()));
        assert_eq!(tx.try_send(msg_encoded), Err(()));
        assert_eq!(rx.try_recv(), Ok(msg));
        assert_eq!(tx.try_unblock(), Ok(()));
        assert_eq!(tx.try_unblock(), Err(()));
        assert_eq!(rx.try_recv(), Err(()));

        let msg = 0x00;
        let msg_encoded = tx.encode(msg);
        assert_eq!(rx.try_recv(), Err(()));
        assert_eq!(tx.try_send(msg_encoded), Ok(()));
        assert_eq!(tx.try_unblock(), Err(()));
        assert_eq!(tx.try_unblock(), Err(()));
        assert_eq!(tx.try_send(msg_encoded), Err(()));
        assert_eq!(rx.try_recv(), Ok(msg));
        assert_eq!(tx.try_unblock(), Ok(()));
        assert_eq!(tx.try_unblock(), Err(()));
        assert_eq!(rx.try_recv(), Err(()));
    }
}
