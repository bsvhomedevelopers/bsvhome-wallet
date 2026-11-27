pub use payment::PaymentTransaction;
use primitives::ApplicationMetadata;
pub use primitives::payment::{Payment, PaymentAmount, PaymentLink, PaymentRequest};
use primitives::{ChainAddress, TransactionType};

pub type BsvHomePayment = Payment;
pub type BsvHomePaymentAmount = PaymentAmount;
pub type BsvHomePaymentLink = PaymentLink;
pub type BsvHomePaymentRequest = PaymentRequest;
pub type BsvHomePaymentTransaction = PaymentTransaction;

#[uniffi::remote(Record)]
pub struct BsvHomePaymentTransaction {
    pub merchant: ApplicationMetadata,
    pub account: ChainAddress,
    pub transaction: String,
    pub transaction_type: TransactionType,
    pub memo: Option<String>,
    pub request: Option<BsvHomePaymentRequest>,
}
