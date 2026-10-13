// Encoding rules derived from Kotlin v2.3.20 BitEncoding.java/utfEncoding.kt.
// Copyright 2000-2018 JetBrains s.r.o. and Kotlin Programming Language contributors.
// Apache-2.0; see tests/fixtures/metadata_header/{LICENSE,NOTICE}.txt.
use super::header::{Budget, Error};

pub(super) fn decode(strings: &[&[u16]], budget: &mut Budget) -> Result<Vec<u8>, Error> {
    let marker = strings.first().and_then(|s| s.first()).copied();
    let skip = usize::from(matches!(marker, Some(0 | 0xffff)));
    budget.work(strings.len())?;
    let mut count = 0usize;
    for string in strings {
        count = count
            .checked_add(string.len())
            .ok_or(Error::Limit("metadata code units"))?;
    }
    count -= skip;
    let direct = marker == Some(0);
    let size = if direct {
        count
    } else {
        count
            .checked_mul(7)
            .ok_or(Error::Limit("metadata output bytes"))?
            / 8
    };
    budget.work(count)?;
    budget.output(size)?;
    let mut out = Vec::with_capacity(size);
    let units = strings.iter().flat_map(|s| s.iter()).skip(skip);
    if direct {
        out.extend(units.map(|unit| *unit as u8));
    } else {
        let mut bits = 0u16;
        let mut available = 0;
        for unit in units {
            let chunk = (*unit as u8).wrapping_sub(1) & 0x7f;
            bits |= u16::from(chunk) << available;
            available += 7;
            if available >= 8 {
                out.push(bits as u8);
                bits >>= 8;
                available -= 8;
            }
        }
    }
    Ok(out)
}
