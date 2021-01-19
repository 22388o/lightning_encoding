// LNP/BP Core Library implementing LNPBP specifications & standards
// Written in 2020 by
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

use amplify::DumbDefault;
use std::collections::HashSet;
use std::fmt::{self, Display, Formatter};
use std::io;

use bitcoin::hashes::{sha256, Hmac};
use bitcoin::secp256k1::{PublicKey, Signature};
use bitcoin::{Script, Txid};

use super::payment::{
    AddressList, Alias, ChannelId, NodeColor, ShortChannelId, TempChannelId,
};
use super::Features;
use crate::bp::chain::AssetId;
use crate::bp::{HashLock, HashPreimage};
use crate::lightning_encoding::{self, LightningDecode, LightningEncode};
use crate::lnp::{CreateUnmarshaller, Payload, Unmarshall, Unmarshaller};
use crate::SECP256K1_PUBKEY_DUMB;

#[cfg(feature = "rgb")]
use crate::rgb::Consignment;

lazy_static! {
    pub static ref LNPWP_UNMARSHALLER: Unmarshaller<Messages> =
        Messages::create_unmarshaller();
}

#[derive(Clone, Debug, Display, LnpApi)]
#[lnp_api(encoding = "lightning")]
#[lnpbp_crate(crate)]
#[non_exhaustive]
pub enum Messages {
    // Part I: Generic messages outside of channel operations
    // ======================================================
    /// Once authentication is complete, the first message reveals the features
    /// supported or required by this node, even if this is a reconnection.
    #[lnp_api(type = 16)]
    #[display(inner)]
    Init(Init),

    /// For simplicity of diagnosis, it's often useful to tell a peer that
    /// something is incorrect.
    #[lnp_api(type = 17)]
    #[display(inner)]
    Error(Error),

    /// In order to allow for the existence of long-lived TCP connections, at
    /// times it may be required that both ends keep alive the TCP connection
    /// at the application level. Such messages also allow obfuscation of
    /// traffic patterns.
    #[lnp_api(type = 18)]
    #[display(inner)]
    Ping(Ping),

    /// The pong message is to be sent whenever a ping message is received. It
    /// serves as a reply and also serves to keep the connection alive, while
    /// explicitly notifying the other end that the receiver is still active.
    /// Within the received ping message, the sender will specify the number of
    /// bytes to be included within the data payload of the pong message.
    #[lnp_api(type = 19)]
    #[display("pong(...)")]
    Pong(Vec<u8>),

    // Part II: Channel management protocol
    // ====================================
    //
    // 1. Channel establishment
    // ------------------------
    #[lnp_api(type = 32)]
    #[display(inner)]
    OpenChannel(OpenChannel),

    #[lnp_api(type = 33)]
    #[display(inner)]
    AcceptChannel(AcceptChannel),

    #[lnp_api(type = 34)]
    #[display(inner)]
    FundingCreated(FundingCreated),

    #[lnp_api(type = 35)]
    #[display(inner)]
    FundingSigned(FundingSigned),

    #[lnp_api(type = 36)]
    #[display(inner)]
    FundingLocked(FundingLocked),

    #[lnp_api(type = 38)]
    #[display(inner)]
    Shutdown(Shutdown),

    #[lnp_api(type = 39)]
    #[display(inner)]
    ClosingSigned(ClosingSigned),

    // 2. Normal operations
    // --------------------
    #[lnp_api(type = 128)]
    #[display(inner)]
    UpdateAddHtlc(UpdateAddHtlc),

    #[lnp_api(type = 130)]
    #[display(inner)]
    UpdateFulfillHtlc(UpdateFulfillHtlc),

    #[lnp_api(type = 131)]
    #[display(inner)]
    UpdateFailHtlc(UpdateFailHtlc),

    #[lnp_api(type = 135)]
    #[display(inner)]
    UpdateFailMalformedHtlc(UpdateFailMalformedHtlc),

    #[lnp_api(type = 132)]
    #[display(inner)]
    CommitmentSigned(CommitmentSigned),

    #[lnp_api(type = 133)]
    #[display(inner)]
    RevokeAndAck(RevokeAndAck),

    #[lnp_api(type = 134)]
    #[display(inner)]
    UpdateFee(UpdateFee),

    #[lnp_api(type = 136)]
    #[display(inner)]
    ChannelReestablish(ChannelReestablish),

    // 3. Bolt 7 Gossip
    // -----------------
    #[lnp_api(type = 259)]
    #[display(inner)]
    AnnouncementSignatures(AnnouncementSignatures),

    #[lnp_api(type = 256)]
    #[display(inner)]
    ChannelAnnouncements(ChannelAnnouncements),

    #[lnp_api(type = 257)]
    #[display(inner)]
    NodeAnnouncements(NodeAnnouncements),

    #[lnp_api(type = 258)]
    #[display(inner)]
    ChannelUpdate(ChannelUpdate),

    /// Extended Gossip queries
    /// Negotiating the gossip_queries option via init enables a number of
    /// extended queries for gossip synchronization.
    #[lnp_api(type = 261)]
    #[display(inner)]
    QueryShortChannelIds(QueryShortChannelIds),

    #[lnp_api(type = 262)]
    #[display(inner)]
    ReplyShortChannelIdsEnd(ReplyShortChannelIdsEnd),

    #[lnp_api(type = 263)]
    #[display(inner)]
    QueryChannelRange(QueryChannelRange),

    #[lnp_api(type = 264)]
    #[display(inner)]
    ReplyChannelRange(ReplyChannelRange),

    #[lnp_api(type = 265)]
    #[display(inner)]
    GossipTimestampFilter(GossipTimestampFilter),

    // 4. RGB
    // ------
    #[cfg(feature = "rgb")]
    #[lnp_api(type = 57156)]
    #[display(inner)]
    AssignFunds(AssignFunds),
}

/// Once authentication is complete, the first message reveals the features
/// supported or required by this node, even if this is a reconnection.
///
/// # Specification
/// <https://github.com/lightningnetwork/lightning-rfc/blob/master/01-messaging.md#the-init-message>
#[derive(
    Clone, PartialEq, Eq, Debug, Display, LightningEncode, LightningDecode,
)]
#[lnpbp_crate(crate)]
#[display("init({global_features}, {local_features}, {assets:#?})")]
pub struct Init {
    pub global_features: Features,
    pub local_features: Features,
    #[tlv(type = 1)]
    pub assets: HashSet<AssetId>,
    /* #[tlv(unknown)]
     * pub unknown_tlvs: BTreeMap<tlv::Type, tlv::RawRecord>, */
}

/// In order to allow for the existence of long-lived TCP connections, at
/// times it may be required that both ends keep alive the TCP connection
/// at the application level. Such messages also allow obfuscation of
/// traffic patterns.
///
/// # Specification
/// <https://github.com/lightningnetwork/lightning-rfc/blob/master/01-messaging.md#the-ping-and-pong-messages>
#[derive(
    Clone, PartialEq, Eq, Debug, Display, LightningEncode, LightningDecode,
)]
#[lnpbp_crate(crate)]
#[display(Debug)]
pub struct Ping {
    pub ignored: Vec<u8>,
    pub pong_size: u16,
}

/// For simplicity of diagnosis, it's often useful to tell a peer that something
/// is incorrect.
///
/// # Specification
/// <https://github.com/lightningnetwork/lightning-rfc/blob/master/01-messaging.md#the-error-message>
#[derive(Clone, PartialEq, Debug, Error, LightningEncode, LightningDecode)]
#[lnpbp_crate(crate)]
pub struct Error {
    /// The channel is referred to by channel_id, unless channel_id is 0 (i.e.
    /// all bytes are 0), in which case it refers to all channels.
    pub channel_id: Option<ChannelId>,

    /// Any specific error details, either as string or binary data
    pub data: Vec<u8>,
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str("Error")?;
        if let Some(channel_id) = self.channel_id {
            write!(f, " on channel {}", channel_id)?;
        } else {
            f.write_str(" on all channels")?;
        }
        // NB: if data is not composed solely of printable ASCII characters (For
        // reference: the printable character set includes byte values 32
        // through 126, inclusive) SHOULD NOT print out data verbatim.
        if let Ok(msg) = String::from_utf8(self.data.clone()) {
            write!(f, ": {}", msg)?;
        }
        Ok(())
    }
}

#[derive(
    Clone, PartialEq, Eq, Debug, Display, LightningEncode, LightningDecode,
)]
#[lnpbp_crate(crate)]
#[display("open_channel({chain_hash}, {temporary_channel_id}, {funding_satoshis}, {channel_flags}, ...)")]
pub struct OpenChannel {
    /// The genesis hash of the blockchain where the channel is to be opened
    pub chain_hash: AssetId,

    /// A temporary channel ID, until the funding outpoint is announced
    pub temporary_channel_id: TempChannelId,

    /// The channel value
    pub funding_satoshis: u64,

    /// The amount to push to the counter-party as part of the open, in
    /// millisatoshi
    pub push_msat: u64,

    /// The threshold below which outputs on transactions broadcast by sender
    /// will be omitted
    pub dust_limit_satoshis: u64,

    /// The maximum inbound HTLC value in flight towards sender, in
    /// millisatoshi
    pub max_htlc_value_in_flight_msat: u64,

    /// The minimum value unencumbered by HTLCs for the counterparty to keep
    /// in the channel
    pub channel_reserve_satoshis: u64,

    /// The minimum HTLC size incoming to sender, in milli-satoshi
    pub htlc_minimum_msat: u64,

    /// The fee rate per 1000-weight of sender generated transactions, until
    /// updated by update_fee
    pub feerate_per_kw: u32,

    /// The number of blocks which the counterparty will have to wait to claim
    /// on-chain funds if they broadcast a commitment transaction
    pub to_self_delay: u16,

    /// The maximum number of inbound HTLCs towards sender
    pub max_accepted_htlcs: u16,

    /// The sender's key controlling the funding transaction
    pub funding_pubkey: PublicKey,

    /// Used to derive a revocation key for transactions broadcast by
    /// counterparty
    pub revocation_basepoint: PublicKey,

    /// A payment key to sender for transactions broadcast by counterparty
    pub payment_point: PublicKey,

    /// Used to derive a payment key to sender for transactions broadcast by
    /// sender
    pub delayed_payment_basepoint: PublicKey,

    /// Used to derive an HTLC payment key to sender
    pub htlc_basepoint: PublicKey,

    /// The first to-be-broadcast-by-sender transaction's per commitment point
    pub first_per_commitment_point: PublicKey,

    /// Channel flags
    pub channel_flags: u8,
    /* TODO: Uncomment once TLVs derivation will be implemented
     * /// Optionally, a request to pre-set the to-sender output's
     * scriptPubkey /// for when we collaboratively close
     * #[lnpwp(tlv=0)]
     * pub shutdown_scriptpubkey: Option<Script>, */

    /* #[lpwpw(unknown_tlvs)]
     * pub unknown_tlvs: BTreeMap<u64, Vec<u8>>, */
}

#[derive(
    Clone, PartialEq, Eq, Debug, Display, LightningEncode, LightningDecode,
)]
#[lnpbp_crate(crate)]
#[display("accept_channel({temporary_channel_id}, ...)")]
pub struct AcceptChannel {
    /// A temporary channel ID, until the funding outpoint is announced
    pub temporary_channel_id: TempChannelId,

    /// The threshold below which outputs on transactions broadcast by sender
    /// will be omitted
    pub dust_limit_satoshis: u64,

    /// The maximum inbound HTLC value in flight towards sender, in
    /// milli-satoshi
    pub max_htlc_value_in_flight_msat: u64,

    /// The minimum value unencumbered by HTLCs for the counterparty to keep in
    /// the channel
    pub channel_reserve_satoshis: u64,

    /// The minimum HTLC size incoming to sender, in milli-satoshi
    pub htlc_minimum_msat: u64,

    /// Minimum depth of the funding transaction before the channel is
    /// considered open
    pub minimum_depth: u32,

    /// The number of blocks which the counterparty will have to wait to claim
    /// on-chain funds if they broadcast a commitment transaction
    pub to_self_delay: u16,

    /// The maximum number of inbound HTLCs towards sender
    pub max_accepted_htlcs: u16,

    /// The sender's key controlling the funding transaction
    pub funding_pubkey: PublicKey,

    /// Used to derive a revocation key for transactions broadcast by
    /// counterparty
    pub revocation_basepoint: PublicKey,

    /// A payment key to sender for transactions broadcast by counterparty
    pub payment_point: PublicKey,

    /// Used to derive a payment key to sender for transactions broadcast by
    /// sender
    pub delayed_payment_basepoint: PublicKey,

    /// Used to derive an HTLC payment key to sender for transactions broadcast
    /// by counterparty
    pub htlc_basepoint: PublicKey,

    /// The first to-be-broadcast-by-sender transaction's per commitment point
    pub first_per_commitment_point: PublicKey,
    /* TODO: Uncomment once TLVs derivation will be implemented
     * /// Optionally, a request to pre-set the to-sender output's
     * scriptPubkey /// for when we collaboratively close
     * #[lnpwp(tlv=0)]
     * pub shutdown_scriptpubkey: Option<Script>,
     * #[lpwpw(unknown_tlvs)]
     * pub unknown_tlvs: BTreeMap<u64, Vec<u8>>, */
}

#[derive(
    Clone, PartialEq, Eq, Debug, Display, LightningEncode, LightningDecode,
)]
#[lnpbp_crate(crate)]
#[display("funding_created({temporary_channel_id}, {funding_txid}:{funding_output_index}, ...signature)")]
pub struct FundingCreated {
    /// A temporary channel ID, until the funding is established
    pub temporary_channel_id: TempChannelId,

    /// The funding transaction ID
    pub funding_txid: Txid,

    /// The specific output index funding this channel
    pub funding_output_index: u16,

    /// The signature of the channel initiator (funder) on the funding
    /// transaction
    pub signature: Signature,
}

#[derive(
    Clone, PartialEq, Eq, Debug, Display, LightningEncode, LightningDecode,
)]
#[lnpbp_crate(crate)]
#[display("funding_signed({channel_id}, ...signature)")]
pub struct FundingSigned {
    /// The channel ID
    pub channel_id: ChannelId,

    /// The signature of the channel acceptor on the funding transaction
    pub signature: Signature,
}

#[derive(
    Clone, PartialEq, Eq, Debug, Display, LightningEncode, LightningDecode,
)]
#[lnpbp_crate(crate)]
#[display("funding_locked({channel_id}, {next_per_commitment_point})")]
pub struct FundingLocked {
    /// The channel ID
    pub channel_id: ChannelId,

    /// The per-commitment point of the second commitment transaction
    pub next_per_commitment_point: PublicKey,
}

#[derive(
    Clone, PartialEq, Eq, Debug, Display, LightningEncode, LightningDecode,
)]
#[lnpbp_crate(crate)]
#[display("shutdown({channel_id}, {scriptpubkey})")]
pub struct Shutdown {
    /// The channel ID
    pub channel_id: ChannelId,

    /// The destination of this peer's funds on closing.
    /// Must be in one of these forms: p2pkh, p2sh, p2wpkh, p2wsh.
    pub scriptpubkey: Script,
}

#[derive(
    Clone, PartialEq, Eq, Debug, Display, LightningEncode, LightningDecode,
)]
#[lnpbp_crate(crate)]
#[display("closing_signed({channel_id}, ...)")]
pub struct ClosingSigned {
    /// The channel ID
    pub channel_id: ChannelId,

    /// The proposed total fee for the closing transaction
    pub fee_satoshis: u64,

    /// A signature on the closing transaction
    pub signature: Signature,
}

#[derive(
    Clone, PartialEq, Eq, Debug, Display, LightningEncode, LightningDecode,
)]
#[lnpbp_crate(crate)]
#[display("update_add_htlc({channel_id}, {htlc_id}, {amount_msat}, {payment_hash}, {asset_id:#?}, ...)")]
pub struct UpdateAddHtlc {
    /// The channel ID
    pub channel_id: ChannelId,

    /// The HTLC ID
    pub htlc_id: u64,

    /// The HTLC value in milli-satoshi
    pub amount_msat: u64,

    /// The payment hash, the pre-image of which controls HTLC redemption
    pub payment_hash: HashLock,

    /// The expiry height of the HTLC
    pub cltv_expiry: u32,

    /// An obfuscated list of hops and instructions for each hop along the
    /// path. It commits to the HTLC by setting the payment_hash as associated
    /// data, i.e. includes the payment_hash in the computation of HMACs. This
    /// prevents replay attacks that would reuse a previous
    /// onion_routing_packet with a different payment_hash.
    pub onion_routing_packet: OnionPacket,

    /// RGB Extension: TLV
    #[cfg(feature = "rgb")]
    pub asset_id: Option<AssetId>,
}

#[derive(
    Clone, PartialEq, Eq, Debug, Display, LightningEncode, LightningDecode,
)]
#[lnpbp_crate(crate)]
#[display("update_fullfill_htlc({channel_id}, {htlc_id}, ...preimages)")]
pub struct UpdateFulfillHtlc {
    /// The channel ID
    pub channel_id: ChannelId,

    /// The HTLC ID
    pub htlc_id: u64,

    /// The pre-image of the payment hash, allowing HTLC redemption
    pub payment_preimage: HashPreimage,
}

#[derive(
    Clone, PartialEq, Eq, Debug, Display, LightningEncode, LightningDecode,
)]
#[lnpbp_crate(crate)]
#[display("update_fail_htlc({channel_id}, {htlc_id}, ...reason)")]
pub struct UpdateFailHtlc {
    /// The channel ID
    pub channel_id: ChannelId,

    /// The HTLC ID
    pub htlc_id: u64,

    /// The reason field is an opaque encrypted blob for the benefit of the
    /// original HTLC initiator, as defined in BOLT #4; however, there's a
    /// special malformed failure variant for the case where the peer couldn't
    /// parse it: in this case the current node instead takes action,
    /// encrypting it into a update_fail_htlc for relaying.
    pub reason: Vec<u8>,
}

#[derive(
    Clone, PartialEq, Eq, Debug, Display, LightningEncode, LightningDecode,
)]
#[lnpbp_crate(crate)]
#[display("update_fail_malformed_htlc({channel_id}, {htlc_id}, ...onion)")]
pub struct UpdateFailMalformedHtlc {
    /// The channel ID
    pub channel_id: ChannelId,

    /// The HTLC ID
    pub htlc_id: u64,

    /// SHA256 hash of onion data
    pub sha256_of_onion: sha256::Hash,

    /// The failure code
    pub failure_code: u16,
}

#[derive(
    Clone, PartialEq, Eq, Debug, Display, LightningEncode, LightningDecode,
)]
#[lnpbp_crate(crate)]
#[display("commitment_signed({channel_id}, ...signatures)")]
pub struct CommitmentSigned {
    /// The channel ID
    pub channel_id: ChannelId,

    /// A signature on the commitment transaction
    pub signature: Signature,

    /// Signatures on the HTLC transactions
    pub htlc_signatures: Vec<Signature>,
}

#[derive(
    Clone, PartialEq, Eq, Debug, Display, LightningEncode, LightningDecode,
)]
#[lnpbp_crate(crate)]
#[display("revoke_and_ack({channel_id}, {next_per_commitment_point}, ...per_commitment_secret)")]
pub struct RevokeAndAck {
    /// The channel ID
    pub channel_id: ChannelId,

    /// The secret corresponding to the per-commitment point
    pub per_commitment_secret: [u8; 32],

    /// The next sender-broadcast commitment transaction's per-commitment point
    pub next_per_commitment_point: PublicKey,
}

#[derive(
    Clone, PartialEq, Eq, Debug, Display, LightningEncode, LightningDecode,
)]
#[lnpbp_crate(crate)]
#[display("update_fee({channel_id}, {feerate_per_kw})")]
pub struct UpdateFee {
    /// The channel ID
    pub channel_id: ChannelId,

    /// Fee rate per 1000-weight of the transaction
    pub feerate_per_kw: u32,
}

#[derive(
    Clone, PartialEq, Eq, Debug, Display, LightningEncode, LightningDecode,
)]
#[lnpbp_crate(crate)]
#[display("channel_reestablish({channel_id}, {next_commitment_number}, ...)")]
pub struct ChannelReestablish {
    /// The channel ID
    pub channel_id: ChannelId,

    /// The next commitment number for the sender
    pub next_commitment_number: u64,

    /// The next commitment number for the recipient
    pub next_revocation_number: u64,

    /// Proof that the sender knows the per-commitment secret of a specific
    /// commitment transaction belonging to the recipient
    pub your_last_per_commitment_secret: [u8; 32],

    /// The sender's per-commitment point for their current commitment
    /// transaction
    pub my_current_per_commitment_point: PublicKey,
}

/// Bolt 7 Gossip messages
#[derive(
    Clone, PartialEq, Eq, Debug, Display, LightningEncode, LightningDecode,
)]
#[lnpbp_crate(crate)]
#[display(
    "announcement_signature({channel_id}, {short_channel_id}, ...signatures)"
)]
pub struct AnnouncementSignatures {
    /// The channel ID
    pub channel_id: ChannelId,

    /// Short channel Id
    pub short_channel_id: ShortChannelId, //TODO

    /// Node Signature
    pub node_signature: Signature,

    /// Bitcoin Signature
    pub bitcoin_signature: Signature,
}

#[derive(
    Clone, PartialEq, Eq, Debug, Display, LightningEncode, LightningDecode,
)]
#[lnpbp_crate(crate)]
#[display("channel_announcement({chain_hash}, {short_channel_id}, ...)")]
pub struct ChannelAnnouncements {
    /// Node Signature 1
    pub node_signature_1: Signature,

    /// Node Signature 2
    pub node_signature_2: Signature,

    /// Bitcoin Signature 1
    pub bitcoin_signature_1: Signature,

    /// Bitcoin Signature 2
    pub bitcoin_signature_2: Signature,

    /// feature bytes
    pub features: Features,

    /// chain hash
    pub chain_hash: AssetId,

    /// Short channel ID
    pub short_channel_id: ShortChannelId,

    /// Node Id 1
    pub node_id_1: PublicKey,

    /// Node Id 2
    pub node_id_2: PublicKey,

    /// Bitcoin key 1
    pub bitcoin_key_1: PublicKey,

    /// Bitcoin key 2
    pub bitcoin_key_2: PublicKey,
}

#[derive(
    Clone, PartialEq, Eq, Debug, Display, LightningEncode, LightningDecode,
)]
#[lnpbp_crate(crate)]
#[display("node_announcement({node_id}, {alias}, {addresses}, ...)")]
pub struct NodeAnnouncements {
    /// Signature
    pub signature: Signature,

    /// feature bytes
    pub features: Features,

    /// Time stamp
    pub timestamp: u32,

    /// Node Id
    pub node_id: PublicKey,

    /// RGB colour code
    pub rgb_color: NodeColor,

    /// Node Alias
    pub alias: Alias,

    /// Node address
    pub addresses: AddressList,
}

#[derive(
    Clone, PartialEq, Eq, Debug, Display, LightningEncode, LightningDecode,
)]
#[lnpbp_crate(crate)]
#[display("channel_id({chain_hash}, {short_channel_id}, {timestamp}, ...)")]
pub struct ChannelUpdate {
    /// Signature
    pub signature: Signature,

    /// Chainhash
    pub chain_hash: AssetId,

    /// Short Channel Id
    pub short_channel_id: ShortChannelId,

    /// Time stamp
    pub timestamp: u32,

    /// message flags
    pub message_flags: u8,

    /// channle flags
    pub channle_flags: u8,

    /// cltv expiry delta
    pub cltv_expiry_delta: u16,

    /// minimum HTLC in msat
    pub htlc_minimum_msal: u64,

    /// base fee in msat
    pub fee_base_msat: u32,

    /// fee proportional millionth
    pub fee_proportional_millionths: u32,

    /// if option_channel_htlc_max is set
    pub htlc_maximum_msat: u64,
}

/// Extended Gossip messages
#[derive(
    Clone, PartialEq, Eq, Debug, Display, LightningEncode, LightningDecode,
)]
#[lnpbp_crate(crate)]
#[display("query_short_channel_ids({chain_hash}, {short_ids:#?}, ...tlvs)")]
pub struct QueryShortChannelIds {
    /// chain hash
    pub chain_hash: AssetId,

    /// short ids to query
    pub short_ids: Vec<ShortChannelId>,
    /*short id tlv stream
     * TODO: uncomment once tlv implementation is complete
     * pub short_id_tlvs: BTreeMap<u8, Vec<u8>>, */
}

#[derive(
    Clone, PartialEq, Eq, Debug, Display, LightningEncode, LightningDecode,
)]
#[lnpbp_crate(crate)]
#[display("reply_short_channel_ids_end({chain_hash}, {full_information})")]
pub struct ReplyShortChannelIdsEnd {
    /// chain hash
    pub chain_hash: AssetId,

    /// full information
    pub full_information: u8,
}

#[derive(
    Clone, PartialEq, Eq, Debug, Display, LightningEncode, LightningDecode,
)]
#[lnpbp_crate(crate)]
#[display(
    "querry_channel_range({chain_hash}, {first_blocknum}, {number_of_blocks}, ...tlvs)"
)]
pub struct QueryChannelRange {
    /// chain hash
    pub chain_hash: AssetId,

    /// first block number
    pub first_blocknum: u32,

    /// number of blocks
    pub number_of_blocks: u32,
    /*channel range queries
    TODO: uncomment once tlv implementation is complete
     * pub query_channel_range_tlvs: BTreeMap<u8, Vec<u8>>, */
}

#[derive(
    Clone, PartialEq, Eq, Debug, Display, LightningEncode, LightningDecode,
)]
#[lnpbp_crate(crate)]
#[display(
    "reply_channel_range({chain_hash}, {first_blocknum}, {number_of_blocks}, ...)"
)]
pub struct ReplyChannelRange {
    /// chain hash
    pub chain_hash: AssetId,

    /// first block number
    pub first_blocknum: u32,

    /// number of blocks
    pub number_of_blocks: u32,

    /// full information
    pub full_information: u8,

    /// encoded short ids
    pub encoded_short_ids: Vec<ShortChannelId>,
    /* reply channel range tlvs
     * TODO: uncomment once tlv implementation is complete
     *pub reply_channel_range_tlvs: BTreeMap<u8, Vec<u8>>, */
}

#[derive(
    Clone, PartialEq, Eq, Debug, Display, LightningEncode, LightningDecode,
)]
#[lnpbp_crate(crate)]
#[display("gossip_time_stamp_filter({chain_hash}, {first_timestamp}, {timestamp_range})")]
pub struct GossipTimestampFilter {
    /// chain hash
    pub chain_hash: AssetId,

    /// first timestamp
    pub first_timestamp: u32,

    /// timestamp range
    pub timestamp_range: u32,
}

#[cfg(feature = "rgb")]
#[derive(
    Clone, PartialEq, Eq, Debug, Display, LightningEncode, LightningDecode,
)]
#[lnpbp_crate(crate)]
#[display("assign_funds({channel_id}, {outpoint}, ...)")]
pub struct AssignFunds {
    /// The channel ID
    pub channel_id: ChannelId,

    /// Consignment
    pub consignment: Consignment,

    /// Outpoint containing assignments
    pub outpoint: OutPoint,

    /// Blinding factor to decode concealed outpoint
    pub blinding: u64,
}

impl LightningEncode for Messages {
    fn lightning_encode<E: io::Write>(&self, e: E) -> Result<usize, io::Error> {
        Payload::from(self.clone()).lightning_encode(e)
    }
}

impl LightningDecode for Messages {
    fn lightning_decode<D: io::Read>(
        d: D,
    ) -> Result<Self, lightning_encoding::Error> {
        Ok((&*LNPWP_UNMARSHALLER
            .unmarshall(&Vec::<u8>::lightning_decode(d)?)
            .map_err(|err| {
                lightning_encoding::Error::DataIntegrityError(s!(
                    "can't unmarshall LMP message"
                ))
            })?)
            .clone())
    }
}

impl DumbDefault for OpenChannel {
    fn dumb_default() -> Self {
        OpenChannel {
            chain_hash: none!(),
            temporary_channel_id: TempChannelId::dumb_default(),
            funding_satoshis: 0,
            push_msat: 0,
            dust_limit_satoshis: 0,
            max_htlc_value_in_flight_msat: 0,
            channel_reserve_satoshis: 0,
            htlc_minimum_msat: 0,
            feerate_per_kw: 0,
            to_self_delay: 0,
            max_accepted_htlcs: 0,
            funding_pubkey: *SECP256K1_PUBKEY_DUMB,
            revocation_basepoint: *SECP256K1_PUBKEY_DUMB,
            payment_point: *SECP256K1_PUBKEY_DUMB,
            delayed_payment_basepoint: *SECP256K1_PUBKEY_DUMB,
            htlc_basepoint: *SECP256K1_PUBKEY_DUMB,
            first_per_commitment_point: *SECP256K1_PUBKEY_DUMB,
            channel_flags: 0,
            /* shutdown_scriptpubkey: None,
             * unknown_tlvs: none!(), */
        }
    }
}

#[derive(
    Clone, PartialEq, Eq, Debug, Display, LightningEncode, LightningDecode,
)]
#[lnpbp_crate(crate)]
#[display(Debug)]
pub struct OnionPacket {
    pub version: u8,
    pub public_key: bitcoin::secp256k1::PublicKey,
    pub hop_data: Vec<u8>, //[u8; 20 * 65],
    pub hmac: Hmac<sha256::Hash>,
}

impl DumbDefault for OnionPacket {
    fn dumb_default() -> Self {
        OnionPacket {
            version: 0,
            public_key: *SECP256K1_PUBKEY_DUMB,
            hop_data: empty!(),
            hmac: zero!(),
        }
    }
}
