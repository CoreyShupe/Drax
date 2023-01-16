/// Encryption type alias for `cfb8::Encryptor<Aes128>`
pub type Cipher = cfb8::Cfb8<aes::Aes128>;

pub use aes::cipher::NewCipher;
