use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use bitcoin::hashes::{sha256, Hash};
use bitcoin::secp256k1::{PublicKey, SecretKey};
use bitcoin::{secp256k1, KeyPair};
use fedimint_core::Amount;
use futures::stream;
use lightning::ln::PaymentSecret;
use lightning_invoice::{Currency, Invoice, InvoiceBuilder, SignedRawInvoice, DEFAULT_EXPIRY_TIME};
use ln_gateway::gatewaylnrpc::{
    self, CompleteHtlcsRequest, CompleteHtlcsResponse, GetNodeInfoResponse, GetRouteHintsResponse,
    PayInvoiceRequest, PayInvoiceResponse, SubscribeInterceptHtlcsRequest,
};
use ln_gateway::lnrpc_client::{HtlcStream, ILnRpcClient};
use ln_gateway::GatewayError;
use mint_client::modules::ln::contracts::Preimage;
use rand::rngs::OsRng;

use super::LightningTest;

#[derive(Clone, Debug)]
pub struct FakeLightningTest {
    pub preimage: Preimage,
    pub gateway_node_pub_key: secp256k1::PublicKey,
    gateway_node_sec_key: secp256k1::SecretKey,
    amount_sent: Arc<Mutex<u64>>,
    is_connected: bool,
}

impl FakeLightningTest {
    pub fn new() -> Self {
        let ctx = bitcoin::secp256k1::Secp256k1::new();
        let kp = KeyPair::new(&ctx, &mut OsRng);
        let amount_sent = Arc::new(Mutex::new(0));

        FakeLightningTest {
            preimage: Preimage([1; 32]),
            gateway_node_sec_key: SecretKey::from_keypair(&kp),
            gateway_node_pub_key: PublicKey::from_keypair(&kp),
            amount_sent,
            is_connected: true,
        }
    }
}

impl Default for FakeLightningTest {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl LightningTest for FakeLightningTest {
    async fn invoice(
        &self,
        amount: Amount,
        expiry_time: Option<u64>,
    ) -> ln_gateway::Result<Invoice> {
        if !self.is_connected {
            return Err(GatewayError::Other(anyhow::anyhow!(
                "Error not connected to Lightning"
            )));
        }

        let ctx = bitcoin::secp256k1::Secp256k1::new();

        Ok(InvoiceBuilder::new(Currency::Regtest)
            .description("".to_string())
            .payment_hash(sha256::Hash::hash(&self.preimage.0))
            .current_timestamp()
            .min_final_cltv_expiry(0)
            .payment_secret(PaymentSecret([0; 32]))
            .amount_milli_satoshis(amount.msats)
            .expiry_time(Duration::from_secs(
                expiry_time.unwrap_or(DEFAULT_EXPIRY_TIME),
            ))
            .build_signed(|m| ctx.sign_ecdsa_recoverable(m, &self.gateway_node_sec_key))
            .unwrap())
    }

    async fn amount_sent(&self) -> Amount {
        Amount::from_msats(*self.amount_sent.lock().unwrap())
    }

    fn is_shared(&self) -> bool {
        false
    }
}

#[async_trait]
impl ILnRpcClient for FakeLightningTest {
    async fn info(&self) -> ln_gateway::Result<GetNodeInfoResponse> {
        if !self.is_connected {
            return Err(GatewayError::Other(anyhow::anyhow!(
                "Error not connected to Lightning"
            )));
        }

        Ok(GetNodeInfoResponse {
            pub_key: self.gateway_node_pub_key.serialize().to_vec(),
            alias: "FakeLightningNode".to_string(),
        })
    }

    async fn routehints(&self) -> ln_gateway::Result<GetRouteHintsResponse> {
        if !self.is_connected {
            return Err(GatewayError::Other(anyhow::anyhow!(
                "Error not connected to Lightning"
            )));
        }

        Ok(GetRouteHintsResponse {
            route_hints: vec![gatewaylnrpc::get_route_hints_response::RouteHint { hops: vec![] }],
        })
    }

    async fn pay(&self, invoice: PayInvoiceRequest) -> ln_gateway::Result<PayInvoiceResponse> {
        if !self.is_connected {
            return Err(GatewayError::Other(anyhow::anyhow!(
                "Error not connected to Lightning"
            )));
        }

        let signed = invoice.invoice.parse::<SignedRawInvoice>().unwrap();
        *self.amount_sent.lock().unwrap() += Invoice::from_signed(signed)
            .unwrap()
            .amount_milli_satoshis()
            .unwrap();

        Ok(PayInvoiceResponse {
            preimage: self.preimage.0.to_vec(),
        })
    }

    async fn subscribe_htlcs<'a>(
        &self,
        _subscription: SubscribeInterceptHtlcsRequest,
    ) -> ln_gateway::Result<HtlcStream<'a>> {
        if !self.is_connected {
            return Err(GatewayError::Other(anyhow::anyhow!(
                "Error not connected to Lightning"
            )));
        }

        Ok(Box::pin(stream::iter(vec![])))
    }

    async fn complete_htlc(
        &self,
        _complete: CompleteHtlcsRequest,
    ) -> ln_gateway::Result<CompleteHtlcsResponse> {
        if !self.is_connected {
            return Err(GatewayError::Other(anyhow::anyhow!(
                "Error not connected to Lightning"
            )));
        }

        Ok(CompleteHtlcsResponse {})
    }

    async fn connect(&mut self) -> ln_gateway::Result<()> {
        self.is_connected = true;
        Ok(())
    }

    async fn disconnect(&mut self) -> ln_gateway::Result<()> {
        self.is_connected = false;
        Ok(())
    }
}
