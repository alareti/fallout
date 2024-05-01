// We only need compiler_fence for to hint at the
// compiler not to reorder our stuff.
// It works (for now) without this compiler hint
// but it's here in case I want to include it later
// use std::sync::atomic::{compiler_fence, Ordering};

use std::marker::PhantomData;
use std::mem::{align_of, align_of_val, size_of, ManuallyDrop};
use std::{alloc, ptr};

// unsafe because MainSocket and SubSocket do not
// deallocate memory - they leak the channel common between them
pub unsafe fn channel<T: Sized>() -> Result<(MainSocket<T>, SubSocket<T>), alloc::LayoutError> {
    let t_size_bytes = size_of::<T>();
    let usize_count = (t_size_bytes + size_of::<usize>() - 1) / size_of::<usize>();
    let total_size_bytes = usize_count * size_of::<usize>();

    let layout = match alloc::Layout::from_size_align(2 * total_size_bytes, align_of::<usize>()) {
        Ok(layout) => layout,
        Err(e) => {
            return Err(e);
        }
    };

    let channel = alloc::alloc(layout) as *mut usize;
    if channel.is_null() {
        alloc::handle_alloc_error(layout);
    }

    let parity = channel.add(usize_count);
    if parity.is_null() {
        alloc::handle_alloc_error(layout);
    }

    assert_eq!(
        channel as usize % align_of::<usize>(),
        0,
        "Channel is not aligned"
    );
    assert_eq!(
        parity as usize % align_of::<usize>(),
        0,
        "Parity is not aligned"
    );

    println!(
        "Channel address: {:p}, Parity address: {:p}",
        channel, parity
    );

    let main = MainSocket::<T> {
        channel,
        parity,
        can_send: true,
        usize_count,
        _marker: PhantomData,
    };

    let sub = SubSocket::<T> {
        channel,
        parity,
        can_send: false,
        usize_count,
        _marker: PhantomData,
    };

    Ok((main, sub))
}

#[derive(Debug, PartialEq)]
pub enum SendErr<T> {
    NoAck(T),
    MustRecv(T),
}

#[derive(Debug, PartialEq)]
pub enum RecvErr {
    Blocked,
    MustSend,
}

unsafe impl<T> Send for MainSocket<T> {}
pub struct MainSocket<T: Sized> {
    channel: *mut usize,
    parity: *mut usize,
    can_send: bool,
    usize_count: usize,
    _marker: PhantomData<T>,
}

impl<T> MainSocket<T> {
    pub fn try_send(&mut self, t: T) -> Result<(), SendErr<T>> {
        // We must receive before we can send
        if !self.can_send {
            return Err(SendErr::MustRecv(t));
        }

        // Ensure t is not dropped at end of function
        let t = ManuallyDrop::new(t);
        let t_ptr: *const ManuallyDrop<T> = &t;
        let t_ptr = t_ptr as *const usize;

        // Write t along with odd parity
        for i in 0..self.usize_count {
            unsafe {
                ptr::write(self.channel.add(i), *t_ptr.add(i));
                ptr::write(self.parity.add(i), !(*t_ptr.add(i)));
            }
        }

        // We must now receive before we
        // can send again
        self.can_send = false;
        Ok(())
    }

    pub fn try_recv(&mut self) -> Result<T, RecvErr> {
        // We must send before we can receive
        if self.can_send {
            return Err(RecvErr::MustSend);
        }

        // Ensure even parity
        for i in 0..self.usize_count {
            unsafe {
                let u = ptr::read(self.channel.add(i));
                if u != ptr::read(self.parity.add(i)) {
                    return Err(RecvErr::Blocked);
                }
            }
        }

        let t;
        unsafe {
            t = ptr::read(self.channel as *const ManuallyDrop<T>);
        }

        // We take ownership of t and allow it to be dropped
        // again. We also must send before we can receive again
        self.can_send = true;
        Ok(ManuallyDrop::into_inner(t))
    }
}

unsafe impl<T> Send for SubSocket<T> {}
pub struct SubSocket<T> {
    channel: *mut usize,
    parity: *mut usize,
    can_send: bool,
    usize_count: usize,
    _marker: PhantomData<T>,
}

impl<T> SubSocket<T> {
    pub fn try_send(&mut self, t: T) -> Result<(), SendErr<T>> {
        // We must receive before we can send
        if !self.can_send {
            return Err(SendErr::MustRecv(t));
        }

        // Ensure t is not dropped at end of function
        let t = ManuallyDrop::new(t);
        let t_ptr: *const ManuallyDrop<T> = &t;
        let t_ptr = t_ptr as *const usize;

        // Write t along with even parity
        for i in 0..self.usize_count {
            unsafe {
                ptr::write(self.channel.add(i), *t_ptr.add(i));
                ptr::write(self.parity.add(i), *t_ptr.add(i));
            }
        }

        // We must now receive before we
        // can send again
        self.can_send = false;
        Ok(())
    }

    pub fn try_recv(&mut self) -> Result<T, RecvErr> {
        // We must send before we can receive
        if self.can_send {
            return Err(RecvErr::MustSend);
        }

        // Ensure odd parity
        for i in 0..self.usize_count {
            unsafe {
                let u = ptr::read(self.channel.add(i));
                if u != !ptr::read(self.parity.add(i)) {
                    return Err(RecvErr::Blocked);
                }
            }
        }

        let t;
        unsafe {
            t = ptr::read(self.channel as *const ManuallyDrop<T>);
        }

        // We take ownership of t and allow it to be dropped
        // again. We also must send before we can receive again
        self.can_send = true;
        Ok(ManuallyDrop::into_inner(t))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_scalar() {
        let (mut main, mut sub) = unsafe { channel::<String>().unwrap() };

        assert_eq!(main.try_send(String::from("Hello, World!")), Ok(()));
        assert_eq!(sub.try_recv().unwrap(), "Hello, World!");
        assert_eq!(sub.try_send(String::from("Right back at you!")), Ok(()));
        assert_eq!(main.try_recv().unwrap(), "Right back at you!");
    }

    #[test]
    fn simple_transfer() {
        let (mut main, mut sub) = unsafe { channel::<usize>().unwrap() };

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
        let (mut main, mut sub) = unsafe { channel::<usize>().unwrap() };
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

        let (mut main, mut sub) = unsafe { channel::<usize>().unwrap() };

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
