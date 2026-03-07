use std::process::Command;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

/// Build a `Command` that executes `command_line` through the platform shell.
///
/// On macOS/Linux this uses `/bin/zsh -lc` so login-profile environment
/// variables (such as PATH additions from Homebrew or nvm) are available.
///
/// On Windows, SSH and related tools are native executables on the PATH, so
/// we split the command line into tokens and invoke the program directly,
/// which avoids quoting issues with `cmd /C` and single-quoted SSH arguments
/// that are common in the existing configuration.  The `CREATE_NO_WINDOW`
/// flag prevents a console window from flashing when run from a GUI app.
#[cfg(windows)]
pub(crate) fn shell_command(command_line: &str) -> Command {
    let tokens = split_command_tokens(command_line);

    if tokens.is_empty() {
        let mut command = Command::new("cmd");
        command.arg("/C").arg(command_line);
        command.creation_flags(CREATE_NO_WINDOW);
        return command;
    }

    let mut command = Command::new(&tokens[0]);
    for token in &tokens[1..] {
        command.arg(token);
    }
    command.creation_flags(CREATE_NO_WINDOW);
    command
}

#[cfg(not(windows))]
pub(crate) fn shell_command(command_line: &str) -> Command {
    let mut command = Command::new("/bin/zsh");
    command.arg("-lc").arg(command_line);
    command
}

/// Return the program name and arguments for spawning a command line in a PTY.
///
/// On macOS/Linux: `("/bin/zsh", ["-lc", command_line])`
/// On Windows: tokenises the command line and returns `(program, [arg1, arg2, ...])`
///   so that operators like `||` inside SSH remote commands are not misinterpreted
///   by `cmd.exe`.
#[cfg(windows)]
pub(crate) fn pty_program_and_args(command_line: &str) -> (String, Vec<String>) {
    // Detect Unix-style local shell commands and substitute PowerShell on Windows.
    let trimmed = command_line.trim();
    if trimmed.is_empty()
        || trimmed.starts_with("exec /bin/")
        || trimmed.starts_with("/bin/")
        || trimmed == "bash"
        || trimmed == "zsh"
        || trimmed == "sh"
    {
        return ("powershell".into(), vec!["-NoLogo".into()]);
    }

    let tokens = split_command_tokens(command_line);

    if tokens.is_empty() {
        return ("cmd".into(), vec!["/C".into(), command_line.into()]);
    }

    let program = tokens[0].clone();
    let args = tokens[1..].to_vec();
    (program, args)
}

#[cfg(not(windows))]
pub(crate) fn pty_program_and_args(command_line: &str) -> (String, Vec<String>) {
    ("/bin/zsh".into(), vec!["-lc".into(), command_line.into()])
}

/// Minimal command-line tokeniser for Windows.
///
/// Splits on unquoted whitespace, respects both single (`'`) and double (`"`)
/// quotes, and strips the outermost quote layer from each token so that
/// arguments like `'lxc list --format csv -c ns4'` are passed as a single
/// unquoted string to the child process.
#[cfg(windows)]
fn split_command_tokens(input: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut chars = input.chars().peekable();
    let mut in_single_quote = false;
    let mut in_double_quote = false;

    while let Some(ch) = chars.next() {
        match ch {
            '\'' if !in_double_quote => {
                in_single_quote = !in_single_quote;
            }
            '"' if !in_single_quote => {
                in_double_quote = !in_double_quote;
            }
            c if c.is_ascii_whitespace() && !in_single_quote && !in_double_quote => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
            }
            c => {
                current.push(c);
            }
        }
    }

    if !current.is_empty() {
        tokens.push(current);
    }

    tokens
}

#[cfg(test)]
mod tests {
    #[cfg(windows)]
    use super::split_command_tokens;

    #[test]
    #[cfg(windows)]
    fn splits_simple_ssh_command() {
        let tokens = split_command_tokens("ssh domain-dev 'lxc list --format csv -c ns4'");
        assert_eq!(tokens, vec!["ssh", "domain-dev", "lxc list --format csv -c ns4"]);
    }

    #[test]
    #[cfg(windows)]
    fn splits_ssh_with_double_quotes() {
        let tokens = split_command_tokens("ssh domain-dev \"lxc list --format csv\"");
        assert_eq!(tokens, vec!["ssh", "domain-dev", "lxc list --format csv"]);
    }

    #[test]
    #[cfg(windows)]
    fn handles_nested_quotes_in_deploy_command() {
        let tokens = split_command_tokens(
            r#"ssh server 'lxc exec mycontainer -- bash -c "echo hello >> /tmp/test"'"#,
        );
        assert_eq!(
            tokens,
            vec![
                "ssh",
                "server",
                r#"lxc exec mycontainer -- bash -c "echo hello >> /tmp/test""#,
            ]
        );
    }

    #[test]
    #[cfg(windows)]
    fn handles_empty_input() {
        let tokens = split_command_tokens("");
        assert!(tokens.is_empty());
    }

    #[test]
    #[cfg(windows)]
    fn handles_no_quotes() {
        let tokens = split_command_tokens("ssh -o BatchMode=yes -o ConnectTimeout=4 host true");
        assert_eq!(
            tokens,
            vec!["ssh", "-o", "BatchMode=yes", "-o", "ConnectTimeout=4", "host", "true"]
        );
    }
}
