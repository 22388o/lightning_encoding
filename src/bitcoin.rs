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

use bitcoin::{consensus, hashes, secp256k1, PubkeyHash, Script};
use bitcoin_scripts::{hlc, PubkeyScript};
use lnpbp_chain::AssetId;

use super::{strategies, Strategy};
use crate::{Error, LightningDecode, LightningEncode};

// TODO: Verify byte order for lightnin encoded types

impl Strategy for hashes::ripemd160::Hash {
    type Strategy = strategies::AsBitcoinHash;
}

impl Strategy for hashes::hash160::Hash {
    type Strategy = strategies::AsBitcoinHash;
}

impl Strategy for hashes::sha256::Hash {
    type Strategy = strategies::AsBitcoinHash;
}

impl Strategy for hashes::sha256d::Hash {
    type Strategy = strategies::AsBitcoinHash;
}

impl<T> Strategy for hashes::sha256t::Hash<T>
where
    T: hashes::sha256t::Tag,
{
    type Strategy = strategies::AsBitcoinHash;
}

impl<T> Strategy for hashes::hmac::Hmac<T>
where
    T: hashes::Hash,
{
    type Strategy = strategies::AsBitcoinHash;
}

impl Strategy for bitcoin::Txid {
    type Strategy = strategies::AsBitcoinHash;
}

impl Strategy for bitcoin::OutPoint {
    type Strategy = strategies::AsStrict;
}

impl Strategy for bitcoin::PublicKey {
    type Strategy = strategies::AsStrict;
}

impl Strategy for secp256k1::PublicKey {
    type Strategy = strategies::AsStrict;
}

impl Strategy for bitcoin::PrivateKey {
    type Strategy = strategies::AsStrict;
}

impl Strategy for secp256k1::SecretKey {
    type Strategy = strategies::AsStrict;
}

impl Strategy for secp256k1::ecdsa::Signature {
    type Strategy = strategies::AsStrict;
}

impl Strategy for hlc::HashLock {
    type Strategy = strategies::AsStrict;
}

impl Strategy for hlc::HashPreimage {
    type Strategy = strategies::AsStrict;
}

impl LightningEncode for Script {
    #[inline]
    fn lightning_encode<E: Write>(&self, mut e: E) -> Result<usize, Error> {
        e.write_all(self.as_bytes())?;
        Ok(self.len())
    }
}

impl LightningDecode for Script {
    fn lightning_decode<D: Read>(mut d: D) -> Result<Self, Error> {
        let mut buf = vec![];
        d.read_to_end(&mut buf)?;
        let bytes = consensus::serialize(&buf);
        consensus::deserialize(&bytes)
            .map_err(|err| Error::DataIntegrityError(err.to_string()))
    }
}

impl Strategy for PubkeyScript {
    type Strategy = strategies::AsWrapped;
}

impl Strategy for PubkeyHash {
    type Strategy = strategies::AsStrict;
}

impl Strategy for AssetId {
    type Strategy = strategies::AsBitcoinHash;
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn real_clightning_scriptpubkey() {
        // Real scriptpubkey sent by clightning
        let msg_recv = [
            0u8, 22, 0, 20, 42, 238, 172, 27, 222, 161, 61, 181, 251, 208, 97,
            79, 71, 255, 98, 8, 213, 205, 114, 94,
        ];

        let script = PubkeyScript::lightning_deserialize(&msg_recv).unwrap();
        assert_eq!(script.lightning_serialize().unwrap(), msg_recv);
    }
}
