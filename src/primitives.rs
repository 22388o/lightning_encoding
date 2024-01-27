// Network encoding for lightning network peer protocol data types
// Written in 2020-2024 by
//     Dr. Maxim Orlovsky <orlovsky@pandoracore.com>
//
// To the extent possible under law, the author(s) have dedicated all
// copyright and related and neighboring rights to this software to
// the public domain worldwide. This software is distributed without
// any warranty.
//
// You should have received a copy of the MIT License
// along with this software.
// If not, see <https://opensource.org/licenses/MIT>.

use std::io::{Read, Write};

use amplify::flags::FlagVec;
use amplify::num::u24;
use amplify::{Slice32, Wrapper};

use super::{strategies, Strategy};
use crate::{BigSize, Error, LightningDecode, LightningEncode};

impl LightningEncode for u8 {
    fn lightning_encode<E: Write>(&self, mut e: E) -> Result<usize, Error> {
        e.write_all(&[*self])?;
        Ok(1)
    }
}

impl LightningDecode for u8 {
    fn lightning_decode<D: Read>(mut d: D) -> Result<Self, Error> {
        let mut buf = [0u8; 1];
        d.read_exact(&mut buf)?;
        Ok(buf[0])
    }
}

impl LightningEncode for u16 {
    fn lightning_encode<E: Write>(&self, mut e: E) -> Result<usize, Error> {
        let bytes = self.to_be_bytes();
        e.write_all(&bytes)?;
        Ok(bytes.len())
    }
}

impl LightningDecode for u16 {
    fn lightning_decode<D: Read>(mut d: D) -> Result<Self, Error> {
        let mut buf = [0u8; 2];
        d.read_exact(&mut buf)?;
        Ok(u16::from_be_bytes(buf))
    }
}

impl LightningEncode for u24 {
    fn lightning_encode<E: Write>(&self, mut e: E) -> Result<usize, Error> {
        let bytes = self.to_be_bytes();
        e.write_all(&bytes)?;
        Ok(bytes.len())
    }
}

impl LightningDecode for u24 {
    fn lightning_decode<D: Read>(mut d: D) -> Result<Self, Error> {
        let mut buf = [0u8; 3];
        d.read_exact(&mut buf)?;
        Ok(u24::from_be_bytes(buf))
    }
}

impl LightningEncode for u32 {
    fn lightning_encode<E: Write>(&self, mut e: E) -> Result<usize, Error> {
        let bytes = self.to_be_bytes();
        e.write_all(&bytes)?;
        Ok(bytes.len())
    }
}

impl LightningDecode for u32 {
    fn lightning_decode<D: Read>(mut d: D) -> Result<Self, Error> {
        let mut buf = [0u8; 4];
        d.read_exact(&mut buf)?;
        Ok(u32::from_be_bytes(buf))
    }
}

impl LightningEncode for u64 {
    fn lightning_encode<E: Write>(&self, mut e: E) -> Result<usize, Error> {
        let bytes = self.to_be_bytes();
        e.write_all(&bytes)?;
        Ok(bytes.len())
    }
}

impl LightningDecode for u64 {
    fn lightning_decode<D: Read>(mut d: D) -> Result<Self, Error> {
        let mut buf = [0u8; 8];
        d.read_exact(&mut buf)?;
        Ok(u64::from_be_bytes(buf))
    }
}

impl LightningEncode for usize {
    fn lightning_encode<E: Write>(&self, e: E) -> Result<usize, Error> {
        let size = BigSize::from(*self);
        size.lightning_encode(e)
    }
}

impl LightningDecode for usize {
    fn lightning_decode<D: Read>(d: D) -> Result<Self, Error> {
        BigSize::lightning_decode(d).map(|size| size.into_inner() as usize)
    }
}

impl LightningEncode for FlagVec {
    fn lightning_encode<E: Write>(&self, mut e: E) -> Result<usize, Error> {
        let flags = self.shrunk();
        let mut vec = flags.as_inner().to_vec();
        vec.reverse();
        let len = vec.len() as u16;
        len.lightning_encode(&mut e)?;
        e.write_all(&vec)?;
        Ok(vec.len() + 2)
    }
}

impl LightningDecode for FlagVec {
    fn lightning_decode<D: Read>(mut d: D) -> Result<Self, Error> {
        let len = u16::lightning_decode(&mut d)?;
        let mut buf = vec![0u8; len as usize];
        d.read_exact(&mut buf)?;
        buf.reverse();
        Ok(FlagVec::from_inner(buf))
    }
}

impl LightningEncode for Slice32 {
    fn lightning_encode<E: Write>(&self, e: E) -> Result<usize, Error> {
        self.as_inner().lightning_encode(e)
    }
}

impl LightningDecode for Slice32 {
    fn lightning_decode<D: Read>(d: D) -> Result<Self, Error> {
        LightningDecode::lightning_decode(d).map(Slice32::from_inner)
    }
}

mod _chrono {
    use chrono::{DateTime, NaiveDateTime, Utc};

    use super::*;

    impl Strategy for NaiveDateTime {
        type Strategy = strategies::AsStrict;
    }

    impl Strategy for DateTime<Utc> {
        type Strategy = strategies::AsStrict;
    }
}
