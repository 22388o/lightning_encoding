// Network encoding for lightning network peer protocol data types
// Written in 2020-2022 by
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

use bitcoin::consensus;
use bitcoin::{hashes, secp256k1, Script};
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
    fn lightning_encode<E: Write>(&self, e: E) -> Result<usize, Error> {
        self.as_bytes().lightning_encode(e)
    }
}

impl LightningDecode for Script {
    fn lightning_decode<D: Read>(d: D) -> Result<Self, Error> {
        consensus::deserialize(&Vec::<u8>::lightning_decode(d)?)
            .map_err(|err| Error::DataIntegrityError(err.to_string()))
    }
}

impl Strategy for PubkeyScript {
    type Strategy = strategies::AsWrapped;
}

impl Strategy for AssetId {
    type Strategy = strategies::AsBitcoinHash;
}

#[cfg(test)]
mod test {
    use bitcoin_scripts::PubkeyScript;

    use crate::LightningDecode;

    #[test]
    fn real_clightning_scriptpubkey() {
        // Real scriptpubkey sent by clightning
        let msg_recv = [
            0, 22, 0, 20, 42, 238, 172, 27, 222, 161, 61, 181, 251, 208, 97,
            79, 71, 255, 98, 8, 213, 205, 114, 94,
        ];
        PubkeyScript::lightning_deserialize(&msg_recv).unwrap();
    }
}
