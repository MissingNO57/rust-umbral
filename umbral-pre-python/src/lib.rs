use pyo3::class::basic::CompareOp;
use pyo3::create_exception;
use pyo3::exceptions::{PyException, PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::pyclass::PyClass;
use pyo3::types::{PyBytes, PyUnicode};
use pyo3::wrap_pyfunction;
use pyo3::PyObjectProtocol;

use umbral_pre::{
    DecryptionError, DeserializationError, EncryptionError, OpenReencryptedError,
    ReencryptionError, SecretKeyFactoryError, SerializableToArray,
};

// A helper trait to generalize implementing various Python protocol functions for our types.
trait HasSerializableBackend<T> {
    fn as_backend(&self) -> &T;
    fn from_backend(backend: T) -> Self;
}

trait HasName {
    fn name() -> &'static str;
}

fn to_bytes<T: HasSerializableBackend<U>, U: SerializableToArray>(obj: &T) -> PyResult<PyObject> {
    let serialized = obj.as_backend().to_array();
    Python::with_gil(|py| -> PyResult<PyObject> {
        Ok(PyBytes::new(py, serialized.as_slice()).into())
    })
}

fn from_bytes<T: HasSerializableBackend<U> + HasName, U: SerializableToArray>(
    bytes: &[u8],
) -> PyResult<T> {
    U::from_bytes(bytes)
        .map(T::from_backend)
        .map_err(|err| match err {
            DeserializationError::ConstructionFailure => {
                PyValueError::new_err(format!("Failed to deserialize a {} object", T::name()))
            }
            DeserializationError::TooManyBytes => {
                PyValueError::new_err("The given bytestring is too long")
            }
            DeserializationError::NotEnoughBytes => {
                PyValueError::new_err("The given bytestring is too short")
            }
        })
}

fn hash<T: HasSerializableBackend<U> + HasName, U: SerializableToArray>(
    obj: &T,
) -> PyResult<isize> {
    let serialized = obj.as_backend().to_array();

    // call `hash((class_name, bytes(obj)))`
    Python::with_gil(|py| {
        let builtins = PyModule::import(py, "builtins")?;
        let arg1 = PyUnicode::new(py, T::name());
        let arg2: PyObject = PyBytes::new(py, serialized.as_slice()).into();
        builtins.getattr("hash")?.call1(((arg1, arg2),))?.extract()
    })
}

// For some reason this lint is not recognized in Rust 1.46 (the one in CI)
// remove when CI is updated to a newer Rust version.
#[allow(clippy::unknown_clippy_lints)]
#[allow(clippy::unnecessary_wraps)] // Don't want to wrap it in Ok() on every call
fn hexstr<T: HasSerializableBackend<U> + HasName, U: SerializableToArray>(
    obj: &T,
) -> PyResult<String> {
    let hex_str = hex::encode(obj.as_backend().to_array().as_slice());
    Ok(format!("{}:{}", T::name(), &hex_str[0..16]))
}

fn richcmp<T: HasName + PyClass + PartialEq>(
    obj: &T,
    other: PyRef<T>,
    op: CompareOp,
) -> PyResult<bool> {
    match op {
        CompareOp::Eq => Ok(obj == &*other),
        CompareOp::Ne => Ok(obj != &*other),
        _ => Err(PyTypeError::new_err(format!(
            "{} objects are not ordered",
            T::name()
        ))),
    }
}

create_exception!(umbral, GenericError, PyException);

#[pyclass(module = "umbral")]
#[derive(PartialEq)]
pub struct SecretKey {
    backend: umbral_pre::SecretKey,
}

impl HasSerializableBackend<umbral_pre::SecretKey> for SecretKey {
    fn as_backend(&self) -> &umbral_pre::SecretKey {
        &self.backend
    }

    fn from_backend(backend: umbral_pre::SecretKey) -> Self {
        Self { backend }
    }
}

impl HasName for SecretKey {
    fn name() -> &'static str {
        "SecretKey"
    }
}

#[pymethods]
impl SecretKey {
    #[staticmethod]
    pub fn random() -> Self {
        Self {
            backend: umbral_pre::SecretKey::random(),
        }
    }

    #[staticmethod]
    pub fn from_bytes(bytes: &[u8]) -> PyResult<Self> {
        from_bytes(bytes)
    }
}

#[pyproto]
impl PyObjectProtocol for SecretKey {
    fn __richcmp__(&self, other: PyRef<SecretKey>, op: CompareOp) -> PyResult<bool> {
        richcmp(self, other, op)
    }

    fn __bytes__(&self) -> PyResult<PyObject> {
        to_bytes(self)
    }

    fn __str__(&self) -> PyResult<String> {
        Ok(format!("{}:...", Self::name()))
    }
}

#[pyclass(module = "umbral")]
#[derive(PartialEq)]
pub struct SecretKeyFactory {
    backend: umbral_pre::SecretKeyFactory,
}

impl HasSerializableBackend<umbral_pre::SecretKeyFactory> for SecretKeyFactory {
    fn as_backend(&self) -> &umbral_pre::SecretKeyFactory {
        &self.backend
    }

    fn from_backend(backend: umbral_pre::SecretKeyFactory) -> Self {
        Self { backend }
    }
}

impl HasName for SecretKeyFactory {
    fn name() -> &'static str {
        "SecretKeyFactory"
    }
}

#[pymethods]
impl SecretKeyFactory {
    #[staticmethod]
    pub fn random() -> Self {
        Self {
            backend: umbral_pre::SecretKeyFactory::random(),
        }
    }

    pub fn secret_key_by_label(&self, label: &[u8]) -> PyResult<SecretKey> {
        self.backend
            .secret_key_by_label(label)
            .map(|backend_sk| SecretKey {
                backend: backend_sk,
            })
            .map_err(|err| match err {
                // Will be removed when #39 is fixed
                SecretKeyFactoryError::ZeroHash => {
                    GenericError::new_err("Resulting secret key is zero")
                }
            })
    }

    #[staticmethod]
    pub fn from_bytes(bytes: &[u8]) -> PyResult<Self> {
        from_bytes(bytes)
    }
}

#[pyproto]
impl PyObjectProtocol for SecretKeyFactory {
    fn __richcmp__(&self, other: PyRef<SecretKeyFactory>, op: CompareOp) -> PyResult<bool> {
        richcmp(self, other, op)
    }

    fn __bytes__(&self) -> PyResult<PyObject> {
        to_bytes(self)
    }

    fn __str__(&self) -> PyResult<String> {
        Ok(format!("{}:...", Self::name()))
    }
}

#[pyclass(module = "umbral")]
#[derive(PartialEq)]
pub struct PublicKey {
    backend: umbral_pre::PublicKey,
}

impl HasSerializableBackend<umbral_pre::PublicKey> for PublicKey {
    fn as_backend(&self) -> &umbral_pre::PublicKey {
        &self.backend
    }

    fn from_backend(backend: umbral_pre::PublicKey) -> Self {
        Self { backend }
    }
}

impl HasName for PublicKey {
    fn name() -> &'static str {
        "PublicKey"
    }
}

#[pymethods]
impl PublicKey {
    #[staticmethod]
    pub fn from_secret_key(sk: &SecretKey) -> Self {
        Self {
            backend: umbral_pre::PublicKey::from_secret_key(&sk.backend),
        }
    }

    #[staticmethod]
    pub fn from_bytes(bytes: &[u8]) -> PyResult<Self> {
        from_bytes(bytes)
    }
}

#[pyproto]
impl PyObjectProtocol for PublicKey {
    fn __richcmp__(&self, other: PyRef<PublicKey>, op: CompareOp) -> PyResult<bool> {
        richcmp(self, other, op)
    }

    fn __bytes__(&self) -> PyResult<PyObject> {
        to_bytes(self)
    }

    fn __hash__(&self) -> PyResult<isize> {
        hash(self)
    }

    fn __str__(&self) -> PyResult<String> {
        hexstr(self)
    }
}

#[pyclass(module = "umbral")]
#[derive(PartialEq)]
pub struct Signer {
    backend: umbral_pre::Signer,
}

impl HasName for Signer {
    fn name() -> &'static str {
        "Signer"
    }
}

#[pymethods]
impl Signer {
    #[new]
    pub fn new(sk: &SecretKey) -> Self {
        Self {
            backend: umbral_pre::Signer::new(&sk.backend),
        }
    }

    pub fn sign(&self, message: &[u8]) -> Signature {
        Signature {
            backend: self.backend.sign(message),
        }
    }

    pub fn verifying_key(&self) -> PublicKey {
        PublicKey {
            backend: self.backend.verifying_key(),
        }
    }
}

#[pyproto]
impl PyObjectProtocol for Signer {
    fn __richcmp__(&self, other: PyRef<Signer>, op: CompareOp) -> PyResult<bool> {
        richcmp(self, other, op)
    }

    fn __str__(&self) -> PyResult<String> {
        Ok(format!("{}:...", Self::name()))
    }
}

#[pyclass(module = "umbral")]
#[derive(PartialEq)]
pub struct Signature {
    backend: umbral_pre::Signature,
}

impl HasSerializableBackend<umbral_pre::Signature> for Signature {
    fn as_backend(&self) -> &umbral_pre::Signature {
        &self.backend
    }

    fn from_backend(backend: umbral_pre::Signature) -> Self {
        Self { backend }
    }
}

impl HasName for Signature {
    fn name() -> &'static str {
        "Signature"
    }
}

#[pymethods]
impl Signature {
    #[staticmethod]
    pub fn from_bytes(bytes: &[u8]) -> PyResult<Self> {
        from_bytes(bytes)
    }

    pub fn verify(&self, verifying_key: &PublicKey, message: &[u8]) -> bool {
        self.backend.verify(&verifying_key.backend, message)
    }
}

#[pyproto]
impl PyObjectProtocol for Signature {
    fn __richcmp__(&self, other: PyRef<Signature>, op: CompareOp) -> PyResult<bool> {
        richcmp(self, other, op)
    }

    fn __bytes__(&self) -> PyResult<PyObject> {
        to_bytes(self)
    }

    fn __hash__(&self) -> PyResult<isize> {
        hash(self)
    }

    fn __str__(&self) -> PyResult<String> {
        hexstr(self)
    }
}

#[pyclass(module = "umbral")]
#[derive(PartialEq)]
pub struct Capsule {
    backend: umbral_pre::Capsule,
}

impl HasSerializableBackend<umbral_pre::Capsule> for Capsule {
    fn as_backend(&self) -> &umbral_pre::Capsule {
        &self.backend
    }

    fn from_backend(backend: umbral_pre::Capsule) -> Self {
        Self { backend }
    }
}

impl HasName for Capsule {
    fn name() -> &'static str {
        "Capsule"
    }
}

#[pymethods]
impl Capsule {
    #[staticmethod]
    pub fn from_bytes(bytes: &[u8]) -> PyResult<Self> {
        from_bytes(bytes)
    }
}

#[pyproto]
impl PyObjectProtocol for Capsule {
    fn __richcmp__(&self, other: PyRef<Capsule>, op: CompareOp) -> PyResult<bool> {
        richcmp(self, other, op)
    }

    fn __bytes__(&self) -> PyResult<PyObject> {
        to_bytes(self)
    }

    fn __hash__(&self) -> PyResult<isize> {
        hash(self)
    }

    fn __str__(&self) -> PyResult<String> {
        hexstr(self)
    }
}

#[pyfunction]
pub fn encrypt(py: Python, pk: &PublicKey, plaintext: &[u8]) -> PyResult<(Capsule, PyObject)> {
    umbral_pre::encrypt(&pk.backend, plaintext)
        .map(|(backend_capsule, ciphertext)| {
            (
                Capsule {
                    backend: backend_capsule,
                },
                PyBytes::new(py, &ciphertext).into(),
            )
        })
        .map_err(|err| match err {
            EncryptionError::PlaintextTooLarge => {
                GenericError::new_err("Plaintext is too large to encrypt")
            }
        })
}

fn map_decryption_err(err: DecryptionError) -> PyErr {
    match err {
        DecryptionError::CiphertextTooShort => {
            PyValueError::new_err("The ciphertext must include the nonce")
        }
        DecryptionError::AuthenticationFailed => GenericError::new_err(
            "Decryption of ciphertext failed: \
            either someone tampered with the ciphertext or \
            you are using an incorrect decryption key.",
        ),
    }
}

#[pyfunction]
pub fn decrypt_original(
    py: Python,
    sk: &SecretKey,
    capsule: &Capsule,
    ciphertext: &[u8],
) -> PyResult<PyObject> {
    umbral_pre::decrypt_original(&sk.backend, &capsule.backend, &ciphertext)
        .map(|plaintext| PyBytes::new(py, &plaintext).into())
        .map_err(map_decryption_err)
}

#[pyclass(module = "umbral")]
#[derive(PartialEq)]
pub struct KeyFrag {
    backend: umbral_pre::KeyFrag,
}

impl HasSerializableBackend<umbral_pre::KeyFrag> for KeyFrag {
    fn as_backend(&self) -> &umbral_pre::KeyFrag {
        &self.backend
    }

    fn from_backend(backend: umbral_pre::KeyFrag) -> Self {
        Self { backend }
    }
}

impl HasName for KeyFrag {
    fn name() -> &'static str {
        "KeyFrag"
    }
}

#[pymethods]
impl KeyFrag {
    pub fn verify(
        &self,
        signing_pk: &PublicKey,
        delegating_pk: Option<&PublicKey>,
        receiving_pk: Option<&PublicKey>,
    ) -> bool {
        self.backend.verify(
            &signing_pk.backend,
            delegating_pk.map(|pk| &pk.backend),
            receiving_pk.map(|pk| &pk.backend),
        )
    }

    #[staticmethod]
    pub fn from_bytes(bytes: &[u8]) -> PyResult<Self> {
        from_bytes(bytes)
    }
}

#[pyproto]
impl PyObjectProtocol for KeyFrag {
    fn __richcmp__(&self, other: PyRef<KeyFrag>, op: CompareOp) -> PyResult<bool> {
        richcmp(self, other, op)
    }

    fn __bytes__(&self) -> PyResult<PyObject> {
        to_bytes(self)
    }

    fn __hash__(&self) -> PyResult<isize> {
        hash(self)
    }

    fn __str__(&self) -> PyResult<String> {
        hexstr(self)
    }
}

#[allow(clippy::too_many_arguments)]
#[pyfunction]
pub fn generate_kfrags(
    delegating_sk: &SecretKey,
    receiving_pk: &PublicKey,
    signing_sk: &SecretKey,
    threshold: usize,
    num_kfrags: usize,
    sign_delegating_key: bool,
    sign_receiving_key: bool,
) -> Vec<KeyFrag> {
    let backend_kfrags = umbral_pre::generate_kfrags(
        &delegating_sk.backend,
        &receiving_pk.backend,
        &signing_sk.backend,
        threshold,
        num_kfrags,
        sign_delegating_key,
        sign_receiving_key,
    );

    backend_kfrags
        .iter()
        .cloned()
        .map(|val| KeyFrag { backend: val })
        .collect()
}

#[pyclass(module = "umbral")]
#[derive(Clone, PartialEq)]
pub struct CapsuleFrag {
    backend: umbral_pre::CapsuleFrag,
}

impl HasSerializableBackend<umbral_pre::CapsuleFrag> for CapsuleFrag {
    fn as_backend(&self) -> &umbral_pre::CapsuleFrag {
        &self.backend
    }

    fn from_backend(backend: umbral_pre::CapsuleFrag) -> Self {
        Self { backend }
    }
}

impl HasName for CapsuleFrag {
    fn name() -> &'static str {
        "CapsuleFrag"
    }
}

#[pymethods]
impl CapsuleFrag {
    pub fn verify(
        &self,
        capsule: &Capsule,
        delegating_pk: &PublicKey,
        receiving_pk: &PublicKey,
        signing_pk: &PublicKey,
        metadata: Option<&[u8]>,
    ) -> bool {
        self.backend.verify(
            &capsule.backend,
            &delegating_pk.backend,
            &receiving_pk.backend,
            &signing_pk.backend,
            metadata,
        )
    }

    #[staticmethod]
    pub fn from_bytes(bytes: &[u8]) -> PyResult<Self> {
        from_bytes(bytes)
    }
}

#[pyproto]
impl PyObjectProtocol for CapsuleFrag {
    fn __richcmp__(&self, other: PyRef<CapsuleFrag>, op: CompareOp) -> PyResult<bool> {
        richcmp(self, other, op)
    }

    fn __bytes__(&self) -> PyResult<PyObject> {
        to_bytes(self)
    }

    fn __hash__(&self) -> PyResult<isize> {
        hash(self)
    }

    fn __str__(&self) -> PyResult<String> {
        hexstr(self)
    }
}

#[pyfunction]
pub fn reencrypt(capsule: &Capsule, kfrag: &KeyFrag, metadata: Option<&[u8]>) -> CapsuleFrag {
    let backend_cfrag = umbral_pre::reencrypt(&capsule.backend, &kfrag.backend, metadata);
    CapsuleFrag {
        backend: backend_cfrag,
    }
}

#[pyfunction]
pub fn decrypt_reencrypted(
    py: Python,
    decrypting_sk: &SecretKey,
    delegating_pk: &PublicKey,
    capsule: &Capsule,
    cfrags: Vec<CapsuleFrag>,
    ciphertext: &[u8],
) -> PyResult<PyObject> {
    let backend_cfrags: Vec<umbral_pre::CapsuleFrag> =
        cfrags.iter().cloned().map(|cfrag| cfrag.backend).collect();
    umbral_pre::decrypt_reencrypted(
        &decrypting_sk.backend,
        &delegating_pk.backend,
        &capsule.backend,
        &backend_cfrags,
        ciphertext,
    )
    .map(|plaintext| PyBytes::new(py, &plaintext).into())
    .map_err(|err| match err {
        ReencryptionError::OnOpen(err) => match err {
            OpenReencryptedError::NoCapsuleFrags => {
                PyValueError::new_err("Empty CapsuleFrag sequence")
            }
            OpenReencryptedError::MismatchedCapsuleFrags => {
                PyValueError::new_err("CapsuleFrags are not pairwise consistent")
            }
            OpenReencryptedError::RepeatingCapsuleFrags => {
                PyValueError::new_err("Some of the CapsuleFrags are repeated")
            }
            // Will be removed when #39 is fixed
            OpenReencryptedError::ZeroHash => {
                GenericError::new_err("An internally hashed value is zero")
            }
            OpenReencryptedError::ValidationFailed => {
                GenericError::new_err("Internal validation failed")
            }
        },
        ReencryptionError::OnDecryption(err) => map_decryption_err(err),
    })
}

/// A Python module implemented in Rust.
#[pymodule]
fn _umbral(py: Python, m: &PyModule) -> PyResult<()> {
    m.add_class::<SecretKey>()?;
    m.add_class::<SecretKeyFactory>()?;
    m.add_class::<PublicKey>()?;
    m.add_class::<Signer>()?;
    m.add_class::<Signature>()?;
    m.add_class::<Capsule>()?;
    m.add_class::<KeyFrag>()?;
    m.add_class::<CapsuleFrag>()?;
    m.add("GenericError", py.get_type::<GenericError>())?;
    m.add_function(wrap_pyfunction!(encrypt, m)?)?;
    m.add_function(wrap_pyfunction!(decrypt_original, m)?)?;
    m.add_function(wrap_pyfunction!(generate_kfrags, m)?)?;
    m.add_function(wrap_pyfunction!(reencrypt, m)?)?;
    m.add_function(wrap_pyfunction!(decrypt_reencrypted, m)?)?;
    Ok(())
}
