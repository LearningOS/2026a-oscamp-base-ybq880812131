//! # Process and Pipes
//!
//! In this exercise, you will learn how to create child processes and communicate through pipes.
//!
//! ## Concepts
//! - `std::process::Command` creates child processes (corresponds to `fork()` + `execve()` system calls)
//! - `Stdio::piped()` sets up pipes (corresponds to `pipe()` + `dup2()` system calls)
//! - Communicate with child processes via stdin/stdout
//! - Obtain child process exit status (corresponds to `waitpid()` system call)

use std::io::{self, Read, Write};
use std::process::{Command, Stdio};

#[cfg(not(windows))]
fn new_shell_command(command: &str) -> Command {
    let mut c = Command::new("sh");
    c.arg("-c").arg(command);
    c
}

#[cfg(windows)]
fn new_shell_command(command: &str) -> Command {
    let mut c = Command::new("cmd");
    c.arg("/C").arg(command);
    c
}

#[cfg(not(windows))]
fn new_cat_command() -> Command {
    Command::new("cat")
}

#[cfg(windows)]
fn new_cat_command() -> Command {
    let mut c = Command::new("powershell");
    c.args([
        "-NoProfile",
        "-Command",
        "[Console]::Out.Write([Console]::In.ReadToEnd())",
    ]);
    c
}

#[cfg(not(windows))]
fn new_grep_command(pattern: &str) -> Command {
    let mut c = Command::new("grep");
    c.arg(pattern);
    c
}

#[cfg(windows)]
fn new_grep_command(pattern: &str) -> Command {
    let mut c = Command::new("findstr");
    c.arg(pattern);
    c
}

#[cfg(not(windows))]
fn new_echo_command(args: &[&str]) -> Command {
    let mut c = Command::new("echo");
    c.args(args);
    c
}

#[cfg(windows)]
fn new_echo_command(args: &[&str]) -> Command {
    let no_newline = args.first().map(|s| *s == "-n").unwrap_or(false);
    if no_newline {
        let rest: Vec<&str> = args.iter().skip(1).copied().collect();
        let joined_rest = rest.join(" ");
        let mut c = Command::new("powershell");
        c.args([
            "-NoProfile",
            "-Command",
            &format!("Write-Host -NoNewline '{}'", joined_rest),
        ]);
        c
    } else {
        let joined = args.join(" ");
        let mut c = Command::new("cmd");
        c.arg("/C").arg("echo").arg(&joined);
        c
    }
}

/// Execute the given shell command and return its stdout output.
pub fn run_command(program: &str, args: &[&str]) -> String {
    let mut cmd = if program == "echo" {
        new_echo_command(args)
    } else {
        let mut c = Command::new(program);
        c.args(args);
        c
    };
    let output = cmd.stdout(Stdio::piped()).output().unwrap();
    String::from_utf8(output.stdout).unwrap()
}

/// Write data to child process (cat) stdin via pipe and read its stdout output.
pub fn pipe_through_cat(input: &str) -> String {
    let mut child = new_cat_command()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    stdin.write_all(input.as_bytes()).unwrap();
    drop(stdin);
    let mut output = String::new();
    child.stdout.as_mut().unwrap().read_to_string(&mut output).unwrap();
    child.wait().unwrap();
    #[cfg(windows)]
    {
        output = output.replace("\r\n", "\n");
    }
    output
}

/// Get child process exit code.
pub fn get_exit_code(command: &str) -> i32 {
    new_shell_command(command)
        .status()
        .unwrap()
        .code()
        .unwrap()
}

/// Execute the given shell command and return its stdout output as a `Result`.
pub fn run_command_with_result(program: &str, args: &[&str]) -> io::Result<String> {
    let mut cmd = if program == "echo" {
        new_echo_command(args)
    } else {
        let mut c = Command::new(program);
        c.args(args);
        c
    };
    let output = cmd.stdout(Stdio::piped()).output()?;
    String::from_utf8(output.stdout)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

/// Interact with `grep` via bidirectional pipes, filtering lines that contain a pattern.
pub fn pipe_through_grep(pattern: &str, input: &str) -> String {
    let mut child = new_grep_command(pattern)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    stdin.write_all(input.as_bytes()).unwrap();
    drop(stdin);
    let mut output = String::new();
    child.stdout.as_mut().unwrap().read_to_string(&mut output).unwrap();
    child.wait().unwrap();
    #[cfg(windows)]
    {
        output = output.replace("\r\n", "\n");
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_run_echo() {
        let output = run_command("echo", &["hello"]);
        assert_eq!(output.trim(), "hello");
    }

    #[test]
    fn test_run_with_args() {
        let output = run_command("echo", &["-n", "no newline"]);
        assert_eq!(output, "no newline");
    }

    #[test]
    fn test_pipe_cat() {
        let output = pipe_through_cat("hello pipe!");
        assert_eq!(output, "hello pipe!");
    }

    #[test]
    fn test_pipe_multiline() {
        let input = "line1\nline2\nline3";
        assert_eq!(pipe_through_cat(input), input);
    }

    #[test]
    fn test_exit_code_success() {
        assert_eq!(get_exit_code(if cfg!(windows) { "exit 0" } else { "true" }), 0);
    }

    #[test]
    fn test_exit_code_failure() {
        assert_eq!(get_exit_code(if cfg!(windows) { "exit 1" } else { "false" }), 1);
    }

    #[test]
    fn test_run_command_with_result_success() {
        let result = run_command_with_result("echo", &["hello"]);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().trim(), "hello");
    }

    #[test]
    fn test_run_command_with_result_nonexistent() {
        let result = run_command_with_result("nonexistent_command_xyz", &[]);
        assert!(result.is_err());
    }

    #[test]
    fn test_pipe_through_grep_basic() {
        let input = "apple\nbanana\ncherry\n";
        let output = pipe_through_grep("a", input);
        assert_eq!(output, "apple\nbanana\n");
    }

    #[test]
    fn test_pipe_through_grep_no_match() {
        let input = "apple\nbanana\ncherry\n";
        let output = pipe_through_grep("z", input);
        assert_eq!(output, "");
    }

    #[test]
    fn test_pipe_through_grep_multiline() {
        let input = "first line\nsecond line\nthird line\n";
        let output = pipe_through_grep("second", input);
        assert_eq!(output, "second line\n");
    }
}
