use token_enricher::multicall::{decode_decimals_result, decode_symbol_result};

#[test]
fn decodes_weth_symbol_from_returndata() {
    // Returndata for `symbol()` returning "WETH" — ABI: dynamic string.
    let returndata = hex::decode(
        "0000000000000000000000000000000000000000000000000000000000000020\
         0000000000000000000000000000000000000000000000000000000000000004\
         5745544800000000000000000000000000000000000000000000000000000000",
    )
    .unwrap();
    assert_eq!(decode_symbol_result(&returndata).unwrap(), "WETH");
}

#[test]
fn decodes_decimals_18() {
    let returndata =
        hex::decode("0000000000000000000000000000000000000000000000000000000000000012").unwrap();
    assert_eq!(decode_decimals_result(&returndata).unwrap(), 18);
}

#[test]
fn empty_returndata_returns_error() {
    assert!(decode_symbol_result(&[]).is_err());
    assert!(decode_decimals_result(&[]).is_err());
}
