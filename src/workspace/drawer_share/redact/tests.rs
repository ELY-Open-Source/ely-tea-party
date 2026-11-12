use super::redact;

#[test]
fn credentials_in_shell_json_headers_and_urls_are_removed() {
    for input in [
        "export OPENAI_API_KEY=fixture-value",
        r#"{"access_token": "fixture-value", "ok": true}"#,
        "curl --password 'fixture-value' https://example.com",
        "Authorization: Bearer fixture-value",
        "Cookie: session=fixture-value; user=fixture-user",
        "https://user:fixture-value@example.com/path?token=fixture-value&ok=true",
        "AWS_SECRET_ACCESS_KEY=fixture-value",
        "token: fixture-value",
        r#"{"password":"quoted\"fixture-value", "status":403}"#,
        "PRIVATE_KEY=fixture-value",
        r#"{"Authorization": "fixture-value"}"#,
        "curl --cookie 'fixture-value; session=another-value'",
    ] {
        let output = redact(input);
        assert!(!output.contains("fixture-value"), "{output}");
        assert!(output.contains("[REDACTED]"), "{output}");
    }
}

#[test]
fn wrapped_terminal_secrets_are_joined_before_redaction() {
    use alacritty_terminal::{
        event::VoidListener,
        term::{Config, Term, test::TermSize},
        vte::ansi::Processor,
    };
    let mut term = Term::new(Config::default(), &TermSize::new(18, 8), VoidListener);
    let mut parser: Processor = Processor::new();
    parser.advance(
        &mut term,
        b"API_KEY=fixture-secret-across-many-soft-wrapped-rows\r\nerror: HTTP 403",
    );
    let (lines, _) = crate::term::snapshot::tail(&term);
    let output = redact(&lines.join("\n"));
    assert!(!output.contains("fixture-secret"));
    assert!(!output.contains("soft-wrapped"));
    assert!(output.contains("error: HTTP 403"));
}

#[test]
fn standalone_tokens_private_keys_and_personal_paths_are_removed() {
    for secret in [
        "sk-proj-abcdefghijklmnopqrstuv",
        "ghp_abcdefghijklmnopqrstuv",
        "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxIn0.fakeSignature",
        "-----BEGIN RSA PRIVATE KEY-----\nprivate-data\n-----END RSA PRIVATE KEY-----",
        "-----BEGIN OPENSSH PRIVATE KEY-----\nprivate-data",
        "private-data\n-----END PRIVATE KEY-----",
        "person@example.com",
    ] {
        let output = redact(secret);
        assert!(!output.contains(secret), "{output}");
        assert!(!output.contains("private-data"), "{output}");
    }
    assert_eq!(
        redact("/Users/alice/project/main.rs:12"),
        "~/project/main.rs:12"
    );
    assert_eq!(
        redact(r"C:\Users\alice\project\main.rs"),
        r"~\project\main.rs"
    );
}

#[test]
fn diagnostics_survive_and_redaction_is_idempotent() {
    let diagnostic = "error[E0308]: 类型不匹配\n  src/main.rs:12:5\nexpected 200, got 403";
    assert_eq!(redact(diagnostic), diagnostic);
    let once =
        redact("token=fixture-value\nAuthorization: Bearer fixture-value\n/home/alice/project");
    assert_eq!(redact(&once), once);
    assert_eq!(redact("hello\x1b\x03\x00\r\nworld"), "hello\nworld");
}
