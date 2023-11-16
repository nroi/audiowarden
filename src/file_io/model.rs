/// For data that is persisted on the user's hard disk, we use this trait to provide a safety net
/// against deserialization errors in case we do refactorings: For example, suppose we rename a
/// property of a struct, and then forget that this struct also exists on some users' file systems
/// in the form of a JSON document: Then, the application will fail with a deserialization error.
/// To avoid this, we want a clear distinction between structs that are used for our application
/// logic, and structs that are persisted on the file system. Data must therefore always be
/// explicitly converted between the struct used for serialization/deserialization, and the struct
/// used for application logic.
///
/// In more detail, the following rules should be adhered when Serializing data and persisting it
/// to disk:
/// - Every struct annotated with #[derive(Serialize)] must reside in the file_io module (at the
///   moment, we   use serialization only to persist data to disk, so no other module is
///   appropriate).
/// - When a struct is annotated with #[derive(Serialize)], it must not be public: If usage of
///   the struct is required in other modules, then create a separate struct for that module
///   which does not derive #[derive(Serialize)]. Then, simply transform the data between the two
///   structs (the non-public struct with #[derive(Serialize)], and the other struct without
///   #[derive(Serialize)]).
/// - Structs inside the file_io module that are annotated with #[derive(Serialize)] must have a
///   version suffix, e.g. V1, V2, etc. If a variable of a versioned struct should be renamed,
///   we must not change the existing struct, but instead introduce a new struct with a new version
///   which includes the new variable name. Care must be taken to ensure that we are still able to
///   deserialize data into the old struct in order to not break backwards-compatibility. Only
///   after we are sure that (almost) no user has data in the old format lying around, we can get
///   rid of the old struct.
pub trait Versioned<T>: From<T> + Into<T> {}
