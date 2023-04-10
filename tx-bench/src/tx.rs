use criterion::Criterion;
use namada::core::types::token::Amount;

use namada::types::masp::{TransferSource, TransferTarget};
use namada_apps::wallet::defaults;
use namada_benches::{
    BenchShieldedCtx, ALBERT_PAYMENT_ADDRESS, ALBERT_SPENDING_KEY, BERTHA_PAYMENT_ADDRESS,
};

const TX_WITHDRAW_WASM: &str = "tx_withdraw.wasm";
const TX_INIT_ACCOUNT_WASM: &str = "tx_init_account.wasm";
const TX_INIT_VALIDATOR_WASM: &str = "tx_init_validator.wasm";

pub fn transfer(c: &mut Criterion) {
    let mut group = c.benchmark_group("transfer");
    group.sample_size(10);
    let amount = Amount::whole(500);

    for bench_name in ["transparent", "shielding", "unshielding", "shielded"] {
        group.bench_function(bench_name, |b| {
            b.iter_batched_ref(
                || {
                    let mut shielded_ctx = BenchShieldedCtx::default();

                    let albert_payment_addr = shielded_ctx
                        .ctx
                        .wallet
                        .find_payment_addr(ALBERT_PAYMENT_ADDRESS)
                        .unwrap()
                        .to_owned();

                    // Shield some tokens for Albert
                    let shield_tx = shielded_ctx.generate_masp_tx(
                        amount,
                        TransferSource::Address(defaults::albert_address()),
                        TransferTarget::PaymentAddress(albert_payment_addr),
                    );
                    shielded_ctx.shell.execute_tx(&shield_tx);
                    shielded_ctx.shell.wl_storage.commit_tx();
                    shielded_ctx.shell.commit();

                    shielded_ctx
                },
                |shielded_ctx| {
                    let albert_spending_key = shielded_ctx
                        .ctx
                        .wallet
                        .find_spending_key(ALBERT_SPENDING_KEY)
                        .unwrap()
                        .to_owned();
                    let albert_payment_addr = shielded_ctx
                        .ctx
                        .wallet
                        .find_payment_addr(ALBERT_PAYMENT_ADDRESS)
                        .unwrap()
                        .to_owned();
                    let bertha_payment_addr = shielded_ctx
                        .ctx
                        .wallet
                        .find_payment_addr(BERTHA_PAYMENT_ADDRESS)
                        .unwrap()
                        .to_owned();
                    let signed_tx = match bench_name {
                        "transparent" => shielded_ctx.generate_masp_tx(
                            amount,
                            TransferSource::Address(defaults::albert_address()),
                            TransferTarget::Address(defaults::bertha_address()),
                        ),
                        "shielding" => shielded_ctx.generate_masp_tx(
                            amount,
                            TransferSource::Address(defaults::albert_address()),
                            TransferTarget::PaymentAddress(albert_payment_addr),
                        ),
                        "unshielding" => shielded_ctx.generate_masp_tx(
                            amount,
                            TransferSource::ExtendedSpendingKey(albert_spending_key),
                            TransferTarget::Address(defaults::albert_address()),
                        ),
                        "shielded" => shielded_ctx.generate_masp_tx(
                            amount,
                            TransferSource::ExtendedSpendingKey(albert_spending_key),
                            TransferTarget::PaymentAddress(bertha_payment_addr),
                        ),
                        _ => panic!("Unexpected bench test"),
                    };
                    shielded_ctx.shell.execute_tx(&signed_tx);
                },
                criterion::BatchSize::LargeInput,
            )
        });
    }

    group.finish();
}
