use std::ffi::OsString;
use std::path::Path;
use std::process::Stdio;

use engine::{AgentSpec, McpServer};
use serde_json::json;
use tokio::process::Command;

/// The command that starts an agent: `-p --input-format stream-json --output-format stream-json
/// --verbose`, its instructions, its MCP servers and the sandbox, which is never left off. It runs
/// on `config`, the configuration folder of its own, so nothing of the person's setup reaches it.
pub(crate) fn command(binary: &Path, config: &Path, spec: &AgentSpec) -> Command {
    let mut command = Command::new(binary);
    command
        .args(["-p", "--input-format", "stream-json"])
        .args(["--output-format", "stream-json", "--verbose"])
        // Values go after `=` so instructions that start with `-` are not read as a flag.
        .arg(format!("--append-system-prompt={}", spec.instructions))
        .arg(format!("--settings={}", SETTINGS))
        // `user` is `config`, which holds only the sign-in and the settings `config.rs` writes;
        // leaving out `project` and `local` keeps the workdir's CLAUDE.md, rules, skills, hooks and
        // settings away from it.
        .arg("--setting-sources=user")
        // Only these built-in tools reach the model, besides the MCP servers' own; any other, such
        // as messaging the person's other sessions, would act outside the hierarchy the kernel
        // enforces.
        .arg(format!("--tools={}", TOOLS.join(",")))
        .arg("--strict-mcp-config")
        .arg(format!("--mcp-config={}", mcp_config(&spec.mcp)))
        .current_dir(&spec.workdir)
        .env_clear()
        .envs(environment(config))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    if !spec.mcp.is_empty() {
        let tools: Vec<String> = spec
            .mcp
            .iter()
            .map(|server| format!("mcp__{}__*", server.name))
            .collect();
        command.arg(format!("--allowedTools={}", tools.join(",")));
    }
    // Each token reaches the binary only through its environment, never its arguments, which any
    // local process can read.
    for (index, server) in spec.mcp.iter().enumerate() {
        command.env(token_variable(index), format!("Bearer {}", server.token));
    }
    command
}

const TOOLS: [&str; 3] = ["Read", "Glob", "Grep"];

// The only variables of the runtime the binary gets: what it needs to find its tools, the person's
// keychain and a temporary folder, and to read text. Anything else, such as a secret of the person,
// never reaches an agent.
const INHERITED: [&str; 8] = [
    "HOME", "PATH", "USER", "LOGNAME", "TMPDIR", "LANG", "LC_ALL", "LC_CTYPE",
];

/// The whole environment an agent's binary runs with, on `config`, once the runtime's is cleared.
pub(crate) fn environment(config: &Path) -> impl Iterator<Item = (OsString, OsString)> {
    let inherited = INHERITED
        .iter()
        .filter_map(|name| Some((OsString::from(name), std::env::var_os(name)?)));
    inherited.chain([("CLAUDE_CONFIG_DIR".into(), config.as_os_str().to_owned())])
}

// The sandbox refuses to start rather than run unsandboxed, and no command may leave it. No memory
// carries over from one agent to the next.
const SETTINGS: &str = r#"{"autoMemoryEnabled":false,"sandbox":{"enabled":true,"failIfUnavailable":true,"allowUnsandboxedCommands":false}}"#;

fn mcp_config(servers: &[McpServer]) -> String {
    let servers: serde_json::Map<String, serde_json::Value> = servers
        .iter()
        .enumerate()
        .map(|(index, server)| {
            let authorization = format!("${{{}}}", token_variable(index));
            let config = json!({
                "type": "http",
                "url": server.url,
                "headers": { "Authorization": authorization },
            });
            (server.name.clone(), config)
        })
        .collect();
    json!({ "mcpServers": servers }).to_string()
}

fn token_variable(index: usize) -> String {
    format!("MCP_AUTHORIZATION_{index}")
}

/// The stdin line that carries one message of the person.
pub(crate) fn user_message(text: &str) -> String {
    let message = json!({
        "type": "user",
        "message": { "role": "user", "content": text },
    });
    format!("{message}\n")
}

#[cfg(test)]
mod tests {
    use std::ffi::OsStr;
    use std::path::PathBuf;

    use super::*;

    const CONFIG: &str = "/data/claude";

    fn spec(mcp: Vec<McpServer>) -> AgentSpec {
        AgentSpec {
            instructions: "- You are the peer.".into(),
            workdir: PathBuf::from("/work/asset"),
            mcp,
        }
    }

    fn server(name: &str, token: &str) -> McpServer {
        McpServer {
            name: name.into(),
            url: format!("http://127.0.0.1:4000/{name}"),
            token: token.into(),
        }
    }

    fn args(command: &Command) -> Vec<String> {
        let args = command.as_std().get_args();
        args.map(|arg| arg.to_string_lossy().into_owned()).collect()
    }

    fn value<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
        args.iter()
            .find_map(|arg| arg.strip_prefix(&format!("{flag}=")))
    }

    #[test]
    fn speaks_stream_json_in_print_mode_from_the_workdir() {
        let command = command(Path::new("/bin/claude"), Path::new(CONFIG), &spec(vec![]));
        let args = args(&command);
        let expected = ["-p", "--input-format", "stream-json", "--output-format"];
        assert_eq!(args[..4], expected);
        assert_eq!(args[4..6], ["stream-json", "--verbose"]);
        assert_eq!(command.as_std().get_program(), "/bin/claude");
        let workdir = command.as_std().get_current_dir();
        assert_eq!(workdir, Some(Path::new("/work/asset")));
    }

    #[test]
    fn runs_on_its_own_configuration_folder() {
        let command = command(Path::new("claude"), Path::new(CONFIG), &spec(vec![]));
        let envs: Vec<_> = command.as_std().get_envs().collect();
        let config = (OsStr::new("CLAUDE_CONFIG_DIR"), Some(OsStr::new(CONFIG)));
        assert!(envs.contains(&config));
    }

    #[test]
    fn appends_the_instructions_as_one_value() {
        let args = args(&command(
            Path::new("claude"),
            Path::new(CONFIG),
            &spec(vec![]),
        ));
        let instructions = value(&args, "--append-system-prompt");
        assert_eq!(instructions, Some("- You are the peer."));
    }

    #[test]
    fn always_turns_the_sandbox_on_with_no_way_out() {
        let args = args(&command(
            Path::new("claude"),
            Path::new(CONFIG),
            &spec(vec![]),
        ));
        let settings: serde_json::Value =
            serde_json::from_str(value(&args, "--settings").unwrap()).unwrap();
        assert_eq!(settings["sandbox"]["enabled"], true);
        assert_eq!(settings["sandbox"]["failIfUnavailable"], true);
        assert_eq!(settings["sandbox"]["allowUnsandboxedCommands"], false);
    }

    #[test]
    fn loads_nothing_of_the_workdir_nor_any_memory() {
        let args = args(&command(
            Path::new("claude"),
            Path::new(CONFIG),
            &spec(vec![]),
        ));
        assert_eq!(value(&args, "--setting-sources"), Some("user"));
        let settings: serde_json::Value =
            serde_json::from_str(value(&args, "--settings").unwrap()).unwrap();
        assert_eq!(settings["autoMemoryEnabled"], false);
    }

    #[test]
    fn loads_only_its_own_mcp_servers_and_allows_their_tools() {
        let servers = vec![
            server("federa", "secret-one"),
            server("other", "secret-two"),
        ];
        let args = args(&command(
            Path::new("claude"),
            Path::new(CONFIG),
            &spec(servers),
        ));
        assert!(args.contains(&"--strict-mcp-config".to_string()));
        let config: serde_json::Value =
            serde_json::from_str(value(&args, "--mcp-config").unwrap()).unwrap();
        let federa = &config["mcpServers"]["federa"];
        assert_eq!(federa["type"], "http");
        assert_eq!(federa["url"], "http://127.0.0.1:4000/federa");
        let allowed = value(&args, "--allowedTools");
        assert_eq!(allowed, Some("mcp__federa__*,mcp__other__*"));
    }

    #[test]
    fn passes_each_token_through_the_environment_only() {
        let servers = vec![
            server("federa", "secret-one"),
            server("other", "secret-two"),
        ];
        let command = command(Path::new("claude"), Path::new(CONFIG), &spec(servers));
        let args = args(&command);
        assert!(args.iter().all(|arg| !arg.contains("secret")));
        let config: serde_json::Value =
            serde_json::from_str(value(&args, "--mcp-config").unwrap()).unwrap();
        let header = &config["mcpServers"]["other"]["headers"]["Authorization"];
        assert_eq!(header, "${MCP_AUTHORIZATION_1}");
        let envs: Vec<_> = command.as_std().get_envs().collect();
        let token = Some(OsStr::new("Bearer secret-two"));
        assert!(envs.contains(&(OsStr::new("MCP_AUTHORIZATION_1"), token)));
    }

    #[test]
    fn offers_only_the_read_only_built_in_tools() {
        let args = args(&command(
            Path::new("claude"),
            Path::new(CONFIG),
            &spec(vec![]),
        ));
        assert_eq!(value(&args, "--tools"), Some("Read,Glob,Grep"));
    }

    #[test]
    fn allows_no_tools_without_mcp_servers() {
        let args = args(&command(
            Path::new("claude"),
            Path::new(CONFIG),
            &spec(vec![]),
        ));
        assert_eq!(value(&args, "--allowedTools"), None);
    }

    #[test]
    fn writes_a_message_as_one_json_line() {
        let line = user_message("two\nlines \"quoted\"");
        assert_eq!(line.matches('\n').count(), 1);
        let message: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(message["type"], "user");
        assert_eq!(message["message"]["role"], "user");
        assert_eq!(message["message"]["content"], "two\nlines \"quoted\"");
    }
}
