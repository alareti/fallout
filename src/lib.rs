fn tbd(input: i64) -> Result<i64, Invalid> {
    let mut reg: [u16; 1] = [0];

    let input = input.to_be_bytes().map(encode); // network order
    let mut output: [u16; 8] = [0; 8]; // little endian

    for (i, message) in input.iter().enumerate() {
        reg[0] = *message;
        output[7 - i] = reg[0];
    }

    let mut result = 0;
    for (i, message) in output.iter().enumerate() {
        match decode(message) {
            Ok(byte) => result |= (byte as i64) << (8 * i),
            Err(Invalid) => {
                return Err(Invalid);
            }
        }
    }

    Ok(i64::from_le(result))
}

fn encode(input: u8) -> u16 {
    let mut result: u16 = 0;
    for i in 0..8 {
        let bit = (input >> i) & 0b1;
        match bit {
            0b0 => result |= 0b10 << (2 * i),
            0b1 => result |= 0b01 << (2 * i),
            _ => unreachable!(),
        }
    }
    result
}

#[derive(PartialEq, Debug)]
struct Invalid;
fn decode(input: &u16) -> Result<u8, Invalid> {
    let mut result: u8 = 0;

    for i in 0..8 {
        let symbol = (input >> (2 * i)) & 0b11;
        match symbol {
            0b10 => result |= 0b0 << i,
            0b01 => result |= 0b1 << i,
            0b00 | 0b11 => return Err(Invalid),
            _ => unreachable!(),
        }
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run() {
        assert_eq!(Ok(6), tbd(6));
        assert_eq!(Ok(0x123456789ABCDEF0), tbd(0x123456789ABCDEF0));
        assert_eq!(Ok(-420), tbd(-420));
    }
}
