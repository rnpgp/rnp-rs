//! Round-trip: generate a key, sign a message, verify the signature.
//!
//! Tests intentionally exercise the deprecated `generate_test_key` shim — it
//! must keep working until removed.

#![allow(deprecated)]

use rnp::{
    generate_test_key, sign, sign_detached, verify, verify_detached, Context, KeyIdentifier,
};

#[test]
fn inline_sign_verify_roundtrip() {
    let ctx = Context::new().expect("create context");
    let key = generate_test_key(&ctx, "roundtrip <roundtrip@example.com>")
        .expect("generate key");

    let message = b"the quick brown fox jumps over the lazy dog";
    let signed = sign(&ctx, message, &key).expect("sign");
    assert!(!signed.is_empty());

    let ok = verify(&ctx, &signed).expect("verify op ran");
    assert!(ok, "signature should verify");
}

#[test]
fn detached_sign_verify_roundtrip() {
    let ctx = Context::new().expect("create context");
    let key = generate_test_key(&ctx, "detached <detached@example.com>")
        .expect("generate key");

    let message = b"some bytes that need to be authenticated";
    let sig = sign_detached(&ctx, message, &key).expect("sign detached");
    assert!(!sig.is_empty());

    let ok = verify_detached(&ctx, message, &sig).expect("verify detached ran");
    assert!(ok, "detached signature should verify");
}

#[test]
fn verify_rejects_tampered_message() {
    let ctx = Context::new().expect("create context");
    let key = generate_test_key(&ctx, "tamper <tamper@example.com>").expect("generate key");

    let message = b"original message";
    let sig = sign_detached(&ctx, message, &key).expect("sign detached");

    // Flip a byte in the message — verification must fail.
    let mut tampered = message.to_vec();
    tampered[0] ^= 0xff;
    let result = verify_detached(&ctx, &tampered, &sig);
    assert!(result.is_err() || !result.unwrap(), "tampered message must not verify");
}

#[test]
fn find_key_by_userid() {
    let ctx = Context::new().expect("create context");
    let _key = generate_test_key(&ctx, "findable <findable@example.com>").expect("generate key");
    let found = ctx
        .find_key(KeyIdentifier::Userid("findable <findable@example.com>"))
        .expect("find_key call ran");
    assert!(found.is_some(), "key should be findable by userid");
}
