use std::str::FromStr;

use criterion::Criterion;
use namada::core::types::token::Amount;

use namada::types::masp::{TransferSource, TransferTarget};
use namada::ledger::wallet::Store;
use namada::ledger::wallet::Wallet;
use std::path::Path;
use namada::types::address::Address;

use tendermint_config::net::Address as TendermintAddress;
use namada::types::key::common::SecretKey;
use tendermint_rpc::HttpClient;

use namada::ledger::masp;
use masp_primitives::zip32::ExtendedFullViewingKey;
use namada::ledger::args;

use std::fs::File;
use std::fs::OpenOptions;
use std::io::Read;
use std::io::Write;
use namada::ledger::masp::find_valid_diversifier;
use rand::rngs::OsRng;
use namada::ledger::wallet::SdkWalletUtils;
use std::path::PathBuf;
use borsh::BorshSerialize;
use borsh::BorshDeserialize;
use masp_proofs::prover::LocalTxProver;
use std::env;


const TX_WITHDRAW_WASM: &str = "tx_withdraw.wasm";
const TX_INIT_ACCOUNT_WASM: &str = "tx_init_account.wasm";
const TX_INIT_VALIDATOR_WASM: &str = "tx_init_validator.wasm";

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
        if !(spend_path.exists()
            && convert_path.exists()
            && output_path.exists())
        {
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
            LocalTxProver::with_default_location()
                .expect("unable to load MASP Parameters")
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



pub fn transfer(c: &mut Criterion) {
    let mut group = c.benchmark_group("transfer");
    group.sample_size(10);
    let amount = Amount::whole(500);

    for bench_name in ["transparent", "shielding", "unshielding", "shielded"] {
        group.bench_function(bench_name, |b| {
            b.iter_batched_ref(
                || {
                    // change this
                    let mut tx_transfer_wasm = File::open("/home/murisi/namada/wasm/tx_transfer.7bb6b5f6b2126372f68711f133ab7cee1656e0cb0f052490f681b9a3a71aa691.wasm").unwrap();
                    let mut tx_reveal_pk_wasm = File::open("/home/murisi/namada/wasm/tx_reveal_pk.a956c436553d92e1dc8afcf44399e95559b3eb19ca4df5ada3d07fc6917e0591.wasm").unwrap();

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
                    let faucet_addr = Address::from_str("atest1v4ehgw36g9rygd6xgs65ydpsg9qnsv3sxuungwp5xaqnv333xu65gdfexcmng3fkgfryy3psdxyc4w")
                        .expect("Unable to construct source");
                    // Key to withdraw funds from the faucet
                    let faucet_key = SecretKey::from_str("001c1002a48ba1075e2602028697c2bdf182e07636927f399b22ca99e07f92e04a").expect("Invalid secret key");

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

                    let mut shielded_ctx = FuzzerShieldedUtils::new(Path::new("./").to_path_buf());
                    // let mut shielded_ctx = masp::ShieldedContext::default();

                    let addr = TendermintAddress::from_str("127.0.0.1:27657")
                        .expect("Unable to connect to RPC");
                    let client = HttpClient::new(addr).unwrap();
                    // what we want
                    // shielded_ctx.gen_shielded_transfer(client, args, transfer_tx);
                    (shielded_ctx, transfer_tx)
                },
                |(shielded_ctx, transfer_tx)| {
                    // let albert_spending_key = shielded_ctx
                    //     .ctx
                    //     .wallet
                    //     .find_spending_key(ALBERT_SPENDING_KEY)
                    //     .unwrap()
                    //     .to_owned();
                    // let albert_payment_addr = shielded_ctx
                    //     .ctx
                    //     .wallet
                    //     .find_payment_addr(ALBERT_PAYMENT_ADDRESS)
                    //     .unwrap()
                    //     .to_owned();
                    // let bertha_payment_addr = shielded_ctx
                    //     .ctx
                    //     .wallet
                    //     .find_payment_addr(BERTHA_PAYMENT_ADDRESS)
                    //     .unwrap()
                    //     .to_owned();
                    // let signed_tx = match bench_name {
                    //     "transparent" => shielded_ctx.generate_masp_tx(
                    //         amount,
                    //         TransferSource::Address(dev::albert_address()),
                    //         TransferTarget::Address(dev::bertha_address()),
                    //     ),
                    //     "shielding" => shielded_ctx.generate_masp_tx(
                    //         amount,
                    //         TransferSource::Address(dev::albert_address()),
                    //         TransferTarget::PaymentAddress(albert_payment_addr),
                    //     ),
                    //     "unshielding" => shielded_ctx.generate_masp_tx(
                    //         amount,
                    //         TransferSource::ExtendedSpendingKey(albert_spending_key),
                    //         TransferTarget::Address(dev::albert_address()),
                    //     ),
                    //     "shielded" => shielded_ctx.generate_masp_tx(
                    //         amount,
                    //         TransferSource::ExtendedSpendingKey(albert_spending_key),
                    //         TransferTarget::PaymentAddress(bertha_payment_addr),
                    //     ),
                    //     _ => panic!("Unexpected bench test"),
                    // };
                    // shielded_ctx.shell.execute_tx(&signed_tx);
                    ()
                },
                criterion::BatchSize::LargeInput,
            )
        });
    }

    group.finish();
}
