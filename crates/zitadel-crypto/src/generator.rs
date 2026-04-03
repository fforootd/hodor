use serde::{Deserialize, Serialize};

/// Character set options for secret/code generation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CharsetKind {
    /// 0-9
    Digits,
    /// A-Z + 0-9
    UpperDigits,
    /// a-z + A-Z + 0-9
    Alphanumeric,
    /// a-z + A-Z + 0-9 + symbols (!@#$%^&*)
    AlphanumericSymbols,
    /// Use `custom_chars` field
    Custom,
}

/// Configuration for a single secret/code generator.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratorProfile {
    /// Number of characters to generate.
    pub length: usize,
    /// Which character set to draw from.
    pub charset: CharsetKind,
    /// Custom characters (only used when `charset = "custom"`).
    #[serde(default)]
    pub custom_chars: String,
    /// How long the generated code is valid, in seconds. None = no expiry.
    #[serde(default)]
    pub expiry_secs: Option<u64>,
    /// Insert a dash every N characters (e.g., `ABCD-EFGH` with dash_interval=4).
    #[serde(default)]
    pub dash_interval: Option<usize>,
}

const DIGITS: &[u8] = b"0123456789";
const UPPER: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ";
const LOWER: &[u8] = b"abcdefghijklmnopqrstuvwxyz";
const SYMBOLS: &[u8] = b"!@#$%^&*";

impl GeneratorProfile {
    fn charset_bytes(&self) -> Vec<u8> {
        match &self.charset {
            CharsetKind::Digits => DIGITS.to_vec(),
            CharsetKind::UpperDigits => [UPPER, DIGITS].concat(),
            CharsetKind::Alphanumeric => [LOWER, UPPER, DIGITS].concat(),
            CharsetKind::AlphanumericSymbols => [LOWER, UPPER, DIGITS, SYMBOLS].concat(),
            CharsetKind::Custom => {
                if self.custom_chars.is_empty() {
                    // Fallback to alphanumeric if custom_chars is empty.
                    [LOWER, UPPER, DIGITS].concat()
                } else {
                    self.custom_chars.as_bytes().to_vec()
                }
            }
        }
    }
}

/// Generate a random string according to the profile.
///
/// Uses `rand::fill` for cryptographic randomness, then indexes into the
/// resolved charset. If `dash_interval` is set, dashes are inserted between
/// groups (they don't count toward `length`).
pub fn generate(profile: &GeneratorProfile) -> String {
    let charset = profile.charset_bytes();
    assert!(!charset.is_empty(), "charset must not be empty");

    let mut random_bytes = vec![0u8; profile.length];
    rand::fill(random_bytes.as_mut_slice());

    let mut result = String::with_capacity(profile.length + profile.length / 4);
    for (i, &byte) in random_bytes.iter().enumerate() {
        if profile
            .dash_interval
            .is_some_and(|interval| interval > 0 && i > 0 && i % interval == 0)
        {
            result.push('-');
        }
        result.push(charset[byte as usize % charset.len()] as char);
    }

    result
}

/// Well-known generator purposes with compiled-in defaults.
/// These match Go Zitadel's `DefaultInstance.SecretGenerators`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GeneratorPurpose {
    ClientSecret,
    EmailVerificationCode,
    PhoneVerificationCode,
    PasswordVerificationCode,
    PasswordlessInitCode,
    DomainVerification,
    OtpSms,
    OtpEmail,
    InviteCode,
    InitializeUserCode,
    DeviceAuthUserCode,
}

impl GeneratorPurpose {
    /// Return the default profile for this purpose.
    pub fn default_profile(&self) -> GeneratorProfile {
        match self {
            Self::ClientSecret => GeneratorProfile {
                length: 64,
                charset: CharsetKind::Alphanumeric,
                custom_chars: String::new(),
                expiry_secs: None,
                dash_interval: None,
            },
            Self::EmailVerificationCode | Self::PhoneVerificationCode => GeneratorProfile {
                length: 6,
                charset: CharsetKind::UpperDigits,
                custom_chars: String::new(),
                expiry_secs: Some(3600),
                dash_interval: None,
            },
            Self::PasswordVerificationCode => GeneratorProfile {
                length: 6,
                charset: CharsetKind::UpperDigits,
                custom_chars: String::new(),
                expiry_secs: Some(3600),
                dash_interval: None,
            },
            Self::PasswordlessInitCode => GeneratorProfile {
                length: 12,
                charset: CharsetKind::Alphanumeric,
                custom_chars: String::new(),
                expiry_secs: Some(3600),
                dash_interval: None,
            },
            Self::DomainVerification => GeneratorProfile {
                length: 32,
                charset: CharsetKind::Alphanumeric,
                custom_chars: String::new(),
                expiry_secs: None,
                dash_interval: None,
            },
            Self::OtpSms | Self::OtpEmail => GeneratorProfile {
                length: 8,
                charset: CharsetKind::Digits,
                custom_chars: String::new(),
                expiry_secs: Some(300),
                dash_interval: None,
            },
            Self::InviteCode | Self::InitializeUserCode => GeneratorProfile {
                length: 6,
                charset: CharsetKind::UpperDigits,
                custom_chars: String::new(),
                expiry_secs: Some(259200),
                dash_interval: None,
            },
            Self::DeviceAuthUserCode => GeneratorProfile {
                length: 8,
                charset: CharsetKind::Custom,
                custom_chars: "BCDFGHJKLMNPQRSTVWXZ".to_string(),
                expiry_secs: Some(300),
                dash_interval: Some(4),
            },
        }
    }

    /// Settings table key for this purpose: `"generator.client_secret"` etc.
    pub fn settings_key(&self) -> &'static str {
        match self {
            Self::ClientSecret => "generator.client_secret",
            Self::EmailVerificationCode => "generator.email_verification_code",
            Self::PhoneVerificationCode => "generator.phone_verification_code",
            Self::PasswordVerificationCode => "generator.password_verification_code",
            Self::PasswordlessInitCode => "generator.passwordless_init_code",
            Self::DomainVerification => "generator.domain_verification",
            Self::OtpSms => "generator.otp_sms",
            Self::OtpEmail => "generator.otp_email",
            Self::InviteCode => "generator.invite_code",
            Self::InitializeUserCode => "generator.initialize_user_code",
            Self::DeviceAuthUserCode => "generator.device_auth_user_code",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_digits_only() {
        let profile = GeneratorPurpose::OtpSms.default_profile();
        let code = generate(&profile);
        assert_eq!(code.len(), 8);
        assert!(code.chars().all(|c| c.is_ascii_digit()));
    }

    #[test]
    fn generate_alphanumeric() {
        let profile = GeneratorPurpose::ClientSecret.default_profile();
        let code = generate(&profile);
        assert_eq!(code.len(), 64);
        assert!(code.chars().all(|c| c.is_ascii_alphanumeric()));
    }

    #[test]
    fn generate_upper_digits() {
        let profile = GeneratorPurpose::EmailVerificationCode.default_profile();
        let code = generate(&profile);
        assert_eq!(code.len(), 6);
        assert!(
            code.chars()
                .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
        );
    }

    #[test]
    fn generate_with_dashes() {
        let profile = GeneratorPurpose::DeviceAuthUserCode.default_profile();
        let code = generate(&profile);
        // 8 chars + 1 dash (at position 4) = 9 chars total
        assert_eq!(code.len(), 9);
        assert_eq!(code.chars().nth(4), Some('-'));
    }

    #[test]
    fn generate_custom_charset() {
        let profile = GeneratorPurpose::DeviceAuthUserCode.default_profile();
        let code = generate(&profile);
        let valid = "BCDFGHJKLMNPQRSTVWXZ-";
        assert!(code.chars().all(|c| valid.contains(c)));
    }

    #[test]
    fn generate_uniqueness() {
        let profile = GeneratorPurpose::ClientSecret.default_profile();
        let a = generate(&profile);
        let b = generate(&profile);
        assert_ne!(a, b, "two generated secrets should not be identical");
    }

    #[test]
    fn all_purposes_have_settings_keys() {
        let purposes = [
            GeneratorPurpose::ClientSecret,
            GeneratorPurpose::EmailVerificationCode,
            GeneratorPurpose::PhoneVerificationCode,
            GeneratorPurpose::PasswordVerificationCode,
            GeneratorPurpose::PasswordlessInitCode,
            GeneratorPurpose::DomainVerification,
            GeneratorPurpose::OtpSms,
            GeneratorPurpose::OtpEmail,
            GeneratorPurpose::InviteCode,
            GeneratorPurpose::InitializeUserCode,
            GeneratorPurpose::DeviceAuthUserCode,
        ];
        for purpose in purposes {
            assert!(
                purpose.settings_key().starts_with("generator."),
                "{:?} has bad settings key: {}",
                purpose,
                purpose.settings_key()
            );
        }
    }

    #[test]
    fn profile_serde_roundtrip() {
        let profile = GeneratorPurpose::DeviceAuthUserCode.default_profile();
        let json = serde_json::to_string(&profile).unwrap();
        let parsed: GeneratorProfile = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.length, profile.length);
        assert_eq!(parsed.charset, profile.charset);
        assert_eq!(parsed.custom_chars, profile.custom_chars);
        assert_eq!(parsed.dash_interval, profile.dash_interval);
    }
}
