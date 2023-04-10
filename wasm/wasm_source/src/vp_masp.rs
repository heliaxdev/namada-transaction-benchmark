use std::cmp::Ordering;

use masp_primitives::asset_type::AssetType;
use masp_primitives::transaction::components::Amount;
/// Multi-asset shielded pool VP.
use namada_vp_prelude::address::masp;
use namada_vp_prelude::storage::Epoch;
use namada_vp_prelude::*;

/// Convert Namada amount and token type to MASP equivalents
fn convert_amount(
    epoch: Epoch,
    token: &Address,
    val: token::Amount,
) -> (AssetType, Amount) {
    // Timestamp the chosen token with the current epoch
    let token_bytes = (token, epoch.0)
        .try_to_vec()
        .expect("token should serialize");
    // Generate the unique asset identifier from the unique token address
    let asset_type = AssetType::new(token_bytes.as_ref())
        .expect("unable to create asset type");
    // Combine the value and unit into one amount
    let amount = Amount::from_nonnegative(asset_type, u64::from(val))
        .expect("invalid value or asset type for amount");
    (asset_type, amount)
}

#[validity_predicate]
fn validate_tx(
    ctx: &Ctx,
    tx_data: Vec<u8>,
    addr: Address,
    keys_changed: BTreeSet<storage::Key>,
    verifiers: BTreeSet<Address>,
) -> VpResult {
    debug_log!(
        "vp_masp called with {} bytes data, address {}, keys_changed {:?}, \
         verifiers {:?}",
        tx_data.len(),
        addr,
        keys_changed,
        verifiers,
    );

    let signed = SignedTxData::try_from_slice(&tx_data[..]).unwrap();
    // Also get the data as bytes for the VM.
    let data = signed.data.as_ref().unwrap().clone();
    let transfer =
        token::Transfer::try_from_slice(&signed.data.unwrap()[..]).unwrap();

    if let Some(shielded_tx) = transfer.shielded {
        let mut transparent_tx_pool = Amount::zero();
        // The Sapling value balance adds to the transparent tx pool
        transparent_tx_pool += shielded_tx.value_balance.clone();

        // Handle shielding/transparent input
        if transfer.source != masp() {
            // Note that the asset type is timestamped so shields
            // where the shielded value has an incorrect timestamp
            // are automatically rejected
            let (_transp_asset, transp_amt) = convert_amount(
                ctx.get_block_epoch().unwrap(),
                &transfer.token,
                transfer.amount,
            );

            // Non-masp sources add to transparent tx pool
            transparent_tx_pool += transp_amt;
        }

        // Handle unshielding/transparent output
        if transfer.target != masp() {
            // Timestamp is derived to allow unshields for older tokens
            let atype =
                shielded_tx.value_balance.components().next().unwrap().0;

            let transp_amt =
                Amount::from_nonnegative(*atype, u64::from(transfer.amount))
                    .expect("invalid value or asset type for amount");

            // Non-masp destinations subtract from transparent tx pool
            transparent_tx_pool -= transp_amt;
        }

        match transparent_tx_pool.partial_cmp(&Amount::zero()) {
            None | Some(Ordering::Less) => {
                debug_log!(
                    "Transparent transaction value pool must be nonnegative. \
                     Violation may be caused by transaction being constructed \
                     in previous epoch. Maybe try again."
                );
                // Section 3.4: The remaining value in the transparent
                // transaction value pool MUST be nonnegative.
                return reject();
            }
            _ => {}
        }
    }

    // Do the expensive proof verification in the VM at the end.
    ctx.verify_masp(data)
}
