# Serialization Compatibility Tests

The contracts include golden tests that serialize public-facing `#[contracttype]` structs/enums and event payloads to XDR and compare against committed hex outputs.

This catches accidental breaking changes to type layouts that would impact SDKs, indexers, or other external tooling.

## Compatibility Policy

### Breaking Changes

A breaking serialization change is any modification that causes the XDR encoding of a stored type to differ from its previous encoding. This includes:

- Renaming struct fields
- Changing field order
- Changing field types
- Adding required fields (without defaults)
- Removing fields
- Changing enum variant names or ordering

### Migration Requirements

**Breaking serialization changes require a migration plan before merge.** The migration plan must:

1. Document the old and new formats
2. Provide migration code to convert existing storage
3. Include upgrade path testing
4. Be reviewed by at least one maintainer

### Safe Changes

The following changes are safe and do not require migration:

- Adding new types (does not affect existing storage)
- Adding optional fields with `Option<T>`
- Adding new enum variants at the end (for extensible enums)
- Documentation changes

## Updating Goldens

When you intentionally change a public type/event layout:

1. Regenerate the golden files:
   ```bash
   # For grainlify-core:
   GRAINLIFY_PRINT_SERIALIZATION_GOLDENS=1 cargo test -p grainlify-core --lib serialization_compatibility_public_types_and_events -- --nocapture 2>&1 | grep -A1000 "const EXPECTED"

   # For bounty-escrow:
   GRAINLIFY_PRINT_SERIALIZATION_GOLDENS=1 cargo test -p bounty-escrow --lib serialization_compatibility_public_types_and_events -- --nocapture 2>&1 | grep -A1000 "const EXPECTED"
   ```
2. Review the diff and ensure the changes are expected.
3. Update the `serialization_goldens.rs` file with the new output.
4. Commit the updated golden files together with the intentional layout change.

## Adding New Types

When adding new `#[contracttype]` types:

1. Add the type to `test_serialization_compatibility.rs` in the appropriate crate
2. Run the test - it will fail with a message to regenerate goldens
3. Regenerate goldens using the command above
4. Update `serialization_goldens.rs` with the new entries
5. Run tests again to verify all goldens match

