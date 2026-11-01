//! GF(2^8), using primitive polynomial 0x11D and generator 0x02.

const fn tables() -> ([u8; 510], [u8; 256]) {
    let mut exp = [0; 510];
    let mut log = [0; 256];
    let mut value = 1u16;
    let mut i = 0;
    while i < 255 {
        exp[i] = value as u8;
        log[value as usize] = i as u8;
        value <<= 1;
        if value & 256 != 0 {
            value ^= 0x11D;
        }
        i += 1;
    }
    while i < 510 {
        exp[i] = exp[i - 255];
        i += 1;
    }
    (exp, log)
}

const TABLES: ([u8; 510], [u8; 256]) = tables();

pub fn add(a: u8, b: u8) -> u8 {
    a ^ b
}

pub fn mul(a: u8, b: u8) -> u8 {
    if a == 0 || b == 0 {
        0
    } else {
        TABLES.0[TABLES.1[a as usize] as usize + TABLES.1[b as usize] as usize]
    }
}

/// Zero has no multiplicative inverse.
pub fn inverse(a: u8) -> Option<u8> {
    (a != 0).then(|| TABLES.0[255 - TABLES.1[a as usize] as usize])
}

pub fn div(a: u8, b: u8) -> Option<u8> {
    inverse(b).map(|inv| mul(a, inv))
}

/// Defines x^0 = 1, including 0^0, for Vandermonde construction.
pub fn pow(a: u8, exponent: usize) -> u8 {
    if exponent == 0 {
        1
    } else if a == 0 {
        0
    } else {
        TABLES.0[(TABLES.1[a as usize] as usize * (exponent % 255)) % 255]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn polynomial_mul(mut a: u8, mut b: u8) -> u8 {
        let mut result = 0;
        for _ in 0..8 {
            if b & 1 != 0 {
                result ^= a;
            }
            let carry = a & 0x80 != 0;
            a <<= 1;
            if carry {
                a ^= 0x1D;
            }
            b >>= 1;
        }
        result
    }

    #[test]
    fn every_product_matches_polynomial_arithmetic() {
        for a in 0..=255 {
            for b in 0..=255 {
                assert_eq!(mul(a, b), polynomial_mul(a, b));
            }
        }
    }

    #[test]
    fn inverses_and_generator_cycle() {
        let mut seen = [false; 256];
        for i in 0..255 {
            let a = pow(2, i);
            assert!(!seen[a as usize]);
            seen[a as usize] = true;
            assert_eq!(mul(a, inverse(a).unwrap()), 1);
            assert_eq!(div(a, a), Some(1));
        }
        assert_eq!(inverse(0), None);
        assert_eq!(div(1, 0), None);
        assert_eq!(div(0, 7), Some(0));
        assert_eq!(pow(2, 255), 1);
    }
}
