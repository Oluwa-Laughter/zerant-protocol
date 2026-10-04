use serde::Serialize;
use zerant_core::{Error, MAX_SAFE_INTEGER, Result};
use zip321::TransactionRequest;

pub const MAX_PAYMENT_REQUEST_BYTES: usize = 16_384;
pub const MAX_PAYMENT_REQUEST_PAYMENTS: usize = 16;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PaymentRequestPayment {
    pub index: usize,
    pub recipient: String,
    pub amount_zat: Option<u64>,
    pub memo_present: bool,
    pub label: Option<String>,
    pub message: Option<String>,
    pub other_param_names: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PaymentRequestSummary {
    pub canonical_uri: String,
    pub payment_count: usize,
    pub total_zat: Option<u64>,
    pub payments: Vec<PaymentRequestPayment>,
}

pub fn inspect_payment_request(uri: &str) -> Result<PaymentRequestSummary> {
    if uri.is_empty() || uri.len() > MAX_PAYMENT_REQUEST_BYTES {
        return Err(if uri.len() > MAX_PAYMENT_REQUEST_BYTES {
            Error::Size
        } else {
            Error::Encoding
        });
    }

    let request = TransactionRequest::from_uri(uri).map_err(|_| Error::Encoding)?;
    if request.payments().is_empty() || request.payments().len() > MAX_PAYMENT_REQUEST_PAYMENTS {
        return Err(Error::Size);
    }

    let mut payments = Vec::with_capacity(request.payments().len());
    for (index, payment) in request.payments() {
        let amount_zat = payment
            .amount()
            .map(u64::try_from)
            .transpose()
            .map_err(|_| Error::Encoding)?
            .filter(|value| *value <= MAX_SAFE_INTEGER);

        if payment.amount().is_some() && amount_zat.is_none() {
            return Err(Error::UnsafeNumber);
        }

        let mut other_param_names: Vec<String> = payment
            .other_params()
            .iter()
            .map(|(name, _)| name.clone())
            .collect();
        other_param_names.sort();
        other_param_names.dedup();

        payments.push(PaymentRequestPayment {
            index: *index,
            recipient: payment.recipient_address().encode(),
            amount_zat,
            memo_present: payment.memo().is_some(),
            label: payment.label().cloned(),
            message: payment.message().cloned(),
            other_param_names,
        });
    }

    let total_zat = request
        .total()
        .map_err(|_| Error::UnsafeNumber)?
        .map(u64::try_from)
        .transpose()
        .map_err(|_| Error::UnsafeNumber)?;

    if total_zat.is_some_and(|value| value > MAX_SAFE_INTEGER) {
        return Err(Error::UnsafeNumber);
    }

    Ok(PaymentRequestSummary {
        canonical_uri: request.to_uri(),
        payment_count: payments.len(),
        total_zat,
        payments,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const TESTNET_TADDR: &str = "tmEZhbWHTpdKMw5it8YDspUXSMGQyFwovpU";

    #[test]
    fn parses_and_canonicalizes_zip321_request() {
        let uri = format!("zcash:{TESTNET_TADDR}?amount=1.25&label=invoice&message=consulting");
        let summary = inspect_payment_request(&uri).unwrap();

        assert_eq!(summary.payment_count, 1);
        assert_eq!(summary.total_zat, Some(125_000_000));
        assert_eq!(summary.payments[0].amount_zat, Some(125_000_000));
        assert_eq!(summary.payments[0].recipient, TESTNET_TADDR);
        assert_eq!(summary.payments[0].label.as_deref(), Some("invoice"));
        assert_eq!(summary.payments[0].message.as_deref(), Some("consulting"));
        assert!(!summary.payments[0].memo_present);
        assert_eq!(
            inspect_payment_request(&summary.canonical_uri).unwrap(),
            summary
        );
    }

    #[test]
    fn rejects_invalid_and_oversized_requests() {
        assert!(inspect_payment_request("https://example.com/pay").is_err());
        let huge = format!(
            "zcash:{TESTNET_TADDR}?message={}",
            "x".repeat(MAX_PAYMENT_REQUEST_BYTES)
        );
        assert_eq!(inspect_payment_request(&huge), Err(Error::Size));
    }

    #[test]
    fn transparent_recipient_cannot_carry_memo() {
        let uri = format!("zcash:{TESTNET_TADDR}?memo=AA");
        assert!(inspect_payment_request(&uri).is_err());
    }
}
