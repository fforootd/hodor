use serde::Serialize;

/// UINode types that the login SPA renders.
#[derive(Serialize, Clone)]
#[serde(tag = "type")]
pub(crate) enum UINode {
    #[serde(rename = "heading")]
    Heading { text: String },
    #[serde(rename = "description")]
    Description { text: String },
    #[serde(rename = "input")]
    Input {
        name: String,
        label: String,
        input_type: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        value: Option<String>,
        required: bool,
    },
    #[serde(rename = "submit")]
    Submit { label: String, action: String },
    #[serde(rename = "error")]
    Error { message: String },
    #[serde(rename = "hidden")]
    Hidden { name: String, value: String },
    #[serde(rename = "avatar")]
    Avatar { initial: String, text: String },
    #[serde(rename = "fingerprint_collect")]
    FingerprintCollect,
    #[serde(rename = "captcha_challenge")]
    CaptchaChallenge {
        algorithm: String,
        salt: String,
        challenge: String,
        maxnumber: u64,
        signature: String,
    },
}

pub(crate) fn identifier_step_nodes() -> Vec<UINode> {
    vec![
        UINode::Heading {
            text: "Sign in".into(),
        },
        UINode::Description {
            text: "Enter your email or username".into(),
        },
        UINode::Input {
            name: "identifier".into(),
            label: "Email or username".into(),
            input_type: "text".into(),
            value: None,
            required: true,
        },
        UINode::Submit {
            label: "Continue".into(),
            action: "identifier".into(),
        },
        UINode::FingerprintCollect,
    ]
}

pub(crate) fn password_step_nodes(identifier: &str) -> Vec<UINode> {
    vec![
        UINode::Heading {
            text: "Enter your password".into(),
        },
        UINode::Description {
            text: format!("Signing in as {identifier}"),
        },
        UINode::Hidden {
            name: "identifier".into(),
            value: identifier.to_string(),
        },
        UINode::Input {
            name: "password".into(),
            label: "Password".into(),
            input_type: "password".into(),
            value: None,
            required: true,
        },
        UINode::Submit {
            label: "Sign in".into(),
            action: "password".into(),
        },
        UINode::Submit {
            label: "Back".into(),
            action: "back".into(),
        },
    ]
}

pub(crate) fn session_reuse_nodes(identifier: &str, display_name: &str) -> Vec<UINode> {
    let initial = display_name
        .chars()
        .next()
        .or(identifier.chars().next())
        .unwrap_or('?')
        .to_string()
        .to_uppercase();
    let avatar_text = if !display_name.is_empty() && !identifier.is_empty() {
        format!("{} · {}", display_name, identifier)
    } else if !identifier.is_empty() {
        identifier.to_string()
    } else {
        display_name.to_string()
    };
    vec![
        UINode::Heading {
            text: "Use your existing session?".into(),
        },
        UINode::Description {
            text: "You're already signed in. Continue with that session or choose a different account.".into(),
        },
        UINode::Avatar {
            initial,
            text: avatar_text,
        },
        UINode::Submit {
            label: "Continue with this session".into(),
            action: "use_session".into(),
        },
        UINode::Submit {
            label: "Use a different account".into(),
            action: "back".into(),
        },
    ]
}

pub(crate) fn default_branding() -> serde_json::Value {
    serde_json::json!({
        "org_name": "Zitadel",
        "primary_color": "#4A90D9",
        "logo_url": "",
    })
}
