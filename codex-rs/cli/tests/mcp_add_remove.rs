use std::path::Path;

use anyhow::Result;
use codex_core::config::load_global_mcp_servers;
use codex_core::config_types::McpServerTransportConfig;
use codex_core::mcp_registry::McpRegistry;
use predicates::str::contains;
use pretty_assertions::assert_eq;
use serde_json::Value;
use tempfile::TempDir;

fn codex_command(codex_home: &Path) -> Result<assert_cmd::Command> {
    let mut cmd = assert_cmd::Command::cargo_bin("codex")?;
    cmd.env("CODEX_HOME", codex_home);
    cmd.env("CODEX_STATE_HOME", codex_home.join("state"));
    Ok(cmd)
}

fn set_state_home<P: AsRef<Path>>(path: P) {
    // Tests run in a controlled environment; setting env vars requires unsafe on this target.
    unsafe {
        std::env::set_var("CODEX_STATE_HOME", path.as_ref());
    }
}

fn clear_state_home() {
    unsafe {
        std::env::remove_var("CODEX_STATE_HOME");
    }
}

#[tokio::test]
async fn add_and_remove_server_updates_global_config() -> Result<()> {
    let codex_home = TempDir::new()?;

    let mut add_cmd = codex_command(codex_home.path())?;
    add_cmd
        .args(["mcp", "add", "docs", "--", "echo", "hello"])
        .assert()
        .success()
        .stdout(contains("Added global MCP server 'docs'."));

    let servers = load_global_mcp_servers(codex_home.path()).await?;
    assert_eq!(servers.len(), 1);
    let docs = servers.get("docs").expect("server should exist");
    match &docs.transport {
        McpServerTransportConfig::Stdio { command, args, env } => {
            assert_eq!(command, "echo");
            assert_eq!(args, &vec!["hello".to_string()]);
            assert!(env.is_none());
        }
        other => panic!("unexpected transport: {other:?}"),
    }

    set_state_home(codex_home.path().join("state"));
    let registry = McpRegistry::load(codex_home.path())?;
    assert!(!registry.enabled().contains("docs"));
    clear_state_home();

    let mut remove_cmd = codex_command(codex_home.path())?;
    remove_cmd
        .args(["mcp", "remove", "docs"])
        .assert()
        .success()
        .stdout(contains("Removed global MCP server 'docs'."));

    let servers = load_global_mcp_servers(codex_home.path()).await?;
    assert!(servers.is_empty());

    set_state_home(codex_home.path().join("state"));
    let registry = McpRegistry::load(codex_home.path())?;
    assert!(!registry.enabled().contains("docs"));
    clear_state_home();

    let mut remove_again_cmd = codex_command(codex_home.path())?;
    remove_again_cmd
        .args(["mcp", "remove", "docs"])
        .assert()
        .success()
        .stdout(contains("No MCP server named 'docs' found."));

    let servers = load_global_mcp_servers(codex_home.path()).await?;
    assert!(servers.is_empty());

    Ok(())
}

#[tokio::test]
async fn add_with_env_preserves_key_order_and_values() -> Result<()> {
    let codex_home = TempDir::new()?;

    let mut add_cmd = codex_command(codex_home.path())?;
    add_cmd
        .args([
            "mcp",
            "add",
            "envy",
            "--env",
            "FOO=bar",
            "--env",
            "ALPHA=beta",
            "--",
            "python",
            "server.py",
        ])
        .assert()
        .success();

    let servers = load_global_mcp_servers(codex_home.path()).await?;
    let envy = servers.get("envy").expect("server should exist");
    let env = match &envy.transport {
        McpServerTransportConfig::Stdio { env: Some(env), .. } => env,
        other => panic!("unexpected transport: {other:?}"),
    };

    assert_eq!(env.len(), 2);
    assert_eq!(env.get("FOO"), Some(&"bar".to_string()));
    assert_eq!(env.get("ALPHA"), Some(&"beta".to_string()));

    Ok(())
}

#[tokio::test]
async fn add_streamable_http_without_manual_token() -> Result<()> {
    let codex_home = TempDir::new()?;

    let mut add_cmd = codex_command(codex_home.path())?;
    add_cmd
        .args(["mcp", "add", "github", "--url", "https://example.com/mcp"])
        .assert()
        .success();

    let servers = load_global_mcp_servers(codex_home.path()).await?;
    let github = servers.get("github").expect("github server should exist");
    match &github.transport {
        McpServerTransportConfig::StreamableHttp {
            url,
            bearer_token_env_var,
        } => {
            assert_eq!(url, "https://example.com/mcp");
            assert!(bearer_token_env_var.is_none());
        }
        other => panic!("unexpected transport: {other:?}"),
    }

    assert!(!codex_home.path().join(".credentials.json").exists());
    assert!(!codex_home.path().join(".env").exists());

    Ok(())
}

#[tokio::test]
async fn add_streamable_http_with_custom_env_var() -> Result<()> {
    let codex_home = TempDir::new()?;

    let mut add_cmd = codex_command(codex_home.path())?;
    add_cmd
        .args([
            "mcp",
            "add",
            "issues",
            "--url",
            "https://example.com/issues",
            "--bearer-token-env-var",
            "GITHUB_TOKEN",
        ])
        .assert()
        .success();

    let servers = load_global_mcp_servers(codex_home.path()).await?;
    let issues = servers.get("issues").expect("issues server should exist");
    match &issues.transport {
        McpServerTransportConfig::StreamableHttp {
            url,
            bearer_token_env_var,
        } => {
            assert_eq!(url, "https://example.com/issues");
            assert_eq!(bearer_token_env_var.as_deref(), Some("GITHUB_TOKEN"));
        }
        other => panic!("unexpected transport: {other:?}"),
    }
    Ok(())
}

#[tokio::test]
async fn add_streamable_http_rejects_removed_flag() -> Result<()> {
    let codex_home = TempDir::new()?;

    let mut add_cmd = codex_command(codex_home.path())?;
    add_cmd
        .args([
            "mcp",
            "add",
            "github",
            "--url",
            "https://example.com/mcp",
            "--with-bearer-token",
        ])
        .assert()
        .failure()
        .stderr(contains("--with-bearer-token"));

    let servers = load_global_mcp_servers(codex_home.path()).await?;
    assert!(servers.is_empty());

    Ok(())
}

#[tokio::test]
async fn add_cant_add_command_and_url() -> Result<()> {
    let codex_home = TempDir::new()?;

    let mut add_cmd = codex_command(codex_home.path())?;
    add_cmd
        .args([
            "mcp",
            "add",
            "github",
            "--url",
            "https://example.com/mcp",
            "--command",
            "--",
            "echo",
            "hello",
        ])
        .assert()
        .failure()
        .stderr(contains("unexpected argument '--command' found"));

    let servers = load_global_mcp_servers(codex_home.path()).await?;
    assert!(servers.is_empty());

    Ok(())
}

#[tokio::test]
async fn enable_disable_toggles_registry() -> Result<()> {
    let codex_home = TempDir::new()?;

    codex_command(codex_home.path())?
        .args(["mcp", "add", "docs", "--", "printf", "hello"])
        .assert()
        .success();

    let mut enable_cmd = codex_command(codex_home.path())?;
    enable_cmd
        .args(["mcp", "enable", "docs"])
        .assert()
        .success()
        .stdout(contains("Enabled MCP server 'docs'."));

    set_state_home(codex_home.path().join("state"));
    let registry = McpRegistry::load(codex_home.path())?;
    assert!(registry.enabled().contains("docs"));

    let mut list_cmd = codex_command(codex_home.path())?;
    let list_output = list_cmd.args(["mcp", "list", "--json"]).output()?;
    assert!(list_output.status.success());
    let json: Value = serde_json::from_slice(&list_output.stdout)?;
    let arr = json.as_array().expect("list output should be an array");
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["enabled"], Value::Bool(true));

    let mut disable_cmd = codex_command(codex_home.path())?;
    disable_cmd
        .args(["mcp", "disable", "docs"])
        .assert()
        .success()
        .stdout(contains("Disabled MCP server 'docs'."));

    set_state_home(codex_home.path().join("state"));
    let registry = McpRegistry::load(codex_home.path())?;
    assert!(!registry.enabled().contains("docs"));
    clear_state_home();

    let mut list_cmd = codex_command(codex_home.path())?;
    let list_output = list_cmd.args(["mcp", "list", "--json"]).output()?;
    assert!(list_output.status.success());
    let json: Value = serde_json::from_slice(&list_output.stdout)?;
    let arr = json.as_array().expect("list output should be an array");
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["enabled"], Value::Bool(false));

    Ok(())
}
