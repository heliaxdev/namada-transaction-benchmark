use std::str::FromStr;

use criterion::Criterion;
// use namada::core::types::token::Amount;

use masp_primitives::transaction::builder::TransactionMetadata;
use masp_primitives::transaction::Transaction;
use namada::ledger::wallet::Store;
use namada::ledger::wallet::Wallet;
use namada::types::address::Address;
use namada::types::masp::{TransferSource, TransferTarget};
use std::path::Path;

use namada::types::key::common::SecretKey;
use tendermint_config::net::Address as TendermintAddress;
use tendermint_rpc::HttpClient;

use masp_primitives::zip32::ExtendedFullViewingKey;
use namada::ledger::args;
use namada::ledger::masp;

use borsh::BorshDeserialize;
use borsh::BorshSerialize;
use masp_primitives::transaction::builder;
use masp_proofs::prover::LocalTxProver;
use namada::ledger::masp::find_valid_diversifier;
use namada::ledger::wallet::SdkWalletUtils;
use rand::rngs::OsRng;
use std::env;
use std::fs::File;
use std::fs::OpenOptions;
use std::io::Read;
use std::io::Write;
use std::path::PathBuf;
use tokio::runtime::Runtime;

use wasm_bindgen::prelude::*;

/// Shielded context file name
const FILE_NAME: &str = "shielded.dat";
const TMP_FILE_NAME: &str = "shielded.tmp";

#[derive(Debug, BorshSerialize, BorshDeserialize, Clone)]
pub struct FuzzerShieldedUtils {
    #[borsh_skip]
    context_dir: PathBuf,
}

impl FuzzerShieldedUtils {
    /// Initialize a shielded transaction context that identifies notes
    /// decryptable by any viewing key in the given set
    pub fn new(context_dir: PathBuf) -> masp::ShieldedContext<Self> {
        // Make sure that MASP parameters are downloaded to enable MASP
        // transaction building and verification later on
        let params_dir = masp::get_params_dir();
        let spend_path = params_dir.join(masp::SPEND_NAME);
        let convert_path = params_dir.join(masp::CONVERT_NAME);
        let output_path = params_dir.join(masp::OUTPUT_NAME);
        if !(spend_path.exists() && convert_path.exists() && output_path.exists()) {
            println!("MASP parameters not present, downloading...");
            masp_proofs::download_parameters()
                .expect("MASP parameters not present or downloadable");
            println!("MASP parameter download complete, resuming execution...");
        }
        // Finally initialize a shielded context with the supplied directory
        let utils = Self { context_dir };
        masp::ShieldedContext {
            utils,
            ..Default::default()
        }
    }
}

impl Default for FuzzerShieldedUtils {
    fn default() -> Self {
        Self {
            context_dir: PathBuf::from(FILE_NAME),
        }
    }
}

impl masp::ShieldedUtils for FuzzerShieldedUtils {
    type C = tendermint_rpc::HttpClient;

    fn local_tx_prover(&self) -> LocalTxProver {
        if let Ok(params_dir) = env::var(masp::ENV_VAR_MASP_PARAMS_DIR) {
            let params_dir = PathBuf::from(params_dir);
            let spend_path = params_dir.join(masp::SPEND_NAME);
            let convert_path = params_dir.join(masp::CONVERT_NAME);
            let output_path = params_dir.join(masp::OUTPUT_NAME);
            LocalTxProver::new(&spend_path, &output_path, &convert_path)
        } else {
            LocalTxProver::with_default_location().expect("unable to load MASP Parameters")
        }
    }

    /// Try to load the last saved shielded context from the given context
    /// directory. If this fails, then leave the current context unchanged.
    fn load(self) -> std::io::Result<masp::ShieldedContext<Self>> {
        // Try to load shielded context from file
        let mut ctx_file = File::open(self.context_dir.join(FILE_NAME))?;
        let mut bytes = Vec::new();
        ctx_file.read_to_end(&mut bytes)?;
        let mut new_ctx = masp::ShieldedContext::deserialize(&mut &bytes[..])?;
        // Associate the originating context directory with the
        // shielded context under construction
        new_ctx.utils = self;
        Ok(new_ctx)
    }

    /// Save this shielded context into its associated context directory
    fn save(&self, ctx: &masp::ShieldedContext<Self>) -> std::io::Result<()> {
        // TODO: use mktemp crate?
        let tmp_path = self.context_dir.join(TMP_FILE_NAME);
        {
            // First serialize the shielded context into a temporary file.
            // Inability to create this file implies a simultaneuous write is in
            // progress. In this case, immediately fail. This is unproblematic
            // because the data intended to be stored can always be re-fetched
            // from the blockchain.
            let mut ctx_file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(tmp_path.clone())?;
            let mut bytes = Vec::new();
            ctx.serialize(&mut bytes)
                .expect("cannot serialize shielded context");
            ctx_file.write_all(&bytes[..])?;
        }
        // Atomically update the old shielded context file with new data.
        // Atomicity is required to prevent other client instances from reading
        // corrupt data.
        std::fs::rename(tmp_path.clone(), self.context_dir.join(FILE_NAME))?;
        // Finally, remove our temporary file to allow future saving of shielded
        // contexts.
        std::fs::remove_file(tmp_path)?;
        Ok(())
    }
}


async fn shielded(
    ctx: &mut masp::ShieldedContext<FuzzerShieldedUtils>,
    client: &HttpClient,
    args: args::TxTransfer,
) -> Result<Option<(Transaction, TransactionMetadata)>, builder::Error> {
    ctx.gen_shielded_transfer(client, args, true).await
}

pub fn transfer(c: &mut Criterion) {
    let mut group = c.benchmark_group("transfer");
    group.sample_size(10);
    // let amount = Amount::whole(500);

    group.bench_function("shielded", move |b| {
            b.to_async(Runtime::new().unwrap()).iter_batched(
                || {
                    let mut tx_transfer_wasm = File::open("../wasm/tx_transfer.2bceb190b553ea34a653d59f235be5df657c1d900f24de2dada58dff19d53b3c.wasm").unwrap();
                    let mut tx_reveal_pk_wasm = File::open("../wasm/tx_reveal_pk.d5f92e24ee566e5ecbb0def6bade4c942dd3dc5c7258b460fb8edc4cc641ebcf.wasm").unwrap();

                    let mut tx_transfer_bytes = vec![];
                    tx_transfer_wasm.read_to_end(&mut tx_transfer_bytes).unwrap();

                    let mut tx_reveal_pk_bytes = vec![];
                    tx_reveal_pk_wasm.read_to_end(&mut tx_reveal_pk_bytes).unwrap();

                    let mut wallet : Wallet<SdkWalletUtils<PathBuf>> = Wallet::new(
                        Path::new("wallet.toml").to_path_buf(),
                        Store::default(),
                    );

                    // Generate a spending key
                    let (alias, _spending_key) = wallet.gen_spending_key("joe".to_string(), None);
                    let viewing_key = wallet.find_viewing_key(alias.clone()).expect("A viewing key");
                    let (div, _g_d) = find_valid_diversifier(&mut OsRng);


                    let payment_addr = ExtendedFullViewingKey::from(*viewing_key).fvk.vk.to_payment_address(div)
                        .expect("a PaymentAddress");
                    let native_token = Address::from_str("atest1v4ehgw36x3prswzxggunzv6pxqmnvdj9xvcyzvpsggeyvs3cg9qnywf589qnwvfsg5erg3fkl09rg5")
                        .expect("Unable to construct native token");
                    // Address of the faucet
                    let faucet_addr = Address::from_str("atest1v4ehgw36gyerxv6xgyunqv3egsmnv3pj8quny3fc8prrs32rg4qnxv2ygser2djxgcmnzv2y3dnxyq")
                        .expect("Unable to construct source");
                    // Key to withdraw funds from the faucet
                    let faucet_key = SecretKey::from_str("0079b5f7bf9a7634c3ab1f7853bc196283e4190422aa21085a0dbc548e554e0da2").expect("Invalid secret key");

                    // Construct out shielding transaction
                    let transfer_tx: args::TxTransfer = args::TxTransfer {
                        amount: 23000000.into(),
                        native_token: native_token.clone(),
                        source: TransferSource::Address(faucet_addr.clone()),
                        target: TransferTarget::PaymentAddress(payment_addr.clone().into()),
                        token: native_token.clone(),
                        sub_prefix: None,
                        tx_code_path: tx_transfer_bytes,
                        tx: args::Tx {
                            broadcast_only: false,
                            dry_run: false,
                            fee_amount: 0.into(),
                            fee_token: native_token,
                            force: false,
                            gas_limit: 0.into(),
                            initialized_account_alias: None,
                            ledger_address: (),
                            password: None,
                            signer: None,
                            signing_key: Some(faucet_key),
                            tx_code_path: tx_reveal_pk_bytes,
                        },
                    };

                    let shielded_ctx = FuzzerShieldedUtils::new(Path::new("./").to_path_buf());
                    // let mut shielded_ctx = masp::ShieldedContext::default();

                    let addr = TendermintAddress::from_str("127.0.0.1:27657")
                        .expect("Unable to connect to RPC");
                    let client = HttpClient::new(addr).unwrap();
                    // what we want
                    // shielded_ctx.gen_shielded_transfer(client, args, transfer_tx);
                    (shielded_ctx, transfer_tx, client)
                },
                |(mut shielded_ctx, transfer_tx, client)| {
                    async move {
                        let _res = shielded(&mut shielded_ctx, &client, transfer_tx.clone()).await;
                        // println!("Results: {:?}", res);
                        ()
                    }
                },
                criterion::BatchSize::LargeInput,
            )
        });

    group.finish();
}
