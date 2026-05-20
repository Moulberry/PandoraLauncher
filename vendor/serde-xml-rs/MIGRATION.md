# Migration across breaking changes

## From 0.6.x or 0.7.x to 0.8.0

Breaking changes:
- Element content is now deserialized to a field named `#content` instead of `$value`.
- Attributes must now be deserialized to fields named `@...`, mirroring what was introduced in the serializer.
- Tuples become string only. This will be addressed in a future release.
- Namespace support means that namespace prefixes are now added to element names and attributes. You might have to rename some of your struct fields and enum variants to match.

Tips for migrating:
- Replace `$value` with `#content`.
- Rename any fields that must be deserialized from attributes to include the `@` prefix. For example, field `foo` becomes `@foo`.
- If complex tuples are important, please file an issue with details on the XML format you want to (de)serialize and the Rust types you would prefer to (de)serialize to/from.
