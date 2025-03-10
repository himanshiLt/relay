/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 * @generated SignedSource<<1a63b7853a21e0490215fc4524e01ef1>>
 */

mod parse;

use parse::transform_fixture;
use fixture_tests::test_fixture;

#[test]
fn relay_resolver() {
    let input = include_str!("parse/fixtures/relay-resolver.js");
    let expected = include_str!("parse/fixtures/relay-resolver.expected");
    test_fixture(transform_fixture, "relay-resolver.js", "parse/fixtures/relay-resolver.expected", input, expected);
}

#[test]
fn relay_resolver_deprecated() {
    let input = include_str!("parse/fixtures/relay-resolver-deprecated.js");
    let expected = include_str!("parse/fixtures/relay-resolver-deprecated.expected");
    test_fixture(transform_fixture, "relay-resolver-deprecated.js", "parse/fixtures/relay-resolver-deprecated.expected", input, expected);
}

#[test]
fn relay_resolver_deprecated_no_description() {
    let input = include_str!("parse/fixtures/relay-resolver-deprecated-no-description.js");
    let expected = include_str!("parse/fixtures/relay-resolver-deprecated-no-description.expected");
    test_fixture(transform_fixture, "relay-resolver-deprecated-no-description.js", "parse/fixtures/relay-resolver-deprecated-no-description.expected", input, expected);
}

#[test]
fn relay_resolver_invalid_field_invalid() {
    let input = include_str!("parse/fixtures/relay-resolver-invalid-field.invalid.js");
    let expected = include_str!("parse/fixtures/relay-resolver-invalid-field.invalid.expected");
    test_fixture(transform_fixture, "relay-resolver-invalid-field.invalid.js", "parse/fixtures/relay-resolver-invalid-field.invalid.expected", input, expected);
}

#[test]
fn relay_resolver_missing_field_invalid() {
    let input = include_str!("parse/fixtures/relay-resolver-missing-field.invalid.js");
    let expected = include_str!("parse/fixtures/relay-resolver-missing-field.invalid.expected");
    test_fixture(transform_fixture, "relay-resolver-missing-field.invalid.js", "parse/fixtures/relay-resolver-missing-field.invalid.expected", input, expected);
}

#[test]
fn relay_resolver_missing_multiple_fields_invalid() {
    let input = include_str!("parse/fixtures/relay-resolver-missing-multiple-fields.invalid.js");
    let expected = include_str!("parse/fixtures/relay-resolver-missing-multiple-fields.invalid.expected");
    test_fixture(transform_fixture, "relay-resolver-missing-multiple-fields.invalid.js", "parse/fixtures/relay-resolver-missing-multiple-fields.invalid.expected", input, expected);
}
